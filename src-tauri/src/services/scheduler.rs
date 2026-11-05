use crate::container::Container;
use crate::models::{clip, parse_dt, utcnow, ReportScheduleRow};
use crate::services::destinations::{broadcast_reports, linked_ids, Overrides};
use crate::services::reports::build_report;
use chrono::{DateTime, Datelike, Utc};
use rusqlite::{params, OptionalExtension};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

pub struct Zoned {
    pub day: String,
    pub time: String,
    pub weekday: i64,
}

fn zoned_parts(moment: DateTime<Utc>, zone: &str) -> Zoned {
    match zone.parse::<chrono_tz::Tz>() {
        Ok(tz) => {
            let local = moment.with_timezone(&tz);
            Zoned {
                day: local.format("%Y-%m-%d").to_string(),
                time: local.format("%H:%M").to_string(),
                weekday: local.weekday().num_days_from_monday() as i64,
            }
        }
        Err(_) => Zoned {
            day: moment.format("%Y-%m-%d").to_string(),
            time: moment.format("%H:%M").to_string(),
            weekday: moment.weekday().num_days_from_monday() as i64,
        },
    }
}

pub fn is_due(schedule: &ReportScheduleRow, moment: DateTime<Utc>, zone: &str) -> bool {
    if schedule.enabled == 0 {
        return false;
    }
    let now = zoned_parts(moment, zone);
    if let Some(weekday) = schedule.weekday {
        if now.weekday != weekday {
            return false;
        }
    }
    if now.time < schedule.time {
        return false;
    }
    match &schedule.last_run_at {
        None => true,
        Some(last) => match parse_dt(last) {
            Some(parsed) => zoned_parts(parsed, zone).day < now.day,
            None => false,
        },
    }
}

pub async fn run_schedule(container: &Container, schedule_id: i64) {
    let schedule: Option<ReportScheduleRow> = {
        let conn = container.db.lock();
        conn.query_row(
            "SELECT * FROM report_schedules WHERE id = ?1",
            [schedule_id],
            ReportScheduleRow::from_row,
        )
        .optional()
        .unwrap_or(None)
    };
    let schedule = match schedule {
        Some(found) => found,
        None => return,
    };
    let linked = linked_ids(&container.db, schedule_id).unwrap_or_default();
    let mut failures: Vec<String> = Vec::new();
    if linked.is_empty() {
        match container.notifier(&schedule.channel) {
            None => failures.push(format!("{} is not configured", schedule.channel)),
            Some(notifier) => match build_report(container, &schedule.template, schedule.account_id.as_deref(), false).await {
                Ok(report) => {
                    if let Err(error) = notifier.send(&report.title, &report.text).await {
                        failures.push(error.0);
                    }
                }
                Err(error) => failures.push(error.to_string()),
            },
        }
    } else {
        let outcomes = broadcast_reports(
            container,
            &linked,
            &schedule.template,
            schedule.account_id.as_deref(),
            &Overrides {
                no_insight: true,
                ..Overrides::default()
            },
        )
        .await;
        for outcome in outcomes {
            if let Some(message) = outcome.error {
                if !outcome.skipped {
                    failures.push(format!("{}: {}", outcome.name, message));
                }
            }
        }
    }
    let failure: Option<String> = if failures.is_empty() { None } else { Some(failures.join("; ")) };
    if let Some(message) = &failure {
        eprintln!("Scheduled report {} failed {}", schedule_id, message);
    }
    let stored = failure.map(|message| clip(&message, 500));
    let conn = container.db.lock();
    if let Err(error) = conn.execute(
        "UPDATE report_schedules SET last_run_at = ?1, last_error = ?2 WHERE id = ?3",
        params![utcnow(), stored, schedule_id],
    ) {
        eprintln!("Cannot record the schedule run {}", error);
    }
}

fn due_ids(container: &Container, now: DateTime<Utc>) -> Vec<i64> {
    let rows: Vec<ReportScheduleRow> = {
        let conn = container.db.lock();
        let prepared = conn.prepare("SELECT * FROM report_schedules");
        match prepared {
            Ok(mut statement) => match statement.query_map([], ReportScheduleRow::from_row) {
                Ok(mapped) => mapped.filter_map(|item| item.ok()).collect(),
                Err(_) => Vec::new(),
            },
            Err(_) => Vec::new(),
        }
    };
    let zone = container.settings().timezone.clone();
    rows.iter()
        .filter(|row| is_due(row, now, &zone))
        .map(|row| row.id)
        .collect()
}

pub async fn run_scheduler(container: Arc<Container>, stop: Arc<AtomicBool>) {
    while !stop.load(Ordering::SeqCst) {
        let due = due_ids(&container, Utc::now());
        for id in due {
            if stop.load(Ordering::SeqCst) {
                return;
            }
            run_schedule(&container, id).await;
        }
        for _ in 0..60 {
            if stop.load(Ordering::SeqCst) {
                return;
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    }
}
