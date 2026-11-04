use crate::db::Db;
use crate::models::{iso_or_null, utcnow, MetaConnectionRow};
use crate::router::{Failure, HttpError};
use crate::services::app_settings::{mask, SettingsStore};
use rusqlite::{params, OptionalExtension};
use serde::Serialize;
use std::collections::HashMap;

pub struct StoredConnection {
    pub row: MetaConnectionRow,
    pub token: String,
}

#[derive(Serialize)]
pub struct ConnectionOut {
    pub id: i64,
    pub name: String,
    pub enabled: bool,
    pub preview: String,
    pub accounts: i64,
    pub created_at: Option<String>,
    pub last_synced_at: Option<String>,
    pub last_error: Option<String>,
}

const NAME_LIMIT: usize = 60;

fn rows(db: &Db) -> rusqlite::Result<Vec<MetaConnectionRow>> {
    let conn = db.lock();
    let mut statement = conn.prepare("SELECT * FROM meta_connections ORDER BY id")?;
    let collected = statement
        .query_map([], MetaConnectionRow::from_row)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(collected)
}

pub fn load_connections(db: &Db, store: &SettingsStore) -> rusqlite::Result<Vec<StoredConnection>> {
    let mut result = Vec::new();
    for row in rows(db)? {
        match store.decrypt_value(&row.token) {
            Ok(token) => result.push(StoredConnection { row, token }),
            Err(_) => eprintln!("Meta token \"{}\" cannot be decrypted and was ignored", row.name),
        }
    }
    Ok(result)
}

pub fn get_connection(db: &Db, store: &SettingsStore, id: i64) -> Result<StoredConnection, HttpError> {
    load_connections(db, store)?
        .into_iter()
        .find(|item| item.row.id == id)
        .ok_or_else(|| HttpError::new(404, "Token not found"))
}

fn clean_name(raw: &str) -> Result<String, HttpError> {
    let name = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    if name.is_empty() {
        return Err(HttpError::new(422, "Give this token a name"));
    }
    if name.chars().count() > NAME_LIMIT {
        return Err(HttpError::new(
            422,
            format!("Name must be at most {} characters", NAME_LIMIT),
        ));
    }
    Ok(name)
}

fn clean_token(raw: &str) -> Result<String, HttpError> {
    let token = raw.trim();
    if token.is_empty() {
        return Err(HttpError::new(422, "Access token cannot be empty"));
    }
    if token.chars().any(char::is_whitespace) {
        return Err(HttpError::new(422, "Access token cannot contain spaces"));
    }
    Ok(token.to_string())
}

fn ensure_unique(
    db: &Db,
    store: &SettingsStore,
    name: Option<&str>,
    token: Option<&str>,
    except_id: Option<i64>,
) -> Result<(), HttpError> {
    for item in load_connections(db, store)? {
        if Some(item.row.id) == except_id {
            continue;
        }
        if let Some(name) = name {
            if item.row.name.to_lowercase() == name.to_lowercase() {
                return Err(HttpError::new(
                    409,
                    format!("A token named \"{}\" already exists", item.row.name),
                ));
            }
        }
        if let Some(token) = token {
            if item.token == token {
                return Err(HttpError::new(
                    409,
                    format!("This token is already saved as \"{}\"", item.row.name),
                ));
            }
        }
    }
    Ok(())
}

pub fn create_connection(db: &Db, store: &SettingsStore, raw_name: &str, raw_token: &str) -> Result<i64, HttpError> {
    let name = clean_name(raw_name)?;
    let token = clean_token(raw_token)?;
    ensure_unique(db, store, Some(&name), Some(&token), None)?;
    let encrypted = store.encrypt_value(&token)?;
    let conn = db.lock();
    conn.execute(
        "INSERT INTO meta_connections (name, token, enabled, created_at) VALUES (?1, ?2, 1, ?3)",
        params![name, encrypted, utcnow()],
    )?;
    Ok(conn.last_insert_rowid())
}

#[derive(Default)]
pub struct ConnectionUpdate {
    pub name: Option<String>,
    pub token: Option<String>,
    pub enabled: Option<bool>,
}

pub fn update_connection(db: &Db, store: &SettingsStore, id: i64, update: &ConnectionUpdate) -> Result<(), HttpError> {
    get_connection(db, store, id)?;
    let name = match &update.name {
        Some(raw) => Some(clean_name(raw)?),
        None => None,
    };
    let token = match &update.token {
        Some(raw) if !raw.trim().is_empty() => Some(clean_token(raw)?),
        _ => None,
    };
    ensure_unique(db, store, name.as_deref(), token.as_deref(), Some(id))?;
    let encrypted = match &token {
        Some(plain) => Some(store.encrypt_value(plain)?),
        None => None,
    };
    let mut conn = db.lock();
    let transaction = conn.transaction()?;
    if let Some(name) = &name {
        transaction.execute("UPDATE meta_connections SET name = ?1 WHERE id = ?2", params![name, id])?;
    }
    if let Some(encrypted) = &encrypted {
        transaction.execute(
            "UPDATE meta_connections SET token = ?1, last_error = NULL WHERE id = ?2",
            params![encrypted, id],
        )?;
    }
    if let Some(enabled) = update.enabled {
        transaction.execute(
            "UPDATE meta_connections SET enabled = ?1 WHERE id = ?2",
            params![if enabled { 1 } else { 0 }, id],
        )?;
    }
    transaction.commit()?;
    Ok(())
}

