use crate::container::Container;
use crate::db::Db;
use crate::models::{clip, fmt_dt, today_utc, utcnow, ApprovalRequestRow};
use crate::ports::tool::ToolError;
use crate::router::HttpError;
use crate::tools::ToolRegistry;
use chrono::{Duration, Utc};
use rusqlite::{params, OptionalExtension};
use serde_json::Value;

pub fn expire_stale(db: &Db, ttl_hours: i64) -> rusqlite::Result<()> {
    let cutoff = fmt_dt(Utc::now() - Duration::hours(ttl_hours));
    let conn = db.lock();
    conn.execute(
        "UPDATE approval_requests SET status = 'expired', decided_at = ?1 WHERE status = 'pending' AND created_at < ?2",
        params![utcnow(), cutoff],
    )?;
    Ok(())
}

fn get_row(db: &Db, id: i64) -> rusqlite::Result<Option<ApprovalRequestRow>> {
    let conn = db.lock();
    conn.query_row(
        "SELECT * FROM approval_requests WHERE id = ?1",
        [id],
        ApprovalRequestRow::from_row,
    )
    .optional()
}

pub struct NewRequest {
    pub run_id: String,
    pub tool_name: String,
    pub account_id: String,
    pub campaign_id: String,
    pub campaign_name: String,
    pub summary: String,
    pub reason: String,
    pub payload: Value,
    pub ttl_hours: i64,
}

pub fn create_request(db: &Db, request: &NewRequest) -> rusqlite::Result<(ApprovalRequestRow, bool)> {
    expire_stale(db, request.ttl_hours)?;
    let existing: Option<ApprovalRequestRow> = {
        let conn = db.lock();
        conn.query_row(
            "SELECT * FROM approval_requests WHERE status = 'pending' AND tool_name = ?1 AND campaign_id = ?2 LIMIT 1",
            params![request.tool_name, request.campaign_id],
            ApprovalRequestRow::from_row,
        )
        .optional()?
    };
    if let Some(found) = existing {
        return Ok((found, false));
    }
    let id = {
        let conn = db.lock();
        conn.execute(
            "INSERT INTO approval_requests (created_at, run_id, tool_name, account_id, campaign_id, campaign_name, summary, reason, payload, status)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 'pending')",
            params![
                utcnow(),
                request.run_id,
                request.tool_name,
                request.account_id,
                request.campaign_id,
                clip(&request.campaign_name, 512),
                clip(&request.summary, 500),
                clip(&request.reason, 500),
                crate::adapters::llm::common::js_stringify(&request.payload)
            ],
        )?;
        conn.last_insert_rowid()
    };
    match get_row(db, id)? {
        Some(row) => Ok((row, true)),
        None => Err(rusqlite::Error::QueryReturnedNoRows),
    }
}

pub fn list_requests(
    db: &Db,
    ttl_hours: i64,
    status: Option<&str>,
    limit: i64,
) -> rusqlite::Result<(i64, Vec<ApprovalRequestRow>)> {
    expire_stale(db, ttl_hours)?;
    let conn = db.lock();
    let pending: i64 = conn.query_row(
        "SELECT COUNT(*) FROM approval_requests WHERE status = 'pending'",
        [],
        |row| row.get(0),
    )?;
    let rows = match status.filter(|value| !value.is_empty()) {
        Some(value) => {
            let mut statement =
                conn.prepare("SELECT * FROM approval_requests WHERE status = ?1 ORDER BY id DESC LIMIT ?2")?;
            let collected = statement
                .query_map(params![value, limit], ApprovalRequestRow::from_row)?
                .collect::<Result<Vec<_>, _>>()?;
            collected
        }
        None => {
            let mut statement = conn.prepare("SELECT * FROM approval_requests ORDER BY id DESC LIMIT ?1")?;
            let collected = statement
                .query_map(params![limit], ApprovalRequestRow::from_row)?
                .collect::<Result<Vec<_>, _>>()?;
            collected
        }
    };
    Ok((pending, rows))
}

fn executed_today(db: &Db) -> rusqlite::Result<i64> {
    let start = format!("{} 00:00:00.000000", today_utc());
    let conn = db.lock();
    conn.query_row(
        "SELECT COUNT(*) FROM approval_requests WHERE status IN ('approved', 'executed') AND decided_at >= ?1",
        [start],
        |row| row.get(0),
    )
}

pub async fn decide(
    container: &Container,
    registry: &ToolRegistry,
    request_id: i64,
    approve: bool,
) -> Result<ApprovalRequestRow, HttpError> {
    let db = &container.db;
    let settings = container.settings();
    expire_stale(db, settings.approval_ttl_hours)?;
    let row = match get_row(db, request_id)? {
        Some(found) => found,
        None => return Err(HttpError::new(404, "Approval request not found")),
    };
    if row.status != "pending" {
        return Err(HttpError::new(409, format!("This request is already {}", row.status)));
    }
    if !approve {
        {
            let conn = db.lock();
            conn.execute(
                "UPDATE approval_requests SET status = 'rejected', decided_at = ?1 WHERE id = ?2",
                params![utcnow(), row.id],
            )?;
        }
        println!("Approval {} rejected", row.id);
        return get_row(db, row.id)?.ok_or_else(|| HttpError::new(404, "Approval request not found"));
    }
    if executed_today(db)? >= settings.write_daily_limit {
        return Err(HttpError::new(
            429,
            format!("Daily limit of {} approved changes reached", settings.write_daily_limit),
        ));
    }
    {
        let conn = db.lock();
        conn.execute(
            "UPDATE approval_requests SET status = 'approved', decided_at = ?1 WHERE id = ?2",
            params![utcnow(), row.id],
        )?;
    }
    let payload: Value = serde_json::from_str(&row.payload).unwrap_or(Value::Null);
    let tool = registry.get(&row.tool_name);
    let (status, result): (&str, String) = match tool {
        Some(found) if found.risk() == crate::ports::tool::Risk::Write && container.has_meta() => {
            match found.apply(container, &payload).await {
                Ok(outcome) => {
                    let text = match outcome.get("message") {
                        Some(Value::String(message)) => message.clone(),
                        Some(other) if !other.is_null() => other.to_string(),
                        _ => "Done".to_string(),
                    };
                    ("executed", clip(&text, 500))
                }
                Err(ToolError::Failure(failure)) => ("failed", clip(&failure.message, 500)),
                Err(ToolError::Other(message)) => {
                    eprintln!("{}", message);
                    return Err(HttpError::new(500, "Internal Server Error"));
                }
            }
        }
        _ => (
            "failed",
            clip("Meta is not configured or the tool no longer exists", 500),
        ),
    };
    {
        let conn = db.lock();
        conn.execute(
            "UPDATE approval_requests SET status = ?1, result = ?2 WHERE id = ?3",
            params![status, result, row.id],
        )?;
    }
    println!("Approval {} {}", row.id, status);
    get_row(db, row.id)?.ok_or_else(|| HttpError::new(404, "Approval request not found"))
}
