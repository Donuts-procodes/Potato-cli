use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

/// A single source file containing user-defined personality directives or rules.
#[derive(Debug, Clone)]
pub struct RuleEntry {
    pub source: String,
    pub path: PathBuf,
    pub content: String,
    pub is_global: bool,
}

/// Discovers, loads, and merges custom rules and personality configurations
/// from both global user directories and project-local workspace roots.
#[derive(Debug, Clone, Default)]
pub struct RulesEngine {
    entries: Vec<RuleEntry>,
}

impl RulesEngine {
    /// Loads rules using standard resolution:
    /// 1. Global rules (`~/.potato/rules.md`, `~/.potato/POTATO.md`, `~/.config/potato/rules.md`, `~/.potato/rules/*.md`)
    /// 2. Workspace rules (`POTATO.md`, `GEMINI.md`, `AGENTS.md`, `.potato/POTATO.md`, `.potato/rules/*.md`)
    pub fn load() -> Self {
        let current_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        Self::load_from(&current_dir)
    }

    /// Loads rules grounded in a specific workspace root.
    pub fn load_from<P: AsRef<Path>>(workspace_root: P) -> Self {
        let root = workspace_root.as_ref();
        let mut entries = Vec::new();

        // Layer 1: Global user rules
        Self::collect_global_rules(&mut entries);

        // Layer 2: Project-local workspace rules
        Self::collect_workspace_rules(root, &mut entries);

        Self { entries }
    }

    fn collect_global_rules(entries: &mut Vec<RuleEntry>) {
        let mut global_candidates = Vec::new();

        if let Some(home) = dirs::home_dir() {
            global_candidates.push(home.join(".potato").join("rules.md"));
            global_candidates.push(home.join(".potato").join("POTATO.md"));

            let global_rules_dir = home.join(".potato").join("rules");
            if global_rules_dir.is_dir() {
                if let Ok(dir_entries) = fs::read_dir(global_rules_dir) {
                    for entry in dir_entries.flatten() {
                        let path = entry.path();
                        if path.extension().and_then(|s| s.to_str()) == Some("md") {
                            global_candidates.push(path);
                        }
                    }
                }
            }
        }

        if let Some(config_dir) = dirs::config_dir() {
            global_candidates.push(config_dir.join("potato").join("rules.md"));
            global_candidates.push(config_dir.join("potato").join("POTATO.md"));
        }

        for path in global_candidates {
            if path.is_file() {
                if let Ok(content) = fs::read_to_string(&path) {
                    let trimmed = content.trim();
                    if !trimmed.is_empty() {
                        let source_name = path
                            .file_name()
                            .map(|s| s.to_string_lossy().to_string())
                            .unwrap_or_else(|| "global_rules.md".to_string());

                        entries.push(RuleEntry {
                            source: format!("~/.potato/{}", source_name),
                            path,
                            content: trimmed.to_string(),
                            is_global: true,
                        });
                    }
                }
            }
        }
    }

    fn collect_workspace_rules(root: &Path, entries: &mut Vec<RuleEntry>) {
        let candidates = [
            ("POTATO.md", root.join("POTATO.md")),
            ("GEMINI.md", root.join("GEMINI.md")),
            ("AGENTS.md", root.join("AGENTS.md")),
            (".potato/POTATO.md", root.join(".potato").join("POTATO.md")),
            (".potato/rules.md", root.join(".potato").join("rules.md")),
        ];

        for (label, path) in candidates {
            if path.is_file() {
                if let Ok(content) = fs::read_to_string(&path) {
                    let trimmed = content.trim();
                    if !trimmed.is_empty() {
                        entries.push(RuleEntry {
                            source: label.to_string(),
                            path,
                            content: trimmed.to_string(),
                            is_global: false,
                        });
                    }
                }
            }
        }

        // Check `.potato/rules/*.md` directory
        let rules_dir = root.join(".potato").join("rules");
        if rules_dir.is_dir() {
            if let Ok(dir_entries) = fs::read_dir(rules_dir) {
                for entry in dir_entries.flatten() {
                    let path = entry.path();
                    if path.extension().and_then(|s| s.to_str()) == Some("md") {
                        if let Ok(content) = fs::read_to_string(&path) {
                            let trimmed = content.trim();
                            if !trimmed.is_empty() {
                                let name = format!(
                                    ".potato/rules/{}",
                                    path.file_name()
                                        .map(|s| s.to_string_lossy().to_string())
                                        .unwrap_or_default()
                                );
                                entries.push(RuleEntry {
                                    source: name,
                                    path,
                                    content: trimmed.to_string(),
                                    is_global: false,
                                });
                            }
                        }
                    }
                }
            }
        }
    }

