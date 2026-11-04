use crate::db::Db;
use crate::models::{fmt_dt, parse_dt, py_round};
use chrono::{DateTime, Utc};
use serde_json::{json, Value};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RangeKey {
    Day1,
    Day7,
    Day14,
    Day30,
}

impl RangeKey {
    pub fn parse(value: &str) -> Option<RangeKey> {
        match value {
            "24h" => Some(RangeKey::Day1),
            "7d" => Some(RangeKey::Day7),
            "14d" => Some(RangeKey::Day14),
            "30d" => Some(RangeKey::Day30),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            RangeKey::Day1 => "24h",
            RangeKey::Day7 => "7d",
            RangeKey::Day14 => "14d",
            RangeKey::Day30 => "30d",
        }
    }

    pub fn days(&self) -> i64 {
        match self {
            RangeKey::Day1 => 1,
            RangeKey::Day7 => 7,
            RangeKey::Day14 => 14,
            RangeKey::Day30 => 30,
        }
    }
}

fn bucket_label(millis: i64) -> String {
    match DateTime::<Utc>::from_timestamp_millis(millis) {
        Some(moment) => moment.format("%Y-%m-%dT%H:%M:%SZ").to_string(),
        None => String::new(),
    }
}

pub fn llm_dashboard(db: &Db, range: RangeKey, now: DateTime<Utc>) -> rusqlite::Result<Value> {
    let (count, hourly): (i64, bool) = match range {
        RangeKey::Day1 => (24, true),
        RangeKey::Day7 => (7, false),
        RangeKey::Day14 => (14, false),
        RangeKey::Day30 => (30, false),
    };
    let step: i64 = if hourly { 3_600_000 } else { 86_400_000 };
    let millis = now.timestamp_millis();
    let anchor = millis - millis.rem_euclid(step);
    let start = anchor - step * (count - 1);
    let start_text = match DateTime::<Utc>::from_timestamp_millis(start) {
        Some(moment) => fmt_dt(moment),
        None => String::new(),
    };
    let rows: Vec<(String, i64, i64, i64, i64, i64)> = {
        let conn = db.lock();
        let mut statement = conn.prepare(
            "SELECT created_at, input_tokens, output_tokens, cached_tokens, latency_ms, ok FROM llm_usage WHERE created_at >= ?1",
        )?;
        let collected = statement
            .query_map([&start_text], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, i64>(5)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        collected
    };
    let mut buckets: BTreeMap<i64, (i64, i64)> = BTreeMap::new();
    for index in 0..count {
        buckets.insert(start + step * index, (0, 0));
    }
    let mut total_tokens: i64 = 0;
    let mut successes: i64 = 0;
    let mut cached: i64 = 0;
    let mut input_total: i64 = 0;
    let mut latency_total: i64 = 0;
    for (created_at, input_tokens, output_tokens, cached_tokens, latency_ms, ok) in &rows {
        let tokens = input_tokens + output_tokens;
        total_tokens += tokens;
        if let Some(moment) = parse_dt(created_at) {
            let stamp = moment.timestamp_millis();
            let key = stamp - stamp.rem_euclid(step);
            if let Some(bucket) = buckets.get_mut(&key) {
                bucket.0 += tokens;
                bucket.1 += 1;
            }
        }
        if *ok != 0 {
            successes += 1;
            cached += cached_tokens;
            input_total += input_tokens;
            latency_total += latency_ms;
        }
    }
    let total_requests = rows.len() as i64;
    let series: Vec<Value> = buckets
        .iter()
        .map(|(key, values)| {
            json!({
                "bucket": bucket_label(*key),
                "tokens": values.0,
                "requests": values.1,
            })
        })
        .collect();
    let success_rate: Value = if total_requests > 0 {
        json!(successes as f64 / total_requests as f64)
    } else {
        Value::Null
    };
    let cache_hit_rate: Value = if input_total > 0 {
        json!(cached as f64 / input_total as f64)
    } else {
        Value::Null
    };
    let avg_latency: Value = if successes > 0 {
        json!(py_round(latency_total as f64 / successes as f64, 0) as i64)
    } else {
        Value::Null
    };
    Ok(json!({
        "range": range.as_str(),
        "total_tokens": total_tokens,
        "total_requests": total_requests,
        "success_rate": success_rate,
        "cache_hit_rate": cache_hit_rate,
        "avg_latency_ms": avg_latency,
        "series": series,
    }))
}
