use crate::container::Container;
use crate::db::Db;
use crate::models::{add_days, format_number, today_utc};
use crate::ports::llm::{llm_request, message, LLMPort, Role};
use crate::router::Failure;
use crate::services::ads::{currency_for, totals_for};
use crate::services::analysis::detect_anomalies;
use crate::services::app_settings::py_float;
use crate::services::query::aggregate;
use rusqlite::OptionalExtension;
use serde_json::Value;

const INSIGHT_PROMPT: &str = "You write the closing insight of an ad performance report. Use only the numbers provided. Write two or three short plain-text sentences: what changed, the most likely cause visible in the data, and one thing to check next. Never invent numbers.";

pub struct Report {
    pub title: String,
    pub text: String,
}

fn money(value: Option<f64>, currency: &Option<String>) -> String {
    match value {
        None => "n/a".to_string(),
        Some(amount) => match currency {
            Some(code) if !code.is_empty() => format!("{} {}", format_number(amount, 2), code),
            _ => format_number(amount, 2),
        },
    }
}

fn change(current: Option<f64>, baseline: Option<f64>) -> String {
    let current = match current {
        Some(value) => value,
        None => return String::new(),
    };
    let baseline = match baseline {
        Some(value) if value != 0.0 && !value.is_nan() => value,
        _ => return String::new(),
    };
    let percent = ((current - baseline) / baseline) * 100.0;
    let text = format!("{:.1}", percent);
    let sign = if text.starts_with('-') { "" } else { "+" };
    format!(" ({}{}%)", sign, text)
}

fn window_for(template: &str, today: &str) -> ((String, String), (String, String), &'static str) {
    let yesterday = add_days(today, -1);
    if template == "weekly" {
        let current = (add_days(&yesterday, -6), yesterday.clone());
        let baseline = (add_days(&current.0, -7), add_days(&current.0, -1));
        return (current, baseline, "Weekly");
    }
    let before = add_days(&yesterday, -1);
    ((yesterday.clone(), yesterday), (before.clone(), before), "Daily")
}

pub fn build_data(db: &Db, template: &str, account_id: Option<&str>, today: &str) -> rusqlite::Result<Report> {
    let account = account_id.filter(|value| !value.is_empty());
    let (current, baseline, label) = window_for(template, today);
    let found_name: Option<String> = match account {
        Some(id) => {
            let conn = db.lock();
            conn.query_row("SELECT name FROM ad_accounts WHERE id = ?1", [id], |row| row.get::<_, String>(0))
                .optional()?
        }
        None => None,
    };
    let scope = found_name.unwrap_or_else(|| "All accounts".to_string());
    let currency = currency_for(db, &baseline.0, &current.1, account)?;
    let now = totals_for(db, &current.0, &current.1, account)?;
    let before = totals_for(db, &baseline.0, &baseline.1, account)?;
    let campaigns: Vec<_> = aggregate(db, &current.0, &current.1, account, "campaign")?
        .into_iter()
        .take(3)
        .collect();
    let mut anomalies: Vec<(&str, Vec<Value>)> = Vec::new();
    for metric in ["spend", "cpa"] {
        anomalies.push((metric, detect_anomalies(db, &add_days(today, -1), account, metric, 14, 2.0)?));
    }
    let mut lines: Vec<String> = vec![
        if current.0 != current.1 {
            format!("{} · {} to {}", scope, current.0, current.1)
        } else {
            format!("{} · {}", scope, current.0)
        },
        String::new(),
        format!("Spend: {}{}", money(Some(now.spend), &currency), change(Some(now.spend), Some(before.spend))),
        format!(
            "Impressions: {}{}",
            format_number(now.impressions as f64, 0),
            change(Some(now.impressions as f64), Some(before.impressions as f64))
        ),
        format!(
            "Clicks: {}{}",
            format_number(now.clicks as f64, 0),
            change(Some(now.clicks as f64), Some(before.clicks as f64))
        ),
        format!(
            "Conversions: {}{}",
            format_number(now.conversions, 0),
            change(Some(now.conversions), Some(before.conversions))
        ),
        format!("CPA: {}{}", money(now.cpa, &currency), change(now.cpa, before.cpa)),
    ];
    match now.roas {
        Some(roas) => lines.push(format!("ROAS: {:.2}x{}", roas, change(Some(roas), before.roas))),
        None => lines.push("ROAS: n/a".to_string()),
    }
    if !campaigns.is_empty() {
        lines.push(String::new());
        lines.push("Top campaigns by spend:".to_string());
        for row in &campaigns {
            let name = row.get("campaign_name").and_then(Value::as_str).unwrap_or("");
            let spend = row.get("spend").and_then(Value::as_f64);
            lines.push(format!("- {}: {}", name, money(spend, &currency)));
        }
    }
    let mut flagged: Vec<String> = Vec::new();
    for (metric, items) in &anomalies {
        for item in items {
            let direction = item.get("direction").and_then(Value::as_str).unwrap_or("");
            let day = item.get("day").and_then(Value::as_str).unwrap_or("");
            let value = item.get("value").and_then(Value::as_f64).unwrap_or(0.0);
            let z = item.get("z_score").and_then(Value::as_f64).unwrap_or(0.0);
            flagged.push(format!(
                "- {} {} on {}: {} (z {})",
                metric.to_uppercase(),
                direction,
                day,
                py_float(value),
                py_float(z)
            ));
        }
    }
    if !flagged.is_empty() {
        lines.push(String::new());
        lines.push("Unusual days (last 14 days):".to_string());
        lines.extend(flagged);
    }
    Ok(Report {
        title: format!("{} ads report · {}", label, scope),
        text: lines.join("\n"),
    })
}

pub async fn build_report(
    container: &Container,
    template: &str,
    account_id: Option<&str>,
    with_insight: bool,
) -> Result<Report, Failure> {
    let mut report = build_data(&container.db, template, account_id, &today_utc())?;
    if with_insight && !container.ordered_providers().is_empty() {
        if let Ok(llm) = container.router_for(None) {
            let mut request = llm_request(vec![
                message(Role::System, Some(INSIGHT_PROMPT)),
                message(Role::User, Some(&report.text)),
            ]);
            request.max_output_tokens = 300;
            request.timeout_s = container.settings().llm_timeout_s;
            request
                .metadata
                .insert("run_id".to_string(), Value::String(format!("report-{}", template)));
            if let Ok(response) = llm.generate(&request).await {
                if let Some(content) = response.message.content.as_deref() {
                    if !content.is_empty() {
                        report.text = format!("{}\n\nInsight:\n{}", report.text, content.trim());
                    }
                }
            }
        }
    }
    Ok(report)
}
