use serde::{Deserialize, Serialize};
use std::path::Path;
use anyhow::{Context, Result};
use tracing::info;

/// A single atomic task in the implementation roadmap DAG.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoadmapTask {
    pub id: String,
    pub description: String,
    pub status: TaskStatus,
    /// Task IDs that must complete before this one.
    pub depends_on: Vec<String>,
    /// Files this task touches (max 1-2 per spec).
    pub files: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Pending,
    InProgress,
    Complete,
    Failed,
    Skipped,
}

/// The full roadmap: an ordered DAG of implementation tasks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Roadmap {
    pub tasks: Vec<RoadmapTask>,
}

impl Roadmap {
    /// Loads a roadmap from a `ROADMAP.json` file.
    pub fn from_file(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read roadmap: {}", path.display()))?;
        let roadmap: Self = serde_json::from_str(&content)
            .with_context(|| format!("Failed to parse roadmap: {}", path.display()))?;
        Ok(roadmap)
    }

    /// Saves the roadmap back to disk (for status updates).
    pub fn save(&self, path: &Path) -> Result<()> {
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(path, content)
            .with_context(|| format!("Failed to write roadmap: {}", path.display()))?;
        Ok(())
    }

    /// Returns the next task whose dependencies are all complete.
    pub fn next_ready_task(&self) -> Option<&RoadmapTask> {
        self.tasks.iter().find(|task| {
            task.status == TaskStatus::Pending
                && task.depends_on.iter().all(|dep_id| {
                    self.tasks
                        .iter()
                        .any(|t| t.id == *dep_id && t.status == TaskStatus::Complete)
                })
        })
    }

    /// Marks a task as in-progress.
    pub fn start_task(&mut self, task_id: &str) {
        if let Some(task) = self.tasks.iter_mut().find(|t| t.id == task_id) {
            task.status = TaskStatus::InProgress;
            info!(task_id, "Roadmap task started");
        }
    }

    /// Marks a task as complete.
    pub fn complete_task(&mut self, task_id: &str) {
        if let Some(task) = self.tasks.iter_mut().find(|t| t.id == task_id) {
            task.status = TaskStatus::Complete;
            info!(task_id, "Roadmap task completed");
        }
    }

    /// Marks a task as failed.
    pub fn fail_task(&mut self, task_id: &str) {
        if let Some(task) = self.tasks.iter_mut().find(|t| t.id == task_id) {
            task.status = TaskStatus::Failed;
            info!(task_id, "Roadmap task failed");
        }
    }

    /// Returns progress as (completed, total).
    pub fn progress(&self) -> (usize, usize) {
        let completed = self.tasks.iter().filter(|t| t.status == TaskStatus::Complete).count();
        (completed, self.tasks.len())
    }

    /// Returns a formatted progress bar string.
    pub fn progress_bar(&self) -> String {
        let (done, total) = self.progress();
        if total == 0 {
            return "[no tasks]".to_string();
        }
        let pct = (done as f64 / total as f64 * 100.0) as usize;
        let filled = (done as f64 / total as f64 * 30.0) as usize;
        let empty = 30 - filled;
        format!(
            "[{}{}] {}/{} ({}%)",
            "█".repeat(filled),
            "░".repeat(empty),
            done,
            total,
            pct
        )
    }

    /// Returns `true` when all tasks are complete.
    pub fn is_finished(&self) -> bool {
        self.tasks.iter().all(|t| t.status == TaskStatus::Complete || t.status == TaskStatus::Skipped)
    }
}
