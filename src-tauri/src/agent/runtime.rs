use crate::adapters::llm::common::js_stringify;
use crate::agent::prompts::system_prompt;
use crate::container::Container;
use crate::models::{clip, today_utc, utcnow};
use crate::ports::errors::LLMError;
use crate::ports::llm::{llm_request, message, usage_total, LLMPort, Message, Role, ToolCall};
use crate::ports::tool::{ToolContext, ToolError};
use crate::router::Failure;
use crate::tools::{build_registry, ToolRegistry};
use crate::validation::{issues_text, validate};
use rusqlite::params;
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::{Duration, Instant};

const RESULT_LIMIT: usize = 20000;

pub struct AgentResult {
    pub run_id: String,
    pub status: String,
    pub text: String,
    pub error: Option<String>,
}

enum Stop {
    Llm(LLMError),
    Other(Failure),
}

impl From<LLMError> for Stop {
    fn from(error: LLMError) -> Self {
        Stop::Llm(error)
    }
}

impl From<rusqlite::Error> for Stop {
    fn from(error: rusqlite::Error) -> Self {
        Stop::Other(Box::new(error))
    }
}

impl From<Failure> for Stop {
    fn from(error: Failure) -> Self {
        Stop::Other(error)
    }
}

fn failure(code: &str, text: &str, hint: &str) -> Value {
    json!({ "ok": false, "error_code": code, "message": text, "hint": hint })
}

async fn execute(
    container: &Arc<Container>,
    registry: &ToolRegistry,
    ctx: &ToolContext,
    call: &ToolCall,
    step: i64,
) -> Result<Message, Failure> {
    let started = Instant::now();
    let result: Value = match registry.get(&call.name) {
        None => failure("unknown_tool", &format!("Unknown tool: {}", call.name), ""),
        Some(tool) => match validate(&tool.fields(), &call.arguments) {
            Err(issues) => failure("invalid_arguments", &issues_text(&issues), ""),
            Ok(args) => {
                let seconds = container.settings().tool_timeout_s.max(0.001);
                match tokio::time::timeout(Duration::from_secs_f64(seconds), tool.run(ctx, &args)).await {
                    Err(_) => failure("timeout", &format!("{} timed out", call.name), ""),
                    Ok(Ok(value)) => value,
                    Ok(Err(ToolError::Failure(found))) => failure(&found.error_code, &found.message, &found.hint),
                    Ok(Err(ToolError::Other(text))) => {
                        failure("tool_error", &format!("{} failed: {}", call.name, text), "")
                    }
                }
            }
        },
    };
    let content = js_stringify(&result);
    let ok = result.get("ok") == Some(&Value::Bool(true));
    let error_code = result.get("error_code").and_then(Value::as_str).map(String::from);
    {
        let conn = container.db.lock();
        conn.execute(
            "INSERT INTO tool_calls (run_id, created_at, step, tool_name, arguments, ok, error_code, latency_ms, result)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                ctx.run_id,
                utcnow(),
                step,
                call.name,
                clip(&js_stringify(&call.arguments), 4000),
                if ok { 1 } else { 0 },
                error_code,
                started.elapsed().as_millis() as i64,
                clip(&content, 600)
            ],
        )?;
    }
    Ok(Message {
        role: Role::Tool,
        content: Some(clip(&content, RESULT_LIMIT)),
        tool_calls: Vec::new(),
        tool_call_id: Some(call.id.clone()),
        name: Some(call.name.clone()),
    })
}

fn finish(
    container: &Container,
    run_id: &str,
    started: Instant,
    status: &str,
    steps: i64,
    tokens: i64,
    output: Option<&str>,
    error: Option<&str>,
) -> rusqlite::Result<()> {
    let duration = started.elapsed().as_millis() as i64;
    let mut columns: Vec<&str> = vec!["status", "steps", "total_tokens"];
    let mut values: Vec<rusqlite::types::Value> = vec![
        rusqlite::types::Value::Text(status.to_string()),
        rusqlite::types::Value::Integer(steps),
        rusqlite::types::Value::Integer(tokens),
    ];
    if let Some(text) = output {
        columns.push("output");
        values.push(rusqlite::types::Value::Text(text.to_string()));
    }
    if let Some(text) = error {
        columns.push("error");
        values.push(rusqlite::types::Value::Text(text.to_string()));
    }
    let assignments: Vec<String> = columns.iter().map(|column| format!("{} = ?", column)).collect();
    values.push(rusqlite::types::Value::Integer(duration));
    values.push(rusqlite::types::Value::Text(run_id.to_string()));
    let sql = format!(
        "UPDATE agent_runs SET {}, duration_ms = ? WHERE id = ?",
        assignments.join(", ")
    );
    let conn = container.db.lock();
    conn.execute(&sql, rusqlite::params_from_iter(values.iter()))?;
    Ok(())
}

