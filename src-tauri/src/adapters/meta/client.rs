use crate::adapters::llm::common::{describe, http_client, truthy};
use crate::models::valid_day;
use crate::ports::meta::{
    AccountInfo, CampaignInfo, CampaignListing, InsightRow, MetaAdsPort, MetaError,
};
use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use std::time::Duration;

const CONVERSION_ACTIONS: [&str; 4] = ["omni_purchase", "purchase", "lead", "omni_complete_registration"];

fn account_status(code: i64) -> &'static str {
    match code {
        1 => "active",
        2 => "disabled",
        3 => "unsettled",
        7 => "in_review",
        9 => "grace_period",
        100 => "pending_closure",
        101 => "closed",
        _ => "unknown",
    }
}

fn number(value: Option<&Value>) -> f64 {
    match value {
        Some(item) if truthy(item) => match item {
            Value::Number(entry) => entry.as_f64().unwrap_or(0.0),
            Value::String(text) => text.trim().parse::<f64>().unwrap_or(0.0),
            Value::Bool(true) => 1.0,
            _ => 0.0,
        },
        _ => 0.0,
    }
}

fn text_of(value: Option<&Value>) -> String {
    match value {
        None | Some(Value::Null) => String::new(),
        Some(Value::String(text)) => text.clone(),
        Some(other) => other.to_string(),
    }
}

fn or_default(value: Option<&Value>, fallback: &str) -> String {
    match value {
        Some(item) if truthy(item) => text_of(Some(item)),
        _ => fallback.to_string(),
    }
}

fn pick_action(items: Option<&Value>) -> f64 {
    let mut values: HashMap<String, f64> = HashMap::new();
    if let Some(Value::Array(entries)) = items {
        for entry in entries {
            let kind = text_of(entry.get("action_type"));
            values.insert(kind, number(entry.get("value")));
        }
    }
    for action in CONVERSION_ACTIONS.iter() {
        if let Some(found) = values.get(*action) {
            return *found;
        }
    }
    0.0
}

pub fn parse_insight(item: &Value) -> Result<InsightRow, MetaError> {
    let day = match item.get("date_start") {
        None => "undefined".to_string(),
        other => text_of(other),
    };
    if !valid_day(&day) {
        return Err(MetaError::new(format!("Invalid isoformat string: '{}'", day)));
    }
    Ok(InsightRow {
        campaign_id: text_of(item.get("campaign_id")),
        campaign_name: text_of(item.get("campaign_name")),
        day,
        spend: number(item.get("spend")),
        impressions: number(item.get("impressions")).trunc() as i64,
        clicks: number(item.get("clicks")).trunc() as i64,
        conversions: pick_action(item.get("actions")),
        conversion_value: pick_action(item.get("action_values")),
    })
}

pub struct MetaAdsClient {
    access_token: String,
    root: String,
}

impl MetaAdsClient {
    pub fn new(access_token: &str, api_version: &str, base_url: &str) -> Self {
        MetaAdsClient {
            access_token: access_token.to_string(),
            root: format!("{}/{}", base_url.trim_end_matches('/'), api_version),
        }
    }

    fn unreachable(error: &reqwest::Error) -> MetaError {
        MetaError::new(format!("Cannot reach Meta: {}", describe(error)))
    }

    async fn request(&self, url: &str, params: Option<&[(String, String)]>, post: bool) -> Result<Value, MetaError> {
        let timeout = Duration::from_secs(60);
        let sent = if post {
            http_client()
                .post(url)
                .timeout(timeout)
                .form(params.unwrap_or(&[]))
                .send()
                .await
        } else {
            let mut target = match url::Url::parse(url) {
                Ok(target) => target,
                Err(error) => return Err(MetaError::new(format!("Cannot reach Meta: {}", error))),
            };
            if let Some(pairs) = params {
                let mut query = target.query_pairs_mut();
                for (key, value) in pairs {
                    query.append_pair(key, value);
                }
            }
            http_client().get(target).timeout(timeout).send().await
        };
        let response = match sent {
            Ok(response) => response,
            Err(error) => return Err(Self::unreachable(&error)),
        };
        let status = response.status();
        if status.as_u16() >= 400 {
            let reason = status.canonical_reason().unwrap_or("").to_string();
            let mut error = Value::Null;
            if let Ok(body) = response.json::<Value>().await {
                if let Some(found) = body.get("error") {
                    if truthy(found) {
                        error = found.clone();
                    }
                }
            }
            let code = error.get("code").and_then(Value::as_i64);
            let message = match error.get("message") {
                Some(item) if truthy(item) => text_of(Some(item)),
                _ => {
                    if reason.is_empty() {
                        "Meta request failed".to_string()
                    } else {
                        reason
                    }
                }
            };
            return Err(MetaError::with_code(
                message,
                code,
                code == Some(190) || status.as_u16() == 401,
            ));
        }
        match response.json::<Value>().await {
            Ok(body) => Ok(body),
            Err(error) => Err(MetaError::new(format!("Meta returned an unreadable response: {}", describe(&error)))),
        }
    }

