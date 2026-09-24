use std::collections::HashMap;
use std::path::PathBuf;
use tracing::info;

/// Manages lifecycle hooks: shell scripts executed at specific points in the agent loop.
#[derive(Debug, Clone)]
pub struct HookManager {
    hooks: HashMap<HookType, Vec<HookEntry>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum HookType {
    PreStart,
    PreWrite,
    PostWrite,
    PreExec,
    PostCommit,
    OnFailure,
    OnComplete,
    OnShutdown,
}

#[derive(Debug, Clone)]
pub struct HookEntry {
    pub hook_type: HookType,
    pub script_path: PathBuf,
}

impl Default for HookManager {
    fn default() -> Self {
        Self::new()
    }
}

impl HookManager {
    pub fn new() -> Self {
        Self {
            hooks: HashMap::new(),
        }
    }

    /// Loads hooks from config key-value pairs (hook_name → script_path).
    pub fn from_config(config: &HashMap<String, String>) -> Self {
        let mut manager = Self::new();

        for (key, path) in config {
            if let Some(hook_type) = parse_hook_type(key) {
                manager.register(hook_type, PathBuf::from(path));
            }
        }

        manager
    }

    /// Registers a hook script for a given lifecycle event.
    pub fn register(&mut self, hook_type: HookType, script_path: PathBuf) {
        info!(
            hook = ?hook_type,
            script = %script_path.display(),
            "Registered lifecycle hook"
        );
        self.hooks
            .entry(hook_type.clone())
            .or_default()
            .push(HookEntry {
                hook_type,
                script_path,
            });
    }

    /// Executes all hooks registered for the given event.
    /// Passes context as environment variables.
    pub fn run_hooks(
        &self,
        hook_type: &HookType,
        context: &HookContext,
    ) -> Vec<HookResult> {
        let mut results = Vec::new();

        if let Some(entries) = self.hooks.get(hook_type) {
            for entry in entries {
                let result = execute_hook_script(entry, context);
                results.push(result);
            }
        }

        results
    }

    /// Returns true if any hooks are registered for this event.
    pub fn has_hooks(&self, hook_type: &HookType) -> bool {
        self.hooks.get(hook_type).map(|v| !v.is_empty()).unwrap_or(false)
    }
}

/// Context passed to hook scripts as environment variables.
#[derive(Debug, Clone)]
pub struct HookContext {
    pub file_path: Option<String>,
    pub action_type: Option<String>,
    pub exit_code: Option<i32>,
    pub turn_number: Option<usize>,
    pub agent_name: Option<String>,
}

impl HookContext {
    pub fn empty() -> Self {
        Self {
            file_path: None,
            action_type: None,
            exit_code: None,
            turn_number: None,
            agent_name: None,
        }
    }

    fn to_env_vars(&self) -> Vec<(String, String)> {
        let mut vars = Vec::new();
        if let Some(ref p) = self.file_path {
            vars.push(("POTATO_FILE_PATH".to_string(), p.clone()));
        }
        if let Some(ref a) = self.action_type {
            vars.push(("POTATO_ACTION_TYPE".to_string(), a.clone()));
        }
        if let Some(c) = self.exit_code {
            vars.push(("POTATO_EXIT_CODE".to_string(), c.to_string()));
        }
        if let Some(t) = self.turn_number {
            vars.push(("POTATO_TURN".to_string(), t.to_string()));
        }
        if let Some(ref n) = self.agent_name {
            vars.push(("POTATO_AGENT".to_string(), n.clone()));
        }
        vars
    }
}

#[derive(Debug)]
pub struct HookResult {
    pub hook_type: HookType,
    pub script_path: PathBuf,
    pub success: bool,
    pub output: String,
}

fn execute_hook_script(entry: &HookEntry, context: &HookContext) -> HookResult {
    if !entry.script_path.exists() {
        return HookResult {
            hook_type: entry.hook_type.clone(),
            script_path: entry.script_path.clone(),
            success: false,
            output: format!("Hook script not found: {}", entry.script_path.display()),
        };
    }

    #[cfg(target_os = "windows")]
    let mut cmd = {
        let mut c = std::process::Command::new("cmd");
        c.args(["/C", &entry.script_path.display().to_string()]);
        c
    };

    #[cfg(not(target_os = "windows"))]
    let mut cmd = {
        let mut c = std::process::Command::new("sh");
        c.args(["-c", &entry.script_path.display().to_string()]);
        c
    };

    // Inject context as env vars
    for (key, value) in context.to_env_vars() {
        cmd.env(key, value);
    }

    match cmd.output() {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            HookResult {
                hook_type: entry.hook_type.clone(),
                script_path: entry.script_path.clone(),
                success: output.status.success(),
                output: if stdout.is_empty() { stderr } else { stdout },
            }
        }
        Err(e) => HookResult {
            hook_type: entry.hook_type.clone(),
            script_path: entry.script_path.clone(),
            success: false,
            output: format!("Failed to execute hook: {}", e),
        },
    }
}

fn parse_hook_type(key: &str) -> Option<HookType> {
    match key {
        "pre_start" => Some(HookType::PreStart),
        "pre_write" => Some(HookType::PreWrite),
        "post_write" => Some(HookType::PostWrite),
        "pre_exec" => Some(HookType::PreExec),
        "post_commit" => Some(HookType::PostCommit),
        "on_failure" => Some(HookType::OnFailure),
        "on_complete" => Some(HookType::OnComplete),
        "on_shutdown" => Some(HookType::OnShutdown),
        _ => None,
    }
}
