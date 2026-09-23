use anyhow::Result;
use async_trait::async_trait;

use crate::agents::traits::{Issue, Subagent, SubagentResult, SubagentTask, TaskCategory};
use crate::llm::{ChatMessage, LlmClient};

/// Specialist agent that reviews code output from the Implementer.
pub struct ReviewerAgent;

#[async_trait]
impl Subagent for ReviewerAgent {
    fn name(&self) -> &str {
        "Reviewer"
    }

    fn system_prompt(&self) -> String {
        r#"You are a Senior Code Reviewer. Your ONLY job is to review code for:
1. Correctness: logic errors, off-by-one, race conditions
2. Style: idiomatic patterns, naming conventions
3. Security: injection, path traversal, secret leaks
4. Completeness: missing error handling, edge cases

Output JSON with:
- "thought": your analysis
- "phase": "VERIFY"
- "action": {"name": "finish", "params": {"summary": "review findings as JSON array of issues"}}

Each issue must have: severity (critical/warning/info), file, line (if known), description, suggestion."#
            .to_string()
    }

    fn can_handle(&self, category: &TaskCategory) -> bool {
        *category == TaskCategory::Review
    }

    fn max_turns(&self) -> usize {
        10
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
                    "Review this implementation:\n\nDescription: {}\n\nContext:\n{}\n\nFiles to review: {:?}",
                    task.description, task.context, task.relevant_files
                ),
            },
        ];

        let response = client.send_turn(&messages).await?;

        // Parse issues from the response if possible
        let issues = parse_issues_from_summary(&response.thought);

        Ok(SubagentResult {
            agent_name: self.name().to_string(),
            success: true,
            summary: response.thought,
            modified_files: Vec::new(),
            issues,
            messages,
            token_usage: None,
        })
    }
}

fn parse_issues_from_summary(summary: &str) -> Vec<Issue> {
    // Best-effort parse: if the LLM included JSON issues, extract them.
    // Otherwise return empty — the summary itself serves as the review.
    if let Some(start) = summary.find('[') {
        if let Some(end) = summary.rfind(']') {
            if end > start {
                let json_slice = &summary[start..=end];
                if let Ok(issues) = serde_json::from_str::<Vec<Issue>>(json_slice) {
                    return issues;
                }
            }
        }
    }
    Vec::new()
}