    async fn paged(&self, path: &str, params: Vec<(String, String)>) -> Result<Vec<Value>, MetaError> {
        let mut rows: Vec<Value> = Vec::new();
        let mut url: Option<String> = Some(format!("{}/{}", self.root, path));
        let mut query: Option<Vec<(String, String)>> = {
            let mut first = params;
            first.push(("access_token".to_string(), self.access_token.clone()));
            Some(first)
        };
        while let Some(target) = url {
            let body = self.request(&target, query.as_deref(), false).await?;
            if let Some(Value::Array(items)) = body.get("data") {
                rows.extend(items.iter().cloned());
            }
            url = body
                .get("paging")
                .and_then(|paging| paging.get("next"))
                .filter(|next| truthy(next))
                .map(|next| text_of(Some(next)));
            query = None;
        }
        Ok(rows)
    }

    async fn update(&self, campaign_id: &str, fields: Vec<(String, String)>) -> Result<(), MetaError> {
        let mut params = fields;
        params.push(("access_token".to_string(), self.access_token.clone()));
        let url = format!("{}/{}", self.root, campaign_id);
        let body = self.request(&url, Some(&params), true).await?;
        if body.get("success") != Some(&Value::Bool(true)) {
            return Err(MetaError::new("Meta did not confirm the change"));
        }
        Ok(())
    }
}

fn pair(key: &str, value: &str) -> (String, String) {
    (key.to_string(), value.to_string())
}

fn amount(value: Option<&Value>) -> Option<f64> {
    match value {
        Some(item) if truthy(item) => Some(number(Some(item))),
        _ => None,
    }
}

#[async_trait]
impl MetaAdsPort for MetaAdsClient {
    async fn list_accounts(&self) -> Result<Vec<AccountInfo>, MetaError> {
        let items = self
            .paged(
                "me/adaccounts",
                vec![
                    pair("fields", "id,name,currency,timezone_name,account_status"),
                    pair("limit", "100"),
                ],
            )
            .await?;
        Ok(items
            .iter()
            .map(|item| {
                let id = text_of(item.get("id"));
                AccountInfo {
                    name: or_default(item.get("name"), &id),
                    currency: or_default(item.get("currency"), ""),
                    timezone: or_default(item.get("timezone_name"), "UTC"),
                    status: account_status(item.get("account_status").and_then(Value::as_i64).unwrap_or(-1)).to_string(),
                    id,
                }
            })
            .collect())
    }

    async fn list_campaigns(&self, account_id: &str) -> Result<Vec<CampaignListing>, MetaError> {
        let items = self
            .paged(
                &format!("{}/campaigns", account_id),
                vec![
                    pair(
                        "fields",
                        "id,name,objective,status,effective_status,daily_budget,lifetime_budget,created_time",
                    ),
                    pair("limit", "200"),
                ],
            )
            .await?;
        Ok(items
            .iter()
            .map(|item| CampaignListing {
                id: text_of(item.get("id")),
                name: text_of(item.get("name")),
                objective: item
                    .get("objective")
                    .filter(|entry| truthy(entry))
                    .map(|entry| text_of(Some(entry))),
                status: text_of(item.get("status")).to_lowercase(),
                effective_status: item
                    .get("effective_status")
                    .filter(|entry| truthy(entry))
                    .map(|entry| text_of(Some(entry)).to_lowercase()),
                daily_budget: amount(item.get("daily_budget")),
                lifetime_budget: amount(item.get("lifetime_budget")),
                created_time: item
                    .get("created_time")
                    .filter(|entry| truthy(entry))
                    .map(|entry| text_of(Some(entry))),
            })
            .collect())
    }

    async fn fetch_insights(&self, account_id: &str, since: &str, until: &str) -> Result<Vec<InsightRow>, MetaError> {
        let range = serde_json::json!({ "since": since, "until": until }).to_string();
        let items = self
            .paged(
                &format!("{}/insights", account_id),
                vec![
                    pair("level", "campaign"),
                    pair("time_increment", "1"),
                    pair(
                        "fields",
                        "campaign_id,campaign_name,spend,impressions,clicks,actions,action_values",
                    ),
                    pair("time_range", &range),
                    pair("limit", "500"),
                ],
            )
            .await?;
        let mut rows = Vec::new();
        for item in &items {
            rows.push(parse_insight(item)?);
        }
        Ok(rows)
    }

    async fn get_campaign(&self, campaign_id: &str) -> Result<CampaignInfo, MetaError> {
        let params = vec![
            pair("fields", "id,name,account_id,status,daily_budget"),
            pair("access_token", &self.access_token),
        ];
        let url = format!("{}/{}", self.root, campaign_id);
        let body = self.request(&url, Some(&params), false).await?;
        let account_id = text_of(body.get("account_id"));
        let id = match body.get("id") {
            Some(item) if truthy(item) => text_of(Some(item)),
            _ => campaign_id.to_string(),
        };
        Ok(CampaignInfo {
            id,
            account_id: if account_id.starts_with("act_") {
                account_id
            } else {
                format!("act_{}", account_id)
            },
            name: text_of(body.get("name")),
            status: text_of(body.get("status")).to_lowercase(),
            daily_budget: amount(body.get("daily_budget")).map(|budget| budget.trunc() as i64),
        })
    }

    async fn pause_campaign(&self, campaign_id: &str) -> Result<(), MetaError> {
        self.update(campaign_id, vec![pair("status", "PAUSED")]).await
    }

    async fn set_daily_budget(&self, campaign_id: &str, amount_minor: i64) -> Result<(), MetaError> {
        self.update(campaign_id, vec![pair("daily_budget", &amount_minor.to_string())]).await
    }
}
