use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::info;

/// Manages tool access policies: which actions each agent is allowed to use,
/// and which shell commands are globally blocked.
#[derive(Debug, Clone)]
pub struct ToolPolicy {
    /// Commands that are always blocked regardless of agent.
    pub global_blocked_commands: Vec<String>,
    /// Per-agent allowlists. If an agent has an entry here, it can ONLY use listed actions.
    pub agent_allowlists: HashMap<String, Vec<String>>,
    /// Per-agent blocked commands (in addition to global).
    pub agent_blocked_commands: HashMap<String, Vec<String>>,
}

impl Default for ToolPolicy {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolPolicy {
    pub fn new() -> Self {
        Self {
            global_blocked_commands: default_blocked_commands(),
            agent_allowlists: HashMap::new(),
            agent_blocked_commands: HashMap::new(),
        }
    }

    /// Loads tool policy from config sections.
    pub fn from_config(
        blocked_commands: Vec<String>,
        agent_configs: HashMap<String, AgentToolConfig>,
    ) -> Self {
        let mut policy = Self {
            global_blocked_commands: blocked_commands,
            agent_allowlists: HashMap::new(),
            agent_blocked_commands: HashMap::new(),
        };

        for (agent_name, config) in agent_configs {
            if !config.allowed_actions.is_empty() {
                policy.agent_allowlists.insert(agent_name.clone(), config.allowed_actions);
            }
            if !config.blocked_commands.is_empty() {
                policy.agent_blocked_commands.insert(agent_name, config.blocked_commands);
            }
        }

        policy
    }

    /// Checks if an action type is allowed for a given agent.
    pub fn is_action_allowed(&self, agent_name: &str, action_name: &str) -> bool {
        if let Some(allowlist) = self.agent_allowlists.get(agent_name) {
            return allowlist.iter().any(|a| a == action_name);
        }
        true // No allowlist means all actions are permitted
    }

    /// Checks if a shell command is allowed for a given agent.
    pub fn is_command_allowed(&self, agent_name: &str, command: &str) -> bool {
        let cmd_lower = command.to_lowercase();

        // Check global blocklist
        for blocked in &self.global_blocked_commands {
            if cmd_lower.contains(&blocked.to_lowercase()) {
                info!(
                    agent = agent_name,
                    command,
                    blocked_pattern = blocked.as_str(),
                    "Command blocked by global policy"
                );
                return false;
            }
        }

        // Check per-agent blocklist
        if let Some(agent_blocked) = self.agent_blocked_commands.get(agent_name) {
            for blocked in agent_blocked {
                if cmd_lower.contains(&blocked.to_lowercase()) {
                    info!(
                        agent = agent_name,
                        command,
                        blocked_pattern = blocked.as_str(),
                        "Command blocked by agent-specific policy"
                    );
                    return false;
                }
            }
        }

        true
    }

    /// Returns a denial message explaining why an action was blocked.
    pub fn denial_message(&self, agent_name: &str, action_or_command: &str) -> String {
        format!(
            "POLICY DENIAL: Agent '{}' is not permitted to execute '{}'. \
             Check [tools] section in potato.toml to adjust permissions.",
            agent_name, action_or_command
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct AgentToolConfig {
    pub allowed_actions: Vec<String>,
    pub blocked_commands: Vec<String>,
}

fn default_blocked_commands() -> Vec<String> {
    vec![
        "rm -rf /".to_string(),
        "rm -rf ~".to_string(),
        "format c:".to_string(),
        "del /s /q c:\\".to_string(),
        "shutdown".to_string(),
        "reboot".to_string(),
        "mkfs".to_string(),
        "dd if=".to_string(),
        ":(){:|:&};:".to_string(), // fork bomb
    ]
}
