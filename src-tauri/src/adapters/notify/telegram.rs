use crate::adapters::llm::common::{describe, http_client};
use crate::ports::notify::{NotifierPort, NotifyError};
use crate::services::render::telegram_body;
use async_trait::async_trait;
use serde_json::{json, Value};
use std::time::Duration;

pub struct TelegramNotifier {
    bot_token: String,
    chat_id: String,
}

impl TelegramNotifier {
    pub fn new(bot_token: &str, chat_id: &str) -> Self {
        TelegramNotifier {
            bot_token: bot_token.to_string(),
            chat_id: chat_id.to_string(),
        }
    }
}

#[async_trait]
impl NotifierPort for TelegramNotifier {
    fn name(&self) -> &str {
        "telegram"
    }

    async fn send(&self, subject: &str, text: &str) -> Result<(), NotifyError> {
        let (body, _) = telegram_body(subject, text);
        let url = format!("https://api.telegram.org/bot{}/sendMessage", self.bot_token);
        let payload = json!({ "chat_id": self.chat_id, "text": body }).to_string();
        let response = http_client()
            .post(&url)
            .header("Content-Type", "application/json")
            .timeout(Duration::from_secs(20))
            .body(payload)
            .send()
            .await
            .map_err(|error| NotifyError(format!("Telegram is unreachable: {}", describe(&error))))?;
        let status = response.status().as_u16();
        if status >= 400 {
            let mut detail = String::new();
            if let Ok(data) = response.json::<Value>().await {
                if let Some(text) = data.get("description").and_then(Value::as_str) {
                    detail = text.to_string();
                }
            }
            let shown = if detail.is_empty() { status.to_string() } else { detail };
            return Err(NotifyError(format!("Telegram rejected the message: {}", shown)));
        }
        Ok(())
    }
}