pub fn delete_connection(db: &Db, id: i64) -> rusqlite::Result<()> {
    let mut conn = db.lock();
    let transaction = conn.transaction()?;
    transaction.execute(
        "UPDATE ad_accounts SET connection_id = NULL WHERE connection_id = ?1",
        params![id],
    )?;
    transaction.execute("DELETE FROM meta_connections WHERE id = ?1", params![id])?;
    transaction.commit()
}

pub fn mark_connection(db: &Db, id: i64, error: Option<&str>, synced: bool) -> rusqlite::Result<()> {
    let clipped: Option<String> = error.map(|text| text.chars().take(500).collect());
    let conn = db.lock();
    if synced {
        conn.execute(
            "UPDATE meta_connections SET last_error = ?1, last_synced_at = ?2 WHERE id = ?3",
            params![clipped, utcnow(), id],
        )?;
    } else {
        conn.execute(
            "UPDATE meta_connections SET last_error = ?1 WHERE id = ?2",
            params![clipped, id],
        )?;
    }
    Ok(())
}

pub fn connections_payload(db: &Db, store: &SettingsStore) -> rusqlite::Result<Vec<ConnectionOut>> {
    let mut counts: HashMap<i64, i64> = HashMap::new();
    {
        let conn = db.lock();
        let mut statement = conn.prepare(
            "SELECT connection_id, COUNT(*) AS total FROM ad_accounts WHERE connection_id IS NOT NULL GROUP BY connection_id",
        )?;
        let grouped = statement
            .query_map([], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)))?
            .collect::<Result<Vec<_>, _>>()?;
        for (connection_id, total) in grouped {
            counts.insert(connection_id, total);
        }
    }
    let mut result = Vec::new();
    for StoredConnection { row, token } in load_connections(db, store)? {
        result.push(ConnectionOut {
            id: row.id,
            name: row.name.clone(),
            enabled: row.enabled == 1,
            preview: mask(&token),
            accounts: counts.get(&row.id).copied().unwrap_or(0),
            created_at: iso_or_null(&Some(row.created_at.clone())),
            last_synced_at: iso_or_null(&row.last_synced_at),
            last_error: row.last_error.clone(),
        });
    }
    Ok(result)
}

pub fn migrate_legacy_token(db: &Db, store: &SettingsStore) -> Result<(), Failure> {
    let legacy: Option<(String, i64)> = {
        let conn = db.lock();
        conn.query_row(
            r#"SELECT value, encrypted FROM app_setting WHERE "key" = 'meta_access_token'"#,
            [],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
        )
        .optional()?
    };
    let (value, encrypted) = match legacy {
        Some(found) => found,
        None => return Ok(()),
    };
    let existing: i64 = {
        let conn = db.lock();
        conn.query_row("SELECT COUNT(*) FROM meta_connections", [], |row| row.get(0))?
    };
    if existing == 0 {
        let decoded: Result<String, Failure> = if encrypted != 0 {
            store.decrypt_value(&value)
        } else {
            Ok(value.clone())
        };
        let token = match decoded {
            Ok(token) => token,
            Err(_) => {
                eprintln!("The saved Meta access token cannot be decrypted, so it was not migrated");
                return Ok(());
            }
        };
        if !token.trim().is_empty() {
            let sealed = store.encrypt_value(token.trim())?;
            let mut conn = db.lock();
            let transaction = conn.transaction()?;
            transaction.execute(
                "INSERT INTO meta_connections (name, token, enabled, created_at) VALUES (?1, ?2, 1, ?3)",
                params!["Main token", sealed, utcnow()],
            )?;
            let created = transaction.last_insert_rowid();
            transaction.execute(
                "UPDATE ad_accounts SET connection_id = ?1 WHERE connection_id IS NULL",
                params![created],
            )?;
            transaction.execute(r#"DELETE FROM app_setting WHERE "key" = 'meta_access_token'"#, [])?;
            transaction.commit()?;
            return Ok(());
        }
    }
    let conn = db.lock();
    conn.execute(r#"DELETE FROM app_setting WHERE "key" = 'meta_access_token'"#, [])?;
    Ok(())
}
