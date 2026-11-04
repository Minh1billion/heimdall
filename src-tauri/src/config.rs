#[derive(Clone, Debug)]
pub enum SettingValue {
    Text(String),
    Int(i64),
    Float(f64),
}

#[derive(Clone, Debug)]
pub struct Settings {
    pub llm_provider_order: String,
    pub llm_max_attempts: i64,
    pub llm_backoff_base_s: f64,
    pub llm_timeout_s: f64,
    pub openai_api_key: String,
    pub openai_base_url: String,
    pub openai_model: String,
    pub anthropic_api_key: String,
    pub anthropic_base_url: String,
    pub anthropic_model: String,
    pub openai_compat_base_url: String,
    pub openai_compat_api_key: String,
    pub openai_compat_model: String,
    pub timezone: String,
    pub telegram_bot_token: String,
    pub telegram_chat_id: String,
    pub slack_webhook_url: String,
    pub zalo_bot_token: String,
    pub zalo_chat_id: String,
    pub smtp_host: String,
    pub smtp_port: i64,
    pub smtp_user: String,
    pub smtp_password: String,
    pub email_from: String,
    pub email_to: String,
    pub agent_max_steps: i64,
    pub agent_token_budget: i64,
    pub tool_timeout_s: f64,
    pub meta_api_version: String,
    pub meta_base_url: String,
    pub meta_sync_days: i64,
    pub meta_sync_interval_min: i64,
    pub write_budget_cap_pct: f64,
    pub write_daily_limit: i64,
    pub approval_ttl_hours: i64,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            llm_provider_order: "openai,anthropic".to_string(),
            llm_max_attempts: 3,
            llm_backoff_base_s: 0.5,
            llm_timeout_s: 60.0,
            openai_api_key: "".to_string(),
            openai_base_url: "https://api.openai.com/v1".to_string(),
            openai_model: "gpt-4.1-mini".to_string(),
            anthropic_api_key: "".to_string(),
            anthropic_base_url: "https://api.anthropic.com".to_string(),
            anthropic_model: "claude-sonnet-5-5".to_string(),
            openai_compat_base_url: "".to_string(),
            openai_compat_api_key: "".to_string(),
            openai_compat_model: "".to_string(),
            timezone: "UTC".to_string(),
            telegram_bot_token: "".to_string(),
            telegram_chat_id: "".to_string(),
            slack_webhook_url: "".to_string(),
            zalo_bot_token: "".to_string(),
            zalo_chat_id: "".to_string(),
            smtp_host: "".to_string(),
            smtp_port: 587,
            smtp_user: "".to_string(),
            smtp_password: "".to_string(),
            email_from: "".to_string(),
            email_to: "".to_string(),
            agent_max_steps: 8,
            agent_token_budget: 80000,
            tool_timeout_s: 30.0,
            meta_api_version: "v23.0".to_string(),
            meta_base_url: "https://graph.facebook.com".to_string(),
            meta_sync_days: 30,
            meta_sync_interval_min: 60,
            write_budget_cap_pct: 20.0,
            write_daily_limit: 10,
            approval_ttl_hours: 24,
        }
    }
}

impl Settings {
    pub fn get(&self, key: &str) -> Option<SettingValue> {
        match key {
            "llm_provider_order" => Some(SettingValue::Text(self.llm_provider_order.clone())),
            "llm_max_attempts" => Some(SettingValue::Int(self.llm_max_attempts)),
            "llm_backoff_base_s" => Some(SettingValue::Float(self.llm_backoff_base_s)),
            "llm_timeout_s" => Some(SettingValue::Float(self.llm_timeout_s)),
            "openai_api_key" => Some(SettingValue::Text(self.openai_api_key.clone())),
            "openai_base_url" => Some(SettingValue::Text(self.openai_base_url.clone())),
            "openai_model" => Some(SettingValue::Text(self.openai_model.clone())),
            "anthropic_api_key" => Some(SettingValue::Text(self.anthropic_api_key.clone())),
            "anthropic_base_url" => Some(SettingValue::Text(self.anthropic_base_url.clone())),
            "anthropic_model" => Some(SettingValue::Text(self.anthropic_model.clone())),
            "openai_compat_base_url" => Some(SettingValue::Text(self.openai_compat_base_url.clone())),
            "openai_compat_api_key" => Some(SettingValue::Text(self.openai_compat_api_key.clone())),
            "openai_compat_model" => Some(SettingValue::Text(self.openai_compat_model.clone())),
            "timezone" => Some(SettingValue::Text(self.timezone.clone())),
            "telegram_bot_token" => Some(SettingValue::Text(self.telegram_bot_token.clone())),
            "telegram_chat_id" => Some(SettingValue::Text(self.telegram_chat_id.clone())),
            "slack_webhook_url" => Some(SettingValue::Text(self.slack_webhook_url.clone())),
            "zalo_bot_token" => Some(SettingValue::Text(self.zalo_bot_token.clone())),
            "zalo_chat_id" => Some(SettingValue::Text(self.zalo_chat_id.clone())),
            "smtp_host" => Some(SettingValue::Text(self.smtp_host.clone())),
            "smtp_port" => Some(SettingValue::Int(self.smtp_port)),
            "smtp_user" => Some(SettingValue::Text(self.smtp_user.clone())),
            "smtp_password" => Some(SettingValue::Text(self.smtp_password.clone())),
            "email_from" => Some(SettingValue::Text(self.email_from.clone())),
            "email_to" => Some(SettingValue::Text(self.email_to.clone())),
            "agent_max_steps" => Some(SettingValue::Int(self.agent_max_steps)),
            "agent_token_budget" => Some(SettingValue::Int(self.agent_token_budget)),
            "tool_timeout_s" => Some(SettingValue::Float(self.tool_timeout_s)),
            "meta_api_version" => Some(SettingValue::Text(self.meta_api_version.clone())),
            "meta_base_url" => Some(SettingValue::Text(self.meta_base_url.clone())),
            "meta_sync_days" => Some(SettingValue::Int(self.meta_sync_days)),
            "meta_sync_interval_min" => Some(SettingValue::Int(self.meta_sync_interval_min)),
            "write_budget_cap_pct" => Some(SettingValue::Float(self.write_budget_cap_pct)),
            "write_daily_limit" => Some(SettingValue::Int(self.write_daily_limit)),
            "approval_ttl_hours" => Some(SettingValue::Int(self.approval_ttl_hours)),
            _ => None,
        }
    }

