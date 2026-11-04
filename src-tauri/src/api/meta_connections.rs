use crate::adapters::meta::client::MetaAdsClient;
use crate::container::Container;
use crate::ports::meta::MetaAdsPort;
use crate::router::{issues_error, object_of, read_bool, read_string, HttpError, Router};
use crate::services::meta_connections::{
    connections_payload, create_connection, delete_connection, get_connection, mark_connection,
    update_connection, ConnectionUpdate,
};
use crate::services::sync::sync_guarded;
use serde_json::{json, Value};
use std::sync::Arc;

fn payload(container: &Container) -> Result<Value, HttpError> {
    Ok(json!({ "connections": connections_payload(&container.db, &container.store)? }))
}

fn connection_id(raw: &str) -> Result<i64, HttpError> {
    match raw.trim().parse::<f64>() {
        Ok(value) if value.fract() == 0.0 && value >= 1.0 && value <= 9e15 => Ok(value as i64),
        _ => Err(HttpError::new(404, "Token not found")),
    }
}

fn sync_soon(container: &Arc<Container>) {
    if !container.has_meta() || container.sync_lock.locked() {
        return;
    }
    let shared = container.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(error) = sync_guarded(&shared).await {
            eprintln!("Sync after token change failed {}", error);
        }
    });
}

pub fn register(router: &mut Router) {
    router.get("/meta/connections", |context| async move { payload(&context.container) });

    router.post("/meta/connections", |context| async move {
        let object = object_of(&context.body)?;
        let mut issues: Vec<String> = Vec::new();
        let name = read_string(object, "name", true, false, &mut issues);
        let token = read_string(object, "token", true, false, &mut issues);
        if !issues.is_empty() {
            return Err(issues_error(issues));
        }
        create_connection(
            &context.container.db,
            &context.container.store,
            &name.unwrap_or_default(),
            &token.unwrap_or_default(),
        )?;
        context.container.reload()?;
        sync_soon(&context.container);
        payload(&context.container)
    });

    router.put("/meta/connections/:id", |context| async move {
        let id = connection_id(context.param("id"))?;
        let object = object_of(&context.body)?;
        let mut issues: Vec<String> = Vec::new();
        let update = ConnectionUpdate {
            name: read_string(object, "name", false, false, &mut issues),
            token: read_string(object, "token", false, false, &mut issues),
            enabled: read_bool(object, "enabled", &mut issues),
        };
        if !issues.is_empty() {
            return Err(issues_error(issues));
        }
        update_connection(&context.container.db, &context.container.store, id, &update)?;
        context.container.reload()?;
        if update.token.is_some() || update.enabled == Some(true) {
            sync_soon(&context.container);
        }
        payload(&context.container)
    });

    router.delete("/meta/connections/:id", |context| async move {
        let id = connection_id(context.param("id"))?;
        get_connection(&context.container.db, &context.container.store, id)?;
        delete_connection(&context.container.db, id)?;
        context.container.reload()?;
        payload(&context.container)
    });

    router.post("/meta/connections/:id/test", |context| async move {
        let id = connection_id(context.param("id"))?;
        let found = get_connection(&context.container.db, &context.container.store, id)?;
        let settings = context.container.settings();
        let client = MetaAdsClient::new(&found.token, &settings.meta_api_version, &settings.meta_base_url);
        match client.list_accounts().await {
            Ok(accounts) => {
                mark_connection(&context.container.db, id, None, false)?;
                let count = accounts.len();
                Ok(json!({
                    "message": format!(
                        "{} accepted. {} ad account{} reachable.",
                        found.row.name,
                        count,
                        if count != 1 { "s" } else { "" }
                    ),
                    "accounts": count,
                    "connections": connections_payload(&context.container.db, &context.container.store)?,
                }))
            }
            Err(error) => {
                mark_connection(&context.container.db, id, Some(&error.message), false)?;
                Err(HttpError::new(502, error.message))
            }
        }
    });
}
