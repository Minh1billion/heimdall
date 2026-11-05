use crate::config::email_recipients;
use crate::container::{Container, CHANNEL_LABELS};
use crate::models::{iso_or_null, AlertRuleRow, ReportScheduleRow};
use crate::router::{HttpError, Router};
use crate::services::alerts::{preview_alert, AlertDraft};
use crate::services::destinations::{broadcast_reports, get_destination, linked_ids, outcome_out, Overrides};
use crate::services::reports::{build_report, build_report_with};
use crate::services::templates;
use crate::services::render::render_all;
use crate::tools::common::METRICS;
use crate::validation::{parse, Field, Kind};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Map, Value};
use std::collections::HashMap;

const CHANNELS: [&str; 4] = ["telegram", "slack", "zalo", "email"];
const TEMPLATES: [&str; 2] = ["daily", "weekly"];
const PERIODS: [&str; 3] = ["today", "yesterday", "last_7d"];

fn template_field() -> Field {
    Field::new("template", Kind::Enum(&TEMPLATES)).with_default(json!("daily"))
}

fn account_field() -> Field {
    Field::text("account_id").nullish()
}

fn template_id_field() -> Field {
    Field::new("template_id", Kind::Int { min: Some(1), max: None }).nullish()
}

fn read_text_list(body: &Value, key: &str) -> Result<Option<Vec<String>>, HttpError> {
    match body.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Array(items)) => {
            let mut out: Vec<String> = Vec::new();
            for item in items {
                match item.as_str() {
                    Some(text) if !text.trim().is_empty() => {
                        let clean = text.trim().to_string();
                        if !out.contains(&clean) {
                            out.push(clean);
                        }
                    }
                    _ => return Err(HttpError::new(422, format!("{}: Expected strings", key))),
                }
            }
            Ok(Some(out))
        }
        Some(_) => Err(HttpError::new(422, format!("{}: Expected array", key))),
    }
}

fn enabled_fields() -> Vec<Field> {
    vec![Field::new("enabled", Kind::Bool)]
}

fn schedule_fields() -> Vec<Field> {
    vec![
        template_field(),
        account_field(),
        Field::new("name", Kind::Str { min: Some(1), max: Some(120) }),
        Field::new("channel", Kind::Enum(&CHANNELS)).nullish(),
        Field::new("destination_id", Kind::Int { min: Some(1), max: None }).nullish(),
        Field::new("time", Kind::Time),
        Field::new("weekday", Kind::Int { min: Some(0), max: Some(6) }).nullish(),
    ]
}

fn period_field() -> Field {
    Field::new("period", Kind::Enum(&PERIODS)).with_default(json!("yesterday"))
}

fn rule_fields() -> Vec<Field> {
    vec![
        Field::new("name", Kind::Str { min: Some(1), max: Some(120) }),
        account_field(),
        Field::new("metric", Kind::Enum(&METRICS)),
        Field::new("operator", Kind::Enum(&["gt", "lt"])),
        Field::new("threshold", Kind::Num { min: None, max: None }),
        period_field(),
        Field::new("channel", Kind::Enum(&CHANNELS)),
        Field::new("cooldown_hours", Kind::Int { min: Some(1), max: Some(168) }).with_default(json!(24)),
    ]
}

fn alert_preview_fields() -> Vec<Field> {
    vec![
        Field::new("name", Kind::Str { min: None, max: Some(120) }),
        account_field(),
        Field::new("metric", Kind::Enum(&METRICS)),
        Field::new("operator", Kind::Enum(&["gt", "lt"])),
        Field::new("threshold", Kind::Num { min: None, max: None }),
        period_field(),
    ]
}

fn label_of(name: &str) -> &'static str {
    CHANNEL_LABELS
        .iter()
        .find(|(key, _)| *key == name)
        .map(|(_, label)| *label)
        .unwrap_or("")
}

fn text_of<'a>(input: &'a Map<String, Value>, key: &str) -> Option<&'a str> {
    input.get(key).and_then(Value::as_str)
}

