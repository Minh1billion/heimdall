use crate::adapters::notify::email::EmailNotifier;
use crate::adapters::notify::slack::SlackNotifier;
use crate::adapters::notify::telegram::TelegramNotifier;
use crate::adapters::notify::zalo::ZaloNotifier;
use crate::config::Settings;
use crate::container::Container;
use crate::db::Db;
use crate::models::{clip, iso_or_null, utcnow, DestinationRow};
use crate::ports::notify::NotifierPort;
use crate::router::HttpError;
use crate::services::app_settings::{mask, SettingsStore};
use crate::services::reports::{build_report_with, Report};
use crate::services::templates;
use futures::future::join_all;
use rusqlite::{params, OptionalExtension};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

pub type Config = HashMap<String, String>;

pub struct StoredDestination {
    pub row: DestinationRow,
    pub config: Config,
}

const NAME_LIMIT: usize = 60;

const TELEGRAM_FIELDS: &[(&str, bool)] = &[("bot_token", true), ("chat_id", false)];
const SLACK_FIELDS: &[(&str, bool)] = &[("webhook_url", true)];
const ZALO_FIELDS: &[(&str, bool)] = &[("bot_token", true), ("chat_id", false)];
const EMAIL_FIELDS: &[(&str, bool)] = &[("to", false)];

fn spec_for(channel: &str) -> Option<&'static [(&'static str, bool)]> {
    match channel {
        "telegram" => Some(TELEGRAM_FIELDS),
        "slack" => Some(SLACK_FIELDS),
        "zalo" => Some(ZALO_FIELDS),
        "email" => Some(EMAIL_FIELDS),
        _ => None,
    }
}

fn label_of(key: &str) -> &'static str {
    match key {
        "bot_token" => "Bot token",
        "chat_id" => "Chat ID",
        "webhook_url" => "Webhook URL",
        "to" => "Recipients",
        _ => "Value",
    }
}

pub fn split_recipients(raw: &str) -> Vec<String> {
    raw.split(|character: char| character == ',' || character == ';' || character.is_whitespace())
        .map(|item| item.trim())
        .filter(|item| !item.is_empty())
        .map(String::from)
        .collect()
}

pub fn recipients_of(config: &Config) -> Vec<String> {
    split_recipients(config.get("to").map(String::as_str).unwrap_or(""))
}

fn rows(db: &Db) -> rusqlite::Result<Vec<DestinationRow>> {
    let conn = db.lock();
    let mut statement = conn.prepare("SELECT * FROM destinations ORDER BY id")?;
    let collected = statement
        .query_map([], DestinationRow::from_row)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(collected)
}

pub fn load_destinations(db: &Db, store: &SettingsStore) -> rusqlite::Result<Vec<StoredDestination>> {
    let mut result = Vec::new();
    for row in rows(db)? {
        let parsed = store
            .decrypt_value(&row.config)
            .ok()
            .and_then(|plain| serde_json::from_str::<Config>(&plain).ok());
        match parsed {
            Some(config) => result.push(StoredDestination { row, config }),
            None => eprintln!("Destination \"{}\" cannot be decrypted and was ignored", row.name),
        }
    }
    Ok(result)
}

pub fn get_destination(db: &Db, store: &SettingsStore, id: i64) -> Result<StoredDestination, HttpError> {
    load_destinations(db, store)?
        .into_iter()
        .find(|item| item.row.id == id)
        .ok_or_else(|| HttpError::new(404, "Destination not found"))
}

fn clean_name(raw: &str) -> Result<String, HttpError> {
    let name = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    if name.is_empty() {
        return Err(HttpError::new(422, "Give this destination a name"));
    }
    if name.chars().count() > NAME_LIMIT {
        return Err(HttpError::new(422, format!("Name must be at most {} characters", NAME_LIMIT)));
    }
    Ok(name)
}

