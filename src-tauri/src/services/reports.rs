use crate::container::Container;
use crate::db::Db;
use crate::models::{add_days, format_number, today_utc};
use crate::ports::llm::{llm_request, message, LLMPort, Role};
use crate::router::Failure;
use crate::services::ads::{currency_for, totals_for, Metrics};
use crate::services::analysis::detect_anomalies;
use crate::services::app_settings::py_float;
use crate::services::query::aggregate;
use crate::services::templates::{TemplateConfig, METRIC_KEYS};
use rusqlite::OptionalExtension;
use serde_json::Value;

const INSIGHT_PROMPT: &str = "You write the closing insight of an ad performance report. Use only the numbers provided. Write two or three short plain-text sentences: what changed, the most likely cause visible in the data, and one thing to check next. Never invent numbers.";

pub struct Report {
    pub title: String,
    pub text: String,
    pub footer: String,
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

fn window_for(template: &str, today: &str) -> ((String, String), (String, String), bool) {
    let yesterday = add_days(today, -1);
    if template == "weekly" {
        let current = (add_days(&yesterday, -6), yesterday.clone());
        let baseline = (add_days(&current.0, -7), add_days(&current.0, -1));
        return (current, baseline, true);
    }
    let before = add_days(&yesterday, -1);
    ((yesterday.clone(), yesterday), (before.clone(), before), false)
}

fn label(key: &str, vi: bool) -> &'static str {
    match (key, vi) {
        ("spend", false) => "Spend",
        ("spend", true) => "Chi tiêu",
        ("impressions", false) => "Impressions",
        ("impressions", true) => "Lượt hiển thị",
        ("clicks", false) => "Clicks",
        ("clicks", true) => "Lượt nhấp",
        ("conversions", false) => "Conversions",
        ("conversions", true) => "Chuyển đổi",
        ("ctr", _) => "CTR",
        ("cpc", _) => "CPC",
        ("cpm", _) => "CPM",
        ("cpa", _) => "CPA",
        ("roas", _) => "ROAS",
        _ => "",
    }
}

fn metric_line(key: &str, now: &Metrics, before: &Metrics, currency: &Option<String>, vi: bool) -> Option<String> {
    let body = match key {
        "spend" => format!(
            "{}{}",
            money(Some(now.spend), currency),
            change(Some(now.spend), Some(before.spend))
        ),
        "impressions" => format!(
            "{}{}",
            format_number(now.impressions as f64, 0),
            change(Some(now.impressions as f64), Some(before.impressions as f64))
        ),
        "clicks" => format!(
            "{}{}",
            format_number(now.clicks as f64, 0),
            change(Some(now.clicks as f64), Some(before.clicks as f64))
        ),
        "conversions" => format!(
            "{}{}",
            format_number(now.conversions, 0),
            change(Some(now.conversions), Some(before.conversions))
        ),
        "ctr" => match now.ctr {
            Some(value) => format!("{:.2}%{}", value * 100.0, change(Some(value), before.ctr)),
            None => "n/a".to_string(),
        },
        "cpc" => format!("{}{}", money(now.cpc, currency), change(now.cpc, before.cpc)),
        "cpm" => format!("{}{}", money(now.cpm, currency), change(now.cpm, before.cpm)),
        "cpa" => format!("{}{}", money(now.cpa, currency), change(now.cpa, before.cpa)),
        "roas" => match now.roas {
            Some(value) => format!("{:.2}x{}", value, change(Some(value), before.roas)),
            None => "n/a".to_string(),
        },
        _ => return None,
    };
    Some(format!("{}: {}", label(key, vi), body))
}

fn fill(text: &str, scope: &str, period: &str) -> String {
    text.replace("{account}", scope).replace("{period}", period)
}