    /// Returns a reference to all loaded rule sources.
    pub fn entries(&self) -> &[RuleEntry] {
        &self.entries
    }

    /// Whether any rules or personality files were found.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Number of active rule files discovered.
    pub fn source_count(&self) -> usize {
        self.entries.len()
    }

    /// Formats all discovered rules into an authoritative, high-priority markdown prompt block.
    pub fn formatted_prompt_block(&self) -> String {
        if self.entries.is_empty() {
            return String::new();
        }

        let mut block = String::with_capacity(4096);
        block.push_str("\n\n## USER PERSONALITY & OPERATIONAL DIRECTIVES (MANDATORY RULES)\n");
        block.push_str("The user has established the following project and global rules. You MUST strictly comply with every directive below without exception:\n\n");

        for entry in &self.entries {
            block.push_str(&format!("### [Source: {}]\n", entry.source));
            block.push_str(&entry.content);
            block.push_str("\n\n");
        }

        block
    }

    /// Formatted one-line summary for banners and status reporting.
    pub fn summary(&self) -> String {
        if self.entries.is_empty() {
            "None active (create POTATO.md or use /rules init)".to_string()
        } else {
            let sources: Vec<String> = self.entries.iter().map(|e| e.source.clone()).collect();
            format!("{} active sources ({})", self.entries.len(), sources.join(", "))
        }
    }

    /// Scaffolds a starter `POTATO.md` rule manifest in the specified directory.
    pub fn init_starter_rules<P: AsRef<Path>>(root: P) -> Result<PathBuf> {
        let path = root.as_ref().join("POTATO.md");
        if path.exists() {
            return Ok(path);
        }

        let content = r#"# 🥔 Potato CLI — Custom Rules & Personality

## 🎭 Role & Persona
- You are an elite Principal Software Architect and Staff Engineer.
- You maintain strict separation of concerns, zero monolithic files, and high cohesion.
- All code must be production-ready with strict typing and complete implementations.

## 🛠️ Code Quality Directives
- **Zero Placeholders:** Never generate `TODO`, `pass`, or mock comments in production code.
- **Surgical Mutations:** Always prefer atomic patches and surgical edits over full-file rewrites.
- **Error Handling:** Return structured `Result` types; never use `.unwrap()` or unchecked panics in production.
- **Modularity:** Touch at most 1–2 tightly coupled files per implementation step.

## 🔒 Verification & Safety
- Run relevant unit or integration tests after every modification.
- Ensure all exit codes return 0 before declaring a task complete.
"#;
        fs::write(&path, content).with_context(|| format!("Failed to create {}", path.display()))?;
        Ok(path)
    }

    /// Appends a new directive directly to the project's `POTATO.md`.
    pub fn append_rule<P: AsRef<Path>>(root: P, rule_text: &str) -> Result<PathBuf> {
        let path = root.as_ref().join("POTATO.md");
        let rule_line = if rule_text.trim().starts_with('-') || rule_text.trim().starts_with('*') {
            format!("\n{}\n", rule_text.trim())
        } else {
            format!("\n- {}\n", rule_text.trim())
        };

        if path.exists() {
            let mut existing = fs::read_to_string(&path)?;
            existing.push_str(&rule_line);
            fs::write(&path, existing)?;
        } else {
            let header = format!("# 🥔 Potato CLI — Custom Rules & Personality\n\n## Custom Directives{}\n", rule_line);
            fs::write(&path, header)?;
        }

        Ok(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init_starter_rules_creates_file() {
        let temp_dir = std::env::temp_dir().join(format!("potato_test_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        let _ = fs::create_dir_all(&temp_dir);

        let created_path = RulesEngine::init_starter_rules(&temp_dir).unwrap();
        assert!(created_path.exists());
        let content = fs::read_to_string(&created_path).unwrap();
        assert!(content.contains("Role & Persona"));

        // Appending rule
        let _ = RulesEngine::append_rule(&temp_dir, "Always format with cargo fmt").unwrap();
        let updated = fs::read_to_string(&created_path).unwrap();
        assert!(updated.contains("Always format with cargo fmt"));

        // Test loading
        let engine = RulesEngine::load_from(&temp_dir);
        assert!(!engine.is_empty());
        let prompt = engine.formatted_prompt_block();
        assert!(prompt.contains("USER PERSONALITY & OPERATIONAL DIRECTIVES"));
        assert!(prompt.contains("POTATO.md"));

        let _ = fs::remove_dir_all(&temp_dir);
    }
}
