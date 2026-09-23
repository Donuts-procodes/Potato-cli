use anyhow::Result;
use async_trait::async_trait;

use crate::agents::traits::{Subagent, SubagentResult, SubagentTask, TaskCategory};
use crate::llm::{ChatMessage, LlmClient};

/// Designs schemas, writes migrations, generates seed data, optimizes queries.
pub struct DatabaseAgent;

#[async_trait]
impl Subagent for DatabaseAgent {
    fn name(&self) -> &str { "Database" }

    fn system_prompt(&self) -> String {
        r#"You are a Senior Database Engineer. Your ONLY job is data persistence.

Responsibilities:
1. Design normalized database schemas (3NF minimum, denormalize only with justification).
2. Write SQL migrations (up and down) for schema changes.
3. Create ORM models/entities matching the schema.
4. Write seed data scripts for development/testing.
5. Optimize queries: add indexes, rewrite N+1 queries, use CTEs.
6. Design connection pooling and transaction strategies.

Supported databases: PostgreSQL, SQLite, MySQL, MongoDB, Redis.

Rules:
- Every migration must have a rollback (down migration).
- All queries must use parameterized statements (no string interpolation).
- Index every column used in WHERE, JOIN, or ORDER BY clauses.
- Foreign keys must have ON DELETE/UPDATE constraints.

Output JSON with: "thought", "phase": "IMPLEMENTATION", "action": tool action."#.to_string()
    }

    fn can_handle(&self, category: &TaskCategory) -> bool { *category == TaskCategory::Database }
    fn max_turns(&self) -> usize { 30 }

    async fn execute(&self, client: &LlmClient, task: &SubagentTask) -> Result<SubagentResult> {
        let messages = vec![
            ChatMessage { role: "system".to_string(), content: self.system_prompt() },
            ChatMessage { role: "user".to_string(), content: format!("Database: {}\n\nContext:\n{}\n\nFiles: {:?}", task.description, task.context, task.relevant_files) },
        ];
        let response = client.send_turn(&messages).await?;
        Ok(SubagentResult { agent_name: self.name().to_string(), success: true, summary: response.thought, modified_files: Vec::new(), issues: Vec::new(), messages, token_usage: None })
    }
}
