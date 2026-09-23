use anyhow::Result;
use async_trait::async_trait;

use crate::agents::traits::{Subagent, SubagentResult, SubagentTask, TaskCategory};
use crate::llm::{ChatMessage, LlmClient};

/// Generates README, API docs, inline doc comments, CHANGELOG entries.
pub struct DocGeneratorAgent;

#[async_trait]
impl Subagent for DocGeneratorAgent {
    fn name(&self) -> &str { "DocGenerator" }

    fn system_prompt(&self) -> String {
        r#"You are a Senior Technical Writer. Your ONLY job is to write documentation.

Responsibilities:
1. Write/update README.md with usage, installation, architecture overview.
2. Add inline doc comments to every public function, struct, enum, trait.
3. Generate API documentation (cargo doc / typedoc compatible).
4. Write CHANGELOG.md entries following Keep a Changelog format.
5. Add code examples in doc comments.

Rules:
- Documentation must be accurate to the actual code. Read files before writing docs.
- Use the project's language conventions for doc comments (/// for Rust, /** */ for TS).
- Every public API surface must be documented.

Output JSON with: "thought", "phase": "IMPLEMENTATION", "action": tool action."#.to_string()
    }

    fn can_handle(&self, category: &TaskCategory) -> bool { *category == TaskCategory::Documentation }
    fn max_turns(&self) -> usize { 30 }

    async fn execute(&self, client: &LlmClient, task: &SubagentTask) -> Result<SubagentResult> {
        let messages = vec![
            ChatMessage { role: "system".to_string(), content: self.system_prompt() },
            ChatMessage { role: "user".to_string(), content: format!("Document: {}\n\nContext:\n{}\n\nFiles: {:?}", task.description, task.context, task.relevant_files) },
        ];
        let response = client.send_turn(&messages).await?;
        Ok(SubagentResult { agent_name: self.name().to_string(), success: true, summary: response.thought, modified_files: Vec::new(), issues: Vec::new(), messages, token_usage: None })
    }
}
