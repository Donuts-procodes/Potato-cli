use anyhow::{anyhow, Result};
use colored::*;
use std::io::{self, BufRead, Write};
use std::path::Path;
use std::process::Command;

use crate::agents::{Coordinator, CustomAgent, SubagentTask, TaskCategory};
use crate::config::PotatoConfig;
use crate::engine::{CostTracker, HookManager, LoopRunner, ToolPolicy};
use crate::llm::LlmClient;

pub const REPL_BANNER: &str = r#"
  ╔══════════════════════════════════════════════════╗
  ║       🥔  P O T A T O   C L I  🥔               ║
  ║     Autonomous Super Loop Agent Engine           ║
  ║     Interactive REPL Mode • Type /help for cmds  ║
  ╚══════════════════════════════════════════════════╝
"#;

pub struct ReplEngine {
    config: PotatoConfig,
    coordinator: Coordinator,
    total_tokens_used: u64,
    total_cost_usd: f64,
}

impl ReplEngine {
    pub fn new(config: PotatoConfig) -> Self {
        let mut coordinator = Coordinator::new();
        for def in &config.custom_agents {
            if let Some(agent) = CustomAgent::from_def(def.clone()) {
                coordinator.register(Box::new(agent));
            }
        }
        Self {
            config,
            coordinator,
            total_tokens_used: 0,
            total_cost_usd: 0.0,
        }
    }

    pub async fn run(mut self) -> Result<()> {
        println!("{}", REPL_BANNER.bold().yellow());

        // Check for missing API Key and prompt first-time onboarding
        if self.config.llm.api_key.is_none() {
            self.run_onboarding_wizard()?;
        }

        // Print workspace and environment status
        self.print_status_header();

        let stdin = io::stdin();
        let mut reader = stdin.lock();

        loop {
            print!("{}", "🥔 ❯ ".bold().yellow());
            io::stdout().flush()?;

            let mut input = String::new();
            let bytes_read = reader.read_line(&mut input)?;
            if bytes_read == 0 {
                // EOF (Ctrl+D)
                println!("\n{}", "Exiting Potato REPL. Goodbye! 🥔".bold().green());
                break;
            }

            let trimmed = input.trim();
            if trimmed.is_empty() {
                continue;
            }

            if trimmed.starts_with('/') {
                if !self.handle_slash_command(trimmed).await? {
                    break;
                }
            } else {
                // Natural language coding objective
                self.execute_objective(trimmed).await?;
            }
        }

        Ok(())
    }

    fn print_status_header(&self) {
        let cwd = std::env::current_dir()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| ".".to_string());

        let git_status = get_git_branch();

