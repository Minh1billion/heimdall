use crate::ports::notify::{NotifierPort, NotifyError};
use async_trait::async_trait;
use lettre::message::Mailbox;
use lettre::transport::smtp::authentication::Credentials;
use lettre::{Message, SmtpTransport, Transport};
use std::time::Duration;

pub struct EmailNotifier {
    host: String,
    port: i64,
    user: String,
    password: String,
    sender: String,
    recipients: Vec<String>,
}

impl EmailNotifier {
    pub fn new(host: &str, port: i64, user: &str, password: &str, sender: &str, recipients: Vec<String>) -> Self {
        EmailNotifier {
            host: host.to_string(),
            port,
            user: user.to_string(),
            password: password.to_string(),
            sender: sender.to_string(),
            recipients,
        }
    }
}

fn deliver(
    host: String,
    port: i64,
    user: String,
    password: String,
    sender: String,
    recipients: Vec<String>,
    subject: String,
    text: String,
) -> Result<(), String> {
    let port = u16::try_from(port).map_err(|_| "invalid SMTP port".to_string())?;
    let from: Mailbox = sender.parse().map_err(|error: lettre::address::AddressError| error.to_string())?;
    let mut builder = Message::builder().from(from).subject(subject);
    for recipient in &recipients {
        let to: Mailbox = recipient
            .parse()
            .map_err(|error: lettre::address::AddressError| error.to_string())?;
        builder = builder.to(to);
    }
    let email = builder.body(text).map_err(|error| error.to_string())?;
    let relay = if port == 465 {
        SmtpTransport::relay(&host)
    } else {
        SmtpTransport::starttls_relay(&host)
    }
    .map_err(|error| error.to_string())?;
    let mut relay = relay.port(port).timeout(Some(Duration::from_secs(20)));
    if !user.is_empty() {
        relay = relay.credentials(Credentials::new(user, password));
    }
    relay
        .build()
        .send(&email)
        .map(|_| ())
        .map_err(|error| error.to_string())
}

#[async_trait]
impl NotifierPort for EmailNotifier {
    fn name(&self) -> &str {
        "email"
    }

    async fn send(&self, subject: &str, text: &str) -> Result<(), NotifyError> {
        let host = self.host.clone();
        let port = self.port;
        let user = self.user.clone();
        let password = self.password.clone();
        let sender = self.sender.clone();
        let recipients = self.recipients.clone();
        let subject = subject.to_string();
        let text = text.to_string();
        let outcome = tokio::task::spawn_blocking(move || {
            deliver(host, port, user, password, sender, recipients, subject, text)
        })
        .await;
        match outcome {
            Ok(Ok(())) => Ok(()),
            Ok(Err(message)) => Err(NotifyError(format!("Email failed: {}", message))),
            Err(error) => Err(NotifyError(format!("Email failed: {}", error))),
        }
    }
}
