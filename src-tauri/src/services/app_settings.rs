use crate::config::{Settings, SettingValue};
use crate::db::Db;
use crate::models::utcnow;
use crate::router::Failure;
use crate::services::settings_groups::{field_spec, FieldSpec};
use fernet::Fernet;
use serde_json::Value;
use std::collections::HashMap;
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

pub const PROVIDER_NAMES: [&str; 4] = ["openai", "anthropic", "openai_compat", "fake"];

#[derive(Debug)]
pub struct SettingsError(pub String);

impl std::fmt::Display for SettingsError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

impl std::error::Error for SettingsError {}

pub fn mask(value: &str) -> String {
    let characters: Vec<char> = value.chars().collect();
    if characters.len() < 12 {
        return "\u{2022}\u{2022}\u{2022}\u{2022}".to_string();
    }
    let tail: String = characters[characters.len() - 4..].iter().collect();
    format!("\u{2022}\u{2022}\u{2022}\u{2022}{}", tail)
}

pub fn py_float(value: f64) -> String {
    if value.is_finite() && value.fract() == 0.0 {
        format!("{:.1}", value)
    } else {
        format!("{}", value)
    }
}

pub fn js_number(value: f64) -> String {
    if value.is_finite() && value.fract() == 0.0 && value.abs() < 1e15 {
        format!("{}", value as i64)
    } else {
        format!("{}", value)
    }
}

pub fn display_value(settings: &Settings, spec: &FieldSpec) -> String {
    match settings.get(spec.key) {
        Some(SettingValue::Text(text)) => text,
        Some(SettingValue::Int(number)) => number.to_string(),
        Some(SettingValue::Float(number)) => {
            if spec.kind == "float" {
                py_float(number)
            } else {
                js_number(number)
            }
        }
        None => String::new(),
    }
}

fn split_list(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(|item| item.trim())
        .filter(|item| !item.is_empty())
        .map(String::from)
        .collect()
}

fn known_time_zone(value: &str) -> bool {
    value.parse::<chrono_tz::Tz>().is_ok()
}

fn is_int_text(text: &str) -> bool {
    let digits = text.strip_prefix(|sign: char| sign == '+' || sign == '-').unwrap_or(text);
    !digits.is_empty() && digits.bytes().all(|byte| byte.is_ascii_digit())
}

fn is_float_text(text: &str) -> bool {
    let bytes = text.as_bytes();
    let mut index = 0;
    if index < bytes.len() && (bytes[index] == b'+' || bytes[index] == b'-') {
        index += 1;
    }
    let integer_start = index;
    while index < bytes.len() && bytes[index].is_ascii_digit() {
        index += 1;
    }
    let integer_digits = index - integer_start;
    if index < bytes.len() && bytes[index] == b'.' {
        index += 1;
        let fraction_start = index;
        while index < bytes.len() && bytes[index].is_ascii_digit() {
            index += 1;
        }
        let fraction_digits = index - fraction_start;
        if integer_digits == 0 && fraction_digits == 0 {
            return false;
        }
    } else if integer_digits == 0 {
        return false;
    }
    if index < bytes.len() && (bytes[index] == b'e' || bytes[index] == b'E') {
        index += 1;
        if index < bytes.len() && (bytes[index] == b'+' || bytes[index] == b'-') {
            index += 1;
        }
        let exponent_start = index;
        while index < bytes.len() && bytes[index].is_ascii_digit() {
            index += 1;
        }
        if index == exponent_start {
            return false;
        }
    }
    index == bytes.len()
}

pub fn raw_text(raw: &Value) -> String {
    match raw {
        Value::String(text) => text.clone(),
        Value::Number(number) => match number.as_f64() {
            Some(value) if number.is_f64() => js_number(value),
            _ => number.to_string(),
        },
        other => other.to_string(),
    }
}

pub fn validate_value(spec: &FieldSpec, raw: &Value) -> Result<String, SettingsError> {
    let text = raw_text(raw);
    let value = text.trim();
    if value.is_empty() {
        return Err(SettingsError(format!("{} cannot be empty", spec.label)));
    }
    if spec.kind == "int" || spec.kind == "float" {
        let matches = if spec.kind == "int" { is_int_text(value) } else { is_float_text(value) };
        let number = if matches {
            value.parse::<f64>().ok().filter(|parsed| parsed.is_finite())
        } else {
            None
        };
        let number = match number {
            Some(number) => number,
            None => return Err(SettingsError(format!("{} must be a number", spec.label))),
        };
        if let Some(minimum) = spec.minimum {
            if number < minimum {
                return Err(SettingsError(format!("{} must be at least {}", spec.label, js_number(minimum))));
            }
        }
        if let Some(maximum) = spec.maximum {
            if number > maximum {
                return Err(SettingsError(format!("{} must be at most {}", spec.label, js_number(maximum))));
            }
        }
        return Ok(if spec.kind == "int" {
            js_number(number.trunc())
        } else {
            py_float(number)
        });
    }
    if spec.kind == "url" {
        if !(value.starts_with("http://") || value.starts_with("https://")) || value.contains(' ') {
            return Err(SettingsError(format!("{} must start with http:// or https://", spec.label)));
        }
        return Ok(value.to_string());
    }
    if spec.kind == "email" {
        if !value.contains('@') || value.contains(' ') {
            return Err(SettingsError(format!("{} must be an email address", spec.label)));
        }
        return Ok(value.to_string());
    }
    if spec.kind == "emails" {
        let items = split_list(value);
        if items.is_empty() || items.iter().any(|item| !item.contains('@') || item.contains(' ')) {
            return Err(SettingsError(format!(
                "{} must be email addresses separated by commas",
                spec.label
            )));
        }
        return Ok(items.join(","));
    }
    if spec.kind == "order" {
        let items = split_list(value);
        let unknown = items.iter().any(|item| !PROVIDER_NAMES.contains(&item.as_str()));
        if items.is_empty() || unknown {
            return Err(SettingsError(format!(
                "{} accepts only: {}",
                spec.label,
                PROVIDER_NAMES.join(", ")
            )));
        }
        let mut unique: Vec<String> = Vec::new();
        for item in items {
            if !unique.contains(&item) {
                unique.push(item);
            }
        }
        return Ok(unique.join(","));
    }
    if spec.kind == "timezone" {
        if !known_time_zone(value) {
            return Err(SettingsError(format!("{} is not a known time zone", value)));
        }
        return Ok(value.to_string());
    }
    Ok(value.to_string())
}

