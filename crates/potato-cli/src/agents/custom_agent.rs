use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::agents::traits::{Subagent, SubagentResult, SubagentTask, TaskCategory};
use crate::llm::{ChatMessage, LlmClient};

/// A user-defined agent loaded from TOML config.
/// Enables custom agents without writing Rust code.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomAgentDef {
    pub name: String,
    pub category: String,
    #[serde(default = "default_max_turns")]
    pub max_turns: usize,
    pub system_prompt: String,
    pub model: Option<String>,
    #[serde(default)]
    pub prompt_includes: Vec<String>,
}

fn default_max_turns() -> usize {
    30
}

/// Runtime wrapper around a TOML-defined custom agent.
pub struct CustomAgent {
    def: CustomAgentDef,
    resolved_category: TaskCategory,
}

impl CustomAgent {
    pub fn from_def(def: CustomAgentDef) -> Option<Self> {
        let resolved_category = parse_category(&def.category)?;
        Some(Self {
            def,
            resolved_category,
        })
    }
}

#[async_trait]
impl Subagent for CustomAgent {
    fn name(&self) -> &str {
        &self.def.name
    }

    fn system_prompt(&self) -> String {
        self.def.system_prompt.clone()
    }

    fn can_handle(&self, category: &TaskCategory) -> bool {
        *category == self.resolved_category
    }

    fn max_turns(&self) -> usize {
        self.def.max_turns
    }

    fn model_override(&self) -> Option<&str> {
        self.def.model.as_deref()
    }

    async fn execute(
        &self,
        client: &LlmClient,
        task: &SubagentTask,
    ) -> Result<SubagentResult> {
        let messages = vec![
            ChatMessage {
                role: "system".to_string(),
                content: self.system_prompt(),
            },
            ChatMessage {
                role: "user".to_string(),
                content: format!(
                    "Task: {}\n\nContext:\n{}\n\nFiles: {:?}",
                    task.description, task.context, task.relevant_files
                ),
            },
        ];

        let response = client.send_turn(&messages).await?;

        Ok(SubagentResult {
            agent_name: self.name().to_string(),
            success: true,
            summary: response.thought,
            modified_files: Vec::new(),
            issues: Vec::new(),
            messages,
            token_usage: None,
        })
    }
}

fn parse_category(s: &str) -> Option<TaskCategory> {
    match s.to_lowercase().as_str() {
        "architecture" => Some(TaskCategory::Architecture),
        "implementation" => Some(TaskCategory::Implementation),
        "review" => Some(TaskCategory::Review),
        "testing" => Some(TaskCategory::Testing),
        "repair" => Some(TaskCategory::Repair),
        "security" => Some(TaskCategory::Security),
        "documentation" => Some(TaskCategory::Documentation),
        "refactoring" => Some(TaskCategory::Refactoring),
        "performance" => Some(TaskCategory::Performance),
        "migration" => Some(TaskCategory::Migration),
        "dependencies" => Some(TaskCategory::Dependencies),
        "devops" => Some(TaskCategory::DevOps),
        "database" => Some(TaskCategory::Database),
        "api_design" => Some(TaskCategory::ApiDesign),
        "planning" => Some(TaskCategory::Planning),
        "retrospective" => Some(TaskCategory::Retrospective),
        _ => None,
    }
}
