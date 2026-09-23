use anyhow::{Context, Result};
use std::path::PathBuf;
use tracing::info;

/// Loads and concatenates prompt template fragments from `.potato/prompts/`.
/// Templates are referenced by name (without extension) in config.
pub struct PromptTemplateLoader {
    search_dirs: Vec<PathBuf>,
}

impl PromptTemplateLoader {
    /// Creates a loader that searches the given directories for `.md` prompt files.
    pub fn new(search_dirs: Vec<PathBuf>) -> Self {
        Self { search_dirs }
    }

    /// Default search paths: `./.potato/prompts/` and `~/.config/potato/prompts/`.
    pub fn default_paths() -> Self {
        let mut dirs = vec![PathBuf::from(".potato/prompts")];
        if let Some(config_dir) = dirs::config_dir() {
            dirs.push(config_dir.join("potato").join("prompts"));
        }
        Self::new(dirs)
    }

    /// Resolves a template name to its file content.
    /// Searches all directories in order, returns first match.
    pub fn load(&self, template_name: &str) -> Result<String> {
        let filename = if template_name.ends_with(".md") {
            template_name.to_string()
        } else {
            format!("{}.md", template_name)
        };

        for dir in &self.search_dirs {
            let path = dir.join(&filename);
            if path.exists() {
                let content = std::fs::read_to_string(&path)
                    .with_context(|| format!("Failed to read prompt template: {}", path.display()))?;
                info!(
                    template = template_name,
                    path = %path.display(),
                    "Loaded prompt template"
                );
                return Ok(content);
            }
        }

        Err(anyhow::anyhow!(
            "Prompt template '{}' not found in search paths: {:?}",
            template_name,
            self.search_dirs
        ))
    }

    /// Processes an `@include(name1, name2)` directive in a string.
    /// Replaces the directive with concatenated template contents.
    pub fn resolve_includes(&self, text: &str) -> Result<String> {
        let mut result = text.to_string();

        while let Some(start) = result.find("@include(") {
            let end = result[start..]
                .find(')')
                .ok_or_else(|| anyhow::anyhow!("Unclosed @include( directive"))?
                + start;

            let directive = &result[start + 9..end]; // skip "@include("
            let names: Vec<&str> = directive.split(',').map(|s| s.trim()).collect();

            let mut replacement = String::new();
            for name in names {
                let content = self.load(name)?;
                replacement.push_str(&content);
                replacement.push('\n');
            }

            result.replace_range(start..=end, &replacement);
        }

        Ok(result)
    }

    /// Lists all available prompt templates across search directories.
    pub fn list_available(&self) -> Vec<String> {
        let mut templates = Vec::new();
        for dir in &self.search_dirs {
            if let Ok(entries) = std::fs::read_dir(dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().map(|e| e == "md").unwrap_or(false) {
                        if let Some(stem) = path.file_stem() {
                            templates.push(stem.to_string_lossy().to_string());
                        }
                    }
                }
            }
        }
        templates.sort();
        templates.dedup();
        templates
    }
}
