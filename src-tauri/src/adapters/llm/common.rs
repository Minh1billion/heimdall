use crate::ports::errors::{LLMError, LLMErrorKind};
use crate::ports::llm::Message;
use serde_json::{Number, Value};
use std::sync::OnceLock;
use std::time::Duration;

pub fn http_client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(reqwest::Client::new)
}

pub fn truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(flag) => *flag,
        Value::Number(number) => number.as_f64().map_or(true, |item| item != 0.0 && !item.is_nan()),
        Value::String(text) => !text.is_empty(),
        Value::Array(_) | Value::Object(_) => true,
    }
}

fn quote(text: &str) -> String {
    serde_json::to_string(text).unwrap_or_else(|_| "\"\"".to_string())
}

fn number_text(number: &Number) -> String {
    if let Some(item) = number.as_i64() {
        return item.to_string();
    }
    if let Some(item) = number.as_u64() {
        return item.to_string();
    }
    match number.as_f64() {
        Some(item) if item.is_finite() => {
            if item.fract() == 0.0 && item.abs() < 1e21 {
                format!("{:.0}", item)
            } else {
                format!("{}", item)
            }
        }
        _ => "null".to_string(),
    }
}

fn render(value: &Value) -> String {
    match value {
        Value::Array(items) => {
            let parts: Vec<String> = items.iter().map(render).collect();
            format!("[{}]", parts.join(", "))
        }
        Value::Object(map) => {
            let parts: Vec<String> = map
                .iter()
                .map(|(key, item)| format!("{}: {}", quote(key), render(item)))
                .collect();
            format!("{{{}}}", parts.join(", "))
        }
        Value::String(text) => quote(text),
        Value::Number(number) => number_text(number),
        Value::Bool(flag) => flag.to_string(),
        Value::Null => "null".to_string(),
    }
}

fn render_compact(value: &Value) -> String {
    match value {
        Value::Array(items) => {
            let parts: Vec<String> = items.iter().map(render_compact).collect();
            format!("[{}]", parts.join(","))
        }
        Value::Object(map) => {
            let parts: Vec<String> = map
                .iter()
                .map(|(key, item)| format!("{}:{}", quote(key), render_compact(item)))
                .collect();
            format!("{{{}}}", parts.join(","))
        }
        Value::String(text) => quote(text),
        Value::Number(number) => number_text(number),
        Value::Bool(flag) => flag.to_string(),
        Value::Null => "null".to_string(),
    }
}

pub fn js_stringify(value: &Value) -> String {
    render_compact(value)
}

pub fn py_json(value: &Value) -> String {
    let rendered = render(value);
    let mut escaped = String::with_capacity(rendered.len());
    for character in rendered.chars() {
        if (character as u32) >= 0x80 {
            let mut buffer = [0u16; 2];
            for unit in character.encode_utf16(&mut buffer).iter() {
                escaped.push_str(&format!("\\u{:04x}", unit));
            }
        } else {
            escaped.push(character);
        }
    }
    escaped
}

pub fn to_int(value: &Value) -> i64 {
    let number = match value {
        Value::Number(item) => item.as_f64().unwrap_or(0.0),
        Value::String(text) => text.trim().parse::<f64>().unwrap_or(0.0),
        Value::Bool(true) => 1.0,
        _ => 0.0,
    };
    if number.is_finite() {
        number.trunc() as i64
    } else {
        0
    }
}

pub fn field_int(value: &Value, key: &str) -> i64 {
    to_int(value.get(key).unwrap_or(&Value::Null))
}

fn length(text: &str) -> i64 {
    text.chars().count() as i64
}

pub fn estimate_tokens(messages: &[Message]) -> i64 {
    let mut total: i64 = 0;
    for item in messages {
        total += length(item.content.as_deref().unwrap_or(""));
        for call in &item.tool_calls {
            total += length(&call.name) + length(&py_json(&call.arguments));
        }
    }
    std::cmp::max(1, total / 4)
}

pub fn raise_for_status(provider: &str, status: u16, text: &str) -> Result<(), LLMError> {
    if status < 400 {
        return Ok(());
    }
    let body: String = text.chars().take(500).collect();
    let message = format!("{} returned HTTP {}: {}", provider, status, body);
    let lowered = body.to_lowercase();
    if status == 401 || status == 403 {
        return Err(LLMError::new(LLMErrorKind::AuthError, message, provider));
    }
    if status == 429 {
        return Err(LLMError::new(LLMErrorKind::RateLimited, message, provider));
    }
    if status == 408 {
        return Err(LLMError::new(LLMErrorKind::Timeout, message, provider));
    }
    if status >= 500 {
        return Err(LLMError::new(LLMErrorKind::ProviderDown, message, provider));
    }
    if status == 400 || status == 413 || status == 422 {
        let markers = [
            "context length",
            "context window",
            "too long",
            "too many tokens",
            "maximum context",
        ];
        if markers.iter().any(|marker| lowered.contains(marker)) {
            return Err(LLMError::new(LLMErrorKind::ContextTooLong, message, provider));
        }
    }
    Err(LLMError::new(LLMErrorKind::InvalidRequest, message, provider))
}

pub fn describe(error: &(dyn std::error::Error + 'static)) -> String {
    let mut current: &(dyn std::error::Error + 'static) = error;
    while let Some(source) = current.source() {
        current = source;
    }
    current.to_string()
}

fn transport_failure(provider: &str, error: &reqwest::Error) -> LLMError {
    if error.is_timeout() {
        return LLMError::new(
            LLMErrorKind::Timeout,
            format!("{} request timed out", provider),
            provider,
        );
    }
    LLMError::new(
        LLMErrorKind::ProviderDown,
        format!("{} is unreachable: {}", provider, describe(error)),
        provider,
    )
}

pub async fn post_json(
    provider: &str,
    url: &str,
    headers: &[(&str, String)],
    payload: &Value,
    timeout_s: f64,
) -> Result<Value, LLMError> {
    let mut request = http_client()
        .post(url)
        .timeout(Duration::from_secs_f64(timeout_s.max(0.001)))
        .body(payload.to_string());
    for (name, value) in headers {
        request = request.header(*name, value.as_str());
    }
    let response = match request.send().await {
        Ok(response) => response,
        Err(error) => return Err(transport_failure(provider, &error)),
    };
    let status = response.status().as_u16();
    let text = match response.text().await {
        Ok(text) => text,
        Err(error) => return Err(transport_failure(provider, &error)),
    };
    raise_for_status(provider, status, &text)?;
    let data: Value = match serde_json::from_str(&text) {
        Ok(data) => data,
        Err(_) => {
            return Err(LLMError::new(
                LLMErrorKind::ProviderDown,
                format!("{} returned invalid JSON", provider),
                provider,
            ))
        }
    };
    if !data.is_object() {
        return Err(LLMError::new(
            LLMErrorKind::ProviderDown,
            format!("{} returned an unexpected payload", provider),
            provider,
        ));
    }
    Ok(data)
}