pub fn build_data(
    db: &Db,
    template: &str,
    account_id: Option<&str>,
    today: &str,
    config: &TemplateConfig,
) -> rusqlite::Result<Report> {
    let vi = config.language == "vi";
    let account = account_id.filter(|value| !value.is_empty());
    let (current, baseline, weekly) = window_for(template, today);
    let found_name: Option<String> = match account {
        Some(id) => {
            let conn = db.lock();
            conn.query_row("SELECT name FROM ad_accounts WHERE id = ?1", [id], |row| row.get::<_, String>(0))
                .optional()?
        }
        None => None,
    };
    let scope = found_name.unwrap_or_else(|| if vi { "Tất cả tài khoản" } else { "All accounts" }.to_string());
    let currency = currency_for(db, &baseline.0, &current.1, account)?;
    let now = totals_for(db, &current.0, &current.1, account)?;
    let before = totals_for(db, &baseline.0, &baseline.1, account)?;
    let joiner = if vi { "đến" } else { "to" };
    let period_text = if current.0 != current.1 {
        format!("{} {} {}", current.0, joiner, current.1)
    } else {
        current.0.clone()
    };
    let mut lines: Vec<String> = Vec::new();
    if !config.header.is_empty() {
        lines.push(fill(&config.header, &scope, &period_text));
        lines.push(String::new());
    }
    lines.push(format!("{} · {}", scope, period_text));
    lines.push(String::new());
    for key in METRIC_KEYS {
        if config.metrics.iter().any(|item| item == key) {
            if let Some(line) = metric_line(key, &now, &before, &currency, vi) {
                lines.push(line);
            }
        }
    }
    if config.campaigns > 0 {
        let campaigns: Vec<_> = aggregate(db, &current.0, &current.1, account, "campaign")?
            .into_iter()
            .take(config.campaigns as usize)
            .collect();
        if !campaigns.is_empty() {
            lines.push(String::new());
            lines.push(if vi { "Chiến dịch chi tiêu cao nhất:" } else { "Top campaigns by spend:" }.to_string());
            for row in &campaigns {
                let name = row.get("campaign_name").and_then(Value::as_str).unwrap_or("");
                let spend = row.get("spend").and_then(Value::as_f64);
                lines.push(format!("- {}: {}", name, money(spend, &currency)));
            }
        }
    }
    if config.anomalies {
        let mut flagged: Vec<String> = Vec::new();
        for metric in ["spend", "cpa"] {
            let items = detect_anomalies(db, &add_days(today, -1), account, metric, 14, 2.0)?;
            for item in &items {
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
            lines.push(if vi { "Ngày bất thường (14 ngày gần nhất):" } else { "Unusual days (last 14 days):" }.to_string());
            lines.extend(flagged);
        }
    }
    let title = match (weekly, vi) {
        (true, false) => format!("Weekly ads report · {}", scope),
        (false, false) => format!("Daily ads report · {}", scope),
        (true, true) => format!("Báo cáo quảng cáo hằng tuần · {}", scope),
        (false, true) => format!("Báo cáo quảng cáo hằng ngày · {}", scope),
    };
    Ok(Report {
        title,
        text: lines.join("\n"),
        footer: fill(&config.footer, &scope, &period_text),
    })
}

pub async fn build_report(
    container: &Container,
    template: &str,
    account_id: Option<&str>,
    with_insight: bool,
) -> Result<Report, Failure> {
    build_report_with(container, template, account_id, &TemplateConfig::default(), with_insight).await
}

pub async fn build_report_with(
    container: &Container,
    template: &str,
    account_id: Option<&str>,
    config: &TemplateConfig,
    with_insight: bool,
) -> Result<Report, Failure> {
    let vi = config.language == "vi";
    let mut report = build_data(&container.db, template, account_id, &today_utc(), config)?;
    if with_insight && config.insight && !container.ordered_providers().is_empty() {
        if let Ok(llm) = container.router_for(None) {
            let prompt = if vi {
                format!("{} Write the insight in Vietnamese.", INSIGHT_PROMPT)
            } else {
                INSIGHT_PROMPT.to_string()
            };
            let mut request = llm_request(vec![
                message(Role::System, Some(prompt.as_str())),
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
                        report.text = format!(
                            "{}\n\n{}:\n{}",
                            report.text,
                            if vi { "Nhận định" } else { "Insight" },
                            content.trim()
                        );
                    }
                }
            }
        }
    }
    if !report.footer.is_empty() {
        report.text = format!("{}\n\n{}", report.text, report.footer);
    }
    Ok(report)
}
