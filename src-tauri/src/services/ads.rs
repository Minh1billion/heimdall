use crate::db::Db;
use crate::models::{add_days, today_utc};
use crate::services::dashboard::RangeKey;
use rusqlite::params_from_iter;
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::HashMap;

#[derive(Serialize, Clone, Debug)]
pub struct Metrics {
    pub spend: f64,
    pub impressions: i64,
    pub clicks: i64,
    pub conversions: f64,
    pub ctr: Option<f64>,
    pub cpc: Option<f64>,
    pub cpm: Option<f64>,
    pub cpa: Option<f64>,
    pub roas: Option<f64>,
}

fn ratio(numerator: f64, denominator: f64) -> Option<f64> {
    if denominator != 0.0 && !denominator.is_nan() {
        Some(numerator / denominator)
    } else {
        None
    }
}

pub fn derive_metrics(spend: f64, impressions: i64, clicks: i64, conversions: f64, value: f64) -> Metrics {
    Metrics {
        spend,
        impressions,
        clicks,
        conversions,
        ctr: ratio(clicks as f64, impressions as f64),
        cpc: ratio(spend, clicks as f64),
        cpm: ratio(spend * 1000.0, impressions as f64),
        cpa: ratio(spend, conversions),
        roas: if value != 0.0 { ratio(value, spend) } else { None },
    }
}

pub fn window_filter(start: &str, end: &str, account_id: Option<&str>) -> (String, Vec<String>) {
    let mut conditions = vec!["day >= ?", "day <= ?"];
    let mut params = vec![start.to_string(), end.to_string()];
    if let Some(account) = account_id {
        if !account.is_empty() {
            conditions.push("account_id = ?");
            params.push(account.to_string());
        }
    }
    (conditions.join(" AND "), params)
}

pub fn totals_for(db: &Db, start: &str, end: &str, account_id: Option<&str>) -> rusqlite::Result<Metrics> {
    let (filter, params) = window_filter(start, end, account_id);
    let sql = format!(
        "SELECT COALESCE(SUM(spend), 0.0), COALESCE(SUM(impressions), 0), COALESCE(SUM(clicks), 0),
                COALESCE(SUM(conversions), 0.0), COALESCE(SUM(conversion_value), 0.0)
         FROM daily_insights WHERE {}",
        filter
    );
    let conn = db.lock();
    conn.query_row(&sql, params_from_iter(params.iter()), |row| {
        Ok(derive_metrics(
            row.get::<_, f64>(0)?,
            row.get::<_, f64>(1)?.trunc() as i64,
            row.get::<_, f64>(2)?.trunc() as i64,
            row.get::<_, f64>(3)?,
            row.get::<_, f64>(4)?,
        ))
    })
}

pub fn currency_for(db: &Db, start: &str, end: &str, account_id: Option<&str>) -> rusqlite::Result<Option<String>> {
    let rows: Vec<String> = {
        let conn = db.lock();
        match account_id {
            Some(account) if !account.is_empty() => {
                let mut statement = conn.prepare("SELECT DISTINCT currency FROM ad_accounts WHERE id = ?1")?;
                let collected = statement
                    .query_map([account], |row| row.get::<_, String>(0))?
                    .collect::<Result<Vec<_>, _>>()?;
                collected
            }
            _ => {
                let mut statement = conn.prepare(
                    "SELECT DISTINCT ad_accounts.currency AS currency FROM ad_accounts
                     JOIN daily_insights ON daily_insights.account_id = ad_accounts.id
                     WHERE daily_insights.day >= ?1 AND daily_insights.day <= ?2",
                )?;
                let collected = statement
                    .query_map([start, end], |row| row.get::<_, String>(0))?
                    .collect::<Result<Vec<_>, _>>()?;
                collected
            }
        }
    };
    let mut currencies: Vec<String> = Vec::new();
    for value in rows {
        if !value.is_empty() && !currencies.contains(&value) {
            currencies.push(value);
        }
    }
    if currencies.len() == 1 {
        Ok(currencies.pop())
    } else {
        Ok(None)
    }
}

