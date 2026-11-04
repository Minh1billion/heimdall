use crate::db::Db;
use crate::models::utcnow;
use crate::ports::llm::{UsageRecord, UsageSink};
use crate::router::Failure;
use rusqlite::params;

pub struct SqlUsageSink {
    db: Db,
}

impl SqlUsageSink {
    pub fn new(db: Db) -> Self {
        SqlUsageSink { db }
    }
}

impl UsageSink for SqlUsageSink {
    fn record(&self, record: UsageRecord) -> Result<(), Failure> {
        let conn = self.db.lock();
        conn.execute(
            "INSERT INTO llm_usage (created_at, provider, model, input_tokens, output_tokens, cached_tokens, latency_ms, ok, error_code, run_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                utcnow(),
                record.provider,
                record.model,
                record.input_tokens,
                record.output_tokens,
                record.cached_tokens,
                record.latency_ms,
                if record.ok { 1 } else { 0 },
                record.error_code,
                record.run_id
            ],
        )?;
        Ok(())
    }
}
