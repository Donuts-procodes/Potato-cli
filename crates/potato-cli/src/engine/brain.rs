use anyhow::Result;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use tracing::info;

use crate::llm::ChatMessage;

/// Metadata for files tracked by the brain.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackedFile {
    pub path: String,
    pub last_action: String,
    pub last_updated: String,
    pub touch_count: usize,
}

/// Statistics and health metrics for the persistent Brain.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BrainStats {
    pub knowledge_entries: usize,
    pub lessons_count: usize,
    pub tracked_files_count: usize,
    pub brain_dir: String,
    pub last_updated: String,
}

/// Persistent cognitive repository for project knowledge, architectural decisions,
/// learned lessons, and mutated file registries.
#[derive(Clone)]
pub struct Brain {
    brain_dir: PathBuf,
}

impl Brain {
    /// Initializes the Brain at `.potato/brain` or provided custom directory.
    pub fn new(custom_dir: Option<PathBuf>) -> Self {
        let dir = custom_dir.unwrap_or_else(get_default_brain_dir);
        let _ = std::fs::create_dir_all(&dir);
        Self { brain_dir: dir }
    }

    /// Automatically updates all Brain knowledge artifacts following a Super Loop run.
    pub fn auto_update(
        &self,
        objective: &str,
        messages: &[ChatMessage],
        summary: &str,
        success: bool,
    ) -> Result<()> {
        let _ = std::fs::create_dir_all(&self.brain_dir);

        // 1. Update MUTATED_FILES.json
        self.update_mutated_files(messages)?;

        // 2. Extract decisions & update KNOWLEDGE.md
        self.update_knowledge(objective, messages, summary, success)?;

        // 3. Extract lessons from failures / gatekeeper denials & update LESSONS.md
        self.update_lessons(messages)?;

        // 4. Update task.md
        self.update_task_status(objective, summary, success)?;

        info!(dir = %self.brain_dir.display(), "Project Brain auto-updated successfully");
        Ok(())
    }

