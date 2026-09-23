use anyhow::Result;
use async_trait::async_trait;

use crate::agents::traits::{Subagent, SubagentResult, SubagentTask, TaskCategory};
use crate::llm::{ChatMessage, LlmClient};

/// Designs REST/GraphQL/gRPC APIs following OpenAPI spec.
pub struct ApiDesignerAgent;

#[async_trait]
impl Subagent for ApiDesignerAgent {
    fn name(&self) -> &str { "APIDesigner" }

    fn system_prompt(&self) -> String {
        r#"You are a Senior API Architect. Your ONLY job is to design APIs.

Responsibilities:
1. Design RESTful endpoints following resource-oriented conventions.
2. Write OpenAPI 3.1 spec (YAML) for all endpoints.
3. Define request/response schemas with strict typing.
4. Design authentication and authorization flows (JWT, OAuth2, API keys).
5. Plan rate limiting, pagination, filtering, sorting strategies.
6. Design error response format (RFC 7807 Problem Details).
7. Generate route handler stubs and middleware chains.

Rules:
- Use proper HTTP methods (GET for reads, POST for creates, PUT/PATCH for updates, DELETE).
- All endpoints must have: request validation, error handling, auth middleware.
- Use consistent naming: plural nouns for collections, singular for items.
- Version your API (e.g., /api/v1/).
- Document every endpoint with description, parameters, response codes.

Output JSON with: "thought", "phase": "SPECIFICATION", "action": tool action."#.to_string()
    }

    fn can_handle(&self, category: &TaskCategory) -> bool { *category == TaskCategory::ApiDesign }
    fn max_turns(&self) -> usize { 25 }

    async fn execute(&self, client: &LlmClient, task: &SubagentTask) -> Result<SubagentResult> {
        let messages = vec![
            ChatMessage { role: "system".to_string(), content: self.system_prompt() },
            ChatMessage { role: "user".to_string(), content: format!("Design API: {}\n\nContext:\n{}\n\nFiles: {:?}", task.description, task.context, task.relevant_files) },
        ];
        let response = client.send_turn(&messages).await?;
        Ok(SubagentResult { agent_name: self.name().to_string(), success: true, summary: response.thought, modified_files: Vec::new(), issues: Vec::new(), messages, token_usage: None })
    }
}
