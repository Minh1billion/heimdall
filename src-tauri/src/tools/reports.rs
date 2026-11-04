use crate::models::fmt_dt;
use crate::ports::tool::{Risk, Tool, ToolContext, ToolError, ToolFailure};
use crate::services::analysis::detect_anomalies;
use crate::services::reports::build_report;
use crate::tools::common::{check_account, int_arg, num_arg, text_arg, today, METRICS};
use crate::validation::{Field, Kind};
use async_trait::async_trait;
use chrono::{Duration, Utc};
use serde_json::{json, Map, Value};

const HOURLY_LIMIT: i64 = 10;

pub struct DetectAnomalies;

#[async_trait]
impl Tool for DetectAnomalies {
    fn name(&self) -> &'static str {
        "detect_anomalies"
    }

    fn description(&self) -> &'static str {
        "Find days whose value for a metric deviates strongly from the other days in the window (z-score). Use to explain sudden changes in spend or cost per result."
    }

    fn fields(&self) -> Vec<Field> {
        vec![
            Field::text("account_id").nullish(),
            Field::new("metric", Kind::Enum(&METRICS)).with_default(json!("spend")),
            Field::new("days", Kind::Int { min: Some(7), max: Some(60) }).with_default(json!(14)),
            Field::new("threshold", Kind::Num { min: Some(1.0), max: Some(5.0) }).with_default(json!(2)),
        ]
    }

    async fn run(&self, ctx: &ToolContext, args: &Map<String, Value>) -> Result<Value, ToolError> {
        let account = text_arg(args, "account_id");
        check_account(ctx, account)?;
        let metric = text_arg(args, "metric").unwrap_or("spend");
        let days = int_arg(args, "days");
        let found = detect_anomalies(&ctx.container.db, &today(), account, metric, days, num_arg(args, "threshold"))?;
        Ok(json!({ "ok": true, "metric": metric, "days": days, "anomalies": found }))
    }
}

pub struct BuildReport;

#[async_trait]
impl Tool for BuildReport {
    fn name(&self) -> &'static str {
        "build_report"
    }

    fn description(&self) -> &'static str {
        "Build a ready-made report with totals, changes versus the previous period, top campaigns and unusual days. daily covers yesterday, weekly covers the last 7 full days. Returns text; it does not send anything."
    }

    fn fields(&self) -> Vec<Field> {
        vec![
            Field::new("template", Kind::Enum(&["daily", "weekly"])).with_default(json!("daily")),
            Field::text("account_id").nullish(),
        ]
    }

    async fn run(&self, ctx: &ToolContext, args: &Map<String, Value>) -> Result<Value, ToolError> {
        let account = text_arg(args, "account_id");
        check_account(ctx, account)?;
        let template = text_arg(args, "template").unwrap_or("daily");
        let report = build_report(&ctx.container, template, account, false).await?;
        Ok(json!({ "ok": true, "title": report.title, "text": report.text }))
    }
}

pub struct SendNotification;

#[async_trait]
impl Tool for SendNotification {
    fn name(&self) -> &'static str {
        "send_notification"
    }

    fn description(&self) -> &'static str {
        "Send a message to a configured channel. Only use when the user explicitly asks to send or share something. The destination is fixed by the server configuration; you only choose the channel."
    }

    fn risk(&self) -> Risk {
        Risk::Notify
    }

    fn fields(&self) -> Vec<Field> {
        vec![
            Field::new("channel", Kind::Enum(&["telegram", "slack", "zalo", "email"])),
            Field::new("subject", Kind::Str { min: None, max: Some(150) }),
            Field::new("text", Kind::Str { min: None, max: Some(3500) }),
        ]
    }

    async fn run(&self, ctx: &ToolContext, args: &Map<String, Value>) -> Result<Value, ToolError> {
        let channel = text_arg(args, "channel").unwrap_or("");
        let notifier = match ctx.container.notifier(channel) {
            Some(found) => found,
            None => {
                return Err(ToolError::Failure(ToolFailure::new(
                    "channel_not_configured",
                    format!("{} is not configured", channel),
                )))
            }
        };
        let since = fmt_dt(Utc::now() - Duration::hours(1));
        let recent: i64 = {
            let conn = ctx.container.db.lock();
            conn.query_row(
                "SELECT COUNT(*) FROM tool_calls WHERE tool_name = ?1 AND ok = 1 AND created_at >= ?2",
                rusqlite::params!["send_notification", since],
                |row| row.get(0),
            )?
        };
        if recent >= HOURLY_LIMIT {
            return Err(ToolError::Failure(ToolFailure::new(
                "rate_limited",
                "Too many notifications sent in the last hour",
            )));
        }
        let subject = text_arg(args, "subject").unwrap_or("");
        let text = text_arg(args, "text").unwrap_or("");
        if let Err(error) = notifier.send(subject, text).await {
            return Err(ToolError::Failure(ToolFailure::new("delivery_failed", error.0)));
        }
        Ok(json!({ "ok": true, "channel": channel }))
    }
}
