use crate::db::Db;
use crate::models::{add_days, valid_day};
use crate::ports::tool::ToolFailure;
use crate::services::ads::{derive_metrics, window_filter};
use rusqlite::params_from_iter;
use serde_json::{json, Map, Value};
use std::collections::HashMap;

pub fn resolve_range(value: &str, today: &str) -> Result<(String, String), ToolFailure> {
    let key = value.trim().to_lowercase();
    if key == "today" {
        return Ok((today.to_string(), today.to_string()));
    }
    if key == "yesterday" {
        let day = add_days(today, -1);
        return Ok((day.clone(), day));
    }
    if key == "last_7d" {
        return Ok((add_days(today, -6), today.to_string()));
    }
    if key == "last_30d" {
        return Ok((add_days(today, -29), today.to_string()));
    }
    if key == "this_month" {
        let head: String = today.chars().take(8).collect();
        return Ok((format!("{}01", head), today.to_string()));
    }
    if key.len() == 22 && key.is_char_boundary(10) && key.is_char_boundary(12) && &key[10..12] == ".." {
        let start = &key[..10];
        let end = &key[12..];
        if valid_day(start) && valid_day(end) && start <= end {
            return Ok((start.to_string(), end.to_string()));
        }
    }
    Err(ToolFailure::with_hint(
        "invalid_date_range",
        format!("Unrecognized date range: {}", value),
        "Use today, yesterday, last_7d, last_30d, this_month or YYYY-MM-DD..YYYY-MM-DD",
    ))
}

pub fn aggregate(
    db: &Db,
    start: &str,
    end: &str,
    account_id: Option<&str>,
    level: &str,
) -> rusqlite::Result<Vec<Map<String, Value>>> {
    let keys = match level {
        "day" => "day",
        "account" => "account_id",
        _ => "account_id, campaign_id",
    };
    let offset: usize = if keys.contains(',') { 2 } else { 1 };
    let (filter, params) = window_filter(start, end, account_id);
    let order = if level == "day" { "day" } else { "SUM(spend) DESC" };
    let sql = format!(
        "SELECT {keys}, MAX(campaign_name), SUM(spend), SUM(impressions), SUM(clicks), SUM(conversions), SUM(conversion_value)
         FROM daily_insights WHERE {filter} GROUP BY {keys} ORDER BY {order}",
        keys = keys,
        filter = filter,
        order = order
    );
    let conn = db.lock();
    let mut statement = conn.prepare(&sql)?;
    let rows = statement
        .query_map(params_from_iter(params.iter()), |row| {
            let first: String = row.get(0)?;
            let second: Option<String> = if offset == 2 { Some(row.get::<_, String>(1)?) } else { None };
            let name: Option<String> = row.get(offset)?;
            let spend = row.get::<_, Option<f64>>(offset + 1)?.unwrap_or(0.0);
            let impressions = row.get::<_, Option<f64>>(offset + 2)?.unwrap_or(0.0);
            let clicks = row.get::<_, Option<f64>>(offset + 3)?.unwrap_or(0.0);
            let conversions = row.get::<_, Option<f64>>(offset + 4)?.unwrap_or(0.0);
            let value = row.get::<_, Option<f64>>(offset + 5)?.unwrap_or(0.0);
            Ok((first, second, name.unwrap_or_default(), spend, impressions, clicks, conversions, value))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    let mut names: HashMap<String, String> = HashMap::new();
    {
        let mut statement = conn.prepare("SELECT id, name FROM ad_accounts")?;
        let accounts = statement
            .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))?
            .collect::<Result<Vec<_>, _>>()?;
        for (id, name) in accounts {
            names.insert(id, name);
        }
    }
    let mut result: Vec<Map<String, Value>> = Vec::new();
    for (first, second, name, spend, impressions, clicks, conversions, value) in rows {
        let metrics = derive_metrics(spend, impressions.trunc() as i64, clicks.trunc() as i64, conversions, value);
        let mut entry: Map<String, Value> = Map::new();
        if level == "day" {
            entry.insert("day".to_string(), json!(first));
        } else if level == "account" {
            let account_name = names.get(&first).cloned().unwrap_or_else(|| first.clone());
            entry.insert("account_id".to_string(), json!(first));
            entry.insert("account_name".to_string(), json!(account_name));
        } else {
            entry.insert("account_id".to_string(), json!(first));
            entry.insert("campaign_id".to_string(), json!(second.unwrap_or_default()));
            entry.insert("campaign_name".to_string(), json!(name));
        }
        if let Ok(Value::Object(extra)) = serde_json::to_value(&metrics) {
            for (key, item) in extra {
                entry.insert(key, item);
            }
        }
        result.push(entry);
    }
    Ok(result)
}