pub fn effective_settings(base: &Settings, overrides: &HashMap<String, String>) -> Settings {
    let mut settings = base.clone();
    for (key, value) in overrides {
        if field_spec(key).is_some() {
            settings.apply(key, value);
        }
    }
    settings
}

pub fn source_of(overrides: &HashMap<String, String>, key: &str) -> &'static str {
    if overrides.contains_key(key) {
        "app"
    } else {
        "default"
    }
}

pub fn read_or_create<F: FnOnce() -> String>(file_path: &Path, make: F) -> std::io::Result<String> {
    if let Some(parent) = file_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    match options.open(file_path) {
        Ok(mut file) => {
            let value = make();
            file.write_all(value.as_bytes())?;
            Ok(value)
        }
        Err(error) if error.kind() == ErrorKind::AlreadyExists => {
            Ok(std::fs::read_to_string(file_path)?.trim().to_string())
        }
        Err(error) => Err(error),
    }
}

pub fn generate_key() -> String {
    Fernet::generate_key()
}

pub struct SettingsStore {
    db: Db,
    key_path: PathBuf,
    cipher: Mutex<Option<Arc<Fernet>>>,
}

impl SettingsStore {
    pub fn new(db: Db, key_path: PathBuf) -> Self {
        SettingsStore {
            db,
            key_path,
            cipher: Mutex::new(None),
        }
    }

    fn cipher(&self) -> Result<Arc<Fernet>, Failure> {
        let mut slot = self.cipher.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(existing) = slot.as_ref() {
            return Ok(existing.clone());
        }
        let key = read_or_create(&self.key_path, generate_key)?;
        let fernet = Fernet::new(&key).ok_or("secret.key does not hold a valid Fernet key")?;
        let shared = Arc::new(fernet);
        *slot = Some(shared.clone());
        Ok(shared)
    }

    pub fn encrypt_value(&self, value: &str) -> Result<String, Failure> {
        Ok(self.cipher()?.encrypt(value.as_bytes()))
    }

    pub fn decrypt_value(&self, value: &str) -> Result<String, Failure> {
        let cipher = self.cipher()?;
        let bytes = cipher
            .decrypt(value)
            .map_err(|error| -> Failure { format!("{:?}", error).into() })?;
        Ok(String::from_utf8(bytes)?)
    }

    pub fn load(&self) -> Result<HashMap<String, String>, Failure> {
        let rows: Vec<(String, String, i64)> = {
            let conn = self.db.lock();
            let mut statement = conn.prepare(r#"SELECT "key", value, encrypted FROM app_setting"#)?;
            let collected = statement
                .query_map([], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, i64>(2)?,
                    ))
                })?
                .collect::<Result<Vec<_>, _>>()?;
            collected
        };
        let mut values = HashMap::new();
        for (key, value, encrypted) in rows {
            if field_spec(&key).is_none() {
                continue;
            }
            if encrypted != 0 {
                match self.decrypt_value(&value) {
                    Ok(plain) => {
                        values.insert(key, plain);
                    }
                    Err(_) => {
                        eprintln!("Stored value for {} cannot be decrypted and was ignored", key);
                    }
                }
            } else {
                values.insert(key, value);
            }
        }
        Ok(values)
    }

    pub fn save(&self, updates: &[(String, String)], clear: &[String]) -> Result<(), Failure> {
        let mut prepared: Vec<(String, String, i64)> = Vec::new();
        for (key, value) in updates {
            let secret = field_spec(key).map_or(false, |spec| spec.secret);
            if secret {
                prepared.push((key.clone(), self.encrypt_value(value)?, 1));
            } else {
                prepared.push((key.clone(), value.clone(), 0));
            }
        }
        let mut conn = self.db.lock();
        let transaction = conn.transaction()?;
        if !clear.is_empty() {
            let placeholders = vec!["?"; clear.len()].join(",");
            let sql = format!(r#"DELETE FROM app_setting WHERE "key" IN ({})"#, placeholders);
            transaction.execute(&sql, rusqlite::params_from_iter(clear.iter()))?;
        }
        for (key, stored, flag) in prepared {
            transaction.execute(
                r#"INSERT INTO app_setting ("key", value, encrypted, updated_at) VALUES (?1, ?2, ?3, ?4)
                   ON CONFLICT("key") DO UPDATE SET value = excluded.value, encrypted = excluded.encrypted, updated_at = excluded.updated_at"#,
                rusqlite::params![key, stored, flag, utcnow()],
            )?;
        }
        transaction.commit()?;
        Ok(())
    }
}