async fn drive(
    container: &Arc<Container>,
    registry: &ToolRegistry,
    ctx: &ToolContext,
    messages: &mut Vec<Message>,
    tokens: &mut i64,
    step: &mut i64,
    started: Instant,
) -> Result<AgentResult, Stop> {
    let settings = container.settings();
    let run_id = ctx.run_id.clone();
    let llm = container
        .router_for(None)
        .map_err(|error| -> Failure { format!("{} is not configured", error.0).into() })?;
    while *step < settings.agent_max_steps {
        if *tokens >= settings.agent_token_budget {
            finish(container, &run_id, started, "budget_exceeded", *step, *tokens, None, None)?;
            return Ok(AgentResult {
                run_id,
                status: "budget_exceeded".to_string(),
                text: "I stopped because this question used too many tokens. Try a narrower question.".to_string(),
                error: None,
            });
        }
        *step += 1;
        let mut request = llm_request(messages.clone());
        request.tools = registry.specs();
        request.max_output_tokens = 2048;
        request.timeout_s = settings.llm_timeout_s;
        request.metadata.insert("run_id".to_string(), json!(run_id));
        let response = llm.generate(&request).await?;
        *tokens += usage_total(&response.usage);
        messages.push(response.message.clone());
        let calls = response.message.tool_calls.clone();
        if calls.is_empty() {
            let text = response.message.content.clone().unwrap_or_default();
            finish(container, &run_id, started, "ok", *step, *tokens, Some(&text), None)?;
            return Ok(AgentResult {
                run_id,
                status: "ok".to_string(),
                text,
                error: None,
            });
        }
        let current = *step;
        let results = futures::future::join_all(
            calls
                .iter()
                .map(|call| execute(container, registry, ctx, call, current)),
        )
        .await;
        for result in results {
            messages.push(result?);
        }
    }
    finish(container, &run_id, started, "max_steps", *step, *tokens, None, None)?;
    Ok(AgentResult {
        run_id,
        status: "max_steps".to_string(),
        text: "I could not finish within the step limit. Try a more specific question.".to_string(),
        error: None,
    })
}

pub async fn run_agent(
    container: &Arc<Container>,
    run_id: &str,
    question: &str,
    history: Vec<Message>,
    conversation_id: Option<&str>,
) -> Result<AgentResult, Failure> {
    let started = Instant::now();
    {
        let conn = container.db.lock();
        conn.execute(
            "INSERT INTO agent_runs (id, created_at, conversation_id, input, output, status, steps, total_tokens, duration_ms)
             VALUES (?1, ?2, ?3, ?4, '', 'running', 0, 0, 0)",
            params![run_id, utcnow(), conversation_id, question],
        )?;
    }
    let registry = build_registry();
    let ctx = ToolContext {
        run_id: run_id.to_string(),
        container: container.clone(),
        allowed_account_ids: None,
    };
    let mut messages: Vec<Message> = vec![message(Role::System, Some(&system_prompt(&today_utc())))];
    messages.extend(history);
    messages.push(message(Role::User, Some(question)));
    let mut tokens: i64 = 0;
    let mut step: i64 = 0;
    match drive(container, &registry, &ctx, &mut messages, &mut tokens, &mut step, started).await {
        Ok(result) => Ok(result),
        Err(Stop::Llm(error)) => {
            let text = clip(&error.message, 500);
            finish(container, run_id, started, "error", step, tokens, None, Some(&text))?;
            Ok(AgentResult {
                run_id: run_id.to_string(),
                status: "error".to_string(),
                text: String::new(),
                error: Some(error.message),
            })
        }
        Err(Stop::Other(error)) => {
            let text = clip(&error.to_string(), 500);
            let _ = finish(container, run_id, started, "error", step, tokens, None, Some(&text));
            Err(error)
        }
    }
}
