use crate::adapters::llm::common::{estimate_tokens, field_int, post_json, py_json, truthy};
use crate::ports::errors::{LLMError, LLMErrorKind};
use crate::ports::llm::{
    message, LLMCapabilities, LLMPort, LLMRequest, LLMResponse, Message, Role, StopReason, ToolCall, Usage,
};
use async_trait::async_trait;
use serde_json::{json, Map, Value};
use std::time::Instant;

pub struct OpenAIOptions {
    pub api_key: String,
    pub model: String,
    pub base_url: Option<String>,
    pub name: Option<String>,
    pub max_tokens_field: Option<String>,
    pub capabilities: Option<LLMCapabilities>,
}

const DEFAULT_CAPABILITIES: LLMCapabilities = LLMCapabilities {
    tool_calling: true,
    parallel_tool_calls: true,
    structured_output: true,
    streaming: false,
    vision: true,
    max_context_tokens: 128000,
};

pub struct OpenAIAdapter {
    api_key: String,
    model: String,
    base_url: String,
    name: String,
    max_tokens_field: String,
    capabilities: LLMCapabilities,
}

impl OpenAIAdapter {
    pub fn new(options: OpenAIOptions) -> Self {
        let base = options.base_url.unwrap_or_else(|| "https://api.openai.com/v1".to_string());
        OpenAIAdapter {
            api_key: options.api_key,
            model: options.model,
            base_url: base.trim_end_matches('/').to_string(),
            name: options.name.unwrap_or_else(|| "openai".to_string()),
            max_tokens_field: options
                .max_tokens_field
                .unwrap_or_else(|| "max_completion_tokens".to_string()),
            capabilities: options.capabilities.unwrap_or(DEFAULT_CAPABILITIES),
        }
    }

    fn build_payload(&self, req: &LLMRequest) -> Value {
        let mut payload = Map::new();
        payload.insert("model".to_string(), json!(self.model));
        payload.insert(
            "messages".to_string(),
            Value::Array(req.messages.iter().map(|item| self.encode_message(item)).collect()),
        );
        payload.insert(self.max_tokens_field.clone(), json!(req.max_output_tokens));
        if let Some(temperature) = req.temperature {
            payload.insert("temperature".to_string(), json!(temperature));
        }
        if !req.tools.is_empty() {
            let tools: Vec<Value> = req
                .tools
                .iter()
                .map(|tool| {
                    json!({
                        "type": "function",
                        "function": {
                            "name": tool.name,
                            "description": tool.description,
                            "parameters": tool.parameters,
                        },
                    })
                })
                .collect();
            payload.insert("tools".to_string(), Value::Array(tools));
            payload.insert("tool_choice".to_string(), self.encode_choice(&req.tool_choice));
        }
        if let Some(schema) = &req.response_schema {
            if schema.as_object().map_or(false, |entries| !entries.is_empty()) {
                payload.insert(
                    "response_format".to_string(),
                    json!({
                        "type": "json_schema",
                        "json_schema": { "name": "response", "schema": schema },
                    }),
                );
            }
        }
        Value::Object(payload)
    }

    fn encode_choice(&self, choice: &str) -> Value {
        if choice == "auto" || choice == "none" || choice == "required" {
            return json!(choice);
        }
        json!({ "type": "function", "function": { "name": choice } })
    }

    fn encode_message(&self, item: &Message) -> Value {
        if item.role == Role::Tool {
            return json!({
                "role": "tool",
                "tool_call_id": item.tool_call_id,
                "content": item.content.clone().unwrap_or_default(),
            });
        }
        if item.role == Role::Assistant && !item.tool_calls.is_empty() {
            let calls: Vec<Value> = item
                .tool_calls
                .iter()
                .map(|call| {
                    json!({
                        "id": call.id,
                        "type": "function",
                        "function": { "name": call.name, "arguments": py_json(&call.arguments) },
                    })
                })
                .collect();
            return json!({
                "role": "assistant",
                "content": item.content,
                "tool_calls": calls,
            });
        }
        json!({
            "role": item.role.as_str(),
            "content": item.content.clone().unwrap_or_default(),
        })
    }

