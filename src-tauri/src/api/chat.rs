use crate::agent::runtime::run_agent;
use crate::models::{clip, iso, utcnow};
use crate::ports::llm::{message, Message, Role};
use crate::router::{HttpError, Router};
use crate::validation::{parse, Field, Kind};
use rusqlite::{params, OptionalExtension};
use serde_json::{json, Value};

const HISTORY_LIMIT: i64 = 20;

fn role_from(text: &str) -> Role {
    match text {
        "system" => Role::System,
        "assistant" => Role::Assistant,
        "tool" => Role::Tool,
        _ => Role::User,
    }
}

fn charts_for(conn: &rusqlite::Connection, run_id: &Option<String>) -> Vec<Value> {
    let run = match run_id {
        Some(found) => found,
        None => return Vec::new(),
    };
    let mut out: Vec<Value> = Vec::new();
    if let Ok(mut statement) = conn.prepare("SELECT spec FROM run_charts WHERE run_id = ?1 ORDER BY id") {
        if let Ok(rows) = statement.query_map([run], |row| row.get::<_, String>(0)) {
            for spec in rows.flatten() {
                if let Ok(parsed) = serde_json::from_str::<Value>(&spec) {
                    out.push(parsed);
                }
            }
        }
    }
    out
}

fn message_out(
    id: i64,
    role: &str,
    content: &str,
    run_id: &Option<String>,
    created_at: &str,
    charts: Vec<Value>,
) -> Value {
    json!({
        "id": id,
        "role": role,
        "content": content,
        "run_id": run_id,
        "created_at": iso(created_at),
        "charts": charts,
    })
}

fn run_out(row: &rusqlite::Row) -> rusqlite::Result<Value> {
    Ok(json!({
        "id": row.get::<_, String>("id")?,
        "created_at": iso(&row.get::<_, String>("created_at")?),
        "input": row.get::<_, String>("input")?,
        "status": row.get::<_, String>("status")?,
        "steps": row.get::<_, i64>("steps")?,
        "total_tokens": row.get::<_, i64>("total_tokens")?,
        "duration_ms": row.get::<_, i64>("duration_ms")?,
        "error": row.get::<_, Option<String>>("error")?,
    }))
}

fn limit_from(raw: Option<&String>) -> Result<i64, HttpError> {
    let text = match raw {
        Some(found) => found,
        None => return Ok(50),
    };
    let number: f64 = if text.trim().is_empty() {
        0.0
    } else {
        match text.trim().parse::<f64>() {
            Ok(found) if found.is_finite() => found,
            _ => return Err(HttpError::new(422, "Expected number, received nan")),
        }
    };
    if number.fract() != 0.0 {
        return Err(HttpError::new(422, "Expected integer, received float"));
    }
    if number < 1.0 {
        return Err(HttpError::new(422, "Number must be greater than or equal to 1"));
    }
    if number > 200.0 {
        return Err(HttpError::new(422, "Number must be less than or equal to 200"));
    }
    Ok(number as i64)
}

