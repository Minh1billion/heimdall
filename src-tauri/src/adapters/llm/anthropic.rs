use crate::adapters::llm::common::{estimate_tokens, field_int, post_json, py_json, truthy};
use crate::ports::errors::{LLMError, LLMErrorKind};
use crate::ports::llm::{
    message, LLMCapabilities, LLMPort, LLMRequest, LLMResponse, Message, Role, StopReason, ToolCall, Usage,
};
use async_trait::async_trait;
use serde_json::{json, Map, Value};
use std::time::Instant;

const STRUCTURED_TOOL: &str = "structured_response";

pub struct AnthropicOptions {
    pub api_key: String,
    pub model: String,
    pub base_url: Option<String>,
    pub api_version: Option<String>,
}

pub struct AnthropicAdapter {
    name: String,
    api_key: String,
    model: String,
    base_url: String,
    api_version: String,
    capabilities: LLMCapabilities,
}

impl AnthropicAdapter {
    pub fn new(options: AnthropicOptions) -> Self {
        let base = options.base_url.unwrap_or_else(|| "https://api.anthropic.com".to_string());
        AnthropicAdapter {
            name: "anthropic".to_string(),
            api_key: options.api_key,
            model: options.model,
            base_url: base.trim_end_matches('/').to_string(),
            api_version: options.api_version.unwrap_or_else(|| "2023-06-01".to_string()),
            capabilities: LLMCapabilities {
                tool_calling: true,
                parallel_tool_calls: true,
                structured_output: true,
                streaming: false,
                vision: true,
                max_context_tokens: 200000,
            },
        }
    }

    fn build_payload(&self, req: &LLMRequest) -> Value {
        let (system, messages) = self.encode_messages(&req.messages);
        let mut payload = Map::new();
        payload.insert("model".to_string(), json!(self.model));
        payload.insert("max_tokens".to_string(), json!(req.max_output_tokens));
        payload.insert("messages".to_string(), Value::Array(messages));
        if !system.is_empty() {
            payload.insert("system".to_string(), json!(system));
        }
        if let Some(temperature) = req.temperature {
            payload.insert("temperature".to_string(), json!(temperature));
        }
        let mut tools: Vec<Value> = req
            .tools
            .iter()
            .map(|tool| {
                json!({
                    "name": tool.name,
                    "description": tool.description,
                    "input_schema": tool.parameters,
                })
            })
            .collect();
        let mut choice: Option<Value> = if tools.is_empty() {
            None
        } else {
            Some(self.encode_choice(&req.tool_choice))
        };
        if let Some(schema) = &req.response_schema {
            if schema.as_object().map_or(false, |entries| !entries.is_empty()) {
                tools.push(json!({
                    "name": STRUCTURED_TOOL,
                    "description": "Return the final answer in this exact structure.",
                    "input_schema": schema,
                }));
                choice = if tools.len() == 1 {
                    Some(json!({ "type": "tool", "name": STRUCTURED_TOOL }))
                } else {
                    Some(json!({ "type": "any" }))
                };
            }
        }
        if !tools.is_empty() {
            payload.insert("tools".to_string(), Value::Array(tools));
            if let Some(choice) = choice {
                payload.insert("tool_choice".to_string(), choice);
            }
        }
        Value::Object(payload)
    }

    fn encode_choice(&self, choice: &str) -> Value {
        if choice == "auto" {
            return json!({ "type": "auto" });
        }
        if choice == "none" {
            return json!({ "type": "none" });
        }
        if choice == "required" {
            return json!({ "type": "any" });
        }
        json!({ "type": "tool", "name": choice })
    }

