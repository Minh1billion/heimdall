use crate::container::Container;
use crate::models::{format_number, py_round};
use crate::ports::meta::{CampaignInfo, MetaAdsPort, MetaError};
use crate::ports::tool::{Risk, Tool, ToolContext, ToolError, ToolFailure};
use crate::services::app_settings::js_number;
use crate::services::approvals::{create_request, NewRequest};
use crate::tools::common::{check_account, num_arg, text_arg};
use crate::validation::{Field, Kind};
use async_trait::async_trait;
use rusqlite::OptionalExtension;
use serde_json::{json, Map, Value};
use std::sync::Arc;

const ZERO_DECIMAL: [&str; 10] = ["VND", "JPY", "KRW", "CLP", "ISK", "PYG", "UGX", "XAF", "XOF", "XPF"];

fn guard<T>(result: Result<T, MetaError>) -> Result<T, ToolError> {
    result.map_err(|error| {
        let permitted = error.auth || matches!(error.code, Some(10) | Some(200) | Some(294));
        let hint = if permitted {
            "Check that the Meta token has ads_management access"
        } else {
            ""
        };
        ToolError::Failure(ToolFailure::with_hint("meta_error", error.message, hint))
    })
}

fn meta_of(container: &Container, account_id: Option<&str>) -> Result<Arc<dyn MetaAdsPort>, ToolError> {
    if !container.has_meta() {
        return Err(ToolError::Failure(ToolFailure::new(
            "meta_not_configured",
            "No Meta access token is set",
        )));
    }
    let id = match account_id {
        Some(found) if !found.is_empty() => found,
        _ => {
            let clients = container.meta_clients();
            return Ok(clients[0].client.clone());
        }
    };
    match container.meta_for_account(id)? {
        Some(client) => Ok(client),
        None => Err(ToolError::Failure(ToolFailure::with_hint(
            "no_active_token",
            format!("No active Meta token covers {}", id),
            "Turn its token back on or add a token that can access this account in Settings",
        ))),
    }
}

fn money(minor: i64, currency: &str) -> String {
    if ZERO_DECIMAL.contains(&currency) {
        return format!("{} {}", format_number(minor as f64, 0), currency);
    }
    format!("{} {}", format_number(minor as f64 / 100.0, 2), currency)
}

fn general(value: f64) -> String {
    let rounded = format!("{:.5e}", value).parse::<f64>().unwrap_or(value);
    js_number(rounded)
}

async fn load_campaign(
    ctx: &ToolContext,
    account_id: &str,
    campaign_id: &str,
) -> Result<(CampaignInfo, String), ToolError> {
    check_account(ctx, Some(account_id))?;
    let currency: String = {
        let conn = ctx.container.db.lock();
        conn.query_row("SELECT currency FROM ad_accounts WHERE id = ?1", [account_id], |row| {
            row.get::<_, String>(0)
        })
        .optional()?
        .unwrap_or_default()
    };
    let meta = meta_of(&ctx.container, Some(account_id))?;
    let info = guard(meta.get_campaign(campaign_id).await)?;
    if info.account_id != account_id {
        return Err(ToolError::Failure(ToolFailure::with_hint(
            "unknown_campaign",
            format!("Campaign {} does not belong to {}", campaign_id, account_id),
            "Take campaign_id from get_insights",
        )));
    }
    Ok((info, currency))
}

fn pending_result(summary: &str, id: i64, created: bool) -> Value {
    json!({
        "ok": true,
        "status": "pending_approval",
        "approval_id": id,
        "summary": summary,
        "already_pending": !created,
        "message": "A person must approve this in Heimdall before anything changes. Tell the user it is pending; do not retry.",
    })
}

fn payload_text(payload: &Value, key: &str) -> Option<String> {
    payload.get(key).and_then(Value::as_str).map(String::from)
}

pub struct ProposePauseCampaign;

#[async_trait]
impl Tool for ProposePauseCampaign {
    fn name(&self) -> &'static str {
        "propose_pause_campaign"
    }

    fn description(&self) -> &'static str {
        "Request approval to pause one campaign. Nothing changes until a person approves it in Heimdall. Use only when the data clearly shows a problem, such as spend with no results, and give the numbers in the reason."
    }

    fn risk(&self) -> Risk {
        Risk::Write
    }

    fn fields(&self) -> Vec<Field> {
        vec![
            Field::text("account_id").describe("Ad account id such as act_123"),
            Field::new("campaign_id", Kind::Str { min: None, max: Some(64) }).describe("Campaign id from get_insights"),
            Field::new("reason", Kind::Str { min: Some(1), max: Some(300) }),
        ]
    }

    async fn run(&self, ctx: &ToolContext, args: &Map<String, Value>) -> Result<Value, ToolError> {
        let account_id = text_arg(args, "account_id").unwrap_or("");
        let campaign_id = text_arg(args, "campaign_id").unwrap_or("");
        let (info, _) = load_campaign(ctx, account_id, campaign_id).await?;
        if info.status == "paused" {
            return Err(ToolError::Failure(ToolFailure::new(
                "already_paused",
                format!("{} is already paused", info.name),
            )));
        }
        let (row, created) = create_request(
            &ctx.container.db,
            &NewRequest {
                run_id: ctx.run_id.clone(),
                tool_name: self.name().to_string(),
                account_id: account_id.to_string(),
                campaign_id: info.id.clone(),
                campaign_name: info.name.clone(),
                summary: format!("Pause campaign {}", info.name),
                reason: text_arg(args, "reason").unwrap_or("").to_string(),
                payload: json!({ "campaign_id": info.id, "account_id": account_id }),
                ttl_hours: ctx.container.settings().approval_ttl_hours,
            },
        )?;
        Ok(pending_result(&row.summary, row.id, created))
    }

    async fn apply(&self, container: &Container, payload: &Value) -> Result<Value, ToolError> {
        let account = payload_text(payload, "account_id");
        let meta = meta_of(container, account.as_deref())?;
        let campaign = payload_text(payload, "campaign_id").unwrap_or_default();
        let info = guard(meta.get_campaign(&campaign).await)?;
        if info.status == "paused" {
            return Ok(json!({ "message": format!("{} was already paused", info.name) }));
        }
        guard(meta.pause_campaign(&info.id).await)?;
        Ok(json!({ "message": format!("Paused {}", info.name) }))
    }
}

