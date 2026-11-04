use crate::adapters::db::sink::SqlUsageSink;
use crate::adapters::llm::anthropic::{AnthropicAdapter, AnthropicOptions};
use crate::adapters::llm::fake::FakeLLM;
use crate::adapters::llm::openai::{OpenAIAdapter, OpenAIOptions};
use crate::adapters::llm::router::LLMRouter;
use crate::adapters::meta::client::MetaAdsClient;
use crate::adapters::notify::email::EmailNotifier;
use crate::adapters::notify::slack::SlackNotifier;
use crate::adapters::notify::telegram::TelegramNotifier;
use crate::adapters::notify::zalo::ZaloNotifier;
use crate::config::{email_recipients, provider_order, Settings};
use crate::db::Db;
use crate::ports::llm::{LLMCapabilities, LLMPort, UsageSink};
use crate::ports::meta::MetaAdsPort;
use crate::ports::notify::NotifierPort;
use crate::router::Failure;
use crate::services::app_settings::{effective_settings, SettingsStore};
use crate::services::meta_connections::{load_connections, migrate_legacy_token};
use rusqlite::OptionalExtension;
use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, RwLock};
use std::time::Duration;
use tokio::sync::{Mutex as AsyncMutex, MutexGuard as AsyncMutexGuard, Notify};

pub const PROVIDER_LABELS: [(&str, &str); 4] = [
    ("openai", "OpenAI"),
    ("anthropic", "Anthropic"),
    ("openai_compat", "OpenAI-compatible endpoint"),
    ("fake", "Demo provider"),
];

pub const CHANNEL_LABELS: [(&str, &str); 4] = [
    ("telegram", "Telegram"),
    ("slack", "Slack"),
    ("zalo", "Zalo"),
    ("email", "Email"),
];

pub type ProviderMap = HashMap<String, Arc<dyn LLMPort>>;
pub type NotifierMap = HashMap<String, Arc<dyn NotifierPort>>;

#[derive(Debug)]
pub struct ProviderNotConfigured(pub String);

#[derive(Clone)]
pub struct MetaConnectionClient {
    pub id: i64,
    pub name: String,
    pub client: Arc<dyn MetaAdsPort>,
}

#[derive(Serialize, Clone, Debug)]
pub struct ProviderStatus {
    pub name: String,
    pub label: String,
    pub configured: bool,
    pub model: String,
    pub priority: Option<usize>,
}

pub struct Gate {
    pending: AtomicUsize,
    inner: AsyncMutex<()>,
}

pub struct GateGuard<'a> {
    gate: &'a Gate,
    _guard: AsyncMutexGuard<'a, ()>,
}

impl Gate {
    pub fn new() -> Self {
        Gate {
            pending: AtomicUsize::new(0),
            inner: AsyncMutex::new(()),
        }
    }

    pub fn locked(&self) -> bool {
        self.pending.load(Ordering::SeqCst) > 0
    }

    pub async fn acquire(&self) -> GateGuard<'_> {
        self.pending.fetch_add(1, Ordering::SeqCst);
        let guard = self.inner.lock().await;
        GateGuard {
            gate: self,
            _guard: guard,
        }
    }
}

impl<'a> Drop for GateGuard<'a> {
    fn drop(&mut self) {
        self.gate.pending.fetch_sub(1, Ordering::SeqCst);
    }
}

pub struct Wake {
    flag: AtomicBool,
    notify: Notify,
}

impl Wake {
    pub fn new() -> Self {
        Wake {
            flag: AtomicBool::new(false),
            notify: Notify::new(),
        }
    }

    pub fn set(&self) {
        self.flag.store(true, Ordering::SeqCst);
        self.notify.notify_waiters();
    }

    pub fn clear(&self) {
        self.flag.store(false, Ordering::SeqCst);
    }

    pub async fn wait(&self, timeout: Duration) {
        let notified = self.notify.notified();
        if self.flag.load(Ordering::SeqCst) {
            return;
        }
        let _ = tokio::time::timeout(timeout, notified).await;
    }
}

#[derive(Clone)]
struct State {
    settings: Arc<Settings>,
    overrides: Arc<HashMap<String, String>>,
    providers: Arc<ProviderMap>,
    meta_clients: Arc<Vec<MetaConnectionClient>>,
    notifiers: Arc<NotifierMap>,
}

pub struct Container {
    pub db: Db,
    pub store: SettingsStore,
    pub base_settings: Settings,
    pub sink: Arc<dyn UsageSink>,
    pub sync_lock: Gate,
    pub wake: Wake,
    state: RwLock<State>,
}