        println!("{} {}", "Workspace :".bold().cyan(), cwd.white());
        if let Some(git) = git_status {
            println!("{} {}", "Git       :".bold().cyan(), git.green());
        }
        println!("{} {}", "Model     :".bold().cyan(), self.config.llm.model.white());
        println!(
            "{} ${:.2} (spent: ${:.2})",
            "Budget    :".bold().cyan(),
            self.config.cost.max_cost_usd,
            self.total_cost_usd
        );
        println!(
            "{} {} registered specialists",
            "Agents    :".bold().cyan(),
            self.coordinator.agent_names().len().to_string().bold().green()
        );
        println!(
            "{}",
            "Type an objective (e.g. 'Build a Go API') or /help for commands. Exit: /exit".dimmed()
        );
        println!("{}", "─".repeat(60).dimmed());
    }

    fn run_onboarding_wizard(&mut self) -> Result<()> {
        println!("{}", "⚡ First-Time Setup: No LLM API key detected.".bold().cyan());
        println!("Choose an option to get started:");
        println!("  1. Enter OpenAI / Anthropic / Groq API Key");
        println!("  2. Use Local Offline Ollama (http://localhost:11434)");
        println!("  3. Continue in offline mode (slash commands only)");
        print!("\n{}", "Select [1/2/3]: ".bold().yellow());
        io::stdout().flush()?;

        let mut choice = String::new();
        io::stdin().read_line(&mut choice)?;
        match choice.trim() {
            "1" => {
                print!("{}", "Enter your API key: ".bold().yellow());
                io::stdout().flush()?;
                let mut key = String::new();
                io::stdin().read_line(&mut key)?;
                let trimmed_key = key.trim().to_string();
                if !trimmed_key.is_empty() {
                    self.config.llm.api_key = Some(trimmed_key.clone());
                    save_api_key_to_config(&trimmed_key, &self.config.llm.model)?;
                    println!("{} API key saved to potato.toml\n", "✓".green());
                }
            }
            "2" => {
                self.config.llm.api_base = "http://localhost:11434/v1".to_string();
                self.config.llm.api_key = Some("ollama".to_string());
                self.config.llm.model = "llama3".to_string();
                save_ollama_config("http://localhost:11434/v1", "llama3")?;
                println!("{} Configured for local Ollama on http://localhost:11434\n", "✓".green());
            }
            _ => {
                println!("{}", "Continuing in offline mode. Tasks requiring LLM will prompt for a key.\n".yellow());
            }
        }
        Ok(())
    }

    async fn handle_slash_command(&mut self, cmd: &str) -> Result<bool> {
        let parts: Vec<&str> = cmd.split_whitespace().collect();
        let name = parts[0].to_lowercase();
        let arg = if parts.len() > 1 {
            parts[1..].join(" ")
        } else {
            String::new()
        };

        match name.as_str() {
            "/help" | "/h" => {
                self.print_help();
            }
            "/init" => {
                println!("{}", "🥔 Initializing workspace...".cyan());
                handle_init()?;
            }
            "/audit" => {
                self.run_audit().await?;
            }
            "/review" => {
                self.run_review().await?;
            }
            "/spec" => {
                if arg.is_empty() {
                    println!("{}", "Usage: /spec <natural language objective>".yellow());
                } else {
                    self.run_spec(&arg).await?;
                }
            }
            "/pipeline" => {
                if arg.is_empty() {
                    println!("{}", "Usage: /pipeline <natural language objective>".yellow());
                } else {
                    self.run_pipeline(&arg).await?;
                }
            }
            "/agents" => {
                println!("\n{}", "🤖 REGISTERED SUBAGENTS:".bold().cyan());
                println!("{}", "─".repeat(50).dimmed());
                for (i, agent_name) in self.coordinator.agent_names().iter().enumerate() {
                    println!("  {:2}. {}", (i + 1).to_string().dimmed(), agent_name.bold().green());
                }
                if !self.config.custom_agents.is_empty() {
                    println!("\n{}", "📦 CUSTOM TOML AGENTS:".bold().magenta());
                    for custom in &self.config.custom_agents {
                        println!(
                            "  • {} ({}) — model: {}",
                            custom.name.bold(),
                            custom.category.cyan(),
                            custom.model.as_deref().unwrap_or("default")
                        );
                    }
                }
                println!();
            }
            "/model" => {
                if arg.is_empty() {
                    println!("Current active model: {}", self.config.llm.model.bold().green());
                } else {
                    self.config.llm.model = arg.clone();
                    println!("Switched active model to: {}", arg.bold().green());
                }
            }
            "/budget" => {
                if arg.is_empty() {
                    println!("Current budget: ${:.2}", self.config.cost.max_cost_usd);
                } else if let Ok(val) = arg.parse::<f64>() {
                    self.config.cost.max_cost_usd = val;
                    println!("Updated budget to: ${:.2}", val);
                } else {
                    println!("{}", "Invalid budget amount. Example: /budget 10.0".red());
                }
            }
            "/cost" => {
                println!("\n{}", "📊 SESSION USAGE & COST METRICS:".bold().cyan());
                println!("  Tokens Used : {}", self.total_tokens_used.to_string().bold());
                println!("  Total Cost  : ${:.4}", self.total_cost_usd);
                println!(
                    "  Remaining   : ${:.4}\n",
                    (self.config.cost.max_cost_usd - self.total_cost_usd).max(0.0)
                );
            }
            "/clear" => {
                print!("\x1B[2J\x1B[1;1H");
                io::stdout().flush()?;
                self.print_status_header();
            }
            "/exit" | "/quit" | ":q" => {
                println!("{}", "Exiting Potato REPL. Goodbye! 🥔".bold().green());
                return Ok(false);
            }
            _ => {
                println!(
                    "{} Unknown command '{}'. Type /help for available commands.",
                    "⚠️".yellow(),
                    name
                );
            }
        }
        Ok(true)
    }

    fn print_help(&self) {
        println!("\n{}", "🥔 POTATO INTERACTIVE COMMANDS:".bold().cyan());
        println!("{}", "─".repeat(50).dimmed());
        println!("  {}         Show this interactive command guide", "/help, /h".bold().yellow());
        println!("  {}         Initialize .potato/ workspace and starter potato.toml", "/init".bold().yellow());
        println!("  {}        Run security and dependency audit on workspace", "/audit".bold().yellow());
        println!("  {}       Review uncommitted working tree git diffs", "/review".bold().yellow());
        println!("  {}   Generate SPEC.md and ROADMAP.json without writing code", "/spec <goal>".bold().yellow());
        println!("  {} Run 5-stage pipeline (Architect -> Impl -> Review -> Test)", "/pipeline <goal>".bold().yellow());
        println!("  {}       List all 20 built-in and dynamic custom agents", "/agents".bold().yellow());
        println!("  {}  View or switch current LLM model (e.g. /model gpt-4o)", "/model [name]".bold().yellow());
        println!("  {} View or set max budget in USD (e.g. /budget 10.0)", "/budget [usd]".bold().yellow());
        println!("  {}         Display token usage and session expenses", "/cost".bold().yellow());
        println!("  {}        Clear terminal screen", "/clear".bold().yellow());
        println!("  {}  Exit the interactive REPL session", "/exit, /quit".bold().yellow());
        println!("\n  {} Type any request (e.g. 'Build a Go API') to run the Super Loop.\n", "Tip:".bold().green());
    }

    fn ensure_client(&mut self) -> Result<LlmClient> {
        let api_key = match &self.config.llm.api_key {
            Some(k) if !k.trim().is_empty() => k.clone(),
            _ => {
                print!("{}", "Enter API key to proceed: ".bold().yellow());
                io::stdout().flush()?;
                let mut key = String::new();
                io::stdin().read_line(&mut key)?;
                let trimmed = key.trim().to_string();
                if trimmed.is_empty() {
                    return Err(anyhow!("No API key provided"));
                }
                self.config.llm.api_key = Some(trimmed.clone());
                save_api_key_to_config(&trimmed, &self.config.llm.model)?;
                trimmed
            }
        };

        Ok(LlmClient::new(
            self.config.llm.api_base.clone(),
            api_key,
            self.config.llm.model.clone(),
        ))
    }

    async fn run_audit(&mut self) -> Result<()> {
        let client = match self.ensure_client() {
            Ok(c) => c,
            Err(e) => {
                println!("{} {}", "Error:".red(), e);
                return Ok(());
            }
        };
        println!("{}", "🔍 Running security and dependency audit...".cyan());
        let task = SubagentTask {
            id: "audit-task".to_string(),
            description: "Perform comprehensive security and dependency audit on the current workspace".to_string(),
            category: TaskCategory::Security,
            context: "Audit for vulnerabilities, CVEs, secret leaks, and insecure permissions".to_string(),
            relevant_files: Vec::new(),
        };
        match self.coordinator.dispatch(&client, &task).await {
            Ok(res) => println!("\n{}\n", res.summary),
            Err(e) => println!("{} Audit failed: {}\n", "❌".red(), e),
        }
        Ok(())
    }

    async fn run_review(&mut self) -> Result<()> {
        let client = match self.ensure_client() {
            Ok(c) => c,
            Err(e) => {
                println!("{} {}", "Error:".red(), e);
                return Ok(());
            }
        };
        println!("{}", "🔍 Reviewing uncommitted working tree diffs...".cyan());
        let diff = Command::new("git")
            .args(["diff", "HEAD"])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
            .unwrap_or_default();

        let task = SubagentTask {
            id: "review-task".to_string(),
            description: "Review current uncommitted git changes for bugs, style, and correctness".to_string(),
            category: TaskCategory::Review,
            context: if diff.is_empty() {
                "No uncommitted git diffs found. Review repository structure.".to_string()
            } else {
                format!("Git Diff:\n{}", diff)
            },
            relevant_files: Vec::new(),
        };
        match self.coordinator.dispatch(&client, &task).await {
            Ok(res) => println!("\n{}\n", res.summary),
            Err(e) => println!("{} Review failed: {}\n", "❌".red(), e),
        }
        Ok(())
    }

    async fn run_spec(&mut self, objective: &str) -> Result<()> {
        let client = match self.ensure_client() {
            Ok(c) => c,
            Err(e) => {
                println!("{} {}", "Error:".red(), e);
                return Ok(());
            }
        };
        println!("{} {}", "📐 Generating SPEC.md & ROADMAP.json for:".cyan(), objective.bold().white());
        let task = SubagentTask {
            id: "spec-task".to_string(),
            description: objective.to_string(),
            category: TaskCategory::Architecture,
            context: "Analyze requirements and create SPEC.md and ROADMAP.json".to_string(),
            relevant_files: Vec::new(),
        };
        match self.coordinator.dispatch(&client, &task).await {
            Ok(res) => println!("\n{}\n", res.summary),
            Err(e) => println!("{} Spec failed: {}\n", "❌".red(), e),
        }
        Ok(())
    }

    async fn run_pipeline(&mut self, objective: &str) -> Result<()> {
        let client = match self.ensure_client() {
            Ok(c) => c,
            Err(e) => {
                println!("{} {}", "Error:".red(), e);
                return Ok(());
            }
        };
        println!("{} {}", "🚀 Running 5-Stage Pipeline for:".cyan(), objective.bold().white());
        let tasks = vec![
            SubagentTask {
                id: "task-spec".to_string(),
                description: objective.to_string(),
                category: TaskCategory::Architecture,
                context: "Produce SPEC.md and ROADMAP.json".to_string(),
                relevant_files: Vec::new(),
            },
            SubagentTask {
                id: "task-impl".to_string(),
                description: objective.to_string(),
                category: TaskCategory::Implementation,
                context: "Implement core modules according to SPEC.md".to_string(),
                relevant_files: Vec::new(),
            },
            SubagentTask {
                id: "task-test".to_string(),
                description: objective.to_string(),
                category: TaskCategory::Testing,
                context: "Write test suites for newly created modules".to_string(),
                relevant_files: Vec::new(),
            },
        ];
        match self.coordinator.run_pipeline(&client, tasks).await {
            Ok(results) => {
                println!("\n{}", "🎉 PIPELINE COMPLETE".bold().green());
                for r in results {
                    println!("  • {}: {}", r.agent_name.bold(), r.summary);
                }
                println!();
            }
            Err(e) => println!("{} Pipeline failed: {}\n", "❌".red(), e),
        }
        Ok(())
    }

    async fn execute_objective(&mut self, objective: &str) -> Result<()> {
        let client = match self.ensure_client() {
            Ok(c) => c,
            Err(e) => {
                println!("{} {}", "Error:".red(), e);
                return Ok(());
            }
        };

        println!("\n{}", "─".repeat(60).dimmed());
        println!("{} {}", "Objective :".bold().cyan(), objective.bold().white());
        println!("{} {}", "Model     :".bold().cyan(), self.config.llm.model.white());
        println!("{} ${:.2}", "Budget    :".bold().cyan(), self.config.cost.max_cost_usd);
        println!("{}", "─".repeat(60).dimmed());

        let cost_tracker = CostTracker::new(self.config.cost.max_cost_usd, self.config.cost.warn_at_usd)
            .with_pricing(
                self.config.cost.prompt_cost_per_million,
                self.config.cost.completion_cost_per_million,
            );

        let tool_policy = ToolPolicy::from_config(
            self.config.tools.blocked_commands.clone(),
            self.config.tools.agents.clone(),
        );
        let hook_manager = HookManager::from_config(&self.config.hooks);

        let runner = LoopRunner::new(client, objective.to_string())
            .with_max_turns(self.config.agent.max_turns)
            .with_cost_tracker(cost_tracker)
            .with_tool_policy(tool_policy)
            .with_hook_manager(hook_manager);

        match runner.run().await {
            Ok(summary) => {
                println!("\n{}", "═".repeat(60).green());
                println!("{}", "🎉 MISSION COMPLETE".bold().green());
                println!("{}", "═".repeat(60).green());
                println!("{}\n", summary);
            }
            Err(e) => {
                eprintln!("\n{}", "═".repeat(60).red());
                eprintln!("{} {}", "❌ MISSION FAILED:".bold().red(), e);
                eprintln!("{}\n", "═".repeat(60).red());
            }
        }

        Ok(())
    }
}

