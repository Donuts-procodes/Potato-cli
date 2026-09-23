use anyhow::Result;
use async_trait::async_trait;

use crate::agents::traits::{Subagent, SubagentResult, SubagentTask, TaskCategory};
use crate::llm::{ChatMessage, LlmClient};

/// Runs benchmarks, identifies hot paths, suggests optimizations.
pub struct PerformanceProfilerAgent;

#[async_trait]
impl Subagent for PerformanceProfilerAgent {
    fn name(&self) -> &str { "PerformanceProfiler" }

    fn system_prompt(&self) -> String {
        r#"You are a Senior Performance Engineer. Your ONLY job is to optimize runtime performance.

Responsibilities:
1. Identify algorithmic complexity issues (O(n²) where O(n) is possible).
2. Find unnecessary allocations, clones, and copies.
3. Suggest caching strategies for repeated computations.
4. Optimize data structure choices (HashMap vs BTreeMap, Vec vs VecDeque).
5. Identify I/O bottlenecks (synchronous where async would help).
6. Run benchmarks if available (cargo bench, pytest-benchmark).

Rules:
- Profile before optimizing. Read the code, then propose targeted fixes.
- Prefer algorithmic improvements over micro-optimizations.
- Every suggestion must include expected impact (e.g., "reduces O(n²) to O(n log n)").
- Do not sacrifice readability for marginal gains.

Output JSON with: "thought", "phase": "VERIFY", "action": tool action."#.to_string()
    }

    fn can_handle(&self, category: &TaskCategory) -> bool { *category == TaskCategory::Performance }
    fn max_turns(&self) -> usize { 20 }

    async fn execute(&self, client: &LlmClient, task: &SubagentTask) -> Result<SubagentResult> {
        let messages = vec![
            ChatMessage { role: "system".to_string(), content: self.system_prompt() },
            ChatMessage { role: "user".to_string(), content: format!("Profile: {}\n\nContext:\n{}\n\nFiles: {:?}", task.description, task.context, task.relevant_files) },
        ];
        let response = client.send_turn(&messages).await?;
        Ok(SubagentResult { agent_name: self.name().to_string(), success: true, summary: response.thought, modified_files: Vec::new(), issues: Vec::new(), messages, token_usage: None })
    }
}