pub fn build_providers(settings: &Settings) -> ProviderMap {
    let mut providers: ProviderMap = HashMap::new();
    if !settings.openai_api_key.is_empty() {
        providers.insert(
            "openai".to_string(),
            Arc::new(OpenAIAdapter::new(OpenAIOptions {
                api_key: settings.openai_api_key.clone(),
                model: settings.openai_model.clone(),
                base_url: Some(settings.openai_base_url.clone()),
                name: None,
                max_tokens_field: None,
                capabilities: None,
            })),
        );
    }
    if !settings.anthropic_api_key.is_empty() {
        providers.insert(
            "anthropic".to_string(),
            Arc::new(AnthropicAdapter::new(AnthropicOptions {
                api_key: settings.anthropic_api_key.clone(),
                model: settings.anthropic_model.clone(),
                base_url: Some(settings.anthropic_base_url.clone()),
                api_version: None,
            })),
        );
    }
    if !settings.openai_compat_base_url.is_empty() && !settings.openai_compat_model.is_empty() {
        providers.insert(
            "openai_compat".to_string(),
            Arc::new(OpenAIAdapter::new(OpenAIOptions {
                api_key: settings.openai_compat_api_key.clone(),
                model: settings.openai_compat_model.clone(),
                base_url: Some(settings.openai_compat_base_url.clone()),
                name: Some("openai_compat".to_string()),
                max_tokens_field: Some("max_tokens".to_string()),
                capabilities: Some(LLMCapabilities {
                    tool_calling: true,
                    parallel_tool_calls: false,
                    structured_output: false,
                    streaming: false,
                    vision: false,
                    max_context_tokens: 32000,
                }),
            })),
        );
    }
    if provider_order(settings).iter().any(|item| item == "fake") {
        providers.insert("fake".to_string(), Arc::new(FakeLLM::new()));
    }
    providers
}

pub fn build_meta_clients(
    db: &Db,
    store: &SettingsStore,
    settings: &Settings,
) -> rusqlite::Result<Vec<MetaConnectionClient>> {
    Ok(load_connections(db, store)?
        .into_iter()
        .filter(|item| item.row.enabled == 1)
        .map(|item| MetaConnectionClient {
            id: item.row.id,
            name: item.row.name.clone(),
            client: Arc::new(MetaAdsClient::new(
                &item.token,
                &settings.meta_api_version,
                &settings.meta_base_url,
            )),
        })
        .collect())
}

pub fn build_notifiers(settings: &Settings) -> NotifierMap {
    let mut notifiers: NotifierMap = HashMap::new();
    if !settings.telegram_bot_token.is_empty() && !settings.telegram_chat_id.is_empty() {
        notifiers.insert(
            "telegram".to_string(),
            Arc::new(TelegramNotifier::new(&settings.telegram_bot_token, &settings.telegram_chat_id)),
        );
    }
    if !settings.slack_webhook_url.is_empty() {
        notifiers.insert("slack".to_string(), Arc::new(SlackNotifier::new(&settings.slack_webhook_url)));
    }
    if !settings.zalo_bot_token.is_empty() && !settings.zalo_chat_id.is_empty() {
        notifiers.insert(
            "zalo".to_string(),
            Arc::new(ZaloNotifier::new(&settings.zalo_bot_token, &settings.zalo_chat_id)),
        );
    }
    let recipients = email_recipients(settings);
    if !settings.smtp_host.is_empty() && !settings.email_from.is_empty() && !recipients.is_empty() {
        notifiers.insert(
            "email".to_string(),
            Arc::new(EmailNotifier::new(
                &settings.smtp_host,
                settings.smtp_port,
                &settings.smtp_user,
                &settings.smtp_password,
                &settings.email_from,
                recipients,
            )),
        );
    }
    notifiers
}

impl Container {
    pub fn new(db: Db, store: SettingsStore, base_settings: Settings, sink: Arc<dyn UsageSink>) -> Self {
        let state = State {
            settings: Arc::new(base_settings.clone()),
            overrides: Arc::new(HashMap::new()),
            providers: Arc::new(HashMap::new()),
            meta_clients: Arc::new(Vec::new()),
            notifiers: Arc::new(HashMap::new()),
        };
        Container {
            db,
            store,
            base_settings,
            sink,
            sync_lock: Gate::new(),
            wake: Wake::new(),
            state: RwLock::new(state),
        }
    }

    pub fn settings(&self) -> Arc<Settings> {
        self.state
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .settings
            .clone()
    }

