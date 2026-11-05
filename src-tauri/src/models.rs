pub const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS llm_usage (
  id INTEGER NOT NULL,
  created_at DATETIME NOT NULL,
  provider VARCHAR(64) NOT NULL,
  model VARCHAR(128) NOT NULL,
  input_tokens INTEGER NOT NULL,
  output_tokens INTEGER NOT NULL,
  cached_tokens INTEGER NOT NULL,
  latency_ms INTEGER NOT NULL,
  ok BOOLEAN NOT NULL,
  error_code VARCHAR(64),
  run_id VARCHAR(64),
  PRIMARY KEY (id)
);
CREATE INDEX IF NOT EXISTS ix_llm_usage_created_at ON llm_usage (created_at);

CREATE TABLE IF NOT EXISTS ad_accounts (
  id VARCHAR(64) NOT NULL,
  name VARCHAR(255) NOT NULL,
  currency VARCHAR(8) NOT NULL,
  timezone VARCHAR(64) NOT NULL,
  status VARCHAR(32) NOT NULL,
  last_synced_at DATETIME,
  last_error VARCHAR(500),
  connection_id INTEGER,
  PRIMARY KEY (id)
);

CREATE TABLE IF NOT EXISTS meta_connections (
  id INTEGER NOT NULL,
  name VARCHAR(80) NOT NULL,
  token TEXT NOT NULL,
  enabled BOOLEAN NOT NULL,
  created_at DATETIME NOT NULL,
  last_synced_at DATETIME,
  last_error VARCHAR(500),
  PRIMARY KEY (id)
);

