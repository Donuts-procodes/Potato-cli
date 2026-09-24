use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Snapshot of the local engineering environment provided to agents.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectContext {
    pub os: String,
    pub arch: String,
    pub working_dir: String,
    pub detected_toolchains: Vec<String>,
    pub git_branch: Option<String>,
    pub modified_files: Vec<String>,
    pub relevant_lessons: Vec<String>,
}

pub struct ContextAssembler {
    workspace_root: PathBuf,
}

impl ContextAssembler {
    pub fn new<P: AsRef<Path>>(workspace_root: P) -> Self {
        Self {
            workspace_root: workspace_root.as_ref().to_path_buf(),
        }
    }

    /// Assembles comprehensive real-time context from the local filesystem.
    pub fn assemble(&self) -> Result<ProjectContext> {
        let toolchains = self.detect_toolchains();
        let modified_files = self.detect_git_modified_files();
        let lessons = self.load_lessons();

        Ok(ProjectContext {
            os: std::env::consts::OS.to_string(),
            arch: std::env::consts::ARCH.to_string(),
            working_dir: self.workspace_root.display().to_string(),
            detected_toolchains: toolchains,
            git_branch: self.detect_git_branch(),
            modified_files,
            relevant_lessons: lessons,
        })
    }

    fn detect_toolchains(&self) -> Vec<String> {
        let mut tools = Vec::new();
        if self.workspace_root.join("Cargo.toml").exists() {
            tools.push("Rust (Cargo)".to_string());
        }
        if self.workspace_root.join("package.json").exists() {
            if self.workspace_root.join("pnpm-lock.yaml").exists() {
                tools.push("Node.js (pnpm)".to_string());
            } else if self.workspace_root.join("bun.lockb").exists() {
                tools.push("Bun".to_string());
            } else if self.workspace_root.join("yarn.lock").exists() {
                tools.push("Node.js (Yarn)".to_string());
            } else {
                tools.push("Node.js (npm)".to_string());
            }
        }
        if self.workspace_root.join("go.mod").exists() {
            tools.push("Go".to_string());
        }
        if self.workspace_root.join("pyproject.toml").exists()
            || self.workspace_root.join("requirements.txt").exists()
        {
            tools.push("Python".to_string());
        }
        if self.workspace_root.join("Dockerfile").exists() {
            tools.push("Docker".to_string());
        }
        tools
    }

    fn detect_git_branch(&self) -> Option<String> {
        let head_path = self.workspace_root.join(".git").join("HEAD");
        if head_path.exists() {
            if let Ok(content) = std::fs::read_to_string(head_path) {
                if let Some(branch) = content.trim().strip_prefix("ref: refs/heads/") {
                    return Some(branch.to_string());
                }
            }
        }
        None
    }

    fn detect_git_modified_files(&self) -> Vec<String> {
        let output = std::process::Command::new("git")
            .arg("status")
            .arg("--porcelain")
            .current_dir(&self.workspace_root)
            .output();

        if let Ok(out) = output {
            if out.status.success() {
                let stdout = String::from_utf8_lossy(&out.stdout);
                return stdout
                    .lines()
                    .filter_map(|line| {
                        if line.len() > 3 {
                            Some(line[3..].trim().to_string())
                        } else {
                            None
                        }
                    })
                    .collect();
            }
        }
        Vec::new()
    }

    fn load_lessons(&self) -> Vec<String> {
        let mut lessons = Vec::new();

        // 1. Check .potato/brain/LESSONS.md
        let brain_lessons = self.workspace_root.join(".potato").join("brain").join("LESSONS.md");
        if brain_lessons.exists() {
            if let Ok(content) = std::fs::read_to_string(&brain_lessons) {
                for line in content.lines() {
                    let trimmed = line.trim();
                    if trimmed.starts_with("- [") {
                        lessons.push(trimmed.to_string());
                    }
                }
            }
        }

        // 2. Check .potato/lessons/*.md
        let lessons_dir = self.workspace_root.join(".potato").join("lessons");
        if lessons_dir.exists() {
            if let Ok(entries) = std::fs::read_dir(lessons_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().and_then(|s| s.to_str()) == Some("md") {
                        if let Ok(content) = std::fs::read_to_string(&path) {
                            lessons.push(content.trim().to_string());
                        }
                    }
                }
            }
        }
        lessons
    }
}
