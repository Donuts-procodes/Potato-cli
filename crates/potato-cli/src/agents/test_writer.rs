use anyhow::Result;
use async_trait::async_trait;

use crate::agents::traits::{Subagent, SubagentResult, SubagentTask, TaskCategory};
use crate::llm::{ChatMessage, LlmClient};

/// Specialist agent that generates unit and integration tests.
pub struct TestWriterAgent;

#[async_trait]
impl Subagent for TestWriterAgent {
    fn name(&self) -> &str {
        "TestWriter"
    }

    fn system_prompt(&self) -> String {
        r#"You are a Senior Test Engineer. Your ONLY job is to write comprehensive tests.

Rules:
- Write unit tests for every public function.
- Write integration tests for cross-module interactions.
- Cover happy paths, edge cases, error cases, and boundary conditions.
- Use the project's native test framework (e.g., #[test] for Rust, pytest for Python).
- Test files go in the standard location for the language.
- NO mocking unless absolutely necessary — prefer real implementations.

Output JSON with: "thought", "phase": "IMPLEMENTATION", "action": tool action."#
            .to_string()
    }

    fn can_handle(&self, category: &TaskCategory) -> bool {
        *category == TaskCategory::Testing
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
                    "Write tests for: {}\n\nContext:\n{}\n\nFiles to test: {:?}",
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
