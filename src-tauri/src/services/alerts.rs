use crate::container::Container;
use crate::models::{clip, format_number, parse_dt, today_utc, utcnow, AlertRuleRow};
use crate::router::Failure;
use crate::services::ads::{metric_value, totals_for};
use crate::services::query::resolve_range;
use crate::services::render::render_all;
use rusqlite::{params, OptionalExtension};
use serde_json::{json, Value};

fn symbol(operator: &str) -> &'static str {
    if operator == "gt" {
        ">"
    } else if operator == "lt" {
        "<"
    } else {
        "undefined"
    }
}

fn compare(operator: &str, value: f64, limit: f64) -> bool {
    if operator == "gt" {
        value > limit
    } else {
        value < limit
    }
}

pub struct AlertDraft {
    pub name: String,
    pub account_id: Option<String>,
    pub metric: String,
    pub operator: String,
    pub threshold: f64,
    pub period: String,
}

fn scope_of(container: &Container, account_id: Option<&str>) -> rusqlite::Result<String> {
    let found: Option<String> = match account_id.filter(|value| !value.is_empty()) {
        Some(id) => {
            let conn = container.db.lock();
            conn.query_row("SELECT name FROM ad_accounts WHERE id = ?1", [id], |row| row.get::<_, String>(0))
                .optional()?
        }
        None => None,
    };
    Ok(found.unwrap_or_else(|| "all accounts".to_string()))
}

pub fn alert_message(rule: &AlertDraft, value: f64, scope: &str) -> (String, String) {
    (
        format!("Heimdall alert: {}", rule.name),
        format!(
            "{} is {} for {} on {}, which is {} {}.",
            rule.metric,
            format_number(value, 2),
            rule.period,
            scope,
            symbol(&rule.operator),
            format_number(rule.threshold, 2)
        ),
    )
}

pub fn preview_alert(container: &Container, draft: &AlertDraft) -> Result<Value, Failure> {
    let (start, end) = resolve_range(&draft.period, &today_utc())?;
    let totals = totals_for(&container.db, &start, &end, draft.account_id.as_deref())?;
    let current = metric_value(&totals, &draft.metric);
    let sample = current.is_none();
    let margin = f64::max(draft.threshold.abs() * 0.2, 1.0);
    let value = match current {
        Some(found) => found,
        None => draft.threshold + if draft.operator == "gt" { margin } else { -margin },
    };
    let scope = scope_of(container, draft.account_id.as_deref())?;
    let (subject, text) = alert_message(draft, value, &scope);
    let rendered = render_all(&subject, &text);
    Ok(json!({
        "title": subject,
        "text": text,
        "value": if sample { Value::Null } else { json!(value) },
        "sample": sample,
        "breached": compare(&draft.operator, value, draft.threshold),
        "rendered": rendered,
    }))
}

pub async fn evaluate_alerts(container: &Container) -> Result<(), Failure> {
    let now = utcnow();
    let now_ms = parse_dt(&now).map_or(0, |moment| moment.timestamp_millis());
    let ids: Vec<i64> = {
        let conn = container.db.lock();
        let mut statement = conn.prepare("SELECT id FROM alert_rules WHERE enabled = 1")?;
        let collected = statement
            .query_map([], |row| row.get::<_, i64>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        collected
    };
    for id in ids {
        let rule: Option<AlertRuleRow> = {
            let conn = container.db.lock();
            conn.query_row("SELECT * FROM alert_rules WHERE id = ?1", [id], AlertRuleRow::from_row)
                .optional()?
        };
        let rule = match rule {
            Some(found) => found,
            None => continue,
        };
        let (start, end) = resolve_range(&rule.period, &today_utc())?;
        let totals = totals_for(&container.db, &start, &end, rule.account_id.as_deref())?;
        let value = metric_value(&totals, &rule.metric);
        let scope = scope_of(container, rule.account_id.as_deref())?;
        let breached = match value {
            Some(found) => compare(&rule.operator, found, rule.threshold),
            None => false,
        };
        let cooled = match &rule.last_triggered_at {
            None => true,
            Some(last) => {
                let last_ms = parse_dt(last).map_or(i64::MIN / 2, |moment| moment.timestamp_millis());
                now_ms - last_ms >= rule.cooldown_hours * 3600 * 1000
            }
        };
        let mut triggered_at = rule.last_triggered_at.clone();
        let mut last_error = rule.last_error.clone();
        if breached && cooled {
            let draft = AlertDraft {
                name: rule.name.clone(),
                account_id: rule.account_id.clone(),
                metric: rule.metric.clone(),
                operator: rule.operator.clone(),
                threshold: rule.threshold,
                period: rule.period.clone(),
            };
            let (subject, text) = alert_message(&draft, value.unwrap_or(0.0), &scope);
            let outcome = match container.notifier(&rule.channel) {
                Some(notifier) => notifier
                    .send(&subject, &text)
                    .await
                    .map_err(|error| error.0),
                None => Err(format!("{} is not configured", rule.channel)),
            };
            match outcome {
                Ok(()) => {
                    triggered_at = Some(now.clone());
                    last_error = None;
                }
                Err(message) => {
                    eprintln!("Alert {} could not be delivered: {}", rule.name, message);
                    last_error = Some(clip(&message, 500));
                }
            }
        }
        let conn = container.db.lock();
        conn.execute(
            "UPDATE alert_rules SET last_value = ?1, last_triggered_at = ?2, last_error = ?3 WHERE id = ?4",
            params![value, triggered_at, last_error, id],
        )?;
    }
    Ok(())
}
