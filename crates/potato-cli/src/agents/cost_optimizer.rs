use anyhow::Result;
use async_trait::async_trait;

use crate::agents::traits::{Subagent, SubagentResult, SubagentTask, TaskCategory};
use crate::llm::{ChatMessage, LlmClient};

/// Analyzes token usage and suggests cost optimizations.
pub struct CostOptimizerAgent;

#[async_trait]
impl Subagent for CostOptimizerAgent {
    fn name(&self) -> &str { "CostOptimizer" }

    fn system_prompt(&self) -> String {
        r#"You are a Cost Optimization Engineer. Your ONLY job is to reduce LLM token costs without sacrificing output quality.

Given session data (per-turn token usage, agent assignments, success/failure rates), recommend:
1. Which agents should use cheaper models (e.g., gpt-4o-mini instead of gpt-4o).
2. Which context messages can be compressed or summarized.
3. Which tool calls are redundant (e.g., reading the same file twice).
4. Whether batch-reading multiple files is cheaper than sequential reads.
5. Optimal context window trim thresholds.

Output a cost optimization report with:
- Current cost breakdown by agent.
- Projected savings for each recommendation.
- Risk assessment (quality impact of each change).

Output JSON with: "thought", "phase": "COMPLETE", "action": {"name": "finish", "params": {"summary": "cost report"}}."#.to_string()
    }

    fn can_handle(&self, category: &TaskCategory) -> bool { *category == TaskCategory::CostOptimization }
    fn max_turns(&self) -> usize { 10 }

    async fn execute(&self, client: &LlmClient, task: &SubagentTask) -> Result<SubagentResult> {
        let messages = vec![
            ChatMessage { role: "system".to_string(), content: self.system_prompt() },
            ChatMessage { role: "user".to_string(), content: format!("Optimize costs:\n{}", task.context) },
        ];
        let response = client.send_turn(&messages).await?;
        Ok(SubagentResult { agent_name: self.name().to_string(), success: true, summary: response.thought, modified_files: Vec::new(), issues: Vec::new(), messages, token_usage: None })
    }
}
