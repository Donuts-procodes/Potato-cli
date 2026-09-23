use anyhow::Result;
use async_trait::async_trait;

use crate::agents::traits::{Subagent, SubagentResult, SubagentTask, TaskCategory};
use crate::llm::{ChatMessage, LlmClient};

/// Writes Dockerfiles, CI configs, Kubernetes manifests, deployment scripts.
pub struct DevOpsAgent;

#[async_trait]
impl Subagent for DevOpsAgent {
    fn name(&self) -> &str { "DevOps" }

    fn system_prompt(&self) -> String {
        r#"You are a Senior DevOps/Platform Engineer. Your ONLY job is infrastructure and deployment.

Responsibilities:
1. Write multi-stage Dockerfiles optimized for small image size.
2. Create CI/CD pipelines (GitHub Actions, GitLab CI, Jenkins).
3. Write Kubernetes manifests (Deployment, Service, Ingress, ConfigMap).
4. Create docker-compose.yml for local development.
5. Write Terraform/Pulumi for cloud infrastructure.
6. Configure monitoring (Prometheus metrics endpoints, health checks).
7. Set up environment variable management and secrets.

Rules:
- Use multi-stage Docker builds. Final image must be minimal (alpine/distroless).
- CI must include: lint, test, build, security scan stages.
- All configs must be parameterized (no hardcoded URLs, ports, credentials).
- Include health check endpoints in every deployable service.

Output JSON with: "thought", "phase": "IMPLEMENTATION", "action": tool action."#.to_string()
    }

    fn can_handle(&self, category: &TaskCategory) -> bool { *category == TaskCategory::DevOps }
    fn max_turns(&self) -> usize { 30 }

    async fn execute(&self, client: &LlmClient, task: &SubagentTask) -> Result<SubagentResult> {
        let messages = vec![
            ChatMessage { role: "system".to_string(), content: self.system_prompt() },
            ChatMessage { role: "user".to_string(), content: format!("DevOps: {}\n\nContext:\n{}\n\nFiles: {:?}", task.description, task.context, task.relevant_files) },
        ];
        let response = client.send_turn(&messages).await?;
        Ok(SubagentResult { agent_name: self.name().to_string(), success: true, summary: response.thought, modified_files: Vec::new(), issues: Vec::new(), messages, token_usage: None })
    }
}
