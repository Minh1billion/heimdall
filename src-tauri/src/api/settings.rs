use crate::container::Container;
use crate::api::reports::deliver;
use crate::api::routes::test_provider;
use crate::router::{js_type, HttpError, Router};
use crate::services::app_settings::{display_value, mask, source_of, validate_value};
use crate::services::settings_groups::{field_spec, group_spec, GROUPS};
use serde_json::{json, Map, Value};
use std::collections::HashSet;

struct UpdateInput {
    values: Map<String, Value>,
    clear: Vec<String>,
}

fn parse_update(body: &Value) -> Result<UpdateInput, HttpError> {
    let object = match body {
        Value::Object(object) => object,
        Value::Null => return Err(HttpError::new(422, "Required")),
        other => {
            return Err(HttpError::new(
                422,
                format!("Expected object, received {}", js_type(other)),
            ))
        }
    };
    let values = match object.get("values") {
        None | Some(Value::Null) => Map::new(),
        Some(Value::Object(values)) => {
            for (key, value) in values {
                if !matches!(value, Value::String(_) | Value::Number(_)) {
                    return Err(HttpError::new(422, format!("values.{}: Invalid input", key)));
                }
            }
            values.clone()
        }
        Some(other) => {
            return Err(HttpError::new(
                422,
                format!("values: Expected object, received {}", js_type(other)),
            ))
        }
    };
    let clear = match object.get("clear") {
        None | Some(Value::Null) => Vec::new(),
        Some(Value::Array(items)) => {
            let mut keys = Vec::new();
            for (index, item) in items.iter().enumerate() {
                match item {
                    Value::String(text) => keys.push(text.clone()),
                    other => {
                        return Err(HttpError::new(
                            422,
                            format!("clear.{}: Expected string, received {}", index, js_type(other)),
                        ))
                    }
                }
            }
            keys
        }
        Some(other) => {
            return Err(HttpError::new(
                422,
                format!("clear: Expected array, received {}", js_type(other)),
            ))
        }
    };
    Ok(UpdateInput { values, clear })
}

fn group_payload(container: &Container, key: &str) -> Value {
    let group = match group_spec(key) {
        Some(group) => group,
        None => return Value::Null,
    };
    let settings = container.settings();
    let overrides = container.overrides();
    let fields: Vec<Value> = group
        .fields
        .iter()
        .map(|spec| {
            let current = display_value(&settings, spec);
            json!({
                "key": spec.key,
                "label": spec.label,
                "hint": spec.hint,
                "kind": spec.kind,
                "secret": spec.secret,
                "placeholder": spec.placeholder,
                "is_set": !current.is_empty(),
                "value": if spec.secret { Value::Null } else { Value::String(current.clone()) },
                "preview": if spec.secret && !current.is_empty() { Value::String(mask(&current)) } else { Value::Null },
                "source": source_of(&overrides, spec.key),
            })
        })
        .collect();
    let provider = container.provider_status().into_iter().find(|row| row.name == key);
    let configured: Value = if let Some(row) = &provider {
        Value::Bool(row.configured)
    } else if key == "telegram" || key == "slack" || key == "zalo" || key == "email" {
        Value::Bool(container.notifier(key).is_some())
    } else {
        Value::Null
    };
    let priority: Value = match &provider {
        Some(row) => match row.priority {
            Some(rank) => json!(rank),
            None => Value::Null,
        },
        None => Value::Null,
    };
    json!({
        "key": group.key,
        "label": group.label,
        "description": group.description,
        "section": group.section,
        "testable": group.testable,
        "configured": configured,
        "priority": priority,
        "fields": fields,
    })
}

fn full_payload(container: &Container) -> Value {
    let groups: Vec<Value> = GROUPS.iter().map(|group| group_payload(container, group.key)).collect();
    json!({
        "access": { "editable": true, "requires_token": false, "reason": Value::Null },
        "groups": groups,
    })
}

pub fn register(router: &mut Router) {
    router.get("/settings", |context| async move {
        Ok::<Value, HttpError>(full_payload(&context.container))
    });

    router.put("/settings/:group", |context| async move {
        let input = parse_update(&context.body)?;
        let spec = group_spec(context.param("group"))
            .ok_or_else(|| HttpError::new(404, "Unknown settings group"))?;
        let allowed: HashSet<&str> = spec.fields.iter().map(|item| item.key).collect();
        let unknown = input
            .values
            .keys()
            .chain(input.clear.iter())
            .find(|key| !allowed.contains(key.as_str()));
        if let Some(key) = unknown {
            return Err(HttpError::new(422, format!("Unknown setting: {}", key)));
        }
        let mut updates: Vec<(String, String)> = Vec::new();
        for (key, raw) in &input.values {
            let text = crate::services::app_settings::raw_text(raw);
            if text.trim().is_empty() {
                continue;
            }
            let field = field_spec(key).ok_or_else(|| HttpError::new(422, format!("Unknown setting: {}", key)))?;
            let value = validate_value(field, raw).map_err(|error| HttpError::new(422, error.0))?;
            updates.push((key.clone(), value));
        }
        let mut clear: Vec<String> = Vec::new();
        for key in &input.clear {
            if !clear.contains(key) && !updates.iter().any(|(name, _)| name == key) {
                clear.push(key.clone());
            }
        }
        if !updates.is_empty() || !clear.is_empty() {
            context.container.store.save(&updates, &clear)?;
            context.container.reload()?;
        }
        Ok::<Value, HttpError>(full_payload(&context.container))
    });

    router.post("/settings/:group/test", |context| async move {
        let group = context.param("group");
        let spec = group_spec(group).ok_or_else(|| HttpError::new(404, "Unknown settings group"))?;
        if !spec.testable {
            return Err(HttpError::new(400, "Nothing to test for this group"));
        }
        if group == "openai" || group == "anthropic" || group == "openai_compat" {
            let result = test_provider(&context.container, Some(group), None).await?;
            return Ok(json!({
                "message": result["text"],
                "model": result["model"],
                "latency_ms": result["latency_ms"],
            }));
        }
        if group == "telegram" || group == "slack" || group == "zalo" || group == "email" {
            deliver(&context.container, group, "Heimdall test", "This is a test message from Heimdall.").await?;
            return Ok(json!({ "message": "Test message sent.", "model": "", "latency_ms": 0 }));
        }
        Err(HttpError::new(400, "Nothing to test for this group"))
    });
}
