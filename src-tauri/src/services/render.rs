use crate::models::clip;
use serde_json::{json, Value};

pub const TELEGRAM_LIMIT: usize = 4096;
pub const SLACK_LIMIT: usize = 39000;
pub const ZALO_LIMIT: usize = 2000;

fn fit(full: &str, limit: usize) -> (String, bool) {
    let text = clip(full, limit);
    let clipped = text != full;
    (text, clipped)
}

pub fn telegram_body(subject: &str, text: &str) -> (String, bool) {
    fit(&format!("{}\n\n{}", subject, text), TELEGRAM_LIMIT)
}

pub fn slack_body(subject: &str, text: &str) -> (String, bool) {
    fit(&format!("*{}*\n{}", subject, text), SLACK_LIMIT)
}

pub fn zalo_body(subject: &str, text: &str) -> (String, bool) {
    fit(&format!("{}\n\n{}", subject, text), ZALO_LIMIT)
}

pub fn render_all(subject: &str, text: &str) -> Value {
    let (telegram, telegram_clipped) = telegram_body(subject, text);
    let (slack, slack_clipped) = slack_body(subject, text);
    let (zalo, zalo_clipped) = zalo_body(subject, text);
    json!({
        "telegram": { "subject": Value::Null, "body": telegram, "clipped": telegram_clipped, "limit": TELEGRAM_LIMIT },
        "slack": { "subject": Value::Null, "body": slack, "clipped": slack_clipped, "limit": SLACK_LIMIT },
        "zalo": { "subject": Value::Null, "body": zalo, "clipped": zalo_clipped, "limit": ZALO_LIMIT },
        "email": { "subject": subject, "body": text, "clipped": false, "limit": Value::Null },
    })
}
