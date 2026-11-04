use crate::ports::tool::{Tool, ToolContext, ToolError, ToolFailure};
use crate::tools::common::text_arg;
use crate::validation::{Field, Kind};
use async_trait::async_trait;
use serde_json::{json, Map, Value};

enum CalcError {
    Range,
    Message(String),
}

fn message(text: &str) -> CalcError {
    CalcError::Message(text.to_string())
}

fn syntax() -> CalcError {
    message("invalid syntax")
}

fn scan_digits(chars: &[char], mut index: usize) -> usize {
    while index < chars.len() && (chars[index].is_ascii_digit() || chars[index] == '_') {
        index += 1;
    }
    index
}

fn scan_exponent(chars: &[char], index: usize) -> usize {
    if index < chars.len() && (chars[index] == 'e' || chars[index] == 'E') {
        let mut cursor = index + 1;
        if cursor < chars.len() && (chars[cursor] == '+' || chars[cursor] == '-') {
            cursor += 1;
        }
        let digits_start = cursor;
        while cursor < chars.len() && chars[cursor].is_ascii_digit() {
            cursor += 1;
        }
        if cursor > digits_start {
            return cursor;
        }
    }
    index
}

fn tokenize(source: &str) -> Result<Vec<String>, CalcError> {
    let chars: Vec<char> = source.chars().collect();
    let mut tokens: Vec<String> = Vec::new();
    let mut index = 0;
    while index < chars.len() {
        if chars[index].is_whitespace() {
            index += 1;
            continue;
        }
        let character = chars[index];
        let next_is_digit = index + 1 < chars.len() && chars[index + 1].is_ascii_digit();
        if character.is_ascii_digit() {
            let mut end = scan_digits(&chars, index + 1);
            if end < chars.len() && chars[end] == '.' {
                end += 1;
            }
            end = scan_digits(&chars, end);
            end = scan_exponent(&chars, end);
            tokens.push(chars[index..end].iter().collect());
            index = end;
            continue;
        }
        if character == '.' && next_is_digit {
            let mut end = scan_digits(&chars, index + 2);
            end = scan_exponent(&chars, end);
            tokens.push(chars[index..end].iter().collect());
            index = end;
            continue;
        }
        if (character == '*' || character == '/') && index + 1 < chars.len() && chars[index + 1] == character {
            tokens.push(format!("{}{}", character, character));
            index += 2;
            continue;
        }
        if "-+*/%()".contains(character) {
            tokens.push(character.to_string());
            index += 1;
            continue;
        }
        if character.is_ascii_alphabetic() || character == '_' {
            return Err(message("unsupported expression"));
        }
        return Err(syntax());
    }
    Ok(tokens)
}

fn floor_mod(left: f64, right: f64) -> f64 {
    let result = left % right;
    if result != 0.0 && (result < 0.0) != (right < 0.0) {
        result + right
    } else {
        result
    }
}

struct Parser {
    tokens: Vec<String>,
    position: usize,
}

impl Parser {
    fn peek(&self) -> Option<&str> {
        self.tokens.get(self.position).map(String::as_str)
    }

    fn atom(&mut self) -> Result<f64, CalcError> {
        let token = match self.tokens.get(self.position) {
            Some(found) => found.clone(),
            None => return Err(syntax()),
        };
        self.position += 1;
        if token == "(" {
            let value = self.expression()?;
            if self.peek() != Some(")") {
                return Err(syntax());
            }
            self.position += 1;
            return Ok(value);
        }
        let first = token.chars().next().unwrap_or(' ');
        if first.is_ascii_digit() || first == '.' {
            let cleaned: String = token.chars().filter(|character| *character != '_').collect();
            return Ok(cleaned.parse::<f64>().unwrap_or(f64::NAN));
        }
        Err(syntax())
    }

    fn power(&mut self) -> Result<f64, CalcError> {
        let base = self.atom()?;
        if self.peek() != Some("**") {
            return Ok(base);
        }
        self.position += 1;
        let exponent = self.factor()?;
        if exponent.abs() > 16.0 {
            return Err(message("exponent too large"));
        }
        if base == 0.0 && exponent < 0.0 {
            return Err(CalcError::Range);
        }
        Ok(base.powf(exponent))
    }

    fn factor(&mut self) -> Result<f64, CalcError> {
        let token = self.peek().map(String::from);
        if token.as_deref() == Some("+") || token.as_deref() == Some("-") {
            self.position += 1;
            let value = self.factor()?;
            return Ok(if token.as_deref() == Some("-") { -value } else { value });
        }
        self.power()
    }

    fn term(&mut self) -> Result<f64, CalcError> {
        let mut value = self.factor()?;
        loop {
            let operator = match self.peek() {
                Some(found) if found == "*" || found == "/" || found == "//" || found == "%" => found.to_string(),
                _ => break,
            };
            self.position += 1;
            let right = self.factor()?;
            if operator == "*" {
                value *= right;
                continue;
            }
            if right == 0.0 {
                return Err(CalcError::Range);
            }
            if operator == "/" {
                value /= right;
            } else if operator == "//" {
                value = (value / right).floor();
            } else {
                value = floor_mod(value, right);
            }
        }
        Ok(value)
    }

    fn expression(&mut self) -> Result<f64, CalcError> {
        let mut value = self.term()?;
        loop {
            let operator = match self.peek() {
                Some(found) if found == "+" || found == "-" => found.to_string(),
                _ => break,
            };
            self.position += 1;
            let right = self.term()?;
            value = if operator == "+" { value + right } else { value - right };
        }
        Ok(value)
    }
}

fn evaluate(source: &str) -> Result<f64, CalcError> {
    let tokens = tokenize(source)?;
    let mut parser = Parser { tokens, position: 0 };
    let result = parser.expression()?;
    if parser.position < parser.tokens.len() {
        return Err(syntax());
    }
    Ok(result)
}

pub struct Calc;

#[async_trait]
impl Tool for Calc {
    fn name(&self) -> &'static str {
        "calc"
    }

    fn description(&self) -> &'static str {
        "Evaluate an arithmetic expression exactly. Supports + - * / // % ** and parentheses. Always use this instead of doing arithmetic yourself."
    }

    fn fields(&self) -> Vec<Field> {
        vec![Field::new("expression", Kind::Str { min: None, max: Some(200) })
            .describe("For example (1250.5 - 980) / 980 * 100")]
    }

    async fn run(&self, _ctx: &ToolContext, args: &Map<String, Value>) -> Result<Value, ToolError> {
        let value = match evaluate(text_arg(args, "expression").unwrap_or("")) {
            Ok(found) => found,
            Err(CalcError::Range) => {
                return Err(ToolError::Failure(ToolFailure::new("division_by_zero", "Division by zero")))
            }
            Err(CalcError::Message(text)) => {
                let shown = if text.is_empty() { "Invalid expression".to_string() } else { text };
                return Err(ToolError::Failure(ToolFailure::new("invalid_expression", shown)));
            }
        };
        if !value.is_finite() {
            return Err(ToolError::Failure(ToolFailure::new(
                "invalid_expression",
                "Numerical result out of range",
            )));
        }
        Ok(json!({ "ok": true, "result": value }))
    }
}