struct Listed {
    account_id: String,
    id: String,
    name: String,
    objective: Option<String>,
    status: String,
    effective_status: Option<String>,
    daily_budget: Option<f64>,
    created_time: Option<String>,
}

fn with_metrics(base: Value, metrics: &Metrics) -> Value {
    let mut merged = base;
    if let (Value::Object(target), Ok(Value::Object(extra))) = (&mut merged, serde_json::to_value(metrics)) {
        for (key, value) in extra {
            target.insert(key, value);
        }
    }
    merged
}

pub fn ads_insights(db: &Db, range: RangeKey, account_id: Option<&str>) -> rusqlite::Result<Value> {
    let account = account_id.filter(|value| !value.is_empty());
    let days = range.days();
    let latest: Option<String> = {
        let conn = db.lock();
        conn.query_row("SELECT MAX(day) FROM daily_insights", [], |row| row.get::<_, Option<String>>(0))?
    };
    let today = today_utc();
    let end = match latest {
        Some(day) if day > today => day,
        _ => today,
    };
    let start = add_days(&end, -(days - 1));
    let previous_end = add_days(&start, -1);
    let previous_start = add_days(&previous_end, -(days - 1));
    let (filter, params) = window_filter(&start, &end, account);

    let mut by_day: HashMap<String, (f64, f64, f64)> = HashMap::new();
    let campaign_rows: Vec<(String, String, String, f64, f64, f64, f64, f64)>;
    let listed: Vec<Listed>;
    {
        let conn = db.lock();
        let day_sql = format!(
            "SELECT day, SUM(spend), SUM(clicks), SUM(conversions) FROM daily_insights WHERE {} GROUP BY day",
            filter
        );
        let mut statement = conn.prepare(&day_sql)?;
        let day_rows = statement
            .query_map(params_from_iter(params.iter()), |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<f64>>(1)?.unwrap_or(0.0),
                    row.get::<_, Option<f64>>(2)?.unwrap_or(0.0),
                    row.get::<_, Option<f64>>(3)?.unwrap_or(0.0),
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        for (day, spend, clicks, conversions) in day_rows {
            by_day.insert(day, (spend, clicks, conversions));
        }
        let campaign_sql = format!(
            "SELECT account_id, campaign_id, MAX(campaign_name), SUM(spend) AS total_spend, SUM(impressions), SUM(clicks),
                    SUM(conversions), SUM(conversion_value)
             FROM daily_insights WHERE {} GROUP BY account_id, campaign_id ORDER BY total_spend DESC LIMIT 50",
            filter
        );
        let mut statement = conn.prepare(&campaign_sql)?;
        campaign_rows = statement
            .query_map(params_from_iter(params.iter()), |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?.unwrap_or_default(),
                    row.get::<_, Option<f64>>(3)?.unwrap_or(0.0),
                    row.get::<_, Option<f64>>(4)?.unwrap_or(0.0),
                    row.get::<_, Option<f64>>(5)?.unwrap_or(0.0),
                    row.get::<_, Option<f64>>(6)?.unwrap_or(0.0),
                    row.get::<_, Option<f64>>(7)?.unwrap_or(0.0),
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        let listed_sql = format!(
            "SELECT account_id, id, name, objective, status, effective_status, daily_budget, created_time FROM campaigns {}",
            if account.is_some() { "WHERE account_id = ?1" } else { "" }
        );
        let mut statement = conn.prepare(&listed_sql)?;
        let map_row = |row: &rusqlite::Row| -> rusqlite::Result<Listed> {
            Ok(Listed {
                account_id: row.get(0)?,
                id: row.get(1)?,
                name: row.get(2)?,
                objective: row.get(3)?,
                status: row.get(4)?,
                effective_status: row.get(5)?,
                daily_budget: row.get(6)?,
                created_time: row.get(7)?,
            })
        };
        listed = match account {
            Some(value) => statement.query_map([value], map_row)?.collect::<Result<Vec<_>, _>>()?,
            None => statement.query_map([], map_row)?.collect::<Result<Vec<_>, _>>()?,
        };
    }

    let mut series: Vec<Value> = Vec::new();
    for offset in 0..days {
        let day = add_days(&start, offset);
        let (spend, clicks, conversions) = by_day.get(&day).copied().unwrap_or((0.0, 0.0, 0.0));
        series.push(json!({
            "day": day,
            "spend": spend,
            "clicks": clicks.trunc() as i64,
            "conversions": conversions,
        }));
    }

    let mut known: HashMap<String, Listed> = HashMap::new();
    for entry in listed {
        known.insert(format!("{}:{}", entry.account_id, entry.id), entry);
    }

    let mut campaigns: Vec<Value> = Vec::new();
    for (row_account, row_campaign, row_name, spend, impressions, clicks, conversions, value) in campaign_rows {
        let entry = known.remove(&format!("{}:{}", row_account, row_campaign));
        let name = match &entry {
            Some(found) if !found.name.is_empty() => found.name.clone(),
            _ => row_name,
        };
        let status: Value = match &entry {
            Some(found) => match &found.effective_status {
                Some(effective) => json!(effective),
                None => json!(found.status),
            },
            None => Value::Null,
        };
        let objective: Value = match &entry {
            Some(found) => json!(found.objective),
            None => Value::Null,
        };
        let budget: Value = match &entry {
            Some(found) => json!(found.daily_budget),
            None => Value::Null,
        };
        let base = json!({
            "account_id": row_account,
            "campaign_id": row_campaign,
            "name": name,
            "status": status,
            "objective": objective,
            "daily_budget": budget,
        });
        campaigns.push(with_metrics(
            base,
            &derive_metrics(spend, impressions.trunc() as i64, clicks.trunc() as i64, conversions, value),
        ));
    }

    let mut idle: Vec<Listed> = known.into_values().collect();
    idle.sort_by(|a, b| {
        let left = b.created_time.clone().unwrap_or_default();
        let right = a.created_time.clone().unwrap_or_default();
        left.cmp(&right).then_with(|| {
            a.name
                .to_lowercase()
                .cmp(&b.name.to_lowercase())
                .then_with(|| a.name.cmp(&b.name))
        })
    });
    for entry in idle {
        let status: Value = match &entry.effective_status {
            Some(effective) => json!(effective),
            None => json!(entry.status),
        };
        let base = json!({
            "account_id": entry.account_id,
            "campaign_id": entry.id,
            "name": entry.name,
            "status": status,
            "objective": entry.objective,
            "daily_budget": entry.daily_budget,
        });
        campaigns.push(with_metrics(base, &derive_metrics(0.0, 0, 0, 0.0, 0.0)));
    }

    Ok(json!({
        "range": range.as_str(),
        "start": start,
        "end": end,
        "currency": currency_for(db, &start, &end, account)?,
        "totals": totals_for(db, &start, &end, account)?,
        "previous": totals_for(db, &previous_start, &previous_end, account)?,
        "series": series,
        "campaigns": campaigns,
    }))
}

pub fn metric_value(metrics: &Metrics, name: &str) -> Option<f64> {
    match name {
        "spend" => Some(metrics.spend),
        "impressions" => Some(metrics.impressions as f64),
        "clicks" => Some(metrics.clicks as f64),
        "conversions" => Some(metrics.conversions),
        "ctr" => metrics.ctr,
        "cpc" => metrics.cpc,
        "cpm" => metrics.cpm,
        "cpa" => metrics.cpa,
        "roas" => metrics.roas,
        _ => None,
    }
}
