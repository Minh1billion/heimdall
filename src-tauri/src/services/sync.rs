use crate::container::{Container, MetaConnectionClient};
use crate::models::{add_days, clip, parse_dt, today_utc, utcnow};
use crate::ports::meta::{AccountInfo, CampaignListing, InsightRow, MetaError};
use crate::router::Failure;
use crate::services::alerts::evaluate_alerts;
use crate::services::meta_connections::mark_connection;
use chrono::Utc;
use rusqlite::{params, params_from_iter};
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

pub const IDLE_POLL_S: f64 = 60.0;

const SKIPPED_STATUSES: [&str; 3] = ["closed", "disabled", "pending_closure"];

pub enum SyncError {
    Meta(MetaError),
    Other(Failure),
}

impl From<MetaError> for SyncError {
    fn from(error: MetaError) -> Self {
        SyncError::Meta(error)
    }
}

impl From<rusqlite::Error> for SyncError {
    fn from(error: rusqlite::Error) -> Self {
        SyncError::Other(Box::new(error))
    }
}

impl std::fmt::Display for SyncError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SyncError::Meta(error) => write!(formatter, "{}", error.message),
            SyncError::Other(error) => write!(formatter, "{}", error),
        }
    }
}

pub fn today_in(timezone_name: &str) -> String {
    match timezone_name.parse::<chrono_tz::Tz>() {
        Ok(zone) => Utc::now().with_timezone(&zone).format("%Y-%m-%d").to_string(),
        Err(_) => today_utc(),
    }
}

fn save_accounts(container: &Container, accounts: &[AccountInfo], connection_id: i64) -> rusqlite::Result<()> {
    let mut conn = container.db.lock();
    let transaction = conn.transaction()?;
    {
        let mut upsert = transaction.prepare(
            "INSERT INTO ad_accounts (id, name, currency, timezone, status, connection_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(id) DO UPDATE SET name = excluded.name, currency = excluded.currency,
               timezone = excluded.timezone, status = excluded.status, connection_id = excluded.connection_id",
        )?;
        for info in accounts {
            upsert.execute(params![
                info.id,
                info.name,
                info.currency,
                info.timezone,
                info.status,
                connection_id
            ])?;
        }
    }
    transaction.commit()
}