fn email_context(container: &Container) -> Value {
    let settings = container.settings();
    let from: Value = if settings.email_from.is_empty() {
        Value::Null
    } else {
        json!(settings.email_from)
    };
    json!({ "from": from, "to": email_recipients(&settings) })
}

fn require_account(container: &Container, id: Option<&str>) -> Result<(), HttpError> {
    if let Some(found) = id.filter(|value| !value.is_empty()) {
        let exists = {
            let conn = container.db.lock();
            conn.query_row("SELECT 1 FROM ad_accounts WHERE id = ?1", [found], |row| row.get::<_, i64>(0))
                .optional()?
                .is_some()
        };
        if !exists {
            return Err(HttpError::new(404, "Ad account not found"));
        }
    }
    Ok(())
}

pub async fn deliver(container: &Container, name: &str, subject: &str, text: &str) -> Result<(), HttpError> {
    let notifier = match container.notifier(name) {
        Some(found) => found,
        None => return Err(HttpError::new(503, format!("{} is not configured", label_of(name)))),
    };
    match notifier.send(subject, text).await {
        Ok(()) => Ok(()),
        Err(error) => Err(HttpError::new(502, error.0)),
    }
}

fn link_map(conn: &Connection) -> rusqlite::Result<HashMap<i64, Vec<i64>>> {
    let mut statement =
        conn.prepare("SELECT schedule_id, destination_id FROM schedule_destinations ORDER BY destination_id")?;
    let pairs = statement
        .query_map([], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)))?
        .collect::<Result<Vec<_>, _>>()?;
    let mut map: HashMap<i64, Vec<i64>> = HashMap::new();
    for (schedule, destination) in pairs {
        map.entry(schedule).or_default().push(destination);
    }
    Ok(map)
}

fn schedule_out(row: &ReportScheduleRow, destination_ids: Vec<i64>) -> Value {
    json!({
        "id": row.id,
        "name": row.name,
        "template": row.template,
        "account_id": row.account_id,
        "channel": row.channel,
        "destination_ids": destination_ids,
        "time": row.time,
        "weekday": row.weekday,
        "enabled": row.enabled != 0,
        "last_run_at": iso_or_null(&row.last_run_at),
        "last_error": row.last_error,
    })
}

fn rule_out(row: &AlertRuleRow) -> Value {
    json!({
        "id": row.id,
        "name": row.name,
        "account_id": row.account_id,
        "metric": row.metric,
        "operator": row.operator,
        "threshold": row.threshold,
        "period": row.period,
        "channel": row.channel,
        "cooldown_hours": row.cooldown_hours,
        "enabled": row.enabled != 0,
        "last_triggered_at": iso_or_null(&row.last_triggered_at),
        "last_value": row.last_value,
        "last_error": row.last_error,
    })
}

fn find_schedule(container: &Container, id: i64) -> Result<Option<ReportScheduleRow>, HttpError> {
    let conn = container.db.lock();
    Ok(conn
        .query_row("SELECT * FROM report_schedules WHERE id = ?1", [id], ReportScheduleRow::from_row)
        .optional()?)
}

fn find_rule(container: &Container, id: i64) -> Result<Option<AlertRuleRow>, HttpError> {
    let conn = container.db.lock();
    Ok(conn
        .query_row("SELECT * FROM alert_rules WHERE id = ?1", [id], AlertRuleRow::from_row)
        .optional()?)
}

fn read_ids(body: &Value, key: &str, extra: Option<i64>) -> Result<Vec<i64>, HttpError> {
    let mut ids: Vec<i64> = Vec::new();
    match body.get(key) {
        None | Some(Value::Null) => {}
        Some(Value::Array(items)) => {
            for item in items {
                match item.as_i64() {
                    Some(found) if found >= 1 => {
                        if !ids.contains(&found) {
                            ids.push(found);
                        }
                    }
                    _ => return Err(HttpError::new(422, format!("{}: Expected positive integers", key))),
                }
            }
        }
        Some(_) => return Err(HttpError::new(422, format!("{}: Expected array", key))),
    }
    if let Some(found) = extra {
        if !ids.contains(&found) {
            ids.push(found);
        }
    }
    Ok(ids)
}

