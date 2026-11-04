use crate::adapters::llm::common::js_stringify;
use crate::router::{js_type, HttpError};
use crate::services::app_settings::js_number;
use serde_json::{json, Map, Value};

#[derive(Clone, Debug)]
pub enum Kind {
    Str { min: Option<usize>, max: Option<usize> },
    Time,
    Enum(&'static [&'static str]),
    Int { min: Option<i64>, max: Option<i64> },
    Num { min: Option<f64>, max: Option<f64> },
    Bool,
    EnumList(&'static [&'static str]),
}

#[derive(Clone, Debug)]
pub struct Field {
    pub key: &'static str,
    pub kind: Kind,
    pub required: bool,
    pub nullable: bool,
    pub default: Option<Value>,
    pub description: Option<&'static str>,
}

impl Field {
    pub fn new(key: &'static str, kind: Kind) -> Field {
        Field {
            key,
            kind,
            required: true,
            nullable: false,
            default: None,
            description: None,
        }
    }

    pub fn text(key: &'static str) -> Field {
        Field::new(key, Kind::Str { min: None, max: None })
    }

    pub fn nullish(mut self) -> Field {
        self.required = false;
        self.nullable = true;
        self
    }

    pub fn with_default(mut self, value: Value) -> Field {
        self.required = false;
        self.default = Some(value);
        self
    }

    pub fn describe(mut self, text: &'static str) -> Field {
        self.description = Some(text);
        self
    }
}

#[derive(Clone, Debug)]
pub struct Issue {
    pub code: &'static str,
    pub path: Vec<Value>,
    pub message: String,
}

fn issue(code: &'static str, path: Vec<Value>, message: String) -> Issue {
    Issue { code, path, message }
}

fn quoted_options(options: &[&str]) -> String {
    options
        .iter()
        .map(|item| format!("'{}'", item))
        .collect::<Vec<String>>()
        .join(" | ")
}

pub fn enum_message(options: &[&str], received: &Value) -> String {
    match received {
        Value::String(text) => format!(
            "Invalid enum value. Expected {}, received '{}'",
            quoted_options(options),
            text
        ),
        other => format!("Expected {}, received {}", quoted_options(options), js_type(other)),
    }
}

fn is_time(text: &str) -> bool {
    let bytes = text.as_bytes();
    if bytes.len() != 5 || bytes[2] != b':' {
        return false;
    }
    let digit = |index: usize| bytes[index].is_ascii_digit();
    if !(digit(0) && digit(1) && digit(3) && digit(4)) {
        return false;
    }
    let hour = (bytes[0] - b'0') * 10 + (bytes[1] - b'0');
    let minute = bytes[3] - b'0';
    hour <= 23 && minute <= 5
}

fn integral(value: &Value) -> Option<i64> {
    if let Some(item) = value.as_i64() {
        return Some(item);
    }
    match value.as_f64() {
        Some(item) if item.fract() == 0.0 && item.abs() < 9e15 => Some(item as i64),
        _ => None,
    }
}

