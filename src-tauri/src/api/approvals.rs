use crate::models::{fmt_dt, iso, iso_or_null, parse_dt, ApprovalRequestRow};
use crate::router::{HttpError, Router};
use crate::services::approvals::{decide, list_requests};
use crate::tools::build_registry;
use crate::validation::enum_message;
use chrono::Duration;
use serde_json::{json, Value};

const STATUSES: [&str; 5] = ["pending", "executed", "failed", "rejected", "expired"];

fn approval_out(row: &ApprovalRequestRow, ttl_hours: i64) -> Value {
    let expires: Value = if row.status == "pending" {
        match parse_dt(&row.created_at) {
            Some(created) => json!(iso(&fmt_dt(created + Duration::hours(ttl_hours)))),
            None => Value::Null,
        }
    } else {
        Value::Null
    };
    json!({
        "id": row.id,
        "created_at": iso(&row.created_at),
        "expires_at": expires,
        "run_id": row.run_id,
        "tool_name": row.tool_name,
        "account_id": row.account_id,
        "campaign_id": row.campaign_id,
        "campaign_name": row.campaign_name,
        "summary": row.summary,
        "reason": row.reason,
        "status": row.status,
        "decided_at": iso_or_null(&row.decided_at),
        "result": row.result,
    })
}

async fn decide_request(
    container: &crate::container::Container,
    raw: &str,
    approve: bool,
) -> Result<Value, HttpError> {
    let parsed = raw.trim().parse::<f64>();
    let request_id = match parsed {
        Ok(value) if value.fract() == 0.0 && value.abs() < 9e15 => value as i64,
        _ => return Err(HttpError::new(422, "request_id: Input should be a valid integer")),
    };
    let row = decide(container, &build_registry(), request_id, approve).await?;
    Ok(approval_out(&row, container.settings().approval_ttl_hours))
}

pub fn register(router: &mut Router) {
    router.get("/approvals", |context| async move {
        let status = context.query.get("status").map(String::as_str);
        if let Some(value) = status {
            if !STATUSES.contains(&value) {
                return Err(HttpError::new(422, enum_message(&STATUSES, &json!(value))));
            }
        }
        let ttl = context.container.settings().approval_ttl_hours;
        let (pending, rows) = list_requests(&context.container.db, ttl, status, 100)?;
        let items: Vec<Value> = rows.iter().map(|row| approval_out(row, ttl)).collect();
        Ok(json!({
            "pending": pending,
            "writes_enabled": context.container.has_meta(),
            "items": items,
        }))
    });

    router.post("/approvals/:id/approve", |context| async move {
        decide_request(&context.container, context.param("id"), true).await
    });

    router.post("/approvals/:id/reject", |context| async move {
        decide_request(&context.container, context.param("id"), false).await
    });
}
