use async_trait::async_trait;

#[derive(Clone, Debug)]
pub struct MetaError {
    pub message: String,
    pub code: Option<i64>,
    pub auth: bool,
}

impl MetaError {
    pub fn new(message: impl Into<String>) -> Self {
        MetaError {
            message: message.into(),
            code: None,
            auth: false,
        }
    }

    pub fn with_code(message: impl Into<String>, code: Option<i64>, auth: bool) -> Self {
        MetaError {
            message: message.into(),
            code,
            auth,
        }
    }
}

impl std::fmt::Display for MetaError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}", self.message)
    }
}

impl std::error::Error for MetaError {}

#[derive(Clone, Debug)]
pub struct AccountInfo {
    pub id: String,
    pub name: String,
    pub currency: String,
    pub timezone: String,
    pub status: String,
}

#[derive(Clone, Debug)]
pub struct InsightRow {
    pub campaign_id: String,
    pub campaign_name: String,
    pub day: String,
    pub spend: f64,
    pub impressions: i64,
    pub clicks: i64,
    pub conversions: f64,
    pub conversion_value: f64,
}

#[derive(Clone, Debug)]
pub struct CampaignInfo {
    pub id: String,
    pub account_id: String,
    pub name: String,
    pub status: String,
    pub daily_budget: Option<i64>,
}

#[derive(Clone, Debug)]
pub struct CampaignListing {
    pub id: String,
    pub name: String,
    pub objective: Option<String>,
    pub status: String,
    pub effective_status: Option<String>,
    pub daily_budget: Option<f64>,
    pub lifetime_budget: Option<f64>,
    pub created_time: Option<String>,
}

#[async_trait]
pub trait MetaAdsPort: Send + Sync {
    async fn list_accounts(&self) -> Result<Vec<AccountInfo>, MetaError>;
    async fn list_campaigns(&self, account_id: &str) -> Result<Vec<CampaignListing>, MetaError>;
    async fn fetch_insights(&self, account_id: &str, since: &str, until: &str) -> Result<Vec<InsightRow>, MetaError>;
    async fn get_campaign(&self, campaign_id: &str) -> Result<CampaignInfo, MetaError>;
    async fn pause_campaign(&self, campaign_id: &str) -> Result<(), MetaError>;
    async fn set_daily_budget(&self, campaign_id: &str, amount_minor: i64) -> Result<(), MetaError>;
}
