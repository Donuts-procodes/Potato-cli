use anyhow::Result;
use async_trait::async_trait;

use crate::agents::traits::{Subagent, SubagentResult, SubagentTask, TaskCategory};
use crate::llm::{ChatMessage, LlmClient};

/// Specialist agent for diagnosing and fixing build/test failures.
pub struct RepairAgent;

#[async_trait]
impl Subagent for RepairAgent {
    fn name(&self) -> &str {
        "Repair"
    }

    fn system_prompt(&self) -> String {
        r#"You are a Senior Debugging Engineer. Your ONLY job is to fix build and test failures.

Rules:
- ALWAYS read the failing file's relevant lines (read_file) before patching.
- Use apply_patch for surgical fixes. NEVER regenerate entire files.
- If the same fix fails twice, pivot: change the approach entirely.
- After each fix, run the verification command to confirm the fix works.
- If blocked after 3 attempts, report failure and suggest an alternative architecture.

Anti-Oscillation Protocol:
- Track which patches you've tried. Do not repeat a failed patch.
- If a type error persists, inspect the CALLER, not just the callee.
- If a dependency error persists, check Cargo.toml/package.json, not just the source.

Output JSON with: "thought", "phase": "REPAIR", "action": tool action."#
            .to_string()
    }

    fn can_handle(&self, category: &TaskCategory) -> bool {
        *category == TaskCategory::Repair
    }

    fn max_turns(&self) -> usize {
        30
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
                    "Fix this failure:\n\nDescription: {}\n\nError context:\n{}\n\nRelevant files: {:?}",
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