fn clean_config(channel: &str, input: &Config, existing: Option<&Config>) -> Result<Config, HttpError> {
    let spec = spec_for(channel).ok_or_else(|| HttpError::new(422, "Unknown channel"))?;
    let mut output = Config::new();
    for (key, secret) in spec {
        let provided = input
            .get(*key)
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        let value = match provided {
            Some(value) => value,
            None => existing
                .and_then(|config| config.get(*key))
                .cloned()
                .unwrap_or_default(),
        };
        if value.is_empty() {
            return Err(HttpError::new(422, format!("{} is required", label_of(key))));
        }
        if *secret && *key != "webhook_url" && value.chars().any(char::is_whitespace) {
            return Err(HttpError::new(422, format!("{} cannot contain spaces", label_of(key))));
        }
        output.insert((*key).to_string(), value);
    }
    if channel == "slack" {
        let url = output.get("webhook_url").cloned().unwrap_or_default();
        if !url.starts_with("https://") || url.chars().any(char::is_whitespace) {
            return Err(HttpError::new(422, "Webhook URL must start with https:// and contain no spaces"));
        }
    }
    if channel == "email" {
        let list = split_recipients(output.get("to").map(String::as_str).unwrap_or(""));
        if list.is_empty() || list.iter().any(|item| !item.contains('@')) {
            return Err(HttpError::new(422, "Recipients must be valid email addresses"));
        }
        output.insert("to".to_string(), list.join(", "));
    }
    if channel == "zalo" {
        let list = split_recipients(output.get("chat_id").map(String::as_str).unwrap_or(""));
        if list.is_empty() {
            return Err(HttpError::new(422, "Chat ID is required"));
        }
        output.insert("chat_id".to_string(), list.join(", "));
    }
    Ok(output)
}

#[derive(Default, Clone, Debug)]
pub struct Scope {
    pub template_id: Option<i64>,
    pub account_ids: Vec<String>,
}

pub fn accounts_of(row: &DestinationRow) -> Vec<String> {
    serde_json::from_str::<Vec<String>>(row.account_ids.as_deref().unwrap_or("[]")).unwrap_or_default()
}

fn check_scope(db: &Db, scope: &Scope) -> Result<(), HttpError> {
    if let Some(id) = scope.template_id {
        if templates::find(db, id)?.is_none() {
            return Err(HttpError::new(404, "Template not found"));
        }
    }
    let conn = db.lock();
    for account in &scope.account_ids {
        let found: Option<String> = conn
            .query_row("SELECT id FROM ad_accounts WHERE id = ?1", [account.as_str()], |row| {
                row.get::<_, String>(0)
            })
            .optional()?;
        if found.is_none() {
            return Err(HttpError::new(404, format!("Ad account {} not found", account)));
        }
    }
    Ok(())
}

fn scope_json(accounts: &[String]) -> Option<String> {
    if accounts.is_empty() {
        None
    } else {
        serde_json::to_string(accounts).ok()
    }
}

fn ensure_unique(db: &Db, store: &SettingsStore, name: &str, except_id: Option<i64>) -> Result<(), HttpError> {
    for item in load_destinations(db, store)? {
        if Some(item.row.id) == except_id {
            continue;
        }
        if item.row.name.to_lowercase() == name.to_lowercase() {
            return Err(HttpError::new(
                409,
                format!("A destination named \"{}\" already exists", item.row.name),
            ));
        }
    }
    Ok(())
}

fn seal(store: &SettingsStore, config: &Config) -> Result<String, HttpError> {
    let plain = serde_json::to_string(config).map_err(|error| HttpError::new(500, error.to_string()))?;
    Ok(store.encrypt_value(&plain)?)
}

pub fn create_destination(
    db: &Db,
    store: &SettingsStore,
    raw_name: &str,
    channel: &str,
    input: &Config,
    scope: &Scope,
) -> Result<i64, HttpError> {
    let name = clean_name(raw_name)?;
    check_scope(db, scope)?;
    let config = clean_config(channel, input, None)?;
    ensure_unique(db, store, &name, None)?;
    let sealed = seal(store, &config)?;
    let conn = db.lock();
    conn.execute(
        "INSERT INTO destinations (name, channel, config, enabled, created_at, template_id, account_ids)
         VALUES (?1, ?2, ?3, 1, ?4, ?5, ?6)",
        params![name, channel, sealed, utcnow(), scope.template_id, scope_json(&scope.account_ids)],
    )?;
    Ok(conn.last_insert_rowid())
}

#[derive(Default)]
pub struct DestinationUpdate {
    pub name: Option<String>,
    pub config: Option<Config>,
    pub enabled: Option<bool>,
    pub template_id: Option<Option<i64>>,
    pub account_ids: Option<Vec<String>>,
}

