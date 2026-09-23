use anyhow::Result;
use async_trait::async_trait;

use crate::agents::traits::{Subagent, SubagentResult, SubagentTask, TaskCategory};
use crate::llm::{ChatMessage, LlmClient};

/// Specialist agent for Phase 1: writes SPEC.md, ROADMAP.json, and file tree.
pub struct ArchitectAgent;

#[async_trait]
impl Subagent for ArchitectAgent {
    fn name(&self) -> &str {
        "Architect"
    }

    fn system_prompt(&self) -> String {
        let assembler = crate::engine::context::ContextAssembler::new(".");
        let ctx = assembler.assemble().unwrap_or_else(|_| crate::engine::context::ProjectContext {
            os: std::env::consts::OS.to_string(),
            arch: std::env::consts::ARCH.to_string(),
            working_dir: ".".to_string(),
            detected_toolchains: Vec::new(),
            git_branch: None,
            modified_files: Vec::new(),
            relevant_lessons: Vec::new(),
        });
        self.system_prompt_with_context(&ctx)
    }

    fn system_prompt_with_context(&self, context: &crate::engine::context::ProjectContext) -> String {
        crate::llm::SystemPromptBuilder::build(&TaskCategory::Architecture, context)
    }

    fn can_handle(&self, category: &TaskCategory) -> bool {
        *category == TaskCategory::Architecture
    }

    fn max_turns(&self) -> usize {
        20
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
                    "Objective: {}\n\nContext: {}\n\nProduce SPEC.md and ROADMAP.json.",
                    task.description, task.context
                ),
            },
        ];

        let response = client.send_turn(&messages).await?;

        Ok(SubagentResult {
            agent_name: self.name().to_string(),
            success: true,
            summary: response.thought,
            modified_files: vec!["SPEC.md".to_string(), "ROADMAP.json".to_string()],
            issues: Vec::new(),
            messages,
            token_usage: None,
        })
    }
}
