use crate::models::{py_round, today_utc};
use crate::ports::tool::{ToolContext, ToolError, ToolFailure};
use crate::validation::{Field, Kind};
use rusqlite::OptionalExtension;
use serde_json::{json, Map, Value};

pub const METRICS: [&str; 9] = [
    "spend",
    "impressions",
    "clicks",
    "conversions",
    "ctr",
    "cpc",
    "cpm",
    "cpa",
    "roas",
];
pub const DEFAULT_METRICS: [&str; 6] = ["spend", "impressions", "clicks", "conversions", "cpa", "roas"];
pub const RANGE_HELP: &str = "today, yesterday, last_7d, last_30d, this_month or YYYY-MM-DD..YYYY-MM-DD";

pub fn today() -> String {
    today_utc()
}

pub fn metrics_field() -> Field {
    Field::new("metrics", Kind::EnumList(&METRICS)).with_default(json!(DEFAULT_METRICS))
}

pub fn text_arg<'a>(args: &'a Map<String, Value>, key: &str) -> Option<&'a str> {
    args.get(key).and_then(Value::as_str)
}

pub fn int_arg(args: &Map<String, Value>, key: &str) -> i64 {
    args.get(key).and_then(Value::as_i64).unwrap_or(0)
}

pub fn num_arg(args: &Map<String, Value>, key: &str) -> f64 {
    args.get(key).and_then(Value::as_f64).unwrap_or(0.0)
}

pub fn list_arg(args: &Map<String, Value>, key: &str) -> Vec<String> {
    match args.get(key) {
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|item| item.as_str().map(String::from))
            .collect(),
        _ => Vec::new(),
    }
}

pub fn check_account(ctx: &ToolContext, account_id: Option<&str>) -> Result<(), ToolError> {
    let id = match account_id {
        Some(found) => found,
        None => return Ok(()),
    };
    let allowed = match &ctx.allowed_account_ids {
        None => true,
        Some(set) => set.contains(id),
    };
    let exists = {
        let conn = ctx.container.db.lock();
        conn.query_row("SELECT 1 FROM ad_accounts WHERE id = ?1", [id], |row| row.get::<_, i64>(0))
            .optional()?
            .is_some()
    };
    if !allowed || !exists {
        return Err(ToolError::Failure(ToolFailure::with_hint(
            "unknown_account",
            format!("Unknown ad account: {}", id),
            "Call list_ad_accounts for valid ids",
        )));
    }
    Ok(())
}

pub fn pick(row: &Map<String, Value>, metrics: &[String]) -> Map<String, Value> {
    let mut out: Map<String, Value> = Map::new();
    for (key, value) in row {
        if !DEFAULT_METRICS.contains(&key.as_str()) && !["ctr", "cpc", "cpm"].contains(&key.as_str()) {
            out.insert(key.clone(), value.clone());
        }
    }
    for metric in metrics {
        let value = row.get(metric).cloned().unwrap_or(Value::Null);
        let rounded = match &value {
            Value::Number(number) if number.is_f64() => json!(py_round(number.as_f64().unwrap_or(0.0), 4)),
            _ => value,
        };
        out.insert(metric.clone(), rounded);
    }
    out
}
