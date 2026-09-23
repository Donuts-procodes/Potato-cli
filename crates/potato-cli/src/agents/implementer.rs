use anyhow::Result;
use async_trait::async_trait;

use crate::agents::traits::{Subagent, SubagentResult, SubagentTask, TaskCategory};
use crate::llm::{ChatMessage, LlmClient};

/// Specialist agent for Phase 3: writes production code file-by-file.
pub struct ImplementerAgent;

#[async_trait]
impl Subagent for ImplementerAgent {
    fn name(&self) -> &str {
        "Implementer"
    }

    fn system_prompt(&self) -> String {
        r#"You are a Senior Software Engineer. Your ONLY job is to write production-grade code.

Rules:
- Write complete, idiomatic code. NO placeholders (TODO, pass, ...).
- Use strict type hints everywhere.
- For new files: use write_file.
- For existing files: use apply_patch with exact search blocks.
- Follow the spec and roadmap precisely.
- Implement one module at a time in dependency order.

Output JSON with: "thought", "phase": "IMPLEMENTATION", "action": tool action."#
            .to_string()
    }

    fn can_handle(&self, category: &TaskCategory) -> bool {
        *category == TaskCategory::Implementation
    }

    fn max_turns(&self) -> usize {
        50
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
                    "Task: {}\n\nContext:\n{}\n\nRelevant files: {:?}\n\nImplement this module now.",
                    task.description, task.context, task.relevant_files
                ),
            },
        ];

        let response = client.send_turn(&messages).await?;

        Ok(SubagentResult {
            agent_name: self.name().to_string(),
            success: true,
            summary: response.thought,
            modified_files: task.relevant_files.clone(),
            issues: Vec::new(),
            messages,
            token_usage: None,
        })
    }
}
