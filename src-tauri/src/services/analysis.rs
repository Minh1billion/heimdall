use crate::db::Db;
use crate::models::{add_days, py_round};
use crate::services::query::aggregate;
use serde_json::{json, Value};

fn mean(values: &[f64]) -> f64 {
    values.iter().sum::<f64>() / values.len() as f64
}

fn population_stdev(values: &[f64], center: f64) -> f64 {
    (values.iter().map(|value| (value - center).powi(2)).sum::<f64>() / values.len() as f64).sqrt()
}

pub fn detect_anomalies(
    db: &Db,
    end: &str,
    account_id: Option<&str>,
    metric: &str,
    days: i64,
    threshold: f64,
) -> rusqlite::Result<Vec<Value>> {
    let start = add_days(end, -(days - 1));
    let rows = aggregate(db, &start, end, account_id, "day")?;
    let usable: Vec<(String, f64)> = rows
        .iter()
        .filter_map(|row| {
            let value = row.get(metric).and_then(Value::as_f64)?;
            let day = row.get("day").and_then(Value::as_str).unwrap_or("").to_string();
            Some((day, value))
        })
        .collect();
    if usable.len() < 5 {
        return Ok(Vec::new());
    }
    let mut found: Vec<Value> = Vec::new();
    for (index, (day, value)) in usable.iter().enumerate() {
        let others: Vec<f64> = usable
            .iter()
            .enumerate()
            .filter(|(other, _)| *other != index)
            .map(|(_, entry)| entry.1)
            .collect();
        let center = mean(&others);
        let spread = population_stdev(&others, center);
        if spread == 0.0 {
            continue;
        }
        let z = (value - center) / spread;
        if z.abs() >= threshold {
            found.push(json!({
                "day": day,
                "value": py_round(*value, 4),
                "baseline_mean": py_round(center, 4),
                "z_score": py_round(z, 2),
                "direction": if z > 0.0 { "up" } else { "down" },
            }));
        }
    }
    Ok(found)
}