fn check(field: &Field, value: &Value, issues: &mut Vec<Issue>) -> Option<Value> {
    let path = vec![json!(field.key)];
    match &field.kind {
        Kind::Str { min, max } => match value {
            Value::String(text) => {
                let length = text.chars().count();
                if let Some(limit) = min {
                    if length < *limit {
                        issues.push(issue(
                            "too_small",
                            path,
                            format!("String must contain at least {} character(s)", limit),
                        ));
                        return None;
                    }
                }
                if let Some(limit) = max {
                    if length > *limit {
                        issues.push(issue(
                            "too_big",
                            path,
                            format!("String must contain at most {} character(s)", limit),
                        ));
                        return None;
                    }
                }
                Some(value.clone())
            }
            other => {
                issues.push(issue(
                    "invalid_type",
                    path,
                    format!("Expected string, received {}", js_type(other)),
                ));
                None
            }
        },
        Kind::Time => match value {
            Value::String(text) => {
                if is_time(text) {
                    Some(value.clone())
                } else {
                    issues.push(issue("invalid_string", path, "Invalid".to_string()));
                    None
                }
            }
            other => {
                issues.push(issue(
                    "invalid_type",
                    path,
                    format!("Expected string, received {}", js_type(other)),
                ));
                None
            }
        },
        Kind::Enum(options) => match value {
            Value::String(text) if options.contains(&text.as_str()) => Some(value.clone()),
            Value::String(_) => {
                issues.push(issue("invalid_enum_value", path, enum_message(options, value)));
                None
            }
            other => {
                issues.push(issue("invalid_type", path, enum_message(options, other)));
                None
            }
        },
        Kind::Int { min, max } => match value {
            Value::Number(_) => match integral(value) {
                Some(number) => {
                    if let Some(limit) = min {
                        if number < *limit {
                            issues.push(issue(
                                "too_small",
                                path,
                                format!("Number must be greater than or equal to {}", limit),
                            ));
                            return None;
                        }
                    }
                    if let Some(limit) = max {
                        if number > *limit {
                            issues.push(issue(
                                "too_big",
                                path,
                                format!("Number must be less than or equal to {}", limit),
                            ));
                            return None;
                        }
                    }
                    Some(json!(number))
                }
                None => {
                    issues.push(issue(
                        "invalid_type",
                        path,
                        "Expected integer, received float".to_string(),
                    ));
                    None
                }
            },
            other => {
                issues.push(issue(
                    "invalid_type",
                    path,
                    format!("Expected number, received {}", js_type(other)),
                ));
                None
            }
        },
        Kind::Num { min, max } => match value {
            Value::Number(entry) => {
                let number = entry.as_f64().unwrap_or(0.0);
                if let Some(limit) = min {
                    if number < *limit {
                        issues.push(issue(
                            "too_small",
                            path,
                            format!("Number must be greater than or equal to {}", js_number(*limit)),
                        ));
                        return None;
                    }
                }
                if let Some(limit) = max {
                    if number > *limit {
                        issues.push(issue(
                            "too_big",
                            path,
                            format!("Number must be less than or equal to {}", js_number(*limit)),
                        ));
                        return None;
                    }
                }
                Some(value.clone())
            }
            other => {
                issues.push(issue(
                    "invalid_type",
                    path,
                    format!("Expected number, received {}", js_type(other)),
                ));
                None
            }
        },
        Kind::Bool => match value {
            Value::Bool(_) => Some(value.clone()),
            other => {
                issues.push(issue(
                    "invalid_type",
                    path,
                    format!("Expected boolean, received {}", js_type(other)),
                ));
                None
            }
        },
        Kind::EnumList(options) => match value {
            Value::Array(items) => {
                let mut clean: Vec<Value> = Vec::new();
                let mut ok = true;
                for (index, item) in items.iter().enumerate() {
                    let item_path = vec![json!(field.key), json!(index)];
                    match item {
                        Value::String(text) if options.contains(&text.as_str()) => clean.push(item.clone()),
                        Value::String(_) => {
                            issues.push(issue("invalid_enum_value", item_path, enum_message(options, item)));
                            ok = false;
                        }
                        other => {
                            issues.push(issue("invalid_type", item_path, enum_message(options, other)));
                            ok = false;
                        }
                    }
                }
                if ok {
                    Some(Value::Array(clean))
                } else {
                    None
                }
            }
            other => {
                issues.push(issue(
                    "invalid_type",
                    path,
                    format!("Expected array, received {}", js_type(other)),
                ));
                None
            }
        },
    }
}

pub fn validate(fields: &[Field], input: &Value) -> Result<Map<String, Value>, Vec<Issue>> {
    let object = match input {
        Value::Object(object) => object,
        Value::Null => return Err(vec![issue("invalid_type", Vec::new(), "Required".to_string())]),
        other => {
            return Err(vec![issue(
                "invalid_type",
                Vec::new(),
                format!("Expected object, received {}", js_type(other)),
            )])
        }
    };
    let mut out: Map<String, Value> = Map::new();
    let mut issues: Vec<Issue> = Vec::new();
    for field in fields {
        match object.get(field.key) {
            None => {
                if let Some(default) = &field.default {
                    out.insert(field.key.to_string(), default.clone());
                } else if field.required {
                    issues.push(issue("invalid_type", vec![json!(field.key)], "Required".to_string()));
                }
            }
            Some(Value::Null) => {
                if field.nullable {
                    out.insert(field.key.to_string(), Value::Null);
                } else {
                    let message = match &field.kind {
                        Kind::Str { .. } | Kind::Time => "Expected string, received null".to_string(),
                        Kind::Enum(options) => enum_message(options, &Value::Null),
                        Kind::Int { .. } | Kind::Num { .. } => "Expected number, received null".to_string(),
                        Kind::Bool => "Expected boolean, received null".to_string(),
                        Kind::EnumList(_) => "Expected array, received null".to_string(),
                    };
                    issues.push(issue("invalid_type", vec![json!(field.key)], message));
                }
            }
            Some(value) => {
                if let Some(clean) = check(field, value, &mut issues) {
                    out.insert(field.key.to_string(), clean);
                }
            }
        }
    }
    if issues.is_empty() {
        Ok(out)
    } else {
        Err(issues)
    }
}