    fn malformed(&self) -> LLMError {
        LLMError::new(
            LLMErrorKind::ProviderDown,
            format!("{} returned a malformed response", self.name),
            &self.name,
        )
    }

    fn decode_call(&self, item: &Value) -> Option<ToolCall> {
        let function = item.get("function").filter(|entry| entry.is_object())?;
        let id = item.get("id")?;
        let name = function.get("name")?;
        let arguments = function.get("arguments");
        let text: String = match arguments {
            Some(Value::String(text)) if !text.is_empty() => text.clone(),
            None => "{}".to_string(),
            Some(other) if !truthy(other) => "{}".to_string(),
            Some(other) => {
                return Some(ToolCall {
                    id: value_text(id),
                    name: value_text(name),
                    arguments: json!({ "_raw": other }),
                })
            }
        };
        let parsed: Value = match serde_json::from_str(&text) {
            Ok(parsed) => parsed,
            Err(_) => json!({ "_raw": text }),
        };
        let arguments = if parsed.is_object() { parsed } else { json!({ "_raw": parsed }) };
        Some(ToolCall {
            id: value_text(id),
            name: value_text(name),
            arguments,
        })
    }

    fn parse(&self, data: &Value, latency_ms: i64) -> Result<LLMResponse, LLMError> {
        let choice = data
            .get("choices")
            .and_then(|choices| choices.get(0))
            .ok_or_else(|| self.malformed())?;
        let raw_message = choice
            .get("message")
            .filter(|entry| !entry.is_null())
            .ok_or_else(|| self.malformed())?;
        let mut calls: Vec<ToolCall> = Vec::new();
        match raw_message.get("tool_calls") {
            None | Some(Value::Null) => {}
            Some(Value::Array(items)) => {
                for item in items {
                    calls.push(self.decode_call(item).ok_or_else(|| self.malformed())?);
                }
            }
            Some(_) => return Err(self.malformed()),
        }
        let stop = if !calls.is_empty() {
            StopReason::ToolUse
        } else if choice.get("finish_reason").and_then(Value::as_str) == Some("length") {
            StopReason::MaxTokens
        } else {
            StopReason::End
        };
        let empty = json!({});
        let raw_usage = data.get("usage").filter(|entry| truthy(entry)).unwrap_or(&empty);
        let details = raw_usage
            .get("prompt_tokens_details")
            .filter(|entry| truthy(entry))
            .unwrap_or(&empty);
        let usage = Usage {
            input_tokens: field_int(raw_usage, "prompt_tokens"),
            output_tokens: field_int(raw_usage, "completion_tokens"),
            cached_tokens: field_int(details, "cached_tokens"),
        };
        let content = raw_message.get("content").and_then(Value::as_str);
        let mut reply = message(Role::Assistant, content);
        reply.tool_calls = calls;
        let model = data
            .get("model")
            .and_then(Value::as_str)
            .filter(|text| !text.is_empty())
            .unwrap_or(&self.model)
            .to_string();
        Ok(LLMResponse {
            message: reply,
            stop_reason: stop,
            usage,
            model,
            provider: self.name.clone(),
            latency_ms,
        })
    }
}

fn value_text(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        other => other.to_string(),
    }
}

#[async_trait]
impl LLMPort for OpenAIAdapter {
    fn name(&self) -> &str {
        &self.name
    }

    fn model(&self) -> &str {
        &self.model
    }

    fn capabilities(&self) -> &LLMCapabilities {
        &self.capabilities
    }

    fn count_tokens(&self, messages: &[Message]) -> i64 {
        estimate_tokens(messages)
    }

    async fn generate(&self, req: &LLMRequest) -> Result<LLMResponse, LLMError> {
        let payload = self.build_payload(req);
        let mut headers: Vec<(&str, String)> = vec![("Content-Type", "application/json".to_string())];
        if !self.api_key.is_empty() {
            headers.push(("Authorization", format!("Bearer {}", self.api_key)));
        }
        let started = Instant::now();
        let url = format!("{}/chat/completions", self.base_url);
        let data = post_json(&self.name, &url, &headers, &payload, req.timeout_s).await?;
        let latency_ms = started.elapsed().as_millis() as i64;
        self.parse(&data, latency_ms)
    }
}
