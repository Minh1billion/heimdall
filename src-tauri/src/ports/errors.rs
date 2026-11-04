#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LLMErrorKind {
    RateLimited,
    Timeout,
    ProviderDown,
    AuthError,
    InvalidRequest,
    ContextTooLong,
    NoProviderConfigured,
    AllProvidersFailed,
}

impl LLMErrorKind {
    pub fn code(&self) -> &'static str {
        match self {
            LLMErrorKind::RateLimited => "RateLimited",
            LLMErrorKind::Timeout => "Timeout",
            LLMErrorKind::ProviderDown => "ProviderDown",
            LLMErrorKind::AuthError => "AuthError",
            LLMErrorKind::InvalidRequest => "InvalidRequest",
            LLMErrorKind::ContextTooLong => "ContextTooLong",
            LLMErrorKind::NoProviderConfigured => "NoProviderConfigured",
            LLMErrorKind::AllProvidersFailed => "AllProvidersFailed",
        }
    }
}

#[derive(Clone, Debug)]
pub struct LLMError {
    pub kind: LLMErrorKind,
    pub message: String,
    pub provider: String,
    pub failures: Vec<LLMError>,
}

impl LLMError {
    pub fn new(kind: LLMErrorKind, message: impl Into<String>, provider: &str) -> Self {
        LLMError {
            kind,
            message: message.into(),
            provider: provider.to_string(),
            failures: Vec::new(),
        }
    }

    pub fn all_failed(message: impl Into<String>, failures: Vec<LLMError>) -> Self {
        LLMError {
            kind: LLMErrorKind::AllProvidersFailed,
            message: message.into(),
            provider: String::new(),
            failures,
        }
    }

    pub fn code(&self) -> &'static str {
        self.kind.code()
    }
}

impl std::fmt::Display for LLMError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}", self.message)
    }
}

impl std::error::Error for LLMError {}