pub fn format_issues(issues: &[Issue]) -> String {
    issues
        .iter()
        .map(|item| {
            if item.path.is_empty() {
                item.message.clone()
            } else {
                let path: Vec<String> = item
                    .path
                    .iter()
                    .map(|part| match part {
                        Value::String(text) => text.clone(),
                        other => other.to_string(),
                    })
                    .collect();
                format!("{}: {}", path.join("."), item.message)
            }
        })
        .collect::<Vec<String>>()
        .join("; ")
}

pub fn issues_text(issues: &[Issue]) -> String {
    let list: Vec<Value> = issues
        .iter()
        .map(|item| json!({ "type": item.code, "loc": item.path, "msg": item.message }))
        .collect();
    js_stringify(&Value::Array(list))
}

fn property(field: &Field) -> Value {
    let mut entry = Map::new();
    match &field.kind {
        Kind::Str { min, max } => {
            if field.nullable {
                entry.insert("type".to_string(), json!(["string", "null"]));
            } else {
                entry.insert("type".to_string(), json!("string"));
            }
            if let Some(limit) = min {
                entry.insert("minLength".to_string(), json!(limit));
            }
            if let Some(limit) = max {
                entry.insert("maxLength".to_string(), json!(limit));
            }
        }
        Kind::Time => {
            entry.insert("type".to_string(), json!("string"));
            entry.insert("pattern".to_string(), json!("^([01]\\d|2[0-3]):[0-5]\\d$"));
        }
        Kind::Enum(options) => {
            entry.insert("type".to_string(), json!("string"));
            entry.insert("enum".to_string(), json!(options));
        }
        Kind::Int { min, max } => {
            entry.insert("type".to_string(), json!("integer"));
            if let Some(limit) = min {
                entry.insert("minimum".to_string(), json!(limit));
            }
            if let Some(limit) = max {
                entry.insert("maximum".to_string(), json!(limit));
            }
        }
        Kind::Num { min, max } => {
            entry.insert("type".to_string(), json!("number"));
            if let Some(limit) = min {
                entry.insert("minimum".to_string(), json!(limit));
            }
            if let Some(limit) = max {
                entry.insert("maximum".to_string(), json!(limit));
            }
        }
        Kind::Bool => {
            entry.insert("type".to_string(), json!("boolean"));
        }
        Kind::EnumList(options) => {
            entry.insert("type".to_string(), json!("array"));
            entry.insert("items".to_string(), json!({ "type": "string", "enum": options }));
        }
    }
    if let Some(default) = &field.default {
        entry.insert("default".to_string(), default.clone());
    }
    if let Some(text) = field.description {
        entry.insert("description".to_string(), json!(text));
    }
    Value::Object(entry)
}

pub fn schema(fields: &[Field]) -> Value {
    let mut properties = Map::new();
    let mut required: Vec<Value> = Vec::new();
    for field in fields {
        properties.insert(field.key.to_string(), property(field));
        if field.required {
            required.push(json!(field.key));
        }
    }
    let mut root = Map::new();
    root.insert("type".to_string(), json!("object"));
    root.insert("properties".to_string(), Value::Object(properties));
    if !required.is_empty() {
        root.insert("required".to_string(), Value::Array(required));
    }
    root.insert("additionalProperties".to_string(), json!(false));
    Value::Object(root)
}

pub fn parse(fields: &[Field], body: &Value) -> Result<Map<String, Value>, HttpError> {
    validate(fields, body).map_err(|issues| HttpError::new(422, format_issues(&issues)))
}
