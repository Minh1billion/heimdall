use crate::models::{iso_or_null, AdAccountRow};
use crate::ports::tool::{Tool, ToolContext, ToolError};
use crate::services::ads::{currency_for, metric_value, totals_for};
use crate::services::query::{aggregate, resolve_range};
use crate::tools::common::{
    check_account, int_arg, list_arg, metrics_field, pick, text_arg, today, METRICS, RANGE_HELP,
};
use crate::validation::{Field, Kind};
use async_trait::async_trait;
use serde_json::{json, Map, Value};
use std::collections::HashMap;

pub struct ListAdAccounts;

#[async_trait]
impl Tool for ListAdAccounts {
    fn name(&self) -> &'static str {
        "list_ad_accounts"
    }

    fn description(&self) -> &'static str {
        "List the connected Facebook ad accounts with currency, timezone, status, last sync time, latest data date and any sync error. Call this first when the account is unknown."
    }

    fn fields(&self) -> Vec<Field> {
        Vec::new()
    }

    async fn run(&self, ctx: &ToolContext, _args: &Map<String, Value>) -> Result<Value, ToolError> {
        let db = &ctx.container.db;
        let mut latest: HashMap<String, String> = HashMap::new();
        let rows: Vec<AdAccountRow> = {
            let conn = db.lock();
            let mut statement = conn.prepare("SELECT account_id, MAX(day) FROM daily_insights GROUP BY account_id")?;
            let found = statement
                .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))?
                .collect::<Result<Vec<_>, _>>()?;
            for (account, day) in found {
                latest.insert(account, day);
            }
            let mut statement = conn.prepare("SELECT * FROM ad_accounts ORDER BY name")?;
            let collected = statement
                .query_map([], AdAccountRow::from_row)?
                .collect::<Result<Vec<_>, _>>()?;
            collected
        };
        let accounts: Vec<Value> = rows
            .iter()
            .filter(|account| match &ctx.allowed_account_ids {
                None => true,
                Some(set) => set.contains(&account.id),
            })
            .map(|account| {
                json!({
                    "account_id": account.id,
                    "name": account.name,
                    "currency": account.currency,
                    "timezone": account.timezone,
                    "status": account.status,
                    "last_synced_at": iso_or_null(&account.last_synced_at),
                    "data_through": latest.get(&account.id),
                    "last_error": account.last_error,
                })
            })
            .collect();
        Ok(json!({ "ok": true, "accounts": accounts }))
    }
}

pub struct GetInsights;

#[async_trait]
impl Tool for GetInsights {
    fn name(&self) -> &'static str {
        "get_insights"
    }

    fn description(&self) -> &'static str {
        "Get aggregated ad performance from the synced database, grouped by account, campaign or day. Use for questions about spend, results and efficiency. Today's data can lag by a few hours. Money is in the account currency; ctr is a fraction (0.02 means 2%)."
    }

    fn fields(&self) -> Vec<Field> {
        vec![
            Field::text("account_id")
                .nullish()
                .describe("Ad account id such as act_123. Omit for all accounts."),
            Field::new("level", Kind::Enum(&["account", "campaign", "day"])).with_default(json!("campaign")),
            Field::text("date_range").with_default(json!("last_7d")).describe(RANGE_HELP),
            metrics_field(),
            Field::new("limit", Kind::Int { min: Some(1), max: Some(50) }).with_default(json!(20)),
        ]
    }

    async fn run(&self, ctx: &ToolContext, args: &Map<String, Value>) -> Result<Value, ToolError> {
        let (start, end) = resolve_range(text_arg(args, "date_range").unwrap_or("last_7d"), &today())?;
        let account = text_arg(args, "account_id");
        check_account(ctx, account)?;
        let level = text_arg(args, "level").unwrap_or("campaign");
        let rows = aggregate(&ctx.container.db, &start, &end, account, level)?;
        let currency = currency_for(&ctx.container.db, &start, &end, account)?;
        let limit = int_arg(args, "limit") as usize;
        let metrics = list_arg(args, "metrics");
        let shown: Vec<Value> = rows
            .iter()
            .take(limit)
            .map(|row| Value::Object(pick(row, &metrics)))
            .collect();
        Ok(json!({
            "ok": true,
            "start": start,
            "end": end,
            "currency": currency,
            "total_rows": rows.len(),
            "truncated": rows.len() > limit,
            "rows": shown,
        }))
    }
}

pub struct ComparePeriods;