fn get_git_branch() -> Option<String> {
    Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                let branch = String::from_utf8_lossy(&o.stdout).trim().to_string();
                if branch.is_empty() {
                    None
                } else {
                    Some(branch)
                }
            } else {
                None
            }
        })
}

fn save_api_key_to_config(api_key: &str, model: &str) -> Result<()> {
    let path = Path::new("potato.toml");
    let content = if path.exists() {
        let existing = std::fs::read_to_string(path)?;
        if existing.contains("api_key =") {
            // Replace existing api_key line
            let mut lines = Vec::new();
            for line in existing.lines() {
                if line.trim_start().starts_with("api_key =") {
                    lines.push(format!("api_key = \"{}\"", api_key));
                } else {
                    lines.push(line.to_string());
                }
            }
            lines.join("\n")
        } else if existing.contains("[llm]") {
            existing.replace("[llm]", &format!("[llm]\napi_key = \"{}\"", api_key))
        } else {
            format!("[llm]\napi_key = \"{}\"\nmodel = \"{}\"\n\n{}", api_key, model, existing)
        }
    } else {
        format!(
            "# Potato CLI Configuration\n\n[llm]\napi_key = \"{}\"\nmodel = \"{}\"\ntemperature = 0.1\n\n[cost]\nmax_cost_usd = 5.00\n",
            api_key, model
        )
    };
    std::fs::write(path, content)?;
    Ok(())
}

