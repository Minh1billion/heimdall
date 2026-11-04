use crate::adapters::llm::common::estimate_tokens;
use crate::ports::errors::{LLMError, LLMErrorKind};
use crate::ports::llm::{
    LLMCapabilities, LLMPort, LLMRequest, LLMResponse, Message, UsageRecord, UsageSink,
};
use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;
use std::time::{Duration, Instant};

pub struct LLMRouter {
    providers: Vec<Arc<dyn LLMPort>>,
    max_attempts: i64,
    backoff_base_s: f64,
    sink: Option<Arc<dyn UsageSink>>,
    empty: LLMCapabilities,
}

impl LLMRouter {
    pub fn new(
        providers: Vec<Arc<dyn LLMPort>>,
        max_attempts: i64,
        backoff_base_s: f64,
        sink: Option<Arc<dyn UsageSink>>,
    ) -> Self {
        LLMRouter {
            providers,
            max_attempts: std::cmp::max(1, max_attempts),
            backoff_base_s,
            sink,
            empty: LLMCapabilities {
                tool_calling: false,
                parallel_tool_calls: false,
                structured_output: false,
                streaming: false,
                vision: false,
                max_context_tokens: 0,
            },
        }
    }

    fn delay(&self, attempt: i64) -> f64 {
        self.backoff_base_s * 2f64.powi((attempt - 1) as i32) + rand::random::<f64>() * self.backoff_base_s
    }

    fn run_id(&self, req: &LLMRequest) -> Option<String> {
        match req.metadata.get("run_id") {
            Some(Value::String(text)) => Some(text.clone()),
            _ => None,
        }
    }

    fn record_success(&self, provider: &Arc<dyn LLMPort>, req: &LLMRequest, response: &LLMResponse) {
        self.write(UsageRecord {
            provider: provider.name().to_string(),
            model: response.model.clone(),
            input_tokens: response.usage.input_tokens,
            output_tokens: response.usage.output_tokens,
            cached_tokens: response.usage.cached_tokens,
            latency_ms: response.latency_ms,
            ok: true,
            error_code: None,
            run_id: self.run_id(req),
        });
    }

    fn record_failure(&self, provider: &Arc<dyn LLMPort>, req: &LLMRequest, error: &LLMError, started: Instant) {
        self.write(UsageRecord {
            provider: provider.name().to_string(),
            model: provider.model().to_string(),
            input_tokens: 0,
            output_tokens: 0,
            cached_tokens: 0,
            latency_ms: started.elapsed().as_millis() as i64,
            ok: false,
            error_code: Some(error.code().to_string()),
            run_id: self.run_id(req),
        });
    }

    fn write(&self, record: UsageRecord) {
        if let Some(sink) = &self.sink {
            if let Err(error) = sink.record(record) {
                eprintln!("Failed to record LLM usage {}", error);
            }
        }
    }
}

#[async_trait]
impl LLMPort for LLMRouter {
    fn name(&self) -> &str {
        "router"
    }

    fn model(&self) -> &str {
        match self.providers.first() {
            Some(provider) => provider.model(),
            None => "",
        }
    }

    fn capabilities(&self) -> &LLMCapabilities {
        match self.providers.first() {
            Some(provider) => provider.capabilities(),
            None => &self.empty,
        }
    }

    fn count_tokens(&self, messages: &[Message]) -> i64 {
        estimate_tokens(messages)
    }

    async fn generate(&self, req: &LLMRequest) -> Result<LLMResponse, LLMError> {
        if self.providers.is_empty() {
            return Err(LLMError::new(
                LLMErrorKind::NoProviderConfigured,
                "No language model provider is configured",
                "",
            ));
        }
        let mut failures: Vec<LLMError> = Vec::new();
        for provider in &self.providers {
            let mut attempt: i64 = 1;
            while attempt <= self.max_attempts {
                let started = Instant::now();
                match provider.generate(req).await {
                    Ok(response) => {
                        self.record_success(provider, req, &response);
                        return Ok(response);
                    }
                    Err(error) => match error.kind {
                        LLMErrorKind::InvalidRequest | LLMErrorKind::ContextTooLong => {
                            self.record_failure(provider, req, &error, started);
                            return Err(error);
                        }
                        LLMErrorKind::RateLimited | LLMErrorKind::Timeout | LLMErrorKind::ProviderDown => {
                            self.record_failure(provider, req, &error, started);
                            failures.push(error);
                            if attempt < self.max_attempts {
                                tokio::time::sleep(Duration::from_secs_f64(self.delay(attempt).max(0.0))).await;
                            }
                            attempt += 1;
                            continue;
                        }
                        _ => {
                            self.record_failure(provider, req, &error, started);
                            failures.push(error);
                            break;
                        }
                    },
                }
            }
        }
        let start = failures.len().saturating_sub(3);
        let summary: Vec<String> = failures[start..]
            .iter()
            .map(|failure| {
                let provider = if failure.provider.is_empty() {
                    "unknown"
                } else {
                    failure.provider.as_str()
                };
                format!("{}: {}", provider, failure.message)
            })
            .collect();
        Err(LLMError::all_failed(
            format!("All providers failed. {}", summary.join("; ")),
            failures,
        ))
    }
}