    pub fn overrides(&self) -> Arc<HashMap<String, String>> {
        self.state
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .overrides
            .clone()
    }

    pub fn meta_clients(&self) -> Arc<Vec<MetaConnectionClient>> {
        self.state
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .meta_clients
            .clone()
    }

    pub fn providers(&self) -> Arc<ProviderMap> {
        self.state
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .providers
            .clone()
    }

    pub fn notifier(&self, name: &str) -> Option<Arc<dyn NotifierPort>> {
        self.state
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .notifiers
            .get(name)
            .cloned()
    }

    pub fn has_meta(&self) -> bool {
        !self.meta_clients().is_empty()
    }

    pub fn meta_for_account(&self, account_id: &str) -> rusqlite::Result<Option<Arc<dyn MetaAdsPort>>> {
        let found: Option<Option<i64>> = {
            let conn = self.db.lock();
            conn.query_row(
                "SELECT connection_id FROM ad_accounts WHERE id = ?1",
                [account_id],
                |row| row.get::<_, Option<i64>>(0),
            )
            .optional()?
        };
        let connection_id = match found.flatten() {
            Some(id) => id,
            None => return Ok(None),
        };
        Ok(self
            .meta_clients()
            .iter()
            .find(|item| item.id == connection_id)
            .map(|item| item.client.clone()))
    }

    pub fn ordered_providers(&self) -> Vec<Arc<dyn LLMPort>> {
        let providers = self.providers();
        provider_order(&self.settings())
            .iter()
            .filter_map(|name| providers.get(name).cloned())
            .collect()
    }

    pub fn router_for(&self, names: Option<&[String]>) -> Result<LLMRouter, ProviderNotConfigured> {
        let chain: Vec<Arc<dyn LLMPort>> = match names {
            None => self.ordered_providers(),
            Some(list) => {
                let providers = self.providers();
                let mut chain = Vec::new();
                for name in list {
                    match providers.get(name) {
                        Some(provider) => chain.push(provider.clone()),
                        None => return Err(ProviderNotConfigured(name.clone())),
                    }
                }
                chain
            }
        };
        let settings = self.settings();
        Ok(LLMRouter::new(
            chain,
            settings.llm_max_attempts,
            settings.llm_backoff_base_s,
            Some(self.sink.clone()),
        ))
    }

    pub fn reload(&self) -> Result<(), Failure> {
        let overrides = self.store.load()?;
        let settings = effective_settings(&self.base_settings, &overrides);
        let providers = build_providers(&settings);
        let meta_clients = build_meta_clients(&self.db, &self.store, &settings)?;
        let notifiers = build_notifiers(&settings);
        {
            let mut state = self.state.write().unwrap_or_else(|poisoned| poisoned.into_inner());
            state.settings = Arc::new(settings);
            state.overrides = Arc::new(overrides);
            state.providers = Arc::new(providers);
            state.meta_clients = Arc::new(meta_clients);
            state.notifiers = Arc::new(notifiers);
        }
        self.wake.set();
        Ok(())
    }

    pub fn provider_status(&self) -> Vec<ProviderStatus> {
        let settings = self.settings();
        let providers = self.providers();
        let order = provider_order(&settings);
        let model_of = |name: &str| -> String {
            match name {
                "openai" => settings.openai_model.clone(),
                "anthropic" => settings.anthropic_model.clone(),
                "openai_compat" => settings.openai_compat_model.clone(),
                _ => "fake-1".to_string(),
            }
        };
        let ranked: Vec<&String> = order.iter().filter(|name| providers.contains_key(name.as_str())).collect();
        let mut rows: Vec<ProviderStatus> = PROVIDER_LABELS
            .iter()
            .filter(|(name, _)| *name != "fake" || order.iter().any(|item| item == "fake"))
            .map(|(name, label)| ProviderStatus {
                name: name.to_string(),
                label: label.to_string(),
                configured: providers.contains_key(*name),
                model: model_of(*name),
                priority: ranked.iter().position(|item| item.as_str() == *name).map(|index| index + 1),
            })
            .collect();
        rows.sort_by_key(|row| (row.priority.is_none(), row.priority.unwrap_or(0)));
        rows
    }
}

pub fn build_container(db: Db, data_dir: &Path) -> Result<Container, Failure> {
    let store = SettingsStore::new(db.clone(), data_dir.join("secret.key"));
    migrate_legacy_token(&db, &store)?;
    let sink: Arc<dyn UsageSink> = Arc::new(SqlUsageSink::new(db.clone()));
    let container = Container::new(db, store, Settings::default(), sink);
    container.reload()?;
    Ok(container)
}
