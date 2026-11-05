use crate::adapters::llm::common::{describe, http_client};
use crate::models::clip;
use crate::ports::notify::{NotifierPort, NotifyError};
use crate::services::destinations::split_recipients;
use crate::services::render::zalo_body;
use async_trait::async_trait;
use serde_json::{json, Value};
use std::time::Duration;

pub struct ZaloNotifier {
    bot_token: String,
    chat_id: String,
}

impl ZaloNotifier {
    pub fn new(bot_token: &str, chat_id: &str) -> Self {
        ZaloNotifier {
            bot_token: bot_token.to_string(),
            chat_id: chat_id.to_string(),
        }
    }

    async fn send_to(&self, chat_id: &str, subject: &str, text: &str) -> Result<(), NotifyError> {
        let (body, _) = zalo_body(subject, text);
        let url = format!("https://bot-api.zaloplatforms.com/bot{}/sendMessage", self.bot_token);
        let payload = json!({ "chat_id": chat_id, "text": body }).to_string();
        let response = http_client()
            .post(&url)
            .header("Content-Type", "application/json")
            .timeout(Duration::from_secs(20))
            .body(payload)
            .send()
            .await
            .map_err(|error| NotifyError(format!("Zalo is unreachable: {}", describe(&error))))?;
        let status = response.status().as_u16();
        let raw = response.text().await.unwrap_or_default();
        let data: Option<Value> = serde_json::from_str(&raw).ok();
        let rejected = data
            .as_ref()
            .and_then(|entry| entry.get("ok"))
            .map_or(false, |flag| flag == &Value::Bool(false));
        if status >= 400 || rejected {
            let described = data
                .as_ref()
                .and_then(|entry| entry.get("description"))
                .and_then(Value::as_str)
                .map(String::from);
            let detail = match described {
                Some(text) => text,
                None => clip(&raw, 200),
            };
            let shown = if detail.is_empty() { status.to_string() } else { detail };
            return Err(NotifyError(format!("Zalo rejected the message: {}", shown)));
        }
        Ok(())
    }
}

#[async_trait]
impl NotifierPort for ZaloNotifier {
    fn name(&self) -> &str {
        "zalo"
    }

    async fn send(&self, subject: &str, text: &str) -> Result<(), NotifyError> {
        let targets = split_recipients(&self.chat_id);
        if targets.len() <= 1 {
            let only = targets.first().map(String::as_str).unwrap_or(self.chat_id.as_str());
            return self.send_to(only, subject, text).await;
        }
        let mut failures: Vec<String> = Vec::new();
        for (index, chat) in targets.iter().enumerate() {
            if index > 0 {
                tokio::time::sleep(Duration::from_millis(400)).await;
            }
            if let Err(error) = self.send_to(chat, subject, text).await {
                failures.push(format!("{}: {}", chat, error.0));
            }
        }
        if failures.is_empty() {
            Ok(())
        } else {
            Err(NotifyError(failures.join("; ")))
        }
    }
}
