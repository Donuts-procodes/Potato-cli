use anyhow::Result;
use async_trait::async_trait;

use crate::agents::traits::{Subagent, SubagentResult, SubagentTask, TaskCategory};
use crate::llm::{ChatMessage, LlmClient};

/// Audits dependencies for CVEs, outdated versions, license conflicts.
pub struct DependencyAuditorAgent;

#[async_trait]
impl Subagent for DependencyAuditorAgent {
    fn name(&self) -> &str { "DependencyAuditor" }

    fn system_prompt(&self) -> String {
        r#"You are a Senior Supply Chain Security Engineer. Your ONLY job is to audit project dependencies.

Checks to perform:
1. Run dependency audit tools (cargo audit, npm audit, pip-audit).
2. Identify outdated dependencies and recommend updates.
3. Check for known CVEs in current dependency versions.
4. Detect license conflicts (e.g., GPL dependency in MIT project).
5. Find unnecessary dependencies that could be removed.
6. Check for typosquatting (suspiciously named packages).

Report each finding as an issue with severity, package name, and recommendation.

Output JSON with: "thought", "phase": "VERIFY", "action": tool action."#.to_string()
    }

    fn can_handle(&self, category: &TaskCategory) -> bool { *category == TaskCategory::Dependencies }
    fn max_turns(&self) -> usize { 15 }

    async fn execute(&self, client: &LlmClient, task: &SubagentTask) -> Result<SubagentResult> {
        let messages = vec![
            ChatMessage { role: "system".to_string(), content: self.system_prompt() },
            ChatMessage { role: "user".to_string(), content: format!("Audit dependencies: {}\n\nContext:\n{}\n\nManifests: {:?}", task.description, task.context, task.relevant_files) },
        ];
        let response = client.send_turn(&messages).await?;
        Ok(SubagentResult { agent_name: self.name().to_string(), success: true, summary: response.thought, modified_files: Vec::new(), issues: Vec::new(), messages, token_usage: None })
    }
}