pub fn update_destination(db: &Db, store: &SettingsStore, id: i64, update: &DestinationUpdate) -> Result<(), HttpError> {
    let current = get_destination(db, store, id)?;
    let name = match &update.name {
        Some(raw) => clean_name(raw)?,
        None => current.row.name.clone(),
    };
    ensure_unique(db, store, &name, Some(id))?;
    let config = match &update.config {
        Some(input) => clean_config(&current.row.channel, input, Some(&current.config))?,
        None => current.config.clone(),
    };
    let sealed = seal(store, &config)?;
    let enabled = update.enabled.unwrap_or(current.row.enabled != 0);
    let scope = Scope {
        template_id: match update.template_id {
            Some(value) => value,
            None => current.row.template_id,
        },
        account_ids: match &update.account_ids {
            Some(list) => list.clone(),
            None => accounts_of(&current.row),
        },
    };
    if update.template_id.is_some() || update.account_ids.is_some() {
        check_scope(db, &scope)?;
    }
    let conn = db.lock();
    conn.execute(
        "UPDATE destinations SET name = ?1, config = ?2, enabled = ?3, template_id = ?4, account_ids = ?5 WHERE id = ?6",
        params![
            name,
            sealed,
            if enabled { 1 } else { 0 },
            scope.template_id,
            scope_json(&scope.account_ids),
            id
        ],
    )?;
    Ok(())
}

pub fn delete_destination(db: &Db, id: i64) -> Result<(), HttpError> {
    let conn = db.lock();
    conn.execute(
        "UPDATE report_schedules SET enabled = 0, last_error = 'Destination was deleted'
         WHERE id IN (SELECT schedule_id FROM schedule_destinations WHERE destination_id = ?1)
           AND id NOT IN (SELECT schedule_id FROM schedule_destinations WHERE destination_id <> ?1)",
        [id],
    )?;
    conn.execute("DELETE FROM destinations WHERE id = ?1", [id])?;
    Ok(())
}

