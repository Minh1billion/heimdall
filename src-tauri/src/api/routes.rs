use crate::container::Container;
use crate::models::{iso_or_null, AdAccountRow};
use crate::ports::errors::LLMErrorKind;
use crate::ports::llm::{llm_request, message, LLMPort, Role};
use crate::router::{issues_error, object_of, read_string, HttpError, Router};
use crate::services::ads::ads_insights;
use crate::services::dashboard::{llm_dashboard, RangeKey};
use crate::services::sync::{sync_guarded, SyncError};
use chrono::Utc;
use serde_json::{json, Value};
use std::collections::HashMap;

fn parse_range(query: &HashMap<String, String>) -> Result<RangeKey, HttpError> {
    let raw = query.get("range").map(String::as_str).unwrap_or("7d");
    RangeKey::parse(raw).ok_or_else(|| {
        HttpError::new(
            422,
            format!(
                "Invalid enum value. Expected '24h' | '7d' | '14d' | '30d', received '{}'",
                raw
            ),
        )
    })
}

fn parse_test(body: &Value) -> Result<(Option<String>, Option<String>), HttpError> {
    let object = object_of(body)?;
    let mut issues: Vec<String> = Vec::new();
    let provider = read_string(object, "provider", false, true, &mut issues);
    let prompt = read_string(object, "prompt", false, true, &mut issues);
    if !issues.is_empty() {
        return Err(issues_error(issues));
    }
    Ok((provider, prompt))
}

pub async fn test_provider(
    container: &Container,
    provider: Option<&str>,
    prompt: Option<&str>,
) -> Result<Value, HttpError> {
    let names: Option<Vec<String>> = provider
        .filter(|name| !name.is_empty())
        .map(|name| vec![name.to_string()]);
    let llm = container
        .router_for(names.as_deref())
        .map_err(|_| HttpError::new(404, "This provider is not configured"))?;
    let text = prompt
        .filter(|value| !value.is_empty())
        .unwrap_or("Say hello in five words.");
    let mut request = llm_request(vec![
        message(
            Role::System,
            Some("You are a connectivity check. Reply in one short sentence."),
        ),
        message(Role::User, Some(text)),
    ]);
    request.max_output_tokens = 256;
    request.timeout_s = container.settings().llm_timeout_s;
    request
        .metadata
        .insert("run_id".to_string(), json!("connectivity-test"));
    let response = match llm.generate(&request).await {
        Ok(response) => response,
        Err(error) => {
            if error.kind == LLMErrorKind::NoProviderConfigured {
                return Err(HttpError::new(503, error.message));
            }
            return Err(HttpError::new(502, error.message));
        }
    };
    Ok(json!({
        "provider": response.provider,
        "model": response.model,
        "text": response.message.content.clone().unwrap_or_default(),
        "latency_ms": response.latency_ms,
        "input_tokens": response.usage.input_tokens,
        "output_tokens": response.usage.output_tokens,
    }))
}

pub fn accounts_payload(container: &Container) -> Result<Value, HttpError> {
    let rows: Vec<(AdAccountRow, Option<String>)> = {
        let conn = container.db.lock();
        let mut statement = conn.prepare(
            "SELECT a.*, c.name AS connection_name FROM ad_accounts a
             LEFT JOIN meta_connections c ON c.id = a.connection_id ORDER BY a.name",
        )?;
        let collected = statement
            .query_map([], |row| {
                Ok((
                    AdAccountRow::from_row(row)?,
                    row.get::<_, Option<String>>("connection_name")?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        collected
    };
    let accounts: Vec<Value> = rows
        .iter()
        .map(|(row, connection_name)| {
            json!({
                "id": row.id,
                "name": row.name,
                "currency": row.currency,
                "timezone": row.timezone,
                "status": row.status,
                "last_synced_at": iso_or_null(&row.last_synced_at),
                "last_error": row.last_error,
                "connection_id": row.connection_id,
                "connection_name": connection_name,
            })
        })
        .collect();
    Ok(json!({
        "configured": container.has_meta(),
        "syncing": container.sync_lock.locked(),
        "accounts": accounts,
    }))
}

pub fn register(router: &mut Router) {
    router.get("/health", |_context| async move {
        Ok::<Value, HttpError>(json!({ "status": "ok", "version": env!("CARGO_PKG_VERSION") }))
    });

    router.get("/llm/providers", |context| async move {
        Ok::<Value, HttpError>(json!(context.container.provider_status()))
    });

    router.post("/llm/test", |context| async move {
        let (provider, prompt) = parse_test(&context.body)?;
        test_provider(&context.container, provider.as_deref(), prompt.as_deref()).await
    });

    router.get("/dashboard/llm", |context| async move {
        let range = parse_range(&context.query)?;
        Ok::<Value, HttpError>(llm_dashboard(&context.container.db, range, Utc::now())?)
    });

    router.get("/ads/accounts", |context| async move { accounts_payload(&context.container) });

    router.post("/ads/sync", |context| async move {
        let container = &context.container;
        if !container.has_meta() {
            return Err(HttpError::new(503, "No Meta access token is set"));
        }
        if container.sync_lock.locked() {
            return Err(HttpError::new(409, "A sync is already running"));
        }
        match sync_guarded(container).await {
            Ok(()) => {}
            Err(SyncError::Meta(error)) => return Err(HttpError::new(502, error.message)),
            Err(SyncError::Other(error)) => return Err(HttpError::from(error)),
        }
        accounts_payload(container)
    });

    router.get("/ads/insights", |context| async move {
        let range = parse_range(&context.query)?;
        let account = context
            .query
            .get("account_id")
            .map(String::as_str)
            .filter(|value| !value.is_empty());
        Ok::<Value, HttpError>(ads_insights(&context.container.db, range, account)?)
    });
}
