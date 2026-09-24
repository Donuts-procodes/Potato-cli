use anyhow::{anyhow, Result};
use colored::*;
use std::io::{self, BufRead, Write};
use std::path::Path;
use std::process::Command;

use crate::agents::{Coordinator, CustomAgent, SubagentTask, TaskCategory};
use crate::config::PotatoConfig;
use crate::engine::session::{generate_session_id, get_session_dir, list_sessions, SessionCheckpoint};
use crate::engine::{CostTracker, HookManager, LoopRunner, ToolPolicy};
use crate::llm::{ChatMessage, LlmClient};

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
    session_id: String,
    session_messages: Vec<ChatMessage>,
    session_turns: usize,
    session_objective: String,
}

impl ReplEngine {
    pub fn new(config: PotatoConfig) -> Self {
        let mut coordinator = Coordinator::new();
        for def in &config.custom_agents {
            if let Some(agent) = CustomAgent::from_def(def.clone()) {
                coordinator.register(Box::new(agent));
            }
        }
        let session_id = generate_session_id();
        Self {
            config,
            coordinator,
            total_tokens_used: 0,
            total_cost_usd: 0.0,
            session_id,
            session_messages: Vec::new(),
            session_turns: 0,
            session_objective: String::new(),
        }
    }

    pub fn with_checkpoint(mut self, checkpoint: SessionCheckpoint) -> Self {
        self.session_id = checkpoint.session_id;
        self.session_messages = checkpoint.messages;
        self.session_turns = checkpoint.turn;
        self.session_objective = checkpoint.objective;
        self
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
        println!("{} {}", "Session   :".bold().cyan(), self.session_id.bold().yellow());
        if self.session_turns > 0 {
            println!(
                "{} {} turn{}, {} messages in memory",
                "Memory    :".bold().cyan(),
                self.session_turns.to_string().bold(),
                if self.session_turns == 1 { "" } else { "s" },
                self.session_messages.len().to_string().bold()
            );
        }
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
            "/" | "/help" | "/h" | "/?" => {
                self.print_commands_menu(None);
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
                    self.print_models_menu();
                } else {
                    let target_model = if let Ok(idx) = arg.parse::<usize>() {
                        if let Some(info) = crate::llm::ModelRegistry::by_index(idx) {
                            info.id.to_string()
                        } else {
                            println!("{}", format!("Invalid model number {}. Type /model to view options.", idx).red());
                            return Ok(true);
                        }
                    } else {
                        arg.clone()
                    };

                    if let Some(info) = crate::llm::ModelRegistry::find(&target_model) {
                        self.config.cost.prompt_cost_per_million = info.prompt_cost_per_m;
                        self.config.cost.completion_cost_per_million = info.completion_cost_per_m;
                        println!(
                            "Switched active model to: {} ({}) [${:.2} / ${:.2} per 1M tokens]",
                            info.id.bold().green(),
                            info.provider.cyan(),
                            info.prompt_cost_per_m,
                            info.completion_cost_per_m
                        );
                    } else {
                        println!("Switched active model to custom: {}", target_model.bold().green());
                    }
                    self.config.llm.model = target_model;
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
            "/session" | "/memory" => {
                self.print_session_info();
            }
            "/sessions" => {
                self.list_saved_sessions();
            }
            "/resume" => {
                self.resume_session(&arg)?;
            }
            "/reset" | "/new" => {
                self.reset_session();
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
                println!("{} Unknown command '{}'.", "⚠️".yellow(), name);
                self.print_commands_menu(Some(&name));
            }
        }
        Ok(true)
    }