    /// Assembles a dense context block of accumulated Brain knowledge for injection into prompts.
    pub fn assemble_context(&self) -> String {
        let mut out = String::new();
        out.push_str("## PROJECT BRAIN (PERSISTENT ACCUMULATED KNOWLEDGE)\n");

        // Load Knowledge
        let knowledge_path = self.brain_dir.join("KNOWLEDGE.md");
        if knowledge_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&knowledge_path) {
                let trimmed = content.trim();
                if !trimmed.is_empty() {
                    out.push_str("### Architecture & Project Conventions\n");
                    for line in trimmed.lines().take(15) {
                        out.push_str(&format!("{}\n", line));
                    }
                    out.push('\n');
                }
            }
        }

        // Load Lessons
        let lessons_path = self.brain_dir.join("LESSONS.md");
        if lessons_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&lessons_path) {
                let trimmed = content.trim();
                if !trimmed.is_empty() {
                    out.push_str("### Learned Lessons & Best Practices\n");
                    for line in trimmed.lines().take(12) {
                        out.push_str(&format!("{}\n", line));
                    }
                    out.push('\n');
                }
            }
        }

        // Load Mutated Files
        let files_path = self.brain_dir.join("MUTATED_FILES.json");
        if files_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&files_path) {
                if let Ok(map) = serde_json::from_str::<HashMap<String, TrackedFile>>(&content) {
                    if !map.is_empty() {
                        out.push_str("### Known Modified Project Files\n");
                        let mut paths: Vec<_> = map.keys().collect();
                        paths.sort();
                        for path in paths.iter().take(10) {
                            out.push_str(&format!("- {}\n", path));
                        }
                        out.push('\n');
                    }
                }
            }
        }

        out
    }

    fn update_mutated_files(&self, messages: &[ChatMessage]) -> Result<()> {
        let files_path = self.brain_dir.join("MUTATED_FILES.json");
        let mut tracked: HashMap<String, TrackedFile> = if files_path.exists() {
            std::fs::read_to_string(&files_path)
                .ok()
                .and_then(|c| serde_json::from_str(&c).ok())
                .unwrap_or_default()
        } else {
            HashMap::new()
        };

        for msg in messages {
            if msg.role == "assistant" {
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&msg.content) {
                    if let Some(action) = val.get("action") {
                        let action_type = action.get("type").and_then(|t| t.as_str()).unwrap_or("unknown");
                        if let Some(path) = action.get("path").and_then(|p| p.as_str()) {
                            let entry = tracked.entry(path.to_string()).or_insert_with(|| TrackedFile {
                                path: path.to_string(),
                                last_action: action_type.to_string(),
                                last_updated: Utc::now().to_rfc3339(),
                                touch_count: 0,
                            });
                            entry.last_action = action_type.to_string();
                            entry.last_updated = Utc::now().to_rfc3339();
                            entry.touch_count += 1;
                        }
                    }
                }
            }
        }

        let content = serde_json::to_string_pretty(&tracked)?;
        std::fs::write(&files_path, content)?;
        Ok(())
    }

    fn update_knowledge(
        &self,
        objective: &str,
        messages: &[ChatMessage],
        summary: &str,
        success: bool,
    ) -> Result<()> {
        let path = self.brain_dir.join("KNOWLEDGE.md");
        let mut content = if path.exists() {
            std::fs::read_to_string(&path).unwrap_or_default()
        } else {
            String::from("# Project Architectural Knowledge & Conventions\n\n")
        };

        let now = Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
        content.push_str(&format!(
            "## [{}] Objective: {}\n- **Status**: {}\n",
            now,
            objective,
            if success { "Success" } else { "Failed" }
        ));

        if !summary.trim().is_empty() {
            content.push_str(&format!("- **Summary**: {}\n", summary.trim()));
        }

        // Extract key decisions from assistant thoughts
        let mut decisions = Vec::new();
        for msg in messages {
            if msg.role == "assistant" {
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&msg.content) {
                    if let Some(thought) = val.get("thought").and_then(|t| t.as_str()) {
                        let trimmed = thought.trim();
                        if trimmed.len() > 20 && !decisions.contains(&trimmed.to_string()) {
                            decisions.push(trimmed.to_string());
                        }
                    }
                }
            }
        }

        if !decisions.is_empty() {
            content.push_str("- **Decisions**:\n");
            for d in decisions.iter().take(5) {
                let snippet = if d.len() > 140 { format!("{}…", &d[..140]) } else { d.clone() };
                content.push_str(&format!("  * {}\n", snippet));
            }
        }
        content.push('\n');

        std::fs::write(&path, content)?;
        Ok(())
    }

    fn update_lessons(&self, messages: &[ChatMessage]) -> Result<()> {
        let mut new_lessons = Vec::new();

        for msg in messages {
            if msg.role == "user" {
                if msg.content.contains("AST SYNTAX ERROR") {
                    new_lessons.push("Ensure balanced delimiters and valid syntax before calling write_file.".to_string());
                } else if msg.content.contains("consecutive action failures") {
                    new_lessons.push("Avoid repeating identical failing tool commands; verify filesystem state first.".to_string());
                } else if msg.content.contains("BLOCKED") {
                    new_lessons.push("Do not attempt to execute blocked dangerous shell commands or violate tool policies.".to_string());
                }
            }
        }

        if new_lessons.is_empty() {
            return Ok(());
        }

        let path = self.brain_dir.join("LESSONS.md");
        let mut content = if path.exists() {
            std::fs::read_to_string(&path).unwrap_or_default()
        } else {
            String::from("# Learned Engineering Lessons & Anti-Patterns\n\n")
        };

        let now = Utc::now().format("%Y-%m-%d").to_string();
        for lesson in new_lessons {
            let entry = format!("- [{}] {}\n", now, lesson);
            if !content.contains(&lesson) {
                content.push_str(&entry);
            }
        }

        std::fs::write(&path, content)?;
        Ok(())
    }

    fn update_task_status(&self, objective: &str, summary: &str, success: bool) -> Result<()> {
        let path = self.brain_dir.join("task.md");
        let now = Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();

        let content = format!(
            "# Active Task Context\n\n- **Last Updated**: {}\n- **Objective**: {}\n- **Outcome**: {}\n- **Summary**:\n{}\n",
            now,
            objective,
            if success { "COMPLETED" } else { "FAILED" },
            if summary.is_empty() { "No summary provided." } else { summary }
        );

        std::fs::write(&path, content)?;
        Ok(())
    }

    /// Returns statistics regarding the knowledge base size.
    pub fn stats(&self) -> BrainStats {
        let knowledge_entries = self
            .brain_dir
            .join("KNOWLEDGE.md")
            .metadata()
            .map(|m| (m.len() / 100) as usize)
            .unwrap_or(0);

        let lessons_path = self.brain_dir.join("LESSONS.md");
        let lessons_count = if lessons_path.exists() {
            std::fs::read_to_string(lessons_path)
                .map(|c| c.lines().filter(|l| l.starts_with("- [")).count())
                .unwrap_or(0)
        } else {
            0
        };

        let files_path = self.brain_dir.join("MUTATED_FILES.json");
        let tracked_files_count = if files_path.exists() {
            std::fs::read_to_string(files_path)
                .ok()
                .and_then(|c| serde_json::from_str::<HashMap<String, TrackedFile>>(&c).ok())
                .map(|m| m.len())
                .unwrap_or(0)
        } else {
            0
        };

        BrainStats {
            knowledge_entries,
            lessons_count,
            tracked_files_count,
            brain_dir: self.brain_dir.display().to_string(),
            last_updated: Utc::now().to_rfc3339(),
        }
    }
}

/// Resolves default brain directory (.potato/brain or global user fallback).
pub fn get_default_brain_dir() -> PathBuf {
    let local = PathBuf::from(".potato").join("brain");
    if local.exists() || std::fs::create_dir_all(&local).is_ok() {
        return local;
    }

    if let Some(config_dir) = dirs::config_dir() {
        let global = config_dir.join("potato").join("brain");
        let _ = std::fs::create_dir_all(&global);
        return global;
    }

    local
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_brain_auto_update_and_assemble() {
        let dir = tempdir().unwrap();
        let brain = Brain::new(Some(dir.path().to_path_buf()));

        let msgs = vec![
            ChatMessage {
                role: "assistant".to_string(),
                content: r#"{"thought": "Decided to use Actix-web for high throughput", "action": {"type": "write_file", "path": "src/main.rs"}}"#.to_string(),
            },
            ChatMessage {
                role: "user".to_string(),
                content: "PRE-WRITE AST SYNTAX ERROR in src/main.rs".to_string(),
            },
        ];

        brain.auto_update("Build HTTP API", &msgs, "Scaffolded Actix server", true).unwrap();

        let ctx = brain.assemble_context();
        assert!(ctx.contains("PROJECT BRAIN"));
        assert!(ctx.contains("src/main.rs"));

        let stats = brain.stats();
        assert_eq!(stats.tracked_files_count, 1);
        assert!(stats.lessons_count >= 1);
    }
}
