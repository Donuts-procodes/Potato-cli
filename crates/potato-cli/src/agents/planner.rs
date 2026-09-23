use anyhow::Result;
use async_trait::async_trait;

use crate::agents::traits::{Subagent, SubagentResult, SubagentTask, TaskCategory};
use crate::llm::{ChatMessage, LlmClient};

/// Decomposes complex objectives into sub-objectives.
pub struct PlannerAgent;

#[async_trait]
impl Subagent for PlannerAgent {
    fn name(&self) -> &str { "Planner" }

    fn system_prompt(&self) -> String {
        r#"You are a Senior Project Planner. Your ONLY job is to decompose complex objectives into manageable sub-objectives.

Given a high-level objective like "Build a SaaS application", break it into:
1. Independent service boundaries (auth, billing, core logic).
2. Ordered implementation phases with clear milestones.
3. Risk assessment for each sub-objective.
4. Resource estimates (approximate lines of code, number of files).

Output a structured plan as JSON with sub-objectives, their dependencies, and suggested agent routing.

Rules:
- Each sub-objective must be achievable in a single agent pipeline run.
- Identify shared infrastructure (database, config, types) as Phase 0.
- Flag sub-objectives that require human decisions (business rules, design choices).

Output JSON with: "thought", "phase": "SPECIFICATION", "action": tool action."#.to_string()
    }

    fn can_handle(&self, category: &TaskCategory) -> bool { *category == TaskCategory::Planning }
    fn max_turns(&self) -> usize { 15 }

    async fn execute(&self, client: &LlmClient, task: &SubagentTask) -> Result<SubagentResult> {
        let messages = vec![
            ChatMessage { role: "system".to_string(), content: self.system_prompt() },
            ChatMessage { role: "user".to_string(), content: format!("Plan: {}\n\nContext:\n{}", task.description, task.context) },
        ];
        let response = client.send_turn(&messages).await?;
        Ok(SubagentResult { agent_name: self.name().to_string(), success: true, summary: response.thought, modified_files: Vec::new(), issues: Vec::new(), messages, token_usage: None })
    }
}