pub fn register(router: &mut Router) {
    router.post("/chat/messages", |context| async move {
        let input = parse(
            &[
                Field::text("conversation_id").nullish(),
                Field::new("text", Kind::Str { min: Some(1), max: Some(4000) }),
            ],
            &context.body,
        )?;
        let container = &context.container;
        let text = input.get("text").and_then(Value::as_str).unwrap_or("").to_string();
        if container.ordered_providers().is_empty() {
            return Err(HttpError::new(
                503,
                "No language model provider is configured. Add an API key in Settings.",
            ));
        }
        let requested = input
            .get("conversation_id")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            .map(String::from);
        let conversation_id: String = match requested {
            Some(id) => {
                let found: Option<String> = {
                    let conn = container.db.lock();
                    conn.query_row("SELECT id FROM conversations WHERE id = ?1", [&id], |row| {
                        row.get::<_, String>(0)
                    })
                    .optional()?
                };
                match found {
                    Some(existing) => existing,
                    None => return Err(HttpError::new(404, "Conversation not found")),
                }
            }
            None => {
                let id = uuid::Uuid::new_v4().to_string();
                let now = utcnow();
                let conn = container.db.lock();
                conn.execute(
                    "INSERT INTO conversations (id, title, created_at, updated_at) VALUES (?1, ?2, ?3, ?4)",
                    params![id, clip(text.trim(), 80), now, now],
                )?;
                id
            }
        };
        let history: Vec<Message> = {
            let conn = container.db.lock();
            let mut statement = conn.prepare(
                "SELECT role, content FROM chat_messages WHERE conversation_id = ?1 ORDER BY id DESC LIMIT ?2",
            )?;
            let mut rows = statement
                .query_map(params![conversation_id, HISTORY_LIMIT], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })?
                .collect::<Result<Vec<_>, _>>()?;
            rows.reverse();
            rows.iter()
                .map(|(role, content)| message(role_from(role), Some(content)))
                .collect()
        };
        {
            let conn = container.db.lock();
            conn.execute(
                "INSERT INTO chat_messages (conversation_id, role, content, created_at) VALUES (?1, 'user', ?2, ?3)",
                params![conversation_id, text, utcnow()],
            )?;
        }
        let run_id = uuid::Uuid::new_v4().to_string();
        let result = run_agent(container, &run_id, &text, history, Some(&conversation_id)).await?;
        if result.status == "error" {
            let detail = result.error.clone().filter(|value| !value.is_empty());
            return Err(HttpError::new(
                502,
                detail.unwrap_or_else(|| "The model request failed".to_string()),
            ));
        }
        let now = utcnow();
        let reply = {
            let conn = container.db.lock();
            conn.execute(
                "INSERT INTO chat_messages (conversation_id, role, content, run_id, created_at) VALUES (?1, 'assistant', ?2, ?3, ?4)",
                params![conversation_id, result.text, result.run_id, now],
            )?;
            let id = conn.last_insert_rowid();
            conn.execute(
                "UPDATE conversations SET updated_at = ?1 WHERE id = ?2",
                params![now, conversation_id],
            )?;
            message_out(
                id,
                "assistant",
                &result.text,
                &Some(result.run_id.clone()),
                &now,
                charts_for(&conn, &Some(result.run_id.clone())),
            )
        };
        Ok::<Value, HttpError>(json!({ "conversation_id": conversation_id, "message": reply }))
    });

    router.get("/chat/conversations", |context| async move {
        let conn = context.container.db.lock();
        let mut statement =
            conn.prepare("SELECT id, title, updated_at FROM conversations ORDER BY updated_at DESC LIMIT 100")?;
        let rows = statement
            .query_map([], |row| {
                Ok(json!({
                    "id": row.get::<_, String>(0)?,
                    "title": row.get::<_, String>(1)?,
                    "updated_at": iso(&row.get::<_, String>(2)?),
                }))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok::<Value, HttpError>(Value::Array(rows))
    });

    router.get("/chat/conversations/:id", |context| async move {
        let id = context.param("id").to_string();
        let conn = context.container.db.lock();
        let found: Option<(String, String)> = conn
            .query_row(
                "SELECT title, updated_at FROM conversations WHERE id = ?1",
                [&id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()?;
        let (title, updated_at) = match found {
            Some(entry) => entry,
            None => return Err(HttpError::new(404, "Conversation not found")),
        };
        let mut statement = conn.prepare(
            "SELECT id, role, content, run_id, created_at FROM chat_messages WHERE conversation_id = ?1 ORDER BY id",
        )?;
        let messages = statement
            .query_map([&id], |row| {
                let run_id = row.get::<_, Option<String>>(3)?;
                Ok(message_out(
                    row.get::<_, i64>(0)?,
                    &row.get::<_, String>(1)?,
                    &row.get::<_, String>(2)?,
                    &run_id,
                    &row.get::<_, String>(4)?,
                    charts_for(&conn, &run_id),
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(json!({
            "id": id,
            "title": title,
            "updated_at": iso(&updated_at),
            "messages": messages,
        }))
    });

    router.add("DELETE", "/chat/conversations/:id", 204, |context| async move {
        let id = context.param("id").to_string();
        let conn = context.container.db.lock();
        conn.execute(
            "DELETE FROM run_charts WHERE run_id IN (SELECT run_id FROM chat_messages WHERE conversation_id = ?1 AND run_id IS NOT NULL)",
            [&id],
        )?;
        conn.execute("DELETE FROM chat_messages WHERE conversation_id = ?1", [&id])?;
        conn.execute("DELETE FROM conversations WHERE id = ?1", [&id])?;
        Ok::<Value, HttpError>(Value::Null)
    });

    router.get("/logs/runs", |context| async move {
        let limit = limit_from(context.query.get("limit"))?;
        let conn = context.container.db.lock();
        let mut statement = conn.prepare("SELECT * FROM agent_runs ORDER BY created_at DESC LIMIT ?1")?;
        let rows = statement
            .query_map([limit], run_out)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok::<Value, HttpError>(Value::Array(rows))
    });

    router.get("/logs/runs/:id", |context| async move {
        let id = context.param("id").to_string();
        let conn = context.container.db.lock();
        let run = conn
            .query_row("SELECT * FROM agent_runs WHERE id = ?1", [&id], |row| {
                Ok((run_out(row)?, row.get::<_, String>("output")?))
            })
            .optional()?;
        let (mut base, output) = match run {
            Some(entry) => entry,
            None => return Err(HttpError::new(404, "Run not found")),
        };
        let mut statement = conn.prepare("SELECT * FROM tool_calls WHERE run_id = ?1 ORDER BY id")?;
        let calls = statement
            .query_map([&id], |row| {
                Ok(json!({
                    "step": row.get::<_, i64>("step")?,
                    "tool_name": row.get::<_, String>("tool_name")?,
                    "arguments": row.get::<_, String>("arguments")?,
                    "ok": row.get::<_, i64>("ok")? != 0,
                    "error_code": row.get::<_, Option<String>>("error_code")?,
                    "latency_ms": row.get::<_, i64>("latency_ms")?,
                    "result": row.get::<_, String>("result")?,
                }))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        let mut statement = conn.prepare("SELECT * FROM llm_usage WHERE run_id = ?1 ORDER BY id")?;
        let requests = statement
            .query_map([&id], |row| {
                Ok(json!({
                    "created_at": iso(&row.get::<_, String>("created_at")?),
                    "provider": row.get::<_, String>("provider")?,
                    "model": row.get::<_, String>("model")?,
                    "input_tokens": row.get::<_, i64>("input_tokens")?,
                    "output_tokens": row.get::<_, i64>("output_tokens")?,
                    "cached_tokens": row.get::<_, i64>("cached_tokens")?,
                    "latency_ms": row.get::<_, i64>("latency_ms")?,
                    "ok": row.get::<_, i64>("ok")? != 0,
                    "error_code": row.get::<_, Option<String>>("error_code")?,
                }))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        if let Value::Object(map) = &mut base {
            map.insert("output".to_string(), json!(output));
            map.insert("tool_calls".to_string(), Value::Array(calls));
            map.insert("model_requests".to_string(), Value::Array(requests));
        }
        Ok(base)
    });
}
