use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tracing::info;

/// Top-level configuration for potato-cli.
/// Loaded from (in priority order):
/// 1. CLI flags (highest)
/// 2. Environment variables
/// 3. `./potato.toml` (project-local)
/// 4. `~/.config/potato/config.toml` (global)
/// 5. Compiled defaults (lowest)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PotatoConfig {
    pub llm: LlmConfig,
    pub agent: AgentConfig,
    pub sandbox: SandboxConfig,
    pub session: SessionConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LlmConfig {
    /// OpenAI-compatible API key
    pub api_key: Option<String>,
    /// API base URL
    pub api_base: String,
    /// Model identifier
    pub model: String,
    /// Temperature for LLM calls
    pub temperature: f32,
    /// Request timeout in seconds
    pub timeout_seconds: u64,
    /// Max retry attempts on failure
    pub max_retries: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AgentConfig {
    /// Maximum ReAct turns before aborting
    pub max_turns: usize,
    /// Maximum consecutive failures before anti-oscillation triggers
    pub max_consecutive_failures: usize,
    /// Context window message limit before trimming
    pub context_window_limit: usize,
    /// Number of messages to keep after trimming
    pub context_keep_count: usize,
    /// Custom instructions prepended to system prompt
    pub custom_instructions: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SandboxConfig {
    /// Enable sandbox path isolation
    pub enabled: bool,
    /// Allowed directories outside the sandbox root (escape hatches)
    pub allowed_paths: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SessionConfig {
    /// Enable session persistence to disk
    pub persist: bool,
    /// Directory for session checkpoint files
    pub session_dir: String,
}

// --- Defaults ---

impl Default for PotatoConfig {
    fn default() -> Self {
        Self {
            llm: LlmConfig::default(),
            agent: AgentConfig::default(),
            sandbox: SandboxConfig::default(),
            session: SessionConfig::default(),
        }
    }
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            api_key: None,
            api_base: "https://api.openai.com/v1".to_string(),
            model: "gpt-4o".to_string(),
            temperature: 0.1,
            timeout_seconds: 180,
            max_retries: 3,
        }
    }
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            max_turns: 200,
            max_consecutive_failures: 5,
            context_window_limit: 80,
            context_keep_count: 40,
            custom_instructions: None,
        }
    }
}

impl Default for SandboxConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            allowed_paths: Vec::new(),
        }
    }
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            persist: false,
            session_dir: ".potato/sessions".to_string(),
        }
    }
}

// --- Loading ---

impl PotatoConfig {
    /// Loads config with layered resolution:
    /// global config → project config → env vars (env vars win)
    pub fn load() -> Result<Self> {
        let mut config = Self::default();

        // Layer 1: Global config (~/.config/potato/config.toml)
        if let Some(global_path) = global_config_path() {
            if global_path.exists() {
                let global = Self::from_file(&global_path)?;
                config = config.merge(global);
                info!(path = %global_path.display(), "Loaded global config");
            }
        }

        // Layer 2: Project-local config (./potato.toml)
        let local_path = PathBuf::from("potato.toml");
        if local_path.exists() {
            let local = Self::from_file(&local_path)?;
            config = config.merge(local);
            info!("Loaded project config from potato.toml");
        }

        // Layer 3: Environment variable overrides
        config.apply_env_overrides();

        Ok(config)
    }

    fn from_file(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read config file: {}", path.display()))?;
        let parsed: Self = toml::from_str(&content)
            .with_context(|| format!("Failed to parse config file: {}", path.display()))?;
        Ok(parsed)
    }

    /// Merges `other` into `self`. Non-default values in `other` override values in `self`.
    fn merge(mut self, other: Self) -> Self {
        if other.llm.api_key.is_some() {
            self.llm.api_key = other.llm.api_key;
        }
        if other.llm.api_base != LlmConfig::default().api_base {
            self.llm.api_base = other.llm.api_base;
        }
        if other.llm.model != LlmConfig::default().model {
            self.llm.model = other.llm.model;
        }
        if other.llm.temperature != LlmConfig::default().temperature {
            self.llm.temperature = other.llm.temperature;
        }
        if other.agent.max_turns != AgentConfig::default().max_turns {
            self.agent.max_turns = other.agent.max_turns;
        }
        if other.agent.custom_instructions.is_some() {
            self.agent.custom_instructions = other.agent.custom_instructions;
        }
        if other.sandbox.enabled != SandboxConfig::default().enabled {
            self.sandbox.enabled = other.sandbox.enabled;
        }
        if !other.sandbox.allowed_paths.is_empty() {
            self.sandbox.allowed_paths = other.sandbox.allowed_paths;
        }
        if other.session.persist != SessionConfig::default().persist {
            self.session.persist = other.session.persist;
        }
        self
    }

    fn apply_env_overrides(&mut self) {
        if let Ok(key) = std::env::var("OPENAI_API_KEY").or_else(|_| std::env::var("POTATO_API_KEY")) {
            self.llm.api_key = Some(key);
        }
        if let Ok(base) = std::env::var("OPENAI_BASE_URL").or_else(|_| std::env::var("POTATO_API_BASE")) {
            self.llm.api_base = base;
        }
        if let Ok(model) = std::env::var("POTATO_MODEL") {
            self.llm.model = model;
        }
        if let Ok(turns) = std::env::var("POTATO_MAX_TURNS") {
            if let Ok(n) = turns.parse::<usize>() {
                self.agent.max_turns = n;
            }
        }
    }
}

fn global_config_path() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("potato").join("config.toml"))
}