CREATE TABLE IF NOT EXISTS daily_insights (
  id INTEGER NOT NULL,
  account_id VARCHAR(64) NOT NULL,
  campaign_id VARCHAR(64) NOT NULL,
  campaign_name VARCHAR(512) NOT NULL,
  day DATE NOT NULL,
  spend FLOAT NOT NULL,
  impressions INTEGER NOT NULL,
  clicks INTEGER NOT NULL,
  conversions FLOAT NOT NULL,
  conversion_value FLOAT NOT NULL,
  PRIMARY KEY (id),
  UNIQUE (account_id, campaign_id, day),
  FOREIGN KEY(account_id) REFERENCES ad_accounts (id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS ix_daily_insights_account_id ON daily_insights (account_id);
CREATE INDEX IF NOT EXISTS ix_daily_insights_day ON daily_insights (day);

CREATE TABLE IF NOT EXISTS campaigns (
  account_id VARCHAR(64) NOT NULL,
  id VARCHAR(64) NOT NULL,
  name VARCHAR(512) NOT NULL,
  objective VARCHAR(64),
  status VARCHAR(32) NOT NULL,
  effective_status VARCHAR(32),
  daily_budget FLOAT,
  lifetime_budget FLOAT,
  created_time DATETIME,
  PRIMARY KEY (account_id, id),
  FOREIGN KEY(account_id) REFERENCES ad_accounts (id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS conversations (
  id VARCHAR(36) NOT NULL,
  title VARCHAR(120) NOT NULL,
  created_at DATETIME NOT NULL,
  updated_at DATETIME NOT NULL,
  PRIMARY KEY (id)
);
CREATE INDEX IF NOT EXISTS ix_conversations_updated_at ON conversations (updated_at);

CREATE TABLE IF NOT EXISTS chat_messages (
  id INTEGER NOT NULL,
  conversation_id VARCHAR(36) NOT NULL,
  role VARCHAR(16) NOT NULL,
  content TEXT NOT NULL,
  run_id VARCHAR(36),
  created_at DATETIME NOT NULL,
  PRIMARY KEY (id),
  FOREIGN KEY(conversation_id) REFERENCES conversations (id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS ix_chat_messages_conversation_id ON chat_messages (conversation_id);

CREATE TABLE IF NOT EXISTS run_charts (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  run_id VARCHAR(36) NOT NULL,
  spec TEXT NOT NULL,
  created_at DATETIME NOT NULL
);
CREATE INDEX IF NOT EXISTS ix_run_charts_run_id ON run_charts (run_id);

CREATE TABLE IF NOT EXISTS agent_runs (
  id VARCHAR(36) NOT NULL,
  created_at DATETIME NOT NULL,
  conversation_id VARCHAR(36),
  input TEXT NOT NULL,
  output TEXT NOT NULL,
  status VARCHAR(32) NOT NULL,
  steps INTEGER NOT NULL,
  total_tokens INTEGER NOT NULL,
  duration_ms INTEGER NOT NULL,
  error VARCHAR(500),
  PRIMARY KEY (id)
);
CREATE INDEX IF NOT EXISTS ix_agent_runs_created_at ON agent_runs (created_at);

CREATE TABLE IF NOT EXISTS tool_calls (
  id INTEGER NOT NULL,
  run_id VARCHAR(36) NOT NULL,
  created_at DATETIME NOT NULL,
  step INTEGER NOT NULL,
  tool_name VARCHAR(64) NOT NULL,
  arguments TEXT NOT NULL,
  ok BOOLEAN NOT NULL,
  error_code VARCHAR(64),
  latency_ms INTEGER NOT NULL,
  result VARCHAR(600) NOT NULL,
  PRIMARY KEY (id)
);
CREATE INDEX IF NOT EXISTS ix_tool_calls_run_id ON tool_calls (run_id);

CREATE TABLE IF NOT EXISTS report_schedules (
  id INTEGER NOT NULL,
  name VARCHAR(120) NOT NULL,
  template VARCHAR(32) NOT NULL,
  account_id VARCHAR(64),
  channel VARCHAR(32) NOT NULL,
  time VARCHAR(5) NOT NULL,
  weekday INTEGER,
  enabled BOOLEAN NOT NULL,
  last_run_at DATETIME,
  last_error VARCHAR(500),
  PRIMARY KEY (id)
);

CREATE TABLE IF NOT EXISTS alert_rules (
  id INTEGER NOT NULL,
  name VARCHAR(120) NOT NULL,
  account_id VARCHAR(64),
  metric VARCHAR(16) NOT NULL,
  operator VARCHAR(4) NOT NULL,
  threshold FLOAT NOT NULL,
  period VARCHAR(16) NOT NULL,
  channel VARCHAR(32) NOT NULL,
  cooldown_hours INTEGER NOT NULL,
  enabled BOOLEAN NOT NULL,
  last_triggered_at DATETIME,
  last_value FLOAT,
  last_error VARCHAR(500),
  PRIMARY KEY (id)
);

CREATE TABLE IF NOT EXISTS approval_requests (
  id INTEGER NOT NULL,
  created_at DATETIME NOT NULL,
  run_id VARCHAR(36),
  tool_name VARCHAR(64) NOT NULL,
  account_id VARCHAR(64) NOT NULL,
  campaign_id VARCHAR(64) NOT NULL,
  campaign_name VARCHAR(512) NOT NULL,
  summary VARCHAR(500) NOT NULL,
  reason VARCHAR(500) NOT NULL,
  payload TEXT NOT NULL,
  status VARCHAR(16) NOT NULL,
  decided_at DATETIME,
  result VARCHAR(500),
  PRIMARY KEY (id)
);
CREATE INDEX IF NOT EXISTS ix_approval_requests_created_at ON approval_requests (created_at);
CREATE INDEX IF NOT EXISTS ix_approval_requests_status ON approval_requests (status);

CREATE TABLE IF NOT EXISTS app_setting (
  "key" VARCHAR(64) NOT NULL,
  value TEXT NOT NULL,
  encrypted BOOLEAN NOT NULL,
  updated_at DATETIME NOT NULL,
  PRIMARY KEY ("key")
);

CREATE TABLE IF NOT EXISTS destinations (
  id INTEGER NOT NULL,
  name VARCHAR(80) NOT NULL,
  channel VARCHAR(32) NOT NULL,
  config TEXT NOT NULL,
  enabled BOOLEAN NOT NULL,
  created_at DATETIME NOT NULL,
  last_sent_at DATETIME,
  last_error VARCHAR(500),
  PRIMARY KEY (id)
);

CREATE TABLE IF NOT EXISTS schedule_destinations (
  schedule_id INTEGER NOT NULL,
  destination_id INTEGER NOT NULL,
  PRIMARY KEY (schedule_id, destination_id),
  FOREIGN KEY(schedule_id) REFERENCES report_schedules (id) ON DELETE CASCADE,
  FOREIGN KEY(destination_id) REFERENCES destinations (id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS report_templates (
    id INTEGER NOT NULL,
    name VARCHAR(80) NOT NULL,
    config TEXT NOT NULL,
    created_at DATETIME NOT NULL,
    PRIMARY KEY (id)
);
"#;

use chrono::{DateTime, Duration, NaiveDate, NaiveDateTime, Utc};
use rusqlite::Row;

pub fn fmt_dt(moment: DateTime<Utc>) -> String {
    format!("{}000", moment.format("%Y-%m-%d %H:%M:%S%.3f"))
}

pub fn utcnow() -> String {
    fmt_dt(Utc::now())
}

pub fn parse_dt(value: &str) -> Option<DateTime<Utc>> {
    let cut: String = value.replace(' ', "T").chars().take(23).collect();
    NaiveDateTime::parse_from_str(&cut, "%Y-%m-%dT%H:%M:%S%.f")
        .or_else(|_| NaiveDateTime::parse_from_str(&cut, "%Y-%m-%dT%H:%M:%S"))
        .ok()
        .map(|naive| naive.and_utc())
}

pub fn iso(value: &str) -> String {
    let replaced = value.replace(' ', "T");
    let mut parts = replaced.splitn(2, '.');
    let head = parts.next().unwrap_or("");
    let fraction = parts.next().unwrap_or("");
    if fraction.chars().any(|digit| ('1'..='9').contains(&digit)) {
        format!("{}.{}Z", head, fraction)
    } else {
        format!("{}Z", head)
    }
}

pub fn iso_or_null(value: &Option<String>) -> Option<String> {
    value.as_deref().filter(|text| !text.is_empty()).map(iso)
}

pub fn today_utc() -> String {
    Utc::now().format("%Y-%m-%d").to_string()
}

pub fn add_days(day: &str, days: i64) -> String {
    match NaiveDate::parse_from_str(day, "%Y-%m-%d") {
        Ok(date) => (date + Duration::days(days)).format("%Y-%m-%d").to_string(),
        Err(_) => day.to_string(),
    }
}

pub fn valid_day(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.len() != 10 || bytes[4] != b'-' || bytes[7] != b'-' {
        return false;
    }
    if !bytes
        .iter()
        .enumerate()
        .all(|(index, byte)| index == 4 || index == 7 || byte.is_ascii_digit())
    {
        return false;
    }
    match NaiveDate::parse_from_str(value, "%Y-%m-%d") {
        Ok(date) => date.format("%Y-%m-%d").to_string() == value,
        Err(_) => false,
    }
}

pub fn py_round(value: f64, digits: i32) -> f64 {
    let factor = 10f64.powi(digits);
    let scaled = value * factor;
    let floor = scaled.floor();
    let diff = scaled - floor;
    let rounded = if diff > 0.5 {
        floor + 1.0
    } else if diff < 0.5 {
        floor
    } else if floor % 2.0 == 0.0 {
        floor
    } else {
        floor + 1.0
    };
    rounded / factor
}

pub fn clip(text: &str, limit: usize) -> String {
    text.chars().take(limit).collect()
}

#[derive(Clone, Debug)]
pub struct AdAccountRow {
    pub id: String,
    pub name: String,
    pub currency: String,
    pub timezone: String,
    pub status: String,
    pub last_synced_at: Option<String>,
    pub last_error: Option<String>,
    pub connection_id: Option<i64>,
}

impl AdAccountRow {
    pub fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(AdAccountRow {
            id: row.get("id")?,
            name: row.get("name")?,
            currency: row.get("currency")?,
            timezone: row.get("timezone")?,
            status: row.get("status")?,
            last_synced_at: row.get("last_synced_at")?,
            last_error: row.get("last_error")?,
            connection_id: row.get("connection_id")?,
        })
    }
}

#[derive(Clone, Debug)]
pub struct MetaConnectionRow {
    pub id: i64,
    pub name: String,
    pub token: String,
    pub enabled: i64,
    pub created_at: String,
    pub last_synced_at: Option<String>,
    pub last_error: Option<String>,
}

impl MetaConnectionRow {
    pub fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(MetaConnectionRow {
            id: row.get("id")?,
            name: row.get("name")?,
            token: row.get("token")?,
            enabled: row.get("enabled")?,
            created_at: row.get("created_at")?,
            last_synced_at: row.get("last_synced_at")?,
            last_error: row.get("last_error")?,
        })
    }
}

#[derive(Clone, Debug)]
pub struct DestinationRow {
    pub id: i64,
    pub name: String,
    pub channel: String,
    pub config: String,
    pub enabled: i64,
    pub created_at: String,
    pub last_sent_at: Option<String>,
    pub last_error: Option<String>,
    pub template_id: Option<i64>,
    pub account_ids: Option<String>,
}

impl DestinationRow {
    pub fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(DestinationRow {
            id: row.get("id")?,
            name: row.get("name")?,
            channel: row.get("channel")?,
            config: row.get("config")?,
            enabled: row.get("enabled")?,
            created_at: row.get("created_at")?,
            last_sent_at: row.get("last_sent_at")?,
            last_error: row.get("last_error")?,
            template_id: row.get("template_id")?,
            account_ids: row.get("account_ids")?,
        })
    }
}

pub fn format_number(value: f64, digits: usize) -> String {
    let fixed = format!("{:.*}", digits, value.abs());
    let (whole, fraction) = match fixed.split_once('.') {
        Some((whole, fraction)) => (whole.to_string(), Some(fraction.to_string())),
        None => (fixed.clone(), None),
    };
    let mut grouped = String::new();
    for (index, character) in whole.chars().enumerate() {
        if index > 0 && (whole.len() - index) % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(character);
    }
    let negative = value < 0.0 && fixed.chars().any(|character| character.is_ascii_digit() && character != '0');
    let mut result = String::new();
    if negative {
        result.push('-');
    }
    result.push_str(&grouped);
    if let Some(fraction) = fraction {
        result.push('.');
        result.push_str(&fraction);
    }
    result
}

#[derive(Clone, Debug)]
pub struct ReportScheduleRow {
    pub id: i64,
    pub name: String,
    pub template: String,
    pub account_id: Option<String>,
    pub channel: String,
    pub time: String,
    pub weekday: Option<i64>,
    pub enabled: i64,
    pub last_run_at: Option<String>,
    pub last_error: Option<String>,
}

impl ReportScheduleRow {
    pub fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(ReportScheduleRow {
            id: row.get("id")?,
            name: row.get("name")?,
            template: row.get("template")?,
            account_id: row.get("account_id")?,
            channel: row.get("channel")?,
            time: row.get("time")?,
            weekday: row.get("weekday")?,
            enabled: row.get("enabled")?,
            last_run_at: row.get("last_run_at")?,
            last_error: row.get("last_error")?,
        })
    }
}

#[derive(Clone, Debug)]
pub struct AlertRuleRow {
    pub id: i64,
    pub name: String,
    pub account_id: Option<String>,
    pub metric: String,
    pub operator: String,
    pub threshold: f64,
    pub period: String,
    pub channel: String,
    pub cooldown_hours: i64,
    pub enabled: i64,
    pub last_triggered_at: Option<String>,
    pub last_value: Option<f64>,
    pub last_error: Option<String>,
}

impl AlertRuleRow {
    pub fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(AlertRuleRow {
            id: row.get("id")?,
            name: row.get("name")?,
            account_id: row.get("account_id")?,
            metric: row.get("metric")?,
            operator: row.get("operator")?,
            threshold: row.get("threshold")?,
            period: row.get("period")?,
            channel: row.get("channel")?,
            cooldown_hours: row.get("cooldown_hours")?,
            enabled: row.get("enabled")?,
            last_triggered_at: row.get("last_triggered_at")?,
            last_value: row.get("last_value")?,
            last_error: row.get("last_error")?,
        })
    }
}

#[derive(Clone, Debug)]
pub struct ApprovalRequestRow {
    pub id: i64,
    pub created_at: String,
    pub run_id: Option<String>,
    pub tool_name: String,
    pub account_id: String,
    pub campaign_id: String,
    pub campaign_name: String,
    pub summary: String,
    pub reason: String,
    pub payload: String,
    pub status: String,
    pub decided_at: Option<String>,
    pub result: Option<String>,
}

impl ApprovalRequestRow {
    pub fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(ApprovalRequestRow {
            id: row.get("id")?,
            created_at: row.get("created_at")?,
            run_id: row.get("run_id")?,
            tool_name: row.get("tool_name")?,
            account_id: row.get("account_id")?,
            campaign_id: row.get("campaign_id")?,
            campaign_name: row.get("campaign_name")?,
            summary: row.get("summary")?,
            reason: row.get("reason")?,
            payload: row.get("payload")?,
            status: row.get("status")?,
            decided_at: row.get("decided_at")?,
            result: row.get("result")?,
        })
    }
}