    pub fn apply(&mut self, key: &str, value: &str) -> bool {
        match key {
            "llm_provider_order" => {
                self.llm_provider_order = value.to_string();
                true
            }
            "llm_max_attempts" => match parse_number(value) {
                Some(number) => {
                    self.llm_max_attempts = number.trunc() as i64;
                    true
                }
                None => false,
            },
            "llm_backoff_base_s" => match parse_number(value) {
                Some(number) => {
                    self.llm_backoff_base_s = number;
                    true
                }
                None => false,
            },
            "llm_timeout_s" => match parse_number(value) {
                Some(number) => {
                    self.llm_timeout_s = number;
                    true
                }
                None => false,
            },
            "openai_api_key" => {
                self.openai_api_key = value.to_string();
                true
            }
            "openai_base_url" => {
                self.openai_base_url = value.to_string();
                true
            }
            "openai_model" => {
                self.openai_model = value.to_string();
                true
            }
            "anthropic_api_key" => {
                self.anthropic_api_key = value.to_string();
                true
            }
            "anthropic_base_url" => {
                self.anthropic_base_url = value.to_string();
                true
            }
            "anthropic_model" => {
                self.anthropic_model = value.to_string();
                true
            }
            "openai_compat_base_url" => {
                self.openai_compat_base_url = value.to_string();
                true
            }
            "openai_compat_api_key" => {
                self.openai_compat_api_key = value.to_string();
                true
            }
            "openai_compat_model" => {
                self.openai_compat_model = value.to_string();
                true
            }
            "timezone" => {
                self.timezone = value.to_string();
                true
            }
            "telegram_bot_token" => {
                self.telegram_bot_token = value.to_string();
                true
            }
            "telegram_chat_id" => {
                self.telegram_chat_id = value.to_string();
                true
            }
            "slack_webhook_url" => {
                self.slack_webhook_url = value.to_string();
                true
            }
            "zalo_bot_token" => {
                self.zalo_bot_token = value.to_string();
                true
            }
            "zalo_chat_id" => {
                self.zalo_chat_id = value.to_string();
                true
            }
            "smtp_host" => {
                self.smtp_host = value.to_string();
                true
            }
            "smtp_port" => match parse_number(value) {
                Some(number) => {
                    self.smtp_port = number.trunc() as i64;
                    true
                }
                None => false,
            },
            "smtp_user" => {
                self.smtp_user = value.to_string();
                true
            }
            "smtp_password" => {
                self.smtp_password = value.to_string();
                true
            }
            "email_from" => {
                self.email_from = value.to_string();
                true
            }
            "email_to" => {
                self.email_to = value.to_string();
                true
            }
            "agent_max_steps" => match parse_number(value) {
                Some(number) => {
                    self.agent_max_steps = number.trunc() as i64;
                    true
                }
                None => false,
            },
            "agent_token_budget" => match parse_number(value) {
                Some(number) => {
                    self.agent_token_budget = number.trunc() as i64;
                    true
                }
                None => false,
            },
            "tool_timeout_s" => match parse_number(value) {
                Some(number) => {
                    self.tool_timeout_s = number;
                    true
                }
                None => false,
            },
            "meta_api_version" => {
                self.meta_api_version = value.to_string();
                true
            }
            "meta_base_url" => {
                self.meta_base_url = value.to_string();
                true
            }
            "meta_sync_days" => match parse_number(value) {
                Some(number) => {
                    self.meta_sync_days = number.trunc() as i64;
                    true
                }
                None => false,
            },
            "meta_sync_interval_min" => match parse_number(value) {
                Some(number) => {
                    self.meta_sync_interval_min = number.trunc() as i64;
                    true
                }
                None => false,
            },
            "write_budget_cap_pct" => match parse_number(value) {
                Some(number) => {
                    self.write_budget_cap_pct = number;
                    true
                }
                None => false,
            },
            "write_daily_limit" => match parse_number(value) {
                Some(number) => {
                    self.write_daily_limit = number.trunc() as i64;
                    true
                }
                None => false,
            },
            "approval_ttl_hours" => match parse_number(value) {
                Some(number) => {
                    self.approval_ttl_hours = number.trunc() as i64;
                    true
                }
                None => false,
            },
            _ => false,
        }
    }
}

fn parse_number(value: &str) -> Option<f64> {
    let number: f64 = value.trim().parse().ok()?;
    if number.is_finite() {
        Some(number)
    } else {
        None
    }
}

fn split_list(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(|item| item.trim())
        .filter(|item| !item.is_empty())
        .map(String::from)
        .collect()
}

pub fn provider_order(settings: &Settings) -> Vec<String> {
    split_list(&settings.llm_provider_order)
}

pub fn email_recipients(settings: &Settings) -> Vec<String> {
    split_list(&settings.email_to)
}