fn int_id(raw: &str) -> Result<i64, HttpError> {
    let digits = raw.strip_prefix('-').unwrap_or(raw);
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(HttpError::new(422, "id: Input should be a valid integer"));
    }
    Ok(raw.parse::<i64>().unwrap_or(i64::MAX))
}

pub fn register(router: &mut Router) {
    router.get("/channels", |context| async move {
        let rows: Vec<Value> = CHANNEL_LABELS
            .iter()
            .map(|(name, label)| {
                json!({
                    "name": name,
                    "label": label,
                    "configured": context.container.notifier(name).is_some(),
                })
            })
            .collect();
        Ok::<Value, HttpError>(Value::Array(rows))
    });

    router.add("POST", "/channels/:name/test", 204, |context| async move {
        let name = context.param("name").to_string();
        if !CHANNEL_LABELS.iter().any(|(key, _)| *key == name) {
            return Err(HttpError::new(404, "Unknown channel"));
        }
        deliver(&context.container, &name, "Heimdall test", "This is a test message from Heimdall.").await?;
        Ok(Value::Null)
    });

    router.post("/reports/preview", |context| async move {
        let input = parse(&[template_field(), account_field(), template_id_field()], &context.body)?;
        let account = text_of(&input, "account_id");
        require_account(&context.container, account)?;
        let template = text_of(&input, "template").unwrap_or("daily");
        let account = account.filter(|value| !value.is_empty());
        let config = match input.get("template_id").and_then(Value::as_i64) {
            Some(id) => {
                templates::find(&context.container.db, id)?
                    .ok_or_else(|| HttpError::new(404, "Template not found"))?
                    .config
            }
            None => templates::TemplateConfig::default(),
        };
        let report = build_report_with(&context.container, template, account, &config, false).await?;
        Ok::<Value, HttpError>(json!({
            "title": report.title,
            "text": report.text,
            "rendered": render_all(&report.title, &report.text),
            "email": email_context(&context.container),
        }))
    });

    router.post("/alerts/preview", |context| async move {
        let input = parse(&alert_preview_fields(), &context.body)?;
        let account = text_of(&input, "account_id");
        require_account(&context.container, account)?;
        let metric = text_of(&input, "metric").unwrap_or("").to_string();
        let trimmed = text_of(&input, "name").unwrap_or("").trim().to_string();
        let draft = AlertDraft {
            name: if trimmed.is_empty() { format!("{} alert", metric) } else { trimmed },
            account_id: account.filter(|value| !value.is_empty()).map(String::from),
            metric,
            operator: text_of(&input, "operator").unwrap_or("").to_string(),
            threshold: input.get("threshold").and_then(Value::as_f64).unwrap_or(0.0),
            period: text_of(&input, "period").unwrap_or("yesterday").to_string(),
        };
        let mut preview = preview_alert(&context.container, &draft)?;
        if let Value::Object(map) = &mut preview {
            map.insert("email".to_string(), email_context(&context.container));
        }
        Ok::<Value, HttpError>(preview)
    });

    router.add("POST", "/reports/send", 204, |context| async move {
        let mut fields = vec![template_field(), account_field()];
        fields.push(Field::new("channel", Kind::Enum(&CHANNELS)).nullish());
        fields.push(Field::new("destination_id", Kind::Int { min: Some(1), max: None }).nullish());
        let input = parse(&fields, &context.body)?;
        let account = text_of(&input, "account_id");
        require_account(&context.container, account)?;
        let template = text_of(&input, "template").unwrap_or("daily");
        let account = account.filter(|value| !value.is_empty());
        if let Some(id) = input.get("destination_id").and_then(Value::as_i64) {
            let stored = get_destination(&context.container.db, &context.container.store, id)?;
            if stored.row.enabled == 0 {
                return Err(HttpError::new(409, format!("{} is paused", stored.row.name)));
            }
            let outcomes =
                broadcast_reports(&context.container, &[id], template, account, &Overrides::default()).await;
            if let Some(error) = outcomes.first().and_then(|item| item.error.clone()) {
                return Err(HttpError::new(502, error));
            }
            return Ok(Value::Null);
        }
        let channel = text_of(&input, "channel").unwrap_or("").to_string();
        if channel.is_empty() {
            return Err(HttpError::new(422, "destination_id: Required"));
        }
        if context.container.notifier(&channel).is_none() {
            return Err(HttpError::new(503, format!("{} is not configured", label_of(&channel))));
        }
        let report = build_report(&context.container, template, account, true).await?;
        deliver(&context.container, &channel, &report.title, &report.text).await?;
        Ok(Value::Null)
    });

    router.post("/reports/broadcast", |context| async move {
        let input = parse(&[template_field(), account_field(), template_id_field()], &context.body)?;
        let account = text_of(&input, "account_id");
        require_account(&context.container, account)?;
        let ids = read_ids(&context.body, "destination_ids", None)?;
        if ids.is_empty() {
            return Err(HttpError::new(422, "destination_ids: Pick at least one destination"));
        }
        let account_ids = read_text_list(&context.body, "account_ids")?;
        if let Some(list) = &account_ids {
            for found in list {
                require_account(&context.container, Some(found.as_str()))?;
            }
        }
        let template_id = input.get("template_id").and_then(Value::as_i64);
        if let Some(id) = template_id {
            if templates::find(&context.container.db, id)?.is_none() {
                return Err(HttpError::new(404, "Template not found"));
            }
        }
        let template = text_of(&input, "template").unwrap_or("daily");
        let account = account.filter(|value| !value.is_empty());
        let overrides = Overrides {
            account_ids,
            template_id,
            ..Overrides::default()
        };
        let outcomes = broadcast_reports(&context.container, &ids, template, account, &overrides).await;
        let sent = outcomes.iter().filter(|item| item.error.is_none()).count();
        let rows: Vec<Value> = outcomes.iter().map(outcome_out).collect();
        Ok::<Value, HttpError>(json!({
            "results": rows,
            "sent": sent,
            "failed": outcomes.len() - sent,
        }))
    });

    router.get("/reports/schedules", |context| async move {
        let conn = context.container.db.lock();
        let mut statement = conn.prepare("SELECT * FROM report_schedules ORDER BY id")?;
        let rows = statement
            .query_map([], ReportScheduleRow::from_row)?
            .collect::<Result<Vec<_>, _>>()?;
        let links = link_map(&conn)?;
        Ok::<Value, HttpError>(Value::Array(
            rows.iter()
                .map(|row| schedule_out(row, links.get(&row.id).cloned().unwrap_or_default()))
                .collect(),
        ))
    });

    router.post("/reports/schedules", |context| async move {
        let input = parse(&schedule_fields(), &context.body)?;
        let account = text_of(&input, "account_id");
        require_account(&context.container, account)?;
        let template = text_of(&input, "template").unwrap_or("daily").to_string();
        let mut weekday: Option<i64> = input.get("weekday").and_then(Value::as_i64);
        if template == "weekly" && weekday.is_none() {
            weekday = Some(0);
        }
        let single = input.get("destination_id").and_then(Value::as_i64);
        let destination_ids = read_ids(&context.body, "destination_ids", single)?;
        let mut channel = text_of(&input, "channel").unwrap_or("").to_string();
        if let Some(first) = destination_ids.first() {
            let mut first_channel: Option<String> = None;
            for found in &destination_ids {
                let stored = get_destination(&context.container.db, &context.container.store, *found)?;
                if first_channel.is_none() {
                    first_channel = Some(stored.row.channel.clone());
                }
            }
            channel = first_channel.unwrap_or_else(|| format!("destination-{}", first));
        } else if channel.is_empty() {
            return Err(HttpError::new(422, "destination_ids: Pick at least one destination"));
        }
        let id = {
            let conn = context.container.db.lock();
            conn.execute(
                "INSERT INTO report_schedules (name, template, account_id, channel, time, weekday, enabled) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 1)",
                params![
                    text_of(&input, "name").unwrap_or(""),
                    template,
                    account,
                    channel,
                    text_of(&input, "time").unwrap_or(""),
                    weekday
                ],
            )?;
            let created = conn.last_insert_rowid();
            for found in &destination_ids {
                conn.execute(
                    "INSERT INTO schedule_destinations (schedule_id, destination_id) VALUES (?1, ?2)",
                    params![created, found],
                )?;
            }
            created
        };
        match find_schedule(&context.container, id)? {
            Some(row) => Ok(schedule_out(&row, linked_ids(&context.container.db, id)?)),
            None => Err(HttpError::new(404, "Schedule not found")),
        }
    });

    router.patch("/reports/schedules/:id", |context| async move {
        let id = int_id(context.param("id"))?;
        let input = parse(&enabled_fields(), &context.body)?;
        if find_schedule(&context.container, id)?.is_none() {
            return Err(HttpError::new(404, "Schedule not found"));
        }
        let enabled = input.get("enabled").and_then(Value::as_bool).unwrap_or(false);
        {
            let conn = context.container.db.lock();
            conn.execute(
                "UPDATE report_schedules SET enabled = ?1 WHERE id = ?2",
                params![if enabled { 1 } else { 0 }, id],
            )?;
        }
        match find_schedule(&context.container, id)? {
            Some(row) => Ok(schedule_out(&row, linked_ids(&context.container.db, id)?)),
            None => Err(HttpError::new(404, "Schedule not found")),
        }
    });

    router.add("DELETE", "/reports/schedules/:id", 204, |context| async move {
        let id = int_id(context.param("id"))?;
        let conn = context.container.db.lock();
        conn.execute("DELETE FROM report_schedules WHERE id = ?1", [id])?;
        Ok::<Value, HttpError>(Value::Null)
    });

    router.get("/alerts/rules", |context| async move {
        let conn = context.container.db.lock();
        let mut statement = conn.prepare("SELECT * FROM alert_rules ORDER BY id")?;
        let rows = statement
            .query_map([], AlertRuleRow::from_row)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok::<Value, HttpError>(Value::Array(rows.iter().map(rule_out).collect()))
    });

    router.post("/alerts/rules", |context| async move {
        let input = parse(&rule_fields(), &context.body)?;
        let account = text_of(&input, "account_id");
        require_account(&context.container, account)?;
        let id = {
            let conn = context.container.db.lock();
            conn.execute(
                "INSERT INTO alert_rules (name, account_id, metric, operator, threshold, period, channel, cooldown_hours, enabled)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 1)",
                params![
                    text_of(&input, "name").unwrap_or(""),
                    account,
                    text_of(&input, "metric").unwrap_or(""),
                    text_of(&input, "operator").unwrap_or(""),
                    input.get("threshold").and_then(Value::as_f64).unwrap_or(0.0),
                    text_of(&input, "period").unwrap_or("yesterday"),
                    text_of(&input, "channel").unwrap_or(""),
                    input.get("cooldown_hours").and_then(Value::as_i64).unwrap_or(24)
                ],
            )?;
            conn.last_insert_rowid()
        };
        match find_rule(&context.container, id)? {
            Some(row) => Ok(rule_out(&row)),
            None => Err(HttpError::new(404, "Rule not found")),
        }
    });

    router.patch("/alerts/rules/:id", |context| async move {
        let id = int_id(context.param("id"))?;
        let input = parse(&enabled_fields(), &context.body)?;
        if find_rule(&context.container, id)?.is_none() {
            return Err(HttpError::new(404, "Rule not found"));
        }
        let enabled = input.get("enabled").and_then(Value::as_bool).unwrap_or(false);
        {
            let conn = context.container.db.lock();
            conn.execute(
                "UPDATE alert_rules SET enabled = ?1 WHERE id = ?2",
                params![if enabled { 1 } else { 0 }, id],
            )?;
        }
        match find_rule(&context.container, id)? {
            Some(row) => Ok(rule_out(&row)),
            None => Err(HttpError::new(404, "Rule not found")),
        }
    });

    router.add("DELETE", "/alerts/rules/:id", 204, |context| async move {
        let id = int_id(context.param("id"))?;
        let conn = context.container.db.lock();
        conn.execute("DELETE FROM alert_rules WHERE id = ?1", [id])?;
        Ok::<Value, HttpError>(Value::Null)
    });
}
