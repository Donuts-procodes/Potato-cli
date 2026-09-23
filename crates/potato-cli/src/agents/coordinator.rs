use anyhow::Result;
use colored::Colorize;
use tracing::{info, warn};

use crate::agents::traits::{Subagent, SubagentResult, SubagentTask, TaskCategory};
use crate::agents::{
    architect::ArchitectAgent,
    implementer::ImplementerAgent,
    repair::RepairAgent,
    reviewer::ReviewerAgent,
    security::SecurityAuditorAgent,
    test_writer::TestWriterAgent,
};
use crate::llm::LlmClient;

/// The Coordinator is the root orchestrator that decomposes objectives into
/// subtasks and routes them to the appropriate specialist subagent.
pub struct Coordinator {
    agents: Vec<Box<dyn Subagent>>,
}

impl Coordinator {
    /// Creates a coordinator with all built-in specialist subagents.
    pub fn new() -> Self {
        let agents: Vec<Box<dyn Subagent>> = vec![
            Box::new(ArchitectAgent),
            Box::new(ImplementerAgent),
            Box::new(ReviewerAgent),
            Box::new(TestWriterAgent),
            Box::new(RepairAgent),
            Box::new(SecurityAuditorAgent),
        ];

        info!(
            agent_count = agents.len(),
            agents = ?agents.iter().map(|a| a.name()).collect::<Vec<_>>(),
            "Coordinator initialized with specialist subagents"
        );

        Self { agents }
    }

    /// Finds the first subagent capable of handling the given task category.
    pub fn route(&self, category: &TaskCategory) -> Option<&dyn Subagent> {
        self.agents
            .iter()
            .find(|agent| agent.can_handle(category))
            .map(|boxed| boxed.as_ref())
    }

    /// Dispatches a task to the appropriate subagent and returns the result.
    pub async fn dispatch(
        &self,
        client: &LlmClient,
        task: &SubagentTask,
    ) -> Result<SubagentResult> {
        let agent = self
            .route(&task.category)
            .ok_or_else(|| anyhow::anyhow!("No subagent found for category {:?}", task.category))?;

        println!(
            "\n{} {} → {}",
            "🤖 Routing:".bold().blue(),
            format!("{:?}", task.category).cyan(),
            agent.name().bold().green()
        );
        println!(
            "   {} {}",
            "Task:".dimmed(),
            task.description
        );

        let result = agent.execute(client, task).await?;

        if result.success {
            println!(
                "   {} {} completed — {}",
                "✓".green(),
                agent.name().bold(),
                result.summary
            );
        } else {
            println!(
                "   {} {} failed — {}",
                "✗".red(),
                agent.name().bold(),
                result.summary
            );
        }

        if !result.issues.is_empty() {
            println!(
                "   {} {} issues found",
                "⚠".yellow(),
                result.issues.len()
            );
        }

        Ok(result)
    }

    /// Executes a sequential pipeline: Architect → Implementer → Reviewer → TestWriter.
    /// If Reviewer finds critical issues, re-routes to Implementer.
    /// At the end, runs SecurityAuditor.
    pub async fn run_pipeline(
        &self,
        client: &LlmClient,
        tasks: Vec<SubagentTask>,
    ) -> Result<Vec<SubagentResult>> {
        let mut results = Vec::new();

        for task in &tasks {
            let result = self.dispatch(client, task).await?;

            // If this was an implementation task, automatically queue a review
            if task.category == TaskCategory::Implementation && result.success {
                let review_task = SubagentTask {
                    id: format!("{}-review", task.id),
                    description: format!("Review the implementation of: {}", task.description),
                    category: TaskCategory::Review,
                    context: result.summary.clone(),
                    relevant_files: result.modified_files.clone(),
                };

                let review_result = self.dispatch(client, &review_task).await?;

                // If review found critical issues, route back to repair
                let has_critical = review_result
                    .issues
                    .iter()
                    .any(|i| i.severity == crate::agents::traits::IssueSeverity::Critical);

                if has_critical {
                    warn!("Critical issues found in review — routing to repair agent");
                    let repair_task = SubagentTask {
                        id: format!("{}-repair", task.id),
                        description: format!("Fix critical issues found in review of: {}", task.description),
                        category: TaskCategory::Repair,
                        context: serde_json::to_string(&review_result.issues)?,
                        relevant_files: review_result.modified_files.clone(),
                    };
                    let repair_result = self.dispatch(client, &repair_task).await?;
                    results.push(repair_result);
                }

                results.push(review_result);
            }

            results.push(result);
        }

        Ok(results)
    }

    /// Returns a list of all registered subagent names.
    pub fn agent_names(&self) -> Vec<&str> {
        self.agents.iter().map(|a| a.name()).collect()
    }
}
