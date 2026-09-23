use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Immutable snapshot of the entire workspace orchestration state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentState {
    /// Objective being solved
    pub objective: String,
    /// Active project files with their current content hash
    pub manifest: HashMap<String, String>,
    /// Shared memory key-value store across agent boundaries
    pub shared_memory: HashMap<String, String>,
    /// Log of all completed tasks and their outcomes
    pub task_history: Vec<TaskRecord>,
    /// Accumulated token usage across all graph executions
    pub cumulative_tokens: u64,
    /// Number of times specific edges have been traversed (cycle prevention)
    pub edge_traversal_counts: HashMap<String, usize>,
}

impl AgentState {
    pub fn new(objective: String) -> Self {
        Self {
            objective,
            manifest: HashMap::new(),
            shared_memory: HashMap::new(),
            task_history: Vec::new(),
            cumulative_tokens: 0,
            edge_traversal_counts: HashMap::new(),
        }
    }

    /// Pure reducer function: applies a state delta deterministically.
    pub fn apply_delta(&mut self, delta: StateDelta) {
        for (path, hash) in delta.modified_files {
            self.manifest.insert(path, hash);
        }
        for (key, val) in delta.memory_updates {
            self.shared_memory.insert(key, val);
        }
        if let Some(record) = delta.completed_task {
            self.task_history.push(record);
        }
        self.cumulative_tokens += delta.tokens_used as u64;
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateDelta {
    pub modified_files: Vec<(String, String)>,
    pub memory_updates: Vec<(String, String)>,
    pub completed_task: Option<TaskRecord>,
    pub tokens_used: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskRecord {
    pub task_id: String,
    pub agent_name: String,
    pub success: bool,
    pub summary: String,
    pub timestamp_epoch_ms: u64,
}