pub fn linked_ids(db: &Db, schedule_id: i64) -> rusqlite::Result<Vec<i64>> {
    let conn = db.lock();
    let mut statement =
        conn.prepare("SELECT destination_id FROM schedule_destinations WHERE schedule_id = ?1 ORDER BY destination_id")?;
    let collected = statement
        .query_map([schedule_id], |row| row.get::<_, i64>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(collected)
}

pub fn mark_destination(db: &Db, id: i64, error: Option<&str>, sent: bool) {
    let stamp: Option<String> = if sent { Some(utcnow()) } else { None };
    let stored = error.map(|message| clip(message, 500));
    let conn = db.lock();
    if let Err(failure) = conn.execute(
        "UPDATE destinations SET last_error = ?1, last_sent_at = COALESCE(?2, last_sent_at) WHERE id = ?3",
        params![stored, stamp, id],
    ) {
        eprintln!("Cannot record the destination result {}", failure);
    }
}

pub fn build_notifier(stored: &StoredDestination, settings: &Settings) -> Result<Arc<dyn NotifierPort>, HttpError> {
    let get = |key: &str| stored.config.get(key).cloned().unwrap_or_default();
    let notifier: Arc<dyn NotifierPort> = match stored.row.channel.as_str() {
        "telegram" => Arc::new(TelegramNotifier::new(&get("bot_token"), &get("chat_id"))),
        "slack" => Arc::new(SlackNotifier::new(&get("webhook_url"))),
        "zalo" => Arc::new(ZaloNotifier::new(&get("bot_token"), &get("chat_id"))),
        "email" => {
            if settings.smtp_host.is_empty() || settings.email_from.is_empty() {
                return Err(HttpError::new(
                    503,
                    "Email is not configured. Set the SMTP server and sender in Settings",
                ));
            }
            Arc::new(EmailNotifier::new(
                &settings.smtp_host,
                settings.smtp_port,
                &settings.smtp_user,
                &settings.smtp_password,
                &settings.email_from,
                recipients_of(&stored.config),
            ))
        }
        _ => return Err(HttpError::new(422, "Unknown channel")),
    };
    Ok(notifier)
}

pub async fn deliver_to(
    container: &Container,
    stored: &StoredDestination,
    subject: &str,
    text: &str,
) -> Result<(), HttpError> {
    let settings = container.settings();
    let notifier = match build_notifier(stored, &settings) {
        Ok(found) => found,
        Err(error) => {
            mark_destination(&container.db, stored.row.id, Some(&error.detail), false);
            return Err(error);
        }
    };
    match notifier.send(subject, text).await {
        Ok(()) => {
            mark_destination(&container.db, stored.row.id, None, true);
            Ok(())
        }
        Err(error) => {
            mark_destination(&container.db, stored.row.id, Some(&error.0), false);
            Err(HttpError::new(502, error.0))
        }
    }
}

fn summary_of(stored: &StoredDestination) -> String {
    let get = |key: &str| stored.config.get(key).cloned().unwrap_or_default();
    match stored.row.channel.as_str() {
        "telegram" => format!("Chat {}", get("chat_id")),
        "zalo" => {
            let list = split_recipients(&get("chat_id"));
            if list.len() > 1 {
                format!("{} people", list.len())
            } else {
                format!("Chat {}", get("chat_id"))
            }
        }
        "slack" => mask(&get("webhook_url")),
        "email" => recipients_of(&stored.config).join(", "),
        _ => String::new(),
    }
}

pub fn destination_out(stored: &StoredDestination) -> Value {
    let spec = spec_for(&stored.row.channel).unwrap_or(&[]);
    let mut values = serde_json::Map::new();
    let mut secrets = serde_json::Map::new();
    for (key, secret) in spec {
        let value = stored.config.get(*key).cloned().unwrap_or_default();
        if *secret {
            secrets.insert((*key).to_string(), json!(mask(&value)));
        } else {
            values.insert((*key).to_string(), json!(value));
        }
    }
    let recipients: Vec<String> = match stored.row.channel.as_str() {
        "email" => recipients_of(&stored.config),
        "zalo" => split_recipients(stored.config.get("chat_id").map(String::as_str).unwrap_or("")),
        _ => Vec::new(),
    };
    json!({
        "id": stored.row.id,
        "name": stored.row.name,
        "channel": stored.row.channel,
        "enabled": stored.row.enabled != 0,
        "summary": summary_of(stored),
        "recipients": recipients,
        "values": Value::Object(values),
        "secrets": Value::Object(secrets),
        "created_at": iso_or_null(&Some(stored.row.created_at.clone())),
        "last_sent_at": iso_or_null(&stored.row.last_sent_at),
        "last_error": stored.row.last_error,
        "template_id": stored.row.template_id,
        "account_ids": accounts_of(&stored.row),
    })
}

pub struct SendOutcome {
    pub destination_id: i64,
    pub name: String,
    pub channel: String,
    pub skipped: bool,
    pub error: Option<String>,
    pub parts: Vec<Part>,
}

#[derive(Clone, Debug)]
pub struct Part {
    pub label: String,
    pub ok: bool,
    pub error: Option<String>,
}

pub fn outcome_out(outcome: &SendOutcome) -> Value {
    json!({
        "destination_id": outcome.destination_id,
        "name": outcome.name,
        "channel": outcome.channel,
        "ok": outcome.error.is_none(),
        "skipped": outcome.skipped,
        "error": outcome.error,
        "parts": outcome
            .parts
            .iter()
            .map(|part| json!({ "label": part.label, "ok": part.ok, "error": part.error }))
            .collect::<Vec<Value>>(),
    })
}

#[derive(Default, Clone, Debug)]
pub struct Overrides {
    pub account_ids: Option<Vec<String>>,
    pub template_id: Option<i64>,
    pub no_insight: bool,
}

fn account_label(db: &Db, id: &Option<String>) -> String {
    match id {
        None => "All accounts".to_string(),
        Some(found) => {
            let conn = db.lock();
            conn.query_row("SELECT name FROM ad_accounts WHERE id = ?1", [found.as_str()], |row| {
                row.get::<_, String>(0)
            })
            .optional()
            .ok()
            .flatten()
            .unwrap_or_else(|| found.clone())
        }
    }
}

fn scopes_for(stored: &StoredDestination, fallback: Option<&str>, overrides: &Overrides) -> Vec<Option<String>> {
    if let Some(list) = &overrides.account_ids {
        if list.is_empty() {
            return vec![None];
        }
        return list.iter().cloned().map(Some).collect();
    }
    let own = accounts_of(&stored.row);
    if own.is_empty() {
        return vec![fallback.filter(|value| !value.is_empty()).map(String::from)];
    }
    own.into_iter().map(Some).collect()
}

async fn deliver_parts(
    container: &Container,
    stored: &StoredDestination,
    subject: &str,
    text: &str,
    label: &str,
) -> Vec<Part> {
    if stored.row.channel != "zalo" {
        let result = deliver_to(container, stored, subject, text).await;
        return vec![Part {
            label: label.to_string(),
            ok: result.is_ok(),
            error: result.err().map(|error| error.detail),
        }];
    }
    let token = stored.config.get("bot_token").cloned().unwrap_or_default();
    let targets = split_recipients(stored.config.get("chat_id").map(String::as_str).unwrap_or(""));
    let mut parts: Vec<Part> = Vec::new();
    for (index, chat) in targets.iter().enumerate() {
        if index > 0 {
            tokio::time::sleep(Duration::from_millis(400)).await;
        }
        let notifier = ZaloNotifier::new(&token, chat);
        let result = notifier.send(subject, text).await;
        parts.push(Part {
            label: format!("{} → {}", label, chat),
            ok: result.is_ok(),
            error: result.err().map(|error| error.0),
        });
    }
    let failed: Vec<String> = parts
        .iter()
        .filter(|part| !part.ok)
        .map(|part| format!("{}: {}", part.label, part.error.clone().unwrap_or_default()))
        .collect();
    let message = if failed.is_empty() { None } else { Some(failed.join("; ")) };
    mark_destination(&container.db, stored.row.id, message.as_deref(), parts.iter().any(|part| part.ok));
    parts
}

pub async fn broadcast_reports(
    container: &Container,
    ids: &[i64],
    period: &str,
    fallback: Option<&str>,
    overrides: &Overrides,
) -> Vec<SendOutcome> {
    let mut pool = load_destinations(&container.db, &container.store).unwrap_or_default();
    let mut outcomes: Vec<SendOutcome> = Vec::new();
    let mut plans: Vec<(StoredDestination, Option<i64>, Vec<Option<String>>)> = Vec::new();
    for id in ids {
        match pool.iter().position(|item| item.row.id == *id) {
            Some(index) => {
                let stored = pool.remove(index);
                if stored.row.enabled == 0 {
                    outcomes.push(SendOutcome {
                        destination_id: stored.row.id,
                        name: stored.row.name.clone(),
                        channel: stored.row.channel.clone(),
                        skipped: true,
                        error: Some("Paused".to_string()),
                        parts: Vec::new(),
                    });
                } else {
                    let template_id = overrides.template_id.or(stored.row.template_id);
                    let scopes = scopes_for(&stored, fallback, overrides);
                    plans.push((stored, template_id, scopes));
                }
            }
            None => outcomes.push(SendOutcome {
                destination_id: *id,
                name: format!("Destination {}", id),
                channel: String::new(),
                skipped: false,
                error: Some("Destination not found".to_string()),
                parts: Vec::new(),
            }),
        }
    }
    let mut reports: HashMap<(Option<i64>, Option<String>), Result<Report, String>> = HashMap::new();
    for (_, template_id, scopes) in &plans {
        for scope in scopes {
            let key = (*template_id, scope.clone());
            if reports.contains_key(&key) {
                continue;
            }
            let config = templates::config_for(&container.db, *template_id);
            let built = build_report_with(container, period, scope.as_deref(), &config, !overrides.no_insight)
                .await
                .map_err(|error| error.to_string());
            reports.insert(key, built);
        }
    }
    let reports_ref = &reports;
    let sends = plans.iter().map(|plan| async move {
        let (stored, template_id, scopes) = plan;
        let mut parts: Vec<Part> = Vec::new();
        for scope in scopes {
            let label = account_label(&container.db, scope);
            match reports_ref.get(&(*template_id, scope.clone())) {
                Some(Ok(report)) => {
                    parts.extend(deliver_parts(container, stored, &report.title, &report.text, &label).await)
                }
                Some(Err(message)) => parts.push(Part {
                    label,
                    ok: false,
                    error: Some(message.clone()),
                }),
                None => {}
            }
        }
        let failed: Vec<&Part> = parts.iter().filter(|part| !part.ok).collect();
        let error = if failed.is_empty() {
            None
        } else if parts.len() == 1 {
            failed[0].error.clone()
        } else {
            Some(
                failed
                    .iter()
                    .map(|part| format!("{}: {}", part.label, part.error.clone().unwrap_or_default()))
                    .collect::<Vec<String>>()
                    .join("; "),
            )
        };
        SendOutcome {
            destination_id: stored.row.id,
            name: stored.row.name.clone(),
            channel: stored.row.channel.clone(),
            skipped: false,
            error,
            parts,
        }
    });
    outcomes.extend(join_all(sends).await);
    outcomes.sort_by_key(|item| {
        ids.iter()
            .position(|id| *id == item.destination_id)
            .unwrap_or(usize::MAX)
    });
    outcomes
}
