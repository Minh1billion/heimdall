use crate::container::Container;
use crate::router::{issues_error, object_of, read_bool, read_string, HttpError, Router};
use crate::services::destinations::{
    create_destination, delete_destination, deliver_to, destination_out, get_destination, load_destinations,
    update_destination, Config, DestinationUpdate, Scope,
};
use serde_json::{json, Map, Value};

fn payload(container: &Container) -> Result<Value, HttpError> {
    let items = load_destinations(&container.db, &container.store)?;
    let rows: Vec<Value> = items.iter().map(destination_out).collect();
    Ok(json!({ "destinations": rows }))
}

fn destination_id(raw: &str) -> Result<i64, HttpError> {
    match raw.trim().parse::<f64>() {
        Ok(value) if value.fract() == 0.0 && value >= 1.0 && value <= 9e15 => Ok(value as i64),
        _ => Err(HttpError::new(404, "Destination not found")),
    }
}

fn read_config(object: &Map<String, Value>) -> Result<Option<Config>, HttpError> {
    match object.get("config") {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Object(map)) => {
            let mut output = Config::new();
            for (key, value) in map {
                match value {
                    Value::String(text) => {
                        output.insert(key.clone(), text.clone());
                    }
                    Value::Null => {}
                    _ => return Err(HttpError::new(422, format!("config.{}: Expected string", key))),
                }
            }
            Ok(Some(output))
        }
        Some(_) => Err(HttpError::new(422, "config: Expected object")),
    }
}

fn read_scope(object: &Map<String, Value>) -> Result<(Option<Option<i64>>, Option<Vec<String>>), HttpError> {
    let template_id = match object.get("template_id") {
        None => None,
        Some(Value::Null) => Some(None),
        Some(value) => match value.as_i64() {
            Some(found) if found >= 1 => Some(Some(found)),
            _ => return Err(HttpError::new(422, "template_id: Expected positive integer")),
        },
    };
    let account_ids = match object.get("account_ids") {
        None => None,
        Some(Value::Null) => Some(Vec::new()),
        Some(Value::Array(items)) => {
            let mut out: Vec<String> = Vec::new();
            for item in items {
                match item.as_str() {
                    Some(text) if !text.trim().is_empty() => {
                        let clean = text.trim().to_string();
                        if !out.contains(&clean) {
                            out.push(clean);
                        }
                    }
                    _ => return Err(HttpError::new(422, "account_ids: Expected strings")),
                }
            }
            Some(out)
        }
        Some(_) => return Err(HttpError::new(422, "account_ids: Expected array")),
    };
    Ok((template_id, account_ids))
}

pub fn register(router: &mut Router) {
    router.get("/destinations", |context| async move { payload(&context.container) });

    router.post("/destinations", |context| async move {
        let object = object_of(&context.body)?;
        let mut issues: Vec<String> = Vec::new();
        let name = read_string(object, "name", true, false, &mut issues);
        let channel = read_string(object, "channel", true, false, &mut issues);
        if !issues.is_empty() {
            return Err(issues_error(issues));
        }
        let config = read_config(object)?.unwrap_or_default();
        let (template_id, account_ids) = read_scope(object)?;
        let scope = Scope {
            template_id: template_id.flatten(),
            account_ids: account_ids.unwrap_or_default(),
        };
        create_destination(
            &context.container.db,
            &context.container.store,
            &name.unwrap_or_default(),
            &channel.unwrap_or_default(),
            &config,
            &scope,
        )?;
        payload(&context.container)
    });

    router.put("/destinations/:id", |context| async move {
        let id = destination_id(context.param("id"))?;
        let object = object_of(&context.body)?;
        let mut issues: Vec<String> = Vec::new();
        let name = read_string(object, "name", false, false, &mut issues);
        let enabled = read_bool(object, "enabled", &mut issues);
        if !issues.is_empty() {
            return Err(issues_error(issues));
        }
        let (template_id, account_ids) = read_scope(object)?;
        let update = DestinationUpdate {
            name,
            config: read_config(object)?,
            enabled,
            template_id,
            account_ids,
        };
        update_destination(&context.container.db, &context.container.store, id, &update)?;
        payload(&context.container)
    });

    router.delete("/destinations/:id", |context| async move {
        let id = destination_id(context.param("id"))?;
        get_destination(&context.container.db, &context.container.store, id)?;
        delete_destination(&context.container.db, id)?;
        payload(&context.container)
    });

    router.post("/destinations/:id/test", |context| async move {
        let id = destination_id(context.param("id"))?;
        let stored = get_destination(&context.container.db, &context.container.store, id)?;
        deliver_to(
            &context.container,
            &stored,
            "Heimdall test",
            "This is a test message from Heimdall.",
        )
        .await?;
        let rows = payload(&context.container)?;
        Ok::<Value, HttpError>(json!({
            "message": format!("Test message sent to {}.", stored.row.name),
            "destinations": rows.get("destinations").cloned().unwrap_or(Value::Null),
        }))
    });
}
