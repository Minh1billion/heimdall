use crate::adapters::llm::common::{describe, http_client};
use crate::models::clip;
use crate::ports::notify::{NotifierPort, NotifyError};
use crate::services::render::slack_body;
use async_trait::async_trait;
use serde_json::json;
use std::time::Duration;

pub struct SlackNotifier {
    webhook_url: String,
}

impl SlackNotifier {
    pub fn new(webhook_url: &str) -> Self {
        SlackNotifier {
            webhook_url: webhook_url.to_string(),
        }
    }
}

#[async_trait]
impl NotifierPort for SlackNotifier {
    fn name(&self) -> &str {
        "slack"
    }

    async fn send(&self, subject: &str, text: &str) -> Result<(), NotifyError> {
        let (body, _) = slack_body(subject, text);
        let payload = json!({ "text": body }).to_string();
        let response = http_client()
            .post(&self.webhook_url)
            .header("Content-Type", "application/json")
            .timeout(Duration::from_secs(20))
            .body(payload)
            .send()
            .await
            .map_err(|error| NotifyError(format!("Slack is unreachable: {}", describe(&error))))?;
        let status = response.status().as_u16();
        if status >= 400 {
            let raw = response.text().await.unwrap_or_default();
            let detail = clip(&raw, 200);
            let shown = if detail.is_empty() { status.to_string() } else { detail };
            return Err(NotifyError(format!("Slack rejected the message: {}", shown)));
        }
        Ok(())
    }
}
