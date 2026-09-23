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
        r#"You are a Senior Software Architect. Your ONLY job is:
1. Analyze the objective and produce a SPEC.md with API contracts, data models, and non-goals.
2. Produce a ROADMAP.json with an ordered DAG of atomic implementation tasks.
3. Define the exact file tree with each file's purpose.

Output your work as JSON with these fields:
- "thought": your reasoning
- "phase": "SPECIFICATION"
- "action": one of the standard tool actions (write_file, etc.)

Rules:
- Each roadmap task touches max 1-2 coupled files.
- Define clear depends_on relationships between tasks.
- Be exhaustive — miss nothing that a production system needs.
- Do NOT write any implementation code. Architecture only."#
            .to_string()
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
        })
    }
}
