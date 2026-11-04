use crate::container::Container;
use crate::router::Failure;
use crate::validation::Field;
use async_trait::async_trait;
use serde_json::{Map, Value};
use std::collections::HashSet;
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Risk {
    Read,
    Notify,
    Write,
}

#[derive(Clone, Debug)]
pub struct ToolFailure {
    pub error_code: String,
    pub message: String,
    pub hint: String,
}

impl ToolFailure {
    pub fn new(error_code: &str, message: impl Into<String>) -> Self {
        ToolFailure {
            error_code: error_code.to_string(),
            message: message.into(),
            hint: String::new(),
        }
    }

    pub fn with_hint(error_code: &str, message: impl Into<String>, hint: impl Into<String>) -> Self {
        ToolFailure {
            error_code: error_code.to_string(),
            message: message.into(),
            hint: hint.into(),
        }
    }
}

#[derive(Debug)]
pub enum ToolError {
    Failure(ToolFailure),
    Other(String),
}

impl From<ToolFailure> for ToolError {
    fn from(failure: ToolFailure) -> Self {
        ToolError::Failure(failure)
    }
}

impl From<rusqlite::Error> for ToolError {
    fn from(error: rusqlite::Error) -> Self {
        ToolError::Other(error.to_string())
    }
}

impl From<Failure> for ToolError {
    fn from(error: Failure) -> Self {
        ToolError::Other(error.to_string())
    }
}

pub struct ToolContext {
    pub run_id: String,
    pub container: Arc<Container>,
    pub allowed_account_ids: Option<HashSet<String>>,
}

#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn risk(&self) -> Risk {
        Risk::Read
    }
    fn fields(&self) -> Vec<Field>;
    async fn run(&self, ctx: &ToolContext, args: &Map<String, Value>) -> Result<Value, ToolError>;
    async fn apply(&self, _container: &Container, _payload: &Value) -> Result<Value, ToolError> {
        Err(ToolError::Failure(ToolFailure::new(
            "unavailable",
            "Meta is not configured or the tool no longer exists",
        )))
    }
}

impl std::fmt::Display for ToolFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}", self.message)
    }
}

impl std::error::Error for ToolFailure {}