pub struct ProposeBudgetChange;

#[async_trait]
impl Tool for ProposeBudgetChange {
    fn name(&self) -> &'static str {
        "propose_budget_change"
    }

    fn description(&self) -> &'static str {
        "Request approval to raise or lower a campaign's daily budget by a percentage. Nothing changes until a person approves it in Heimdall. The change is capped by the server. Only works for campaigns with a campaign-level daily budget."
    }

    fn risk(&self) -> Risk {
        Risk::Write
    }

    fn fields(&self) -> Vec<Field> {
        vec![
            Field::text("account_id").describe("Ad account id such as act_123"),
            Field::new("campaign_id", Kind::Str { min: None, max: Some(64) }).describe("Campaign id from get_insights"),
            Field::new("change_pct", Kind::Num { min: None, max: None })
                .describe("Percent change, for example 10 to raise by 10% or -15 to lower by 15%"),
            Field::new("reason", Kind::Str { min: Some(1), max: Some(300) }),
        ]
    }

    async fn run(&self, ctx: &ToolContext, args: &Map<String, Value>) -> Result<Value, ToolError> {
        let settings = ctx.container.settings();
        let cap = settings.write_budget_cap_pct;
        let change = num_arg(args, "change_pct");
        if change == 0.0 || change.abs() > cap {
            return Err(ToolError::Failure(ToolFailure::with_hint(
                "invalid_change",
                format!("change_pct must be non-zero and within ±{}%", general(cap)),
                format!("Use a value between -{} and {}", general(cap), general(cap)),
            )));
        }
        let account_id = text_arg(args, "account_id").unwrap_or("");
        let campaign_id = text_arg(args, "campaign_id").unwrap_or("");
        let (info, currency) = load_campaign(ctx, account_id, campaign_id).await?;
        let current = match info.daily_budget {
            Some(budget) => budget,
            None => {
                return Err(ToolError::Failure(ToolFailure::with_hint(
                    "no_campaign_budget",
                    format!("{} has no campaign-level daily budget", info.name),
                    "Its budget may be set on ad sets or be a lifetime budget",
                )))
            }
        };
        let target = f64::max(1.0, py_round(current as f64 * (1.0 + change / 100.0), 0)) as i64;
        if target == current {
            return Err(ToolError::Failure(ToolFailure::new(
                "no_change",
                "The change is too small to alter the budget",
            )));
        }
        let signed = format!("{}{}", if change >= 0.0 { "+" } else { "-" }, general(change.abs()));
        let summary = format!(
            "{} daily budget of {} from {} to {} ({}%)",
            if target > current { "Raise" } else { "Lower" },
            info.name,
            money(current, &currency),
            money(target, &currency),
            signed
        );
        let (row, created) = create_request(
            &ctx.container.db,
            &NewRequest {
                run_id: ctx.run_id.clone(),
                tool_name: self.name().to_string(),
                account_id: account_id.to_string(),
                campaign_id: info.id.clone(),
                campaign_name: info.name.clone(),
                summary,
                reason: text_arg(args, "reason").unwrap_or("").to_string(),
                payload: json!({
                    "campaign_id": info.id,
                    "account_id": account_id,
                    "from_minor": current,
                    "to_minor": target,
                }),
                ttl_hours: settings.approval_ttl_hours,
            },
        )?;
        Ok(pending_result(&row.summary, row.id, created))
    }

    async fn apply(&self, container: &Container, payload: &Value) -> Result<Value, ToolError> {
        let account = payload_text(payload, "account_id");
        let meta = meta_of(container, account.as_deref())?;
        let campaign = payload_text(payload, "campaign_id").unwrap_or_default();
        let info = guard(meta.get_campaign(&campaign).await)?;
        let from_minor = payload.get("from_minor").and_then(Value::as_i64);
        if info.daily_budget != from_minor {
            return Err(ToolError::Failure(ToolFailure::with_hint(
                "stale_proposal",
                format!("The budget of {} changed since this was proposed", info.name),
                "Ask for a new proposal",
            )));
        }
        let to_minor = payload.get("to_minor").and_then(Value::as_i64).unwrap_or(0);
        guard(meta.set_daily_budget(&info.id, to_minor).await)?;
        Ok(json!({ "message": format!("Updated the daily budget of {}", info.name) }))
    }
}
