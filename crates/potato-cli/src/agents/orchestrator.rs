use anyhow::Result;
use async_trait::async_trait;

use crate::agents::traits::{Subagent, SubagentResult, SubagentTask, TaskCategory};
use crate::llm::{ChatMessage, LlmClient};

/// LLM-driven dynamic orchestrator that replaces hard-coded pipeline logic.
/// Uses an LLM to decide which agent to invoke next based on project state.
pub struct OrchestratorAgent;

#[async_trait]
impl Subagent for OrchestratorAgent {
    fn name(&self) -> &str { "Orchestrator" }

    fn system_prompt(&self) -> String {
        r#"You are a Meta-Orchestrator. Your job is to dynamically decide which specialist agent to invoke next.

You receive the current project state:
- Completed tasks and their results
- Current ROADMAP.json status
- Available agents and their capabilities
- Error history and recent failures

For each turn, output a routing decision:
{
  "thought": "reasoning about what to do next",
  "next_agent": "AgentName",
  "task": {
    "id": "unique-id",
    "description": "what the agent should do",
    "category": "category_name",
    "context": "relevant context",
    "relevant_files": ["file1.rs"]
  },
  "pipeline_complete": false
}

Set pipeline_complete=true when all roadmap tasks are done and tests pass.

Rules:
- After Implementation, always route to Reviewer.
- After Reviewer finds critical issues, route to Repair (not back to Implementer).
- Run SecurityAuditor and DependencyAuditor before final completion.
- If an agent fails 3 times on the same task, escalate to Planner for re-decomposition."#.to_string()
    }

    fn can_handle(&self, category: &TaskCategory) -> bool { *category == TaskCategory::Orchestration }
    fn max_turns(&self) -> usize { 100 }

    async fn execute(&self, client: &LlmClient, task: &SubagentTask) -> Result<SubagentResult> {
        let messages = vec![
            ChatMessage { role: "system".to_string(), content: self.system_prompt() },
            ChatMessage { role: "user".to_string(), content: format!("Orchestrate:\n{}\n\nProject State:\n{}", task.description, task.context) },
        ];
        let response = client.send_turn(&messages).await?;
        Ok(SubagentResult { agent_name: self.name().to_string(), success: true, summary: response.thought, modified_files: Vec::new(), issues: Vec::new(), messages, token_usage: None })
    }
}
