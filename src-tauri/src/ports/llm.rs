use crate::ports::errors::LLMError;
use crate::router::Failure;
use async_trait::async_trait;
use serde_json::{Map, Value};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    System,
    User,
    Assistant,
    Tool,
}

impl Role {
    pub fn as_str(&self) -> &'static str {
        match self {
            Role::System => "system",
            Role::User => "user",
            Role::Assistant => "assistant",
            Role::Tool => "tool",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StopReason {
    End,
    ToolUse,
    MaxTokens,
}

#[derive(Clone, Debug)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: Value,
}

#[derive(Clone, Debug)]
pub struct Message {
    pub role: Role,
    pub content: Option<String>,
    pub tool_calls: Vec<ToolCall>,
    pub tool_call_id: Option<String>,
    pub name: Option<String>,
}

pub fn message(role: Role, content: Option<&str>) -> Message {
    Message {
        role,
        content: content.map(String::from),
        tool_calls: Vec::new(),
        tool_call_id: None,
        name: None,
    }
}

#[derive(Clone, Debug)]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    pub parameters: Value,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Usage {
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cached_tokens: i64,
}

pub fn usage_total(usage: &Usage) -> i64 {
    usage.input_tokens + usage.output_tokens
}

#[derive(Clone, Debug)]
pub struct LLMRequest {
    pub messages: Vec<Message>,
    pub tools: Vec<ToolSpec>,
    pub tool_choice: String,
    pub response_schema: Option<Value>,
    pub temperature: Option<f64>,
    pub max_output_tokens: i64,
    pub timeout_s: f64,
    pub metadata: Map<String, Value>,
}

pub fn llm_request(messages: Vec<Message>) -> LLMRequest {
    LLMRequest {
        messages,
        tools: Vec::new(),
        tool_choice: "auto".to_string(),
        response_schema: None,
        temperature: None,
        max_output_tokens: 2048,
        timeout_s: 60.0,
        metadata: Map::new(),
    }
}

#[derive(Clone, Debug)]
pub struct LLMResponse {
    pub message: Message,
    pub stop_reason: StopReason,
    pub usage: Usage,
    pub model: String,
    pub provider: String,
    pub latency_ms: i64,
}

#[derive(Clone, Copy, Debug)]
pub struct LLMCapabilities {
    pub tool_calling: bool,
    pub parallel_tool_calls: bool,
    pub structured_output: bool,
    pub streaming: bool,
    pub vision: bool,
    pub max_context_tokens: i64,
}

#[derive(Clone, Debug)]
pub struct UsageRecord {
    pub provider: String,
    pub model: String,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cached_tokens: i64,
    pub latency_ms: i64,
    pub ok: bool,
    pub error_code: Option<String>,
    pub run_id: Option<String>,
}

#[async_trait]
pub trait LLMPort: Send + Sync {
    fn name(&self) -> &str;
    fn model(&self) -> &str;
    fn capabilities(&self) -> &LLMCapabilities;
    async fn generate(&self, req: &LLMRequest) -> Result<LLMResponse, LLMError>;
    fn count_tokens(&self, messages: &[Message]) -> i64;
}

pub trait UsageSink: Send + Sync {
    fn record(&self, record: UsageRecord) -> Result<(), Failure>;
}
