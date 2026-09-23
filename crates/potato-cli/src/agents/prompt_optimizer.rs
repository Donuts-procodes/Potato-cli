use anyhow::Result;
use async_trait::async_trait;

use crate::agents::traits::{Subagent, SubagentResult, SubagentTask, TaskCategory};
use crate::llm::{ChatMessage, LlmClient};

/// Analyzes prompt effectiveness and suggests improvements.
pub struct PromptOptimizerAgent;

#[async_trait]
impl Subagent for PromptOptimizerAgent {
    fn name(&self) -> &str { "PromptOptimizer" }

    fn system_prompt(&self) -> String {
        r#"You are a Prompt Engineering specialist. Your ONLY job is to optimize system prompts for agent effectiveness.

Given an agent's system prompt and its performance data (success rate, average retries, common failure modes), suggest improvements:
1. Clarity: remove ambiguous instructions.
2. Specificity: add concrete examples for edge cases.
3. Constraints: tighten output format requirements.
4. Context efficiency: reduce token count without losing information.
5. Error prevention: add explicit "do NOT" rules for common mistakes.

Output the improved prompt alongside a changelog of what was changed and why.

Output JSON with: "thought", "phase": "COMPLETE", "action": {"name": "finish", "params": {"summary": "optimized prompt and changelog"}}."#.to_string()
    }

    fn can_handle(&self, category: &TaskCategory) -> bool { *category == TaskCategory::PromptOptimization }
    fn max_turns(&self) -> usize { 10 }

    async fn execute(&self, client: &LlmClient, task: &SubagentTask) -> Result<SubagentResult> {
        let messages = vec![
            ChatMessage { role: "system".to_string(), content: self.system_prompt() },
            ChatMessage { role: "user".to_string(), content: format!("Optimize prompt:\n{}", task.context) },
        ];
        let response = client.send_turn(&messages).await?;
        Ok(SubagentResult { agent_name: self.name().to_string(), success: true, summary: response.thought, modified_files: Vec::new(), issues: Vec::new(), messages, token_usage: None })
    }
}