fn save_insights(
    container: &Container,
    account_id: &str,
    since: &str,
    until: &str,
    rows: &[InsightRow],
) -> rusqlite::Result<()> {
    let mut conn = container.db.lock();
    let transaction = conn.transaction()?;
    transaction.execute(
        "DELETE FROM daily_insights WHERE account_id = ?1 AND day >= ?2 AND day <= ?3",
        params![account_id, since, until],
    )?;
    {
        let mut insert = transaction.prepare(
            "INSERT INTO daily_insights (account_id, campaign_id, campaign_name, day, spend, impressions, clicks, conversions, conversion_value)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        )?;
        for row in rows {
            insert.execute(params![
                account_id,
                row.campaign_id,
                row.campaign_name,
                row.day,
                row.spend,
                row.impressions,
                row.clicks,
                row.conversions,
                row.conversion_value
            ])?;
        }
    }
    transaction.execute(
        "UPDATE ad_accounts SET last_synced_at = ?1, last_error = NULL WHERE id = ?2",
        params![utcnow(), account_id],
    )?;
    transaction.commit()
}

fn save_campaigns(container: &Container, account_id: &str, rows: &[CampaignListing]) -> rusqlite::Result<()> {
    let mut conn = container.db.lock();
    let transaction = conn.transaction()?;
    {
        let mut insert = transaction.prepare(
            "INSERT INTO campaigns (account_id, id, name, objective, status, effective_status, daily_budget, lifetime_budget, created_time)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
             ON CONFLICT(account_id, id) DO UPDATE SET name = excluded.name, objective = excluded.objective,
               status = excluded.status, effective_status = excluded.effective_status,
               daily_budget = excluded.daily_budget, lifetime_budget = excluded.lifetime_budget,
               created_time = excluded.created_time",
        )?;
        for row in rows {
            insert.execute(params![
                account_id,
                row.id,
                row.name,
                row.objective,
                row.status,
                row.effective_status,
                row.daily_budget,
                row.lifetime_budget,
                row.created_time
            ])?;
        }
    }
    let mut values: Vec<String> = vec![account_id.to_string()];
    values.extend(rows.iter().map(|row| row.id.clone()));
    let sql = if rows.is_empty() {
        "DELETE FROM campaigns WHERE account_id = ?".to_string()
    } else {
        format!(
            "DELETE FROM campaigns WHERE account_id = ? AND id NOT IN ({})",
            vec!["?"; rows.len()].join(",")
        )
    };
    transaction.execute(&sql, params_from_iter(values.iter()))?;
    transaction.commit()
}

fn mark_error(container: &Container, account_id: &str, message: &str) -> rusqlite::Result<()> {
    let conn = container.db.lock();
    conn.execute(
        "UPDATE ad_accounts SET last_error = ?1 WHERE id = ?2",
        params![clip(message, 500), account_id],
    )?;
    Ok(())
}

async fn sync_connection(
    container: &Container,
    connection: &MetaConnectionClient,
    claimed: &mut HashSet<String>,
) -> Result<(), SyncError> {
    let meta = connection.client.clone();
    let listed = meta.list_accounts().await?;
    let accounts: Vec<AccountInfo> = listed.into_iter().filter(|info| !claimed.contains(&info.id)).collect();
    for info in &accounts {
        claimed.insert(info.id.clone());
    }
    save_accounts(container, &accounts, connection.id)?;
    for info in &accounts {
        if SKIPPED_STATUSES.contains(&info.status.as_str()) {
            continue;
        }
        let until = today_in(&info.timezone);
        let since = add_days(&until, -(container.settings().meta_sync_days - 1));
        let rows = match meta.fetch_insights(&info.id, &since, &until).await {
            Ok(rows) => rows,
            Err(error) => {
                mark_error(container, &info.id, &error.message)?;
                if error.auth {
                    return Err(SyncError::Meta(error));
                }
                continue;
            }
        };
        save_insights(container, &info.id, &since, &until, &rows)?;
        match meta.list_campaigns(&info.id).await {
            Ok(campaigns) => save_campaigns(container, &info.id, &campaigns)?,
            Err(error) => {
                mark_error(container, &info.id, &format!("Campaign list: {}", error.message))?;
                if error.auth {
                    return Err(SyncError::Meta(error));
                }
            }
        }
    }
    Ok(())
}

pub async fn sync_all(container: &Container) -> Result<(), SyncError> {
    let connections = container.meta_clients();
    if connections.is_empty() {
        return Err(SyncError::Meta(MetaError::new("No Meta access token is configured")));
    }
    let mut claimed: HashSet<String> = HashSet::new();
    let mut succeeded = 0;
    let mut first_error: Option<MetaError> = None;
    for connection in connections.iter() {
        match sync_connection(container, connection, &mut claimed).await {
            Ok(()) => {
                mark_connection(&container.db, connection.id, None, true)?;
                succeeded += 1;
            }
            Err(SyncError::Meta(error)) => {
                mark_connection(&container.db, connection.id, Some(&error.message), false)?;
                if first_error.is_none() {
                    first_error = Some(error);
                }
            }
            Err(other) => return Err(other),
        }
    }
    if succeeded == 0 {
        if let Some(error) = first_error {
            return Err(SyncError::Meta(error));
        }
    }
    Ok(())
}

pub async fn sync_guarded(container: &Container) -> Result<(), SyncError> {
    {
        let _guard = container.sync_lock.acquire().await;
        sync_all(container).await?;
    }
    evaluate_alerts(container).await.map_err(SyncError::Other)?;
    Ok(())
}

fn oldest_sync_age(container: &Container) -> rusqlite::Result<Option<f64>> {
    let (oldest, total): (Option<String>, i64) = {
        let conn = container.db.lock();
        conn.query_row(
            "SELECT MIN(last_synced_at), COUNT(*) FROM ad_accounts",
            [],
            |row| Ok((row.get::<_, Option<String>>(0)?, row.get::<_, i64>(1)?)),
        )?
    };
    if total == 0 {
        return Ok(None);
    }
    let oldest = match oldest {
        Some(value) => value,
        None => return Ok(None),
    };
    Ok(parse_dt(&oldest).map(|moment| (Utc::now() - moment).num_milliseconds() as f64 / 1000.0))
}

async fn nap(container: &Container, seconds: f64) {
    container.wake.wait(Duration::from_secs_f64(seconds.max(1.0))).await;
    container.wake.clear();
}

pub async fn run_periodic(container: Arc<Container>, stop: Arc<AtomicBool>) {
    while !stop.load(Ordering::SeqCst) {
        let interval = (container.settings().meta_sync_interval_min * 60) as f64;
        if !container.has_meta() || interval <= 0.0 {
            nap(&container, IDLE_POLL_S).await;
            continue;
        }
        let age = match oldest_sync_age(&container) {
            Ok(age) => age,
            Err(error) => {
                eprintln!("Cannot read the last sync time {}", error);
                None
            }
        };
        match age {
            Some(age) if age < interval => nap(&container, interval - age).await,
            _ => {
                if stop.load(Ordering::SeqCst) {
                    return;
                }
                if let Err(error) = sync_guarded(&container).await {
                    eprintln!("Scheduled Meta sync failed {}", error);
                }
                nap(&container, interval).await;
            }
        }
    }
}
