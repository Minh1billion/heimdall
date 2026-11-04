use crate::adapters::llm::common::estimate_tokens;
use crate::ports::errors::LLMError;
use crate::ports::llm::{
    message, LLMCapabilities, LLMPort, LLMRequest, LLMResponse, Message, Role, StopReason, Usage,
};
use async_trait::async_trait;

pub struct FakeLLM {
    name: String,
    model: String,
    capabilities: LLMCapabilities,
}

impl FakeLLM {
    pub fn new() -> Self {
        FakeLLM {
            name: "fake".to_string(),
            model: "fake-1".to_string(),
            capabilities: LLMCapabilities {
                tool_calling: true,
                parallel_tool_calls: true,
                structured_output: true,
                streaming: false,
                vision: false,
                max_context_tokens: 128000,
            },
        }
    }
}

#[async_trait]
impl LLMPort for FakeLLM {
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
        let last_user = req
            .messages
            .iter()
            .rev()
            .find(|item| item.role == Role::User && item.content.as_deref().map_or(false, |text| !text.is_empty()))
            .and_then(|item| item.content.clone())
            .unwrap_or_default();
        let reply = message(Role::Assistant, Some(&format!("Demo response: {}", last_user)));
        Ok(LLMResponse {
            usage: Usage {
                input_tokens: self.count_tokens(&req.messages),
                output_tokens: self.count_tokens(std::slice::from_ref(&reply)),
                cached_tokens: 0,
            },
            message: reply,
            stop_reason: StopReason::End,
            model: self.model.clone(),
            provider: self.name.clone(),
            latency_ms: 1,
        })
    }
}
