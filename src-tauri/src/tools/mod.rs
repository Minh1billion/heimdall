pub mod calc;
pub mod charts;
pub mod common;
pub mod insights;
pub mod reports;
pub mod write;

use crate::ports::llm::ToolSpec;
use crate::ports::tool::Tool;
use crate::validation::schema;
use std::sync::Arc;

pub struct ToolRegistry {
    tools: Vec<Arc<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new(tools: Vec<Arc<dyn Tool>>) -> Self {
        ToolRegistry { tools }
    }

    pub fn get(&self, name: &str) -> Option<&Arc<dyn Tool>> {
        self.tools.iter().find(|tool| tool.name() == name)
    }

    pub fn specs(&self) -> Vec<ToolSpec> {
        self.tools
            .iter()
            .map(|tool| ToolSpec {
                name: tool.name().to_string(),
                description: tool.description().to_string(),
                parameters: schema(&tool.fields()),
            })
            .collect()
    }
}

pub fn build_registry() -> ToolRegistry {
    ToolRegistry::new(vec![
        Arc::new(insights::ListAdAccounts),
        Arc::new(insights::GetInsights),
        Arc::new(insights::ComparePeriods),
        Arc::new(insights::TopN),
        Arc::new(charts::RenderChart),
        Arc::new(reports::DetectAnomalies),
        Arc::new(reports::BuildReport),
        Arc::new(reports::SendNotification),
        Arc::new(write::ProposePauseCampaign),
        Arc::new(write::ProposeBudgetChange),
        Arc::new(calc::Calc),
    ])
}
