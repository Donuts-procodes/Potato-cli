use anyhow::Result;
use async_trait::async_trait;

use crate::agents::traits::{Issue, Subagent, SubagentResult, SubagentTask, TaskCategory};
use crate::llm::{ChatMessage, LlmClient};

/// Specialist agent that performs security auditing before final submission.
pub struct SecurityAuditorAgent;

#[async_trait]
impl Subagent for SecurityAuditorAgent {
    fn name(&self) -> &str {
        "SecurityAuditor"
    }

    fn system_prompt(&self) -> String {
        r#"You are a Senior Security Engineer. Your ONLY job is to audit code for security vulnerabilities.

Check for:
1. Path traversal (../../ attacks on file operations)
2. Command injection (unsanitized input passed to shell commands)
3. Secret/credential leaks (API keys, passwords in source)
4. Insecure deserialization
5. Missing input validation
6. SQL injection (if applicable)
7. Dependency vulnerabilities (known CVEs)

For each finding, report:
- severity: critical / warning / info
- file: the affected file
- line: line number if identifiable
- description: what the vulnerability is
- suggestion: how to fix it

Output your findings as a JSON array in the finish action's summary."#
            .to_string()
    }

    fn can_handle(&self, category: &TaskCategory) -> bool {
        *category == TaskCategory::Security
    }

    fn max_turns(&self) -> usize {
        15
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
                    "Audit this codebase for security vulnerabilities:\n\nDescription: {}\n\nContext:\n{}\n\nFiles to audit: {:?}",
                    task.description, task.context, task.relevant_files
                ),
            },
        ];

        let response = client.send_turn(&messages).await?;

        let issues = parse_security_issues(&response.thought);

        Ok(SubagentResult {
            agent_name: self.name().to_string(),
            success: true,
            summary: response.thought,
            modified_files: Vec::new(),
            issues,
            messages,
        })
    }
}

fn parse_security_issues(summary: &str) -> Vec<Issue> {
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
