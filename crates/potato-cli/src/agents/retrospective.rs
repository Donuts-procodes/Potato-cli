use anyhow::Result;
use async_trait::async_trait;

use crate::agents::traits::{Subagent, SubagentResult, SubagentTask, TaskCategory};
use crate::llm::{ChatMessage, LlmClient};

/// Analyzes completed sessions for improvement opportunities.
pub struct RetrospectiveAgent;

#[async_trait]
impl Subagent for RetrospectiveAgent {
    fn name(&self) -> &str { "Retrospective" }

    fn system_prompt(&self) -> String {
        r#"You are a Senior Process Improvement Engineer. Your ONLY job is to analyze completed agent sessions and extract lessons.

Given a session transcript, identify:
1. Wasted turns (actions that were immediately undone or repeated).
2. Oscillation patterns (same error fixed multiple times).
3. Missing context (information the agent should have read earlier).
4. Successful patterns (approaches that worked on first try).
5. Tool usage efficiency (unnecessary file reads, redundant list_dir calls).

Output a structured retrospective with:
- What went well (patterns to reinforce)
- What went poorly (anti-patterns to avoid)
- Concrete lessons (saved to .potato/lessons/ for future sessions)
- Estimated wasted tokens/cost

Output JSON with: "thought", "phase": "COMPLETE", "action": {"name": "finish", "params": {"summary": "retrospective report"}}."#.to_string()
    }

    fn can_handle(&self, category: &TaskCategory) -> bool { *category == TaskCategory::Retrospective }
    fn max_turns(&self) -> usize { 10 }

    async fn execute(&self, client: &LlmClient, task: &SubagentTask) -> Result<SubagentResult> {
        let messages = vec![
            ChatMessage { role: "system".to_string(), content: self.system_prompt() },
            ChatMessage { role: "user".to_string(), content: format!("Analyze session:\n{}", task.context) },
        ];
        let response = client.send_turn(&messages).await?;
        Ok(SubagentResult { agent_name: self.name().to_string(), success: true, summary: response.thought, modified_files: Vec::new(), issues: Vec::new(), messages, token_usage: None })
    }
}
