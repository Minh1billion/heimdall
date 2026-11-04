use crate::container::Container;
use serde::de::DeserializeOwned;
use serde_json::{json, Map, Value};
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

pub type Failure = Box<dyn std::error::Error + Send + Sync>;

#[derive(Debug)]
pub struct HttpError {
    pub status: u16,
    pub detail: String,
}

impl HttpError {
    pub fn new(status: u16, detail: impl Into<String>) -> Self {
        HttpError {
            status,
            detail: detail.into(),
        }
    }
}

impl From<Failure> for HttpError {
    fn from(error: Failure) -> Self {
        eprintln!("{}", error);
        HttpError::new(500, "Internal Server Error")
    }
}

impl From<rusqlite::Error> for HttpError {
    fn from(error: rusqlite::Error) -> Self {
        eprintln!("{}", error);
        HttpError::new(500, "Internal Server Error")
    }
}

pub struct RouteContext {
    pub params: HashMap<String, String>,
    pub query: HashMap<String, String>,
    pub body: Value,
    pub container: Arc<Container>,
}

impl RouteContext {
    pub fn param(&self, name: &str) -> &str {
        self.params.get(name).map(String::as_str).unwrap_or("")
    }
}

pub type HandlerFuture = Pin<Box<dyn Future<Output = Result<Value, HttpError>> + Send>>;
type Handler = Box<dyn Fn(RouteContext) -> HandlerFuture + Send + Sync>;

struct Route {
    method: String,
    segments: Vec<String>,
    handler: Handler,
    status: u16,
}

pub fn parse<T: DeserializeOwned>(value: Value) -> Result<T, HttpError> {
    serde_json::from_value(value).map_err(|error| HttpError::new(422, error.to_string()))
}

pub fn js_type(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

pub fn object_of(body: &Value) -> Result<&Map<String, Value>, HttpError> {
    match body {
        Value::Object(object) => Ok(object),
        Value::Null => Err(HttpError::new(422, "Required")),
        other => Err(HttpError::new(
            422,
            format!("Expected object, received {}", js_type(other)),
        )),
    }
}

pub fn read_string(
    object: &Map<String, Value>,
    key: &str,
    required: bool,
    nullable: bool,
    issues: &mut Vec<String>,
) -> Option<String> {
    match object.get(key) {
        None => {
            if required {
                issues.push(format!("{}: Required", key));
            }
            None
        }
        Some(Value::String(text)) => Some(text.clone()),
        Some(Value::Null) if nullable => None,
        Some(other) => {
            issues.push(format!("{}: Expected string, received {}", key, js_type(other)));
            None
        }
    }
}

pub fn read_bool(object: &Map<String, Value>, key: &str, issues: &mut Vec<String>) -> Option<bool> {
    match object.get(key) {
        None => None,
        Some(Value::Bool(flag)) => Some(*flag),
        Some(other) => {
            issues.push(format!("{}: Expected boolean, received {}", key, js_type(other)));
            None
        }
    }
}

pub fn issues_error(issues: Vec<String>) -> HttpError {
    HttpError::new(422, issues.join("; "))
}

fn split(path: &str) -> Option<Vec<String>> {
    let mut parts = Vec::new();
    for part in path.split('/').filter(|part| !part.is_empty()) {
        let decoded = percent_encoding::percent_decode_str(part).decode_utf8().ok()?;
        parts.push(decoded.into_owned());
    }
    Some(parts)
}

fn match_segments(pattern: &[String], actual: &[String]) -> Option<HashMap<String, String>> {
    if pattern.len() != actual.len() {
        return None;
    }
    let mut params = HashMap::new();
    for (expected, found) in pattern.iter().zip(actual.iter()) {
        if let Some(name) = expected.strip_prefix(':') {
            params.insert(name.to_string(), found.clone());
        } else if expected != found {
            return None;
        }
    }
    Some(params)
}

pub struct Router {
    container: Arc<Container>,
    routes: Vec<Route>,
}

impl Router {
    pub fn new(container: Arc<Container>) -> Self {
        Router {
            container,
            routes: Vec::new(),
        }
    }

    pub fn add<F, Fut>(&mut self, method: &str, pattern: &str, status: u16, handler: F)
    where
        F: Fn(RouteContext) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<Value, HttpError>> + Send + 'static,
    {
        self.routes.push(Route {
            method: method.to_string(),
            segments: pattern
                .split('/')
                .filter(|part| !part.is_empty())
                .map(String::from)
                .collect(),
            handler: Box::new(move |context| Box::pin(handler(context))),
            status,
        });
    }

    pub fn get<F, Fut>(&mut self, pattern: &str, handler: F)
    where
        F: Fn(RouteContext) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<Value, HttpError>> + Send + 'static,
    {
        self.add("GET", pattern, 200, handler);
    }

    pub fn post<F, Fut>(&mut self, pattern: &str, handler: F)
    where
        F: Fn(RouteContext) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<Value, HttpError>> + Send + 'static,
    {
        self.add("POST", pattern, 200, handler);
    }

    pub fn put<F, Fut>(&mut self, pattern: &str, handler: F)
    where
        F: Fn(RouteContext) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<Value, HttpError>> + Send + 'static,
    {
        self.add("PUT", pattern, 200, handler);
    }

    pub fn patch<F, Fut>(&mut self, pattern: &str, handler: F)
    where
        F: Fn(RouteContext) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<Value, HttpError>> + Send + 'static,
    {
        self.add("PATCH", pattern, 200, handler);
    }

    pub fn delete<F, Fut>(&mut self, pattern: &str, handler: F)
    where
        F: Fn(RouteContext) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<Value, HttpError>> + Send + 'static,
    {
        self.add("DELETE", pattern, 200, handler);
    }

    pub async fn dispatch(&self, method: &str, raw_path: &str, body: Value) -> Value {
        let (status, payload) = match self.run(&method.to_uppercase(), raw_path, body).await {
            Ok(result) => result,
            Err(error) => (error.status, json!({ "detail": error.detail })),
        };
        json!({ "status": status, "body": payload })
    }

    async fn run(&self, method: &str, raw_path: &str, body: Value) -> Result<(u16, Value), HttpError> {
        let without_fragment = raw_path.split('#').next().unwrap_or("");
        let (path_part, query_part) = match without_fragment.split_once('?') {
            Some((path, query)) => (path, query),
            None => (without_fragment, ""),
        };
        let segments = split(path_part).ok_or_else(|| HttpError::new(404, "Not Found"))?;
        let mut query: HashMap<String, String> = HashMap::new();
        for (name, value) in url::form_urlencoded::parse(query_part.as_bytes()) {
            query.entry(name.into_owned()).or_insert_with(|| value.into_owned());
        }
        let mut path_matched = false;
        for route in &self.routes {
            let params = match match_segments(&route.segments, &segments) {
                Some(params) => params,
                None => continue,
            };
            path_matched = true;
            if route.method != method {
                continue;
            }
            let context = RouteContext {
                params,
                query,
                body,
                container: self.container.clone(),
            };
            let result = (route.handler)(context).await?;
            let payload = if route.status == 204 { Value::Null } else { result };
            return Ok((route.status, payload));
        }
        if path_matched {
            Err(HttpError::new(405, "Method Not Allowed"))
        } else {
            Err(HttpError::new(404, "Not Found"))
        }
    }
}