    fn print_commands_menu(&self, filter: Option<&str>) {
        let commands = [
            ("/init", "Initialize a new potato workspace (.potato/ and starter potato.toml)"),
            ("/audit", "Run security, vulnerability & dependency audit on current workspace"),
            ("/review", "Review current uncommitted git changes for correctness and style"),
            ("/spec <goal>", "Generate SPEC.md architecture and ROADMAP.json without writing code"),
            ("/pipeline <goal>", "Run full 5-stage multi-agent pipeline (Architect -> Impl -> Review -> Test)"),
            ("/agents", "List all 20 active specialist subagents and dynamic TOML agents"),
            ("/model [name]", "View or switch active LLM model (e.g. /model gpt-4o)"),
            ("/budget [usd]", "View or set session budget in USD (e.g. /budget 10.0)"),
            ("/cost", "Display token usage metrics and real-time USD expenditure"),
            ("/session", "Inspect active session memory, context turns, and mutated files"),
            ("/sessions", "List all saved historical sessions"),
            ("/resume [id]", "Resume a previous session (latest by default, or by session ID)"),
            ("/reset", "Reset session memory and start a fresh session (or /new)"),
            ("/clear", "Clear the terminal screen"),
            ("/help", "Show interactive guide and usage tips"),
            ("/exit", "Exit the interactive REPL session (or /quit)"),
        ];

        let filtered: Vec<_> = if let Some(prefix) = filter {
            let clean = prefix.trim_start_matches('/');
            let matched: Vec<_> = commands
                .iter()
                .filter(|(cmd, _)| cmd.trim_start_matches('/').starts_with(clean))
                .copied()
                .collect();
            if matched.is_empty() {
                commands.to_vec()
            } else {
                matched
            }
        } else {
            commands.to_vec()
        };

        println!("\n{}", "COMMANDS".bold().cyan());
        println!("{}", "─".repeat(70).dimmed());
        for (cmd, desc) in filtered {
            println!("  {:<20} {}", cmd.bold().yellow(), desc);
        }
        println!("{}", "─".repeat(70).dimmed());
        println!("{}\n", "Type a slash command or enter an objective to run the agent loop.".dimmed());
    }

    fn print_models_menu(&self) {
        println!("\n{}", "LATEST SUPPORTED MODELS:".bold().cyan());
        println!("{}", "─".repeat(80).dimmed());
        println!("Current active: {}\n", self.config.llm.model.bold().green());

        for (i, m) in crate::llm::ModelRegistry::all().iter().enumerate() {
            let active = if m.id.eq_ignore_ascii_case(&self.config.llm.model) {
                "★ (active)".bold().green()
            } else {
                "".normal()
            };
            println!(
                "  {:2}. {:<22} {:<16} {:>4}k ctx   ${:.2} / ${:.2}  {}",
                (i + 1).to_string().dimmed(),
                m.id.bold().yellow(),
                format!("[{}]", m.provider).cyan(),
                m.context_tokens / 1000,
                m.prompt_cost_per_m,
                m.completion_cost_per_m,
                active
            );
            println!("      {}", m.description.dimmed());
        }
        println!("{}", "─".repeat(80).dimmed());
        println!("{}\n", "Switch model: /model <name> or /model <number> (e.g. /model 1 or /model o3-mini)".dimmed());
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
            .with_cost_tracker(cost_tracker.clone())
            .with_tool_policy(tool_policy)
            .with_hook_manager(hook_manager)
            .with_history(self.session_messages.clone());

        match runner.run_session().await {
            Ok((summary, updated_messages)) => {
                // Record tokens and cost
                self.total_tokens_used += cost_tracker.prompt_tokens() + cost_tracker.completion_tokens();
                self.total_cost_usd += cost_tracker.current_cost_usd();

                // Update session memory
                self.session_messages = updated_messages;
                self.session_turns += 1;
                if self.session_objective.is_empty() {
                    self.session_objective = objective.to_string();
                } else {
                    self.session_objective = format!("{} | {}", self.session_objective, objective);
                }

                // Compress memory if it exceeds 60 messages to preserve token budget
                if self.session_messages.len() > 60 {
                    self.session_messages = crate::engine::context::ContextCompressor::compress(&self.session_messages, 30);
                    println!("{}", "🧠 Condensed session memory to preserve context window.".dimmed());
                }

                // Auto-save checkpoint
                let session_dir = get_session_dir();
                let checkpoint = SessionCheckpoint::new(
                    self.session_id.clone(),
                    self.session_objective.clone(),
                    self.session_messages.clone(),
                    self.session_turns,
                    0,
                );
                if let Ok(saved_path) = checkpoint.save(&session_dir.to_string_lossy()) {
                    println!("{} Session memory saved: {}", "💾".cyan(), saved_path.display().to_string().dimmed());
                }

                println!("\n{}", "═".repeat(60).green());
                println!("{}", "🎉 MISSION COMPLETE".bold().green());
                println!("{}", "═".repeat(60).green());
                println!("{}\n", summary);
            }
            Err(e) => {
                // Auto-save checkpoint on failure so state is preserved
                let session_dir = get_session_dir();
                let checkpoint = SessionCheckpoint::new(
                    self.session_id.clone(),
                    self.session_objective.clone(),
                    self.session_messages.clone(),
                    self.session_turns,
                    1,
                );
                let _ = checkpoint.save(&session_dir.to_string_lossy());

                eprintln!("\n{}", "═".repeat(60).red());
                eprintln!("{} {}", "❌ MISSION FAILED:".bold().red(), e);
                eprintln!("{}\n", "═".repeat(60).red());
            }
        }

        Ok(())
    }

