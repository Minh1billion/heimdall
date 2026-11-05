use crate::models::SCHEMA;
use crate::router::Failure;
use rusqlite::Connection;
use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

#[derive(Clone)]
pub struct Db {
    inner: Arc<Mutex<Connection>>,
}

fn ensure_column(conn: &Connection, table: &str, column: &str, ddl: &str) -> Result<(), Failure> {
    let present = {
        let mut statement = conn.prepare(&format!("PRAGMA table_info({})", table))?;
        let names = statement
            .query_map([], |row| row.get::<_, String>(1))?
            .collect::<Result<Vec<String>, _>>()?;
        names.iter().any(|name| name == column)
    };
    if !present {
        conn.execute_batch(&format!("ALTER TABLE {} ADD COLUMN {} {}", table, column, ddl))?;
    }
    Ok(())
}

impl Db {
    pub fn open(dir: &Path) -> Result<Db, Failure> {
        std::fs::create_dir_all(dir)?;
        let conn = Connection::open(dir.join("heimdall.db"))?;
        conn.busy_timeout(Duration::from_secs(5))?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.execute_batch(SCHEMA)?;
        let has_connection_id = {
            let mut statement = conn.prepare("PRAGMA table_info(ad_accounts)")?;
            let names = statement
                .query_map([], |row| row.get::<_, String>(1))?
                .collect::<Result<Vec<String>, _>>()?;
            names.iter().any(|name| name == "connection_id")
        };
        if !has_connection_id {
            conn.execute_batch("ALTER TABLE ad_accounts ADD COLUMN connection_id INTEGER")?;
        }
        ensure_column(&conn, "destinations", "template_id", "INTEGER")?;
        ensure_column(&conn, "destinations", "account_ids", "TEXT")?;
        Ok(Db {
            inner: Arc::new(Mutex::new(conn)),
        })
    }

    pub fn lock(&self) -> MutexGuard<'_, Connection> {
        self.inner.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}