fn save_ollama_config(api_base: &str, model: &str) -> Result<()> {
    let path = Path::new("potato.toml");
    let content = format!(
        "# Potato CLI Configuration (Local Ollama)\n\n[llm]\napi_base = \"{}\"\napi_key = \"ollama\"\nmodel = \"{}\"\ntemperature = 0.1\n\n[cost]\nmax_cost_usd = 0.00\n",
        api_base, model
    );
    std::fs::write(path, content)?;
    Ok(())
}

pub fn handle_init() -> Result<()> {
    println!("{}", REPL_BANNER.bold().yellow());
    println!("{}", "🥔 INITIALIZING POTATO WORKSPACE...".bold().cyan());

    std::fs::create_dir_all(".potato/prompts")?;
    std::fs::create_dir_all(".potato/lessons")?;
    std::fs::create_dir_all(".potato/hooks")?;
    std::fs::create_dir_all(".potato/sessions")?;

    let prompt_sample = r#"# Rust Architecture & Style Rules
- Always use `thiserror` for library error types and `anyhow` for application binaries.
- Avoid `.unwrap()` or `.expect()` in production paths; return structured `Result`.
- Ensure all public functions have doc comments.
"#;
    let prompt_path = ".potato/prompts/rust_style.md";
    if !std::path::Path::new(prompt_path).exists() {
        std::fs::write(prompt_path, prompt_sample)?;
        println!("  {} Created sample prompt: {}", "✓".green(), prompt_path);
    }

    let config_path = "potato.toml";
    if !std::path::Path::new(config_path).exists() {
        let sample_config = r#"# Potato CLI Configuration

[llm]
model = "gpt-4o"
temperature = 0.1
timeout_seconds = 180

[cost]
max_cost_usd = 5.00
warn_at_usd = 3.00

[tools]
blocked_commands = ["rm -rf /", "format C:", "mkfs"]

[tools.Reviewer]
allowed_actions = ["read_file", "list_dir", "finish"]

[[agents]]
name = "StyleEnforcer"
category = "review"
max_turns = 10
model = "gpt-4o-mini"
system_prompt = "Verify that all functions have documentation comments."
"#;
        std::fs::write(config_path, sample_config)?;
        println!("  {} Created starter config: {}", "✓".green(), config_path);
    }

    println!("\n{}", "✨ Workspace initialized successfully in .potato/".bold().green());
    Ok(())
}