#[async_trait]
impl Tool for ComparePeriods {
    fn name(&self) -> &'static str {
        "compare_periods"
    }

    fn description(&self) -> &'static str {
        "Compare totals between a current period and a baseline period and return absolute and percent change for each metric. Use for questions like this week versus last week."
    }

    fn fields(&self) -> Vec<Field> {
        vec![
            Field::text("account_id").nullish(),
            Field::text("current").describe(RANGE_HELP),
            Field::text("baseline").describe(RANGE_HELP),
            metrics_field(),
        ]
    }

    async fn run(&self, ctx: &ToolContext, args: &Map<String, Value>) -> Result<Value, ToolError> {
        let now = today();
        let current = resolve_range(text_arg(args, "current").unwrap_or(""), &now)?;
        let baseline = resolve_range(text_arg(args, "baseline").unwrap_or(""), &now)?;
        let account = text_arg(args, "account_id");
        check_account(ctx, account)?;
        let db = &ctx.container.db;
        let a = totals_for(db, &current.0, &current.1, account)?;
        let b = totals_for(db, &baseline.0, &baseline.1, account)?;
        let low = if current.0 < baseline.0 { current.0.clone() } else { baseline.0.clone() };
        let high = if current.1 > baseline.1 { current.1.clone() } else { baseline.1.clone() };
        let currency = currency_for(db, &low, &high, account)?;
        let mut changes: Map<String, Value> = Map::new();
        for metric in list_arg(args, "metrics") {
            let cur = metric_value(&a, &metric);
            let base = metric_value(&b, &metric);
            let delta = match (cur, base) {
                (Some(left), Some(right)) => Some(left - right),
                _ => None,
            };
            let percent = match (delta, base) {
                (Some(change), Some(right)) if right != 0.0 && !right.is_nan() => Some(change / right * 100.0),
                _ => None,
            };
            changes.insert(
                metric.clone(),
                json!({
                    "current": cur,
                    "baseline": base,
                    "change": delta,
                    "change_pct": percent,
                }),
            );
        }
        Ok(json!({
            "ok": true,
            "currency": currency,
            "current_period": [current.0, current.1],
            "baseline_period": [baseline.0, baseline.1],
            "metrics": changes,
        }))
    }
}

pub struct TopN;

#[async_trait]
impl Tool for TopN {
    fn name(&self) -> &'static str {
        "top_n"
    }

    fn description(&self) -> &'static str {
        "Rank campaigns or accounts by one metric and return the top or bottom N. Example: the 5 campaigns with the highest cpa."
    }

    fn fields(&self) -> Vec<Field> {
        vec![
            Field::text("account_id").nullish(),
            Field::new("level", Kind::Enum(&["account", "campaign"])).with_default(json!("campaign")),
            Field::text("date_range").with_default(json!("last_7d")).describe(RANGE_HELP),
            Field::new("metric", Kind::Enum(&METRICS)).with_default(json!("spend")),
            Field::new("n", Kind::Int { min: Some(1), max: Some(20) }).with_default(json!(5)),
            Field::new("order", Kind::Enum(&["desc", "asc"])).with_default(json!("desc")),
        ]
    }

    async fn run(&self, ctx: &ToolContext, args: &Map<String, Value>) -> Result<Value, ToolError> {
        let (start, end) = resolve_range(text_arg(args, "date_range").unwrap_or("last_7d"), &today())?;
        let account = text_arg(args, "account_id");
        check_account(ctx, account)?;
        let level = text_arg(args, "level").unwrap_or("campaign");
        let metric = text_arg(args, "metric").unwrap_or("spend").to_string();
        let descending = text_arg(args, "order").unwrap_or("desc") == "desc";
        let rows = aggregate(&ctx.container.db, &start, &end, account, level)?;
        let currency = currency_for(&ctx.container.db, &start, &end, account)?;
        let mut ranked: Vec<(f64, Map<String, Value>)> = rows
            .into_iter()
            .filter_map(|row| {
                let value = row.get(&metric).and_then(Value::as_f64)?;
                Some((value, row))
            })
            .collect();
        ranked.sort_by(|left, right| {
            let ordering = left.0.partial_cmp(&right.0).unwrap_or(std::cmp::Ordering::Equal);
            if descending {
                ordering.reverse()
            } else {
                ordering
            }
        });
        let picks: Vec<String> = if metric != "spend" {
            vec!["spend".to_string(), metric.clone()]
        } else {
            vec!["spend".to_string()]
        };
        let shown: Vec<Value> = ranked
            .iter()
            .take(int_arg(args, "n") as usize)
            .map(|(_, row)| Value::Object(pick(row, &picks)))
            .collect();
        Ok(json!({
            "ok": true,
            "start": start,
            "end": end,
            "currency": currency,
            "metric": metric,
            "rows": shown,
        }))
    }
}
