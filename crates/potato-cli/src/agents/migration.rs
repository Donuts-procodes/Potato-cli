use anyhow::Result;
use async_trait::async_trait;

use crate::agents::traits::{Subagent, SubagentResult, SubagentTask, TaskCategory};
use crate::llm::{ChatMessage, LlmClient};

/// Handles language/framework migration tasks.
pub struct MigrationAgent;

#[async_trait]
impl Subagent for MigrationAgent {
    fn name(&self) -> &str { "Migration" }

    fn system_prompt(&self) -> String {
        r#"You are a Senior Migration Engineer. Your ONLY job is to migrate codebases between languages, frameworks, or versions.

Responsibilities:
1. Analyze the source codebase and understand its architecture.
2. Map source patterns to idiomatic target patterns (e.g., Express middleware → Axum extractors).
3. Migrate file-by-file in dependency order.
4. Preserve all business logic and edge case handling.
5. Update build configs, manifests, and CI for the target stack.
6. Write migration notes documenting non-obvious translation decisions.

Rules:
- Produce IDIOMATIC code in the target language/framework. Do not transliterate.
- Verify each migrated module compiles/runs before moving to the next.
- If a pattern has no direct equivalent, document the design decision.

Output JSON with: "thought", "phase": "IMPLEMENTATION", "action": tool action."#.to_string()
    }

    fn can_handle(&self, category: &TaskCategory) -> bool { *category == TaskCategory::Migration }
    fn max_turns(&self) -> usize { 60 }

    async fn execute(&self, client: &LlmClient, task: &SubagentTask) -> Result<SubagentResult> {
        let messages = vec![
            ChatMessage { role: "system".to_string(), content: self.system_prompt() },
            ChatMessage { role: "user".to_string(), content: format!("Migrate: {}\n\nContext:\n{}\n\nFiles: {:?}", task.description, task.context, task.relevant_files) },
        ];
        let response = client.send_turn(&messages).await?;
        Ok(SubagentResult { agent_name: self.name().to_string(), success: true, summary: response.thought, modified_files: Vec::new(), issues: Vec::new(), messages, token_usage: None })
    }
}
