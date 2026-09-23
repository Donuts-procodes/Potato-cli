use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::llm::{ChatMessage, LlmClient};

/// A discrete task routed to a subagent by the Coordinator.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubagentTask {
    /// Unique task identifier
    pub id: String,
    /// Natural language description of what to accomplish
    pub description: String,
    /// Category for routing to the correct specialist
    pub category: TaskCategory,
    /// Additional context (e.g., file contents, error logs)
    pub context: String,
    /// Files relevant to this task
    pub relevant_files: Vec<String>,
}

/// All supported task categories for agent routing.
/// Specialist categories map to domain-specific agents.
/// Meta categories map to agents that operate on the agent system itself.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum TaskCategory {
    // --- Core specialist categories ---
    Architecture,
    Implementation,
    Review,
    Testing,
    Repair,
    Security,
    Documentation,
    // --- Extended specialist categories ---
    Refactoring,
    Performance,
    Migration,
    Dependencies,
    DevOps,
    Database,
    ApiDesign,
    // --- Meta categories ---
    Planning,
    Retrospective,
    PromptOptimization,
    CostOptimization,
    Orchestration,
}

/// Result returned by a subagent after completing its task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubagentResult {
    /// The subagent that produced this result
    pub agent_name: String,
    /// Whether the task was completed successfully
    pub success: bool,
    /// Summary of actions taken
    pub summary: String,
    /// Files created or modified
    pub modified_files: Vec<String>,
    /// If review/security: list of issues found
    pub issues: Vec<Issue>,
    /// Raw messages from the subagent's conversation (for context passing)
    pub messages: Vec<ChatMessage>,
    /// Token usage for this subagent run
    pub token_usage: Option<TokenUsage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Issue {
    pub severity: IssueSeverity,
    pub file: String,
    pub line: Option<usize>,
    pub description: String,
    pub suggestion: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum IssueSeverity {
    Critical,
    Warning,
    Info,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TokenUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

/// Trait that all subagents implement.
/// Each subagent has its own system prompt, routing logic, and execution strategy.
#[async_trait]
pub trait Subagent: Send + Sync {
    /// Human-readable name of this subagent.
    fn name(&self) -> &str;

    /// The system prompt that defines this subagent's personality and constraints.
    fn system_prompt(&self) -> String;

    /// Returns `true` if this subagent can handle the given task category.
    fn can_handle(&self, category: &TaskCategory) -> bool;

    /// Maximum number of ReAct turns this subagent is allowed.
    fn max_turns(&self) -> usize {
        50
    }

    /// Optional model override for this subagent.
    /// Returns `None` to use the default model.
    fn model_override(&self) -> Option<&str> {
        None
    }

    /// Execute the task using the provided LLM client.
    /// Returns a SubagentResult with the outcome.
    async fn execute(
        &self,
        client: &LlmClient,
        task: &SubagentTask,
    ) -> Result<SubagentResult>;
}
