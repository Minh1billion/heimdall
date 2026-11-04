use async_trait::async_trait;

#[derive(Debug)]
pub struct NotifyError(pub String);

impl std::fmt::Display for NotifyError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

impl std::error::Error for NotifyError {}

#[async_trait]
pub trait NotifierPort: Send + Sync {
    fn name(&self) -> &str;
    async fn send(&self, subject: &str, text: &str) -> Result<(), NotifyError>;
}