    fn encode_messages(&self, messages: &[Message]) -> (String, Vec<Value>) {
        let mut system_parts: Vec<String> = Vec::new();
        let mut encoded: Vec<Value> = Vec::new();
        for item in messages {
            if item.role == Role::System {
                system_parts.push(item.content.clone().unwrap_or_default());
                continue;
            }
            if item.role == Role::Tool {
                let block = json!({
                    "type": "tool_result",
                    "tool_use_id": item.tool_call_id,
                    "content": item.content.clone().unwrap_or_default(),
                });
                let mut merged = false;
                if let Some(last) = encoded.last_mut() {
                    if last["role"] == "user" {
                        if let Some(blocks) = last.get_mut("content").and_then(|entry| entry.as_array_mut()) {
                            if blocks.first().map_or(false, |first| first["type"] == "tool_result") {
                                blocks.push(block.clone());
                                merged = true;
                            }
                        }
                    }
                }
                if !merged {
                    encoded.push(json!({ "role": "user", "content": [block] }));
                }
                continue;
            }
            if item.role == Role::Assistant {
                let mut blocks: Vec<Value> = Vec::new();
                if let Some(text) = item.content.as_deref() {
                    if !text.is_empty() {
                        blocks.push(json!({ "type": "text", "text": text }));
                    }
                }
                for call in &item.tool_calls {
                    blocks.push(json!({
                        "type": "tool_use",
                        "id": call.id,
                        "name": call.name,
                        "input": call.arguments,
                    }));
                }
                if !blocks.is_empty() {
                    encoded.push(json!({ "role": "assistant", "content": blocks }));
                }
                continue;
            }
            encoded.push(json!({
                "role": "user",
                "content": item.content.clone().unwrap_or_default(),
            }));
        }
        let system: Vec<String> = system_parts.into_iter().filter(|part| !part.is_empty()).collect();
        (system.join("\n\n"), encoded)
    }

    fn malformed(&self) -> LLMError {
        LLMError::new(
            LLMErrorKind::ProviderDown,
            format!("{} returned a malformed response", self.name),
            &self.name,
        )
    }

    fn parse(&self, data: &Value, latency_ms: i64) -> Result<LLMResponse, LLMError> {
        let blocks = data
            .get("content")
            .and_then(Value::as_array)
            .ok_or_else(|| self.malformed())?;
        let mut text_parts: Vec<String> = Vec::new();
        let mut calls: Vec<ToolCall> = Vec::new();
        let mut structured: Option<Value> = None;
        for block in blocks {
            let kind = block.get("type").and_then(Value::as_str).unwrap_or("");
            if kind == "text" {
                text_parts.push(block.get("text").and_then(Value::as_str).unwrap_or("").to_string());
            } else if kind == "tool_use" {
                let args = block
                    .get("input")
                    .filter(|entry| truthy(entry))
                    .cloned()
                    .unwrap_or_else(|| json!({}));
                let name = match block.get("name") {
                    Some(name) => name,
                    None => return Err(self.malformed()),
                };
                let name_text = match name {
                    Value::String(text) => text.clone(),
                    other => other.to_string(),
                };
                let id = block.get("id");
                if name_text != STRUCTURED_TOOL && id.is_none() {
                    return Err(self.malformed());
                }
                if name_text == STRUCTURED_TOOL {
                    structured = Some(args);
                } else {
                    let id_text = match id {
                        Some(Value::String(text)) => text.clone(),
                        Some(other) => other.to_string(),
                        None => String::new(),
                    };
                    calls.push(ToolCall {
                        id: id_text,
                        name: name_text,
                        arguments: args,
                    });
                }
            }
        }
        let joined = text_parts.join("");
        let mut content: Option<String> = if joined.is_empty() { None } else { Some(joined) };
        if let Some(found) = &structured {
            if calls.is_empty() {
                content = Some(py_json(found));
            }
        }
        let stop = if !calls.is_empty() {
            StopReason::ToolUse
        } else if data.get("stop_reason").and_then(Value::as_str) == Some("max_tokens") {
            StopReason::MaxTokens
        } else {
            StopReason::End
        };
        let empty = json!({});
        let raw_usage = data.get("usage").filter(|entry| truthy(entry)).unwrap_or(&empty);
        let usage = Usage {
            input_tokens: field_int(raw_usage, "input_tokens"),
            output_tokens: field_int(raw_usage, "output_tokens"),
            cached_tokens: field_int(raw_usage, "cache_read_input_tokens"),
        };
        let mut reply = message(Role::Assistant, content.as_deref());
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

#[async_trait]
impl LLMPort for AnthropicAdapter {
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
        let headers: Vec<(&str, String)> = vec![
            ("Content-Type", "application/json".to_string()),
            ("x-api-key", self.api_key.clone()),
            ("anthropic-version", self.api_version.clone()),
        ];
        let started = Instant::now();
        let url = format!("{}/v1/messages", self.base_url);
        let data = post_json(&self.name, &url, &headers, &payload, req.timeout_s).await?;
        let latency_ms = started.elapsed().as_millis() as i64;
        self.parse(&data, latency_ms)
    }
}
