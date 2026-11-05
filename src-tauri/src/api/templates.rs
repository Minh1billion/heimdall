use crate::container::Container;
use crate::router::{object_of, HttpError, Router};
use crate::services::templates::{clean_config, create, delete, list, template_out, update, METRIC_KEYS};
use serde_json::{json, Value};

fn payload(container: &Container) -> Result<Value, HttpError> {
    let items = list(&container.db)?;
    let rows: Vec<Value> = items.iter().map(template_out).collect();
    Ok(json!({ "templates": rows, "metrics": METRIC_KEYS }))
}

fn template_id(raw: &str) -> Result<i64, HttpError> {
    match raw.trim().parse::<f64>() {
        Ok(value) if value.fract() == 0.0 && value >= 1.0 && value <= 9e15 => Ok(value as i64),
        _ => Err(HttpError::new(404, "Template not found")),
    }
}

pub fn register(router: &mut Router) {
    router.get("/templates", |context| async move { payload(&context.container) });

    router.post("/templates", |context| async move {
        let object = object_of(&context.body)?;
        let name = object.get("name").and_then(Value::as_str).unwrap_or("");
        let config = clean_config(object)?;
        create(&context.container.db, name, &config)?;
        payload(&context.container)
    });

    router.put("/templates/:id", |context| async move {
        let id = template_id(context.param("id"))?;
        let object = object_of(&context.body)?;
        let name = object.get("name").and_then(Value::as_str).unwrap_or("");
        let config = clean_config(object)?;
        update(&context.container.db, id, name, &config)?;
        payload(&context.container)
    });

    router.delete("/templates/:id", |context| async move {
        let id = template_id(context.param("id"))?;
        delete(&context.container.db, id)?;
        payload(&context.container)
    });
}
