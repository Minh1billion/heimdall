use crate::db::Db;
use crate::models::{clip, utcnow};
use crate::router::HttpError;
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

pub const METRIC_KEYS: [&str; 9] = ["spend", "impressions", "clicks", "conversions", "ctr", "cpc", "cpm", "cpa", "roas"];

const NAME_LIMIT: usize = 60;
const TEXT_LIMIT: usize = 400;
const CAMPAIGN_LIMIT: i64 = 10;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct TemplateConfig {
    pub metrics: Vec<String>,
    pub campaigns: i64,
    pub anomalies: bool,
    pub insight: bool,
    pub language: String,
    pub header: String,
    pub footer: String,
}

impl Default for TemplateConfig {
    fn default() -> Self {
        TemplateConfig {
            metrics: ["spend", "impressions", "clicks", "conversions", "cpa", "roas"]
                .iter()
                .map(|item| item.to_string())
                .collect(),
            campaigns: 3,
            anomalies: true,
            insight: false,
            language: "en".to_string(),
            header: String::new(),
            footer: String::new(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Template {
    pub id: i64,
    pub name: String,
    pub config: TemplateConfig,
    pub used_by: i64,
}

fn parse_config(raw: &str) -> TemplateConfig {
    serde_json::from_str(raw).unwrap_or_default()
}

pub fn list(db: &Db) -> rusqlite::Result<Vec<Template>> {
    let conn = db.lock();
    let mut statement = conn.prepare(
        "SELECT id, name, config, (SELECT COUNT(*) FROM destinations WHERE template_id = report_templates.id)
         FROM report_templates ORDER BY id",
    )?;
    let collected = statement
        .query_map([], |row| {
            Ok(Template {
                id: row.get(0)?,
                name: row.get(1)?,
                config: parse_config(&row.get::<_, String>(2)?),
                used_by: row.get(3)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(collected)
}

pub fn find(db: &Db, id: i64) -> rusqlite::Result<Option<Template>> {
    let conn = db.lock();
    conn.query_row(
        "SELECT id, name, config, (SELECT COUNT(*) FROM destinations WHERE template_id = report_templates.id)
         FROM report_templates WHERE id = ?1",
        [id],
        |row| {
            Ok(Template {
                id: row.get(0)?,
                name: row.get(1)?,
                config: parse_config(&row.get::<_, String>(2)?),
                used_by: row.get(3)?,
            })
        },
    )
    .optional()
}

pub fn config_for(db: &Db, id: Option<i64>) -> TemplateConfig {
    match id {
        Some(found) => find(db, found)
            .ok()
            .flatten()
            .map(|item| item.config)
            .unwrap_or_default(),
        None => TemplateConfig::default(),
    }
}

fn flag(object: &Map<String, Value>, key: &str, default: bool) -> Result<bool, HttpError> {
    match object.get(key) {
        None | Some(Value::Null) => Ok(default),
        Some(Value::Bool(value)) => Ok(*value),
        Some(_) => Err(HttpError::new(422, format!("{}: Expected boolean", key))),
    }
}

fn text(object: &Map<String, Value>, key: &str) -> Result<String, HttpError> {
    match object.get(key) {
        None | Some(Value::Null) => Ok(String::new()),
        Some(Value::String(value)) => Ok(clip(value.trim(), TEXT_LIMIT)),
        Some(_) => Err(HttpError::new(422, format!("{}: Expected string", key))),
    }
}

pub fn clean_config(object: &Map<String, Value>) -> Result<TemplateConfig, HttpError> {
    let mut selected: Vec<String> = Vec::new();
    match object.get("metrics") {
        Some(Value::Array(items)) => {
            for item in items {
                match item.as_str() {
                    Some(key) if METRIC_KEYS.contains(&key) => selected.push(key.to_string()),
                    _ => return Err(HttpError::new(422, "metrics: Unknown metric")),
                }
            }
        }
        _ => return Err(HttpError::new(422, "metrics: Expected array")),
    }
    let metrics: Vec<String> = METRIC_KEYS
        .iter()
        .filter(|key| selected.iter().any(|item| item == *key))
        .map(|key| key.to_string())
        .collect();
    if metrics.is_empty() {
        return Err(HttpError::new(422, "Pick at least one metric"));
    }
    let campaigns = match object.get("campaigns") {
        None | Some(Value::Null) => 3,
        Some(value) => match value.as_i64() {
            Some(found) if (0..=CAMPAIGN_LIMIT).contains(&found) => found,
            _ => {
                return Err(HttpError::new(
                    422,
                    format!("campaigns: Expected a number from 0 to {}", CAMPAIGN_LIMIT),
                ))
            }
        },
    };
    let language = match object.get("language").and_then(Value::as_str) {
        None => "en".to_string(),
        Some("en") => "en".to_string(),
        Some("vi") => "vi".to_string(),
        Some(_) => return Err(HttpError::new(422, "language: Expected 'en' | 'vi'")),
    };
    Ok(TemplateConfig {
        metrics,
        campaigns,
        anomalies: flag(object, "anomalies", true)?,
        insight: flag(object, "insight", false)?,
        language,
        header: text(object, "header")?,
        footer: text(object, "footer")?,
    })
}

fn clean_name(db: &Db, raw: &str, except_id: Option<i64>) -> Result<String, HttpError> {
    let name = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    if name.is_empty() {
        return Err(HttpError::new(422, "Give this template a name"));
    }
    if name.chars().count() > NAME_LIMIT {
        return Err(HttpError::new(422, format!("Name must be at most {} characters", NAME_LIMIT)));
    }
    for item in list(db)? {
        if Some(item.id) != except_id && item.name.to_lowercase() == name.to_lowercase() {
            return Err(HttpError::new(409, format!("A template named \"{}\" already exists", item.name)));
        }
    }
    Ok(name)
}

fn seal(config: &TemplateConfig) -> Result<String, HttpError> {
    serde_json::to_string(config).map_err(|error| HttpError::new(500, error.to_string()))
}

pub fn create(db: &Db, raw_name: &str, config: &TemplateConfig) -> Result<i64, HttpError> {
    let name = clean_name(db, raw_name, None)?;
    let sealed = seal(config)?;
    let conn = db.lock();
    conn.execute(
        "INSERT INTO report_templates (name, config, created_at) VALUES (?1, ?2, ?3)",
        params![name, sealed, utcnow()],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn update(db: &Db, id: i64, raw_name: &str, config: &TemplateConfig) -> Result<(), HttpError> {
    if find(db, id)?.is_none() {
        return Err(HttpError::new(404, "Template not found"));
    }
    let name = clean_name(db, raw_name, Some(id))?;
    let sealed = seal(config)?;
    let conn = db.lock();
    conn.execute(
        "UPDATE report_templates SET name = ?1, config = ?2 WHERE id = ?3",
        params![name, sealed, id],
    )?;
    Ok(())
}

pub fn delete(db: &Db, id: i64) -> Result<(), HttpError> {
    if find(db, id)?.is_none() {
        return Err(HttpError::new(404, "Template not found"));
    }
    let conn = db.lock();
    conn.execute("UPDATE destinations SET template_id = NULL WHERE template_id = ?1", [id])?;
    conn.execute("DELETE FROM report_templates WHERE id = ?1", [id])?;
    Ok(())
}

pub fn template_out(item: &Template) -> Value {
    json!({
        "id": item.id,
        "name": item.name,
        "metrics": item.config.metrics,
        "campaigns": item.config.campaigns,
        "anomalies": item.config.anomalies,
        "insight": item.config.insight,
        "language": item.config.language,
        "header": item.config.header,
        "footer": item.config.footer,
        "used_by": item.used_by,
    })
}