    fn print_session_info(&self) {
        println!("\n{}", "🧠 ACTIVE SESSION MEMORY:".bold().cyan());
        println!("{}", "─".repeat(60).dimmed());
        println!("  Session ID  : {}", self.session_id.bold().yellow());
        println!("  Turns Run   : {}", self.session_turns.to_string().bold());
        println!("  Messages    : {} in active context", self.session_messages.len().to_string().bold());
        if !self.session_objective.is_empty() {
            println!("  Objectives  : {}", self.session_objective.white());
        }

        // Extract touched files
        let mut touched = Vec::new();
        for msg in &self.session_messages {
            if msg.role == "assistant" {
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&msg.content) {
                    if let Some(action) = val.get("action") {
                        if let Some(path) = action.get("path").and_then(|p| p.as_str()) {
                            if !touched.contains(&path.to_string()) {
                                touched.push(path.to_string());
                            }
                        }
                    }
                }
            }
        }

        if !touched.is_empty() {
            println!("  Touched File{}:", if touched.len() == 1 { "" } else { "s" });
            for f in &touched {
                println!("    • {}", f.green());
            }
        }

        let session_dir = get_session_dir();
        let session_file = session_dir.join(format!("session_{}.json", self.session_id));
        println!("  Storage     : {}", session_file.display().to_string().dimmed());
        println!("{}\n", "─".repeat(60).dimmed());
    }

    fn list_saved_sessions(&self) {
        let session_dir = get_session_dir();
        println!("\n{}", "📂 SAVED SESSIONS:".bold().cyan());
        println!("{}", "─".repeat(80).dimmed());
        match list_sessions(&session_dir) {
            Ok(list) if !list.is_empty() => {
                for (i, s) in list.iter().enumerate() {
                    let active_tag = if s.session_id == self.session_id {
                        " ★ (active)".bold().green()
                    } else {
                        "".normal()
                    };
                    println!(
                        "  {:2}. {} {} [{} turns, {} msgs] - {}",
                        (i + 1).to_string().dimmed(),
                        s.session_id.bold().yellow(),
                        active_tag,
                        s.turn,
                        s.message_count,
                        s.timestamp.dimmed()
                    );
                    let obj_preview = if s.objective.len() > 60 {
                        format!("{}…", &s.objective[..60])
                    } else {
                        s.objective.clone()
                    };
                    if !obj_preview.is_empty() {
                        println!("      Goal: {}", obj_preview.dimmed());
                    }
                }
                println!("\n  Resume any session with: {}", "/resume <session_id>".bold().yellow());
            }
            Ok(_) => {
                println!("  No saved sessions found in {}", session_dir.display().to_string().dimmed());
            }
            Err(e) => {
                println!("  {} Failed to list sessions: {}", "⚠️".red(), e);
            }
        }
        println!("{}\n", "─".repeat(80).dimmed());
    }

    fn resume_session(&mut self, target: &str) -> Result<()> {
        let session_dir = get_session_dir();
        let loaded = if target.trim().is_empty() || target.trim() == "latest" {
            SessionCheckpoint::load_latest(&session_dir.to_string_lossy())?
        } else {
            SessionCheckpoint::load_by_id(&session_dir.to_string_lossy(), target.trim())?
        };

        if let Some(cp) = loaded {
            // Save current session if it had any work done
            if self.session_turns > 0 && self.session_id != cp.session_id {
                let cur_cp = SessionCheckpoint::new(
                    self.session_id.clone(),
                    self.session_objective.clone(),
                    self.session_messages.clone(),
                    self.session_turns,
                    0,
                );
                let _ = cur_cp.save(&session_dir.to_string_lossy());
            }

            println!(
                "{} Resumed session: {} ({} turns, {} messages in memory)",
                "✓".green(),
                cp.session_id.bold().yellow(),
                cp.turn.to_string().bold(),
                cp.messages.len().to_string().bold()
            );
            if !cp.objective.is_empty() {
                println!("  Prior Goal: {}", cp.objective.dimmed());
            }
            self.session_id = cp.session_id;
            self.session_messages = cp.messages;
            self.session_turns = cp.turn;
            self.session_objective = cp.objective;
        } else {
            println!(
                "{} Session '{}' not found in {}. Type /sessions to list available.",
                "⚠️".yellow(),
                target.trim(),
                session_dir.display()
            );
        }
        Ok(())
    }

    fn reset_session(&mut self) {
        let session_dir = get_session_dir();
        if self.session_turns > 0 {
            let cur_cp = SessionCheckpoint::new(
                self.session_id.clone(),
                self.session_objective.clone(),
                self.session_messages.clone(),
                self.session_turns,
                0,
            );
            let _ = cur_cp.save(&session_dir.to_string_lossy());
            println!("{} Prior session {} saved to disk.", "💾".cyan(), self.session_id.dimmed());
        }

        self.session_id = generate_session_id();
        self.session_messages.clear();
        self.session_turns = 0;
        self.session_objective.clear();

        println!(
            "{} Session memory reset. Starting new clean session: {}\n",
            "✓".green(),
            self.session_id.bold().yellow()
        );
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

fn get_target_config_path() -> std::path::PathBuf {
    let local = Path::new("potato.toml");
    if local.exists() {
        if let Ok(file) = std::fs::OpenOptions::new().write(true).open(local) {
            drop(file);
            return local.to_path_buf();
        }
    }

    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let cwd_str = cwd.to_string_lossy().to_lowercase();
    let is_system_dir = cwd_str.contains("system32") || cwd_str.contains("windows");

    if !is_system_dir && std::fs::write("potato.toml.tmp", "# test\n").is_ok() {
        let _ = std::fs::remove_file("potato.toml.tmp");
        return local.to_path_buf();
    }

    if let Some(global_dir) = dirs::config_dir() {
        let potato_dir = global_dir.join("potato");
        let _ = std::fs::create_dir_all(&potato_dir);
        return potato_dir.join("config.toml");
    }

    local.to_path_buf()
}

fn save_api_key_to_config(api_key: &str, model: &str) -> Result<()> {
    let path = get_target_config_path();
    let content = if path.exists() {
        let existing = std::fs::read_to_string(&path)?;
        if existing.contains("api_key =") {
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

    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    if let Err(e) = std::fs::write(&path, &content) {
        if let Some(global_dir) = dirs::config_dir() {
            let global_path = global_dir.join("potato").join("config.toml");
            let _ = std::fs::create_dir_all(global_dir.join("potato"));
            std::fs::write(&global_path, content)?;
            return Ok(());
        }
        return Err(e.into());
    }
    Ok(())
}

fn save_ollama_config(api_base: &str, model: &str) -> Result<()> {
    let path = get_target_config_path();
    let content = format!(
        "# Potato CLI Configuration (Local Ollama)\n\n[llm]\napi_base = \"{}\"\napi_key = \"ollama\"\nmodel = \"{}\"\ntemperature = 0.1\n\n[cost]\nmax_cost_usd = 0.00\n",
        api_base, model
    );
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Err(e) = std::fs::write(&path, &content) {
        if let Some(global_dir) = dirs::config_dir() {
            let global_path = global_dir.join("potato").join("config.toml");
            let _ = std::fs::create_dir_all(global_dir.join("potato"));
            std::fs::write(&global_path, content)?;
            return Ok(());
        }
        return Err(e.into());
    }
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

