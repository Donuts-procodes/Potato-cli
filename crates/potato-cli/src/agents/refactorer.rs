use anyhow::Result;
use async_trait::async_trait;

use crate::agents::traits::{Subagent, SubagentResult, SubagentTask, TaskCategory};
use crate::llm::{ChatMessage, LlmClient};

/// Identifies code smells and applies refactoring transformations.
pub struct RefactorerAgent;

#[async_trait]
impl Subagent for RefactorerAgent {
    fn name(&self) -> &str { "Refactorer" }

    fn system_prompt(&self) -> String {
        r#"You are a Senior Refactoring Engineer. Your ONLY job is to improve code quality without changing behavior.

Identify and fix:
1. Code duplication — extract shared logic into functions/modules.
2. Long functions (>50 lines) — break into smaller, focused functions.
3. God classes/modules — split by responsibility.
4. Poor naming — rename for clarity.
5. Missing abstractions — introduce traits/interfaces where patterns repeat.
6. Dead code — remove unused functions, imports, variables.

Rules:
- NEVER change external behavior. All existing tests must still pass.
- Use apply_patch for surgical changes.
- After each refactor, run the test suite to verify no regressions.
- Explain WHY each refactoring improves maintainability.

Output JSON with: "thought", "phase": "IMPLEMENTATION", "action": tool action."#.to_string()
    }

    fn can_handle(&self, category: &TaskCategory) -> bool { *category == TaskCategory::Refactoring }
    fn max_turns(&self) -> usize { 40 }

    async fn execute(&self, client: &LlmClient, task: &SubagentTask) -> Result<SubagentResult> {
        let messages = vec![
            ChatMessage { role: "system".to_string(), content: self.system_prompt() },
            ChatMessage { role: "user".to_string(), content: format!("Refactor: {}\n\nContext:\n{}\n\nFiles: {:?}", task.description, task.context, task.relevant_files) },
        ];
        let response = client.send_turn(&messages).await?;
        Ok(SubagentResult { agent_name: self.name().to_string(), success: true, summary: response.thought, modified_files: task.relevant_files.clone(), issues: Vec::new(), messages, token_usage: None })
    }
}
