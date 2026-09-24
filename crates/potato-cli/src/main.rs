use anyhow::Result;
use clap::{Parser, Subcommand};
use colored::Colorize;
use tracing_subscriber::EnvFilter;

use potato_cli::agents::{Coordinator, CustomAgent, SubagentTask, TaskCategory};
use potato_cli::config::PotatoConfig;
use potato_cli::engine::{
    context::ContextAssembler, cost_tracker::CostTracker, hooks::HookManager,
    install_signal_handler, repl::{handle_init, ReplEngine, REPL_BANNER},
    tool_policy::ToolPolicy, LoopRunner,
};
use potato_cli::llm::LlmClient;

/// 🥔 Potato — Autonomous Super Loop Agent Engine
///
/// Provide a software objective in natural language.
/// Potato will architect, scaffold, implement, and verify it fully autonomously.
#[derive(Parser, Debug)]
#[command(name = "potato", version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// The software objective to accomplish (e.g., "Build a REST API in Go with SQLite")
    #[arg(index = 1)]
    objective: Option<String>,

    /// Run full multi-agent pipeline (Architect -> Implementer -> Reviewer -> Repair -> Security)
    #[arg(long)]
    pipeline: bool,

    /// Maximum number of ReAct turns before aborting
    #[arg(long, env = "POTATO_MAX_TURNS")]
    max_turns: Option<usize>,

    /// Override max cost budget in USD (e.g. --budget 5.0)
    #[arg(long)]
    budget: Option<f64>,

    /// List all registered built-in and custom subagents
    #[arg(long)]
    list_agents: bool,

    /// Verbosity level: 0=warn, 1=info, 2=debug, 3=trace
    #[arg(short, long, action = clap::ArgAction::Count, default_value_t = 0)]
    verbose: u8,

    /// Resume a previous session by ID
    #[arg(long)]
    resume: Option<String>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Initialize a new potato workspace (.potato/ directory and starter potato.toml)
    Init,
    /// Run security and dependency audit on the current workspace
    Audit,
    /// Review current uncommitted git changes
    Review,
    /// Generate SPEC.md and ROADMAP.json for an objective
    Spec {
        /// Objective to architect
        objective: String,
    },
    /// Run the full multi-agent pipeline for an objective
    Pipeline {
        /// Objective to implement
        objective: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize tracing
    let filter = match cli.verbose {
        0 => "warn",
        1 => "info",
        2 => "debug",
        _ => "trace",
    };
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(filter)),
        )
        .with_target(false)
        .compact()
        .init();

    // Install Ctrl+C handler
    install_signal_handler()?;

    // Handle 'potato init'
    if let Some(Commands::Init) = &cli.command {
        return handle_init();
    }

    // Load layered config
    let config = PotatoConfig::load()?;

    // Handle --list-agents
    if cli.list_agents {
        println!("{}", BANNER.bold().yellow());
        println!("{}", "🤖 REGISTERED SUBAGENTS:".bold().cyan());
        println!("{}", "─".repeat(50).dimmed());

        let mut coordinator = Coordinator::new();
        for def in &config.custom_agents {
            if let Some(agent) = CustomAgent::from_def(def.clone()) {
                coordinator.register(Box::new(agent));
            }
        }

        for (i, name) in coordinator.agent_names().iter().enumerate() {
            println!("  {:2}. {}", (i + 1).to_string().dimmed(), name.bold().green());
        }

        if !config.custom_agents.is_empty() {
            println!("\n{}", "📦 CUSTOM TOML AGENTS:".bold().magenta());
            for custom in &config.custom_agents {
                println!(
                    "  • {} ({}) — model: {}",
                    custom.name.bold(),
                    custom.category.cyan(),
                    custom.model.as_deref().unwrap_or("default")
                );
            }
        }
        return Ok(());
    }

    // Check for session resume flag
    let resume_checkpoint = if let Some(ref resume_arg) = cli.resume {
        let session_dir = potato_cli::engine::get_session_dir();
        let cp = if resume_arg.is_empty() || resume_arg == "latest" {
            potato_cli::engine::SessionCheckpoint::load_latest(&session_dir.to_string_lossy())?
        } else {
            potato_cli::engine::SessionCheckpoint::load_by_id(&session_dir.to_string_lossy(), resume_arg)?
        };
        if cp.is_none() {
            eprintln!("{} Session '{}' not found in {}", "⚠️".yellow(), resume_arg, session_dir.display());
        }
        cp
    } else {
        None
    };

    // Launch Interactive REPL Mode if no subcommand and no objective are supplied (like claude or agy)
    if cli.command.is_none() && cli.objective.as_deref().is_none_or(|s| s.trim().is_empty()) {
        let mut repl = ReplEngine::new(config);
        if let Some(cp) = resume_checkpoint {
            println!("{} Resumed session: {}", "🔄".cyan(), cp.session_id.bold().yellow());
            repl = repl.with_checkpoint(cp);
        }
        return repl.run().await;
    }

    // Pre-flight check for Review: avoid requesting API keys if working tree is clean
    if let Some(Commands::Review) = &cli.command {
        println!("{}", BANNER.bold().yellow());
        println!("{}", "🧐 REVIEWING UNCOMMITTED CHANGES...".bold().cyan());
        let assembler = ContextAssembler::new(".");
        let project_ctx = assembler.assemble()?;
        if project_ctx.modified_files.is_empty() {
            println!("{}", "✨ Working tree is clean — no uncommitted modified files to review.".green());
            return Ok(());
        }
    }

    let api_key = config.llm.api_key.clone().ok_or_else(|| {
        anyhow::anyhow!(
            "No API key found. Set OPENAI_API_KEY or POTATO_API_KEY, or add api_key to potato.toml"
        )
    })?;

    let client = LlmClient::new(config.llm.api_base.clone(), api_key, config.llm.model.clone());
    let mut coordinator = Coordinator::new();
    for def in &config.custom_agents {
        if let Some(agent) = CustomAgent::from_def(def.clone()) {
            coordinator.register(Box::new(agent));
        }
    }

    // Handle Subcommands
    match cli.command {
        Some(Commands::Init) => return handle_init(),
        Some(Commands::Audit) => {
            println!("{}", BANNER.bold().yellow());
            println!("{}", "🔍 RUNNING SECURITY & DEPENDENCY AUDIT...".bold().cyan());
            let audit_task = SubagentTask {
                id: "audit-task".to_string(),
                description: "Perform comprehensive security and dependency audit on the current workspace".to_string(),
                category: TaskCategory::Security,
                context: "Audit for vulnerabilities, CVEs, secret leaks, and insecure permissions".to_string(),
                relevant_files: Vec::new(),
            };
            let res = coordinator.dispatch(&client, &audit_task).await?;
            println!("\n{}", res.summary);
            return Ok(());
        }
        Some(Commands::Review) => {
            let assembler = ContextAssembler::new(".");
            let project_ctx = assembler.assemble()?;
            let review_task = SubagentTask {
                id: "review-task".to_string(),
                description: "Review current uncommitted modified files for bugs, correctness, and style".to_string(),
                category: TaskCategory::Review,
                context: format!("Modified files: {:?}", project_ctx.modified_files),
                relevant_files: project_ctx.modified_files,
            };
            let res = coordinator.dispatch(&client, &review_task).await?;
            println!("\n{}", res.summary);
            return Ok(());
        }
        Some(Commands::Spec { objective }) => {
            println!("{}", BANNER.bold().yellow());
            println!("{} {}", "📐 ARCHITECTING SPECIFICATION:".bold().cyan(), objective.bold().white());
            let spec_task = SubagentTask {
                id: "spec-task".to_string(),
                description: objective,
                category: TaskCategory::Architecture,
                context: "Generate SPEC.md and ROADMAP.json".to_string(),
                relevant_files: Vec::new(),
            };
            let res = coordinator.dispatch(&client, &spec_task).await?;
            println!("\n{}", res.summary);
            return Ok(());
        }
        Some(Commands::Pipeline { objective }) => {
            println!("{}", BANNER.bold().yellow());
            println!("{} {}", "🚀 RUNNING MULTI-AGENT PIPELINE:".bold().cyan(), objective.bold().white());
            let tasks = vec![
                SubagentTask {
                    id: "task-spec".to_string(),
                    description: objective.clone(),
                    category: TaskCategory::Architecture,
                    context: "Produce SPEC.md and ROADMAP.json".to_string(),
                    relevant_files: Vec::new(),
                },
                SubagentTask {
                    id: "task-impl".to_string(),
                    description: objective.clone(),
                    category: TaskCategory::Implementation,
                    context: "Implement core modules according to SPEC.md".to_string(),
                    relevant_files: Vec::new(),
                },
                SubagentTask {
                    id: "task-test".to_string(),
                    description: objective,
                    category: TaskCategory::Testing,
                    context: "Write test suites for newly created modules".to_string(),
                    relevant_files: Vec::new(),
                },
            ];
            let results = coordinator.run_pipeline(&client, tasks).await?;
            println!("\n{}", "🎉 PIPELINE COMPLETE".bold().green());
            for r in results {
                println!("  • {}: {}", r.agent_name.bold(), r.summary);
            }
            return Ok(());
        }
        None => {}
    }

    // Default flow: Objective provided directly
    let objective = match cli.objective {
        Some(obj) if !obj.trim().is_empty() => obj,
        _ => {
            eprintln!("{}", "Error: Objective is required. Run 'potato --help' or 'potato --list-agents'.".red());
            std::process::exit(1);
        }
    };

    if cli.pipeline {
        println!("{}", BANNER.bold().yellow());
        println!("{} {}", "🚀 RUNNING MULTI-AGENT PIPELINE:".bold().cyan(), objective.bold().white());
        let tasks = vec![
            SubagentTask {
                id: "task-spec".to_string(),
                description: objective.clone(),
                category: TaskCategory::Architecture,
                context: "Produce SPEC.md and ROADMAP.json".to_string(),
                relevant_files: Vec::new(),
            },
            SubagentTask {
                id: "task-impl".to_string(),
                description: objective.clone(),
                category: TaskCategory::Implementation,
                context: "Implement core modules according to SPEC.md".to_string(),
                relevant_files: Vec::new(),
            },
            SubagentTask {
                id: "task-test".to_string(),
                description: objective,
                category: TaskCategory::Testing,
                context: "Write test suites for newly created modules".to_string(),
                relevant_files: Vec::new(),
            },
        ];
        let results = coordinator.run_pipeline(&client, tasks).await?;
        println!("\n{}", "🎉 PIPELINE COMPLETE".bold().green());
        for r in results {
            println!("  • {}: {}", r.agent_name.bold(), r.summary);
        }
        return Ok(());
    }

    let max_turns = cli.max_turns.unwrap_or(config.agent.max_turns);
    let max_budget = cli.budget.unwrap_or(config.cost.max_cost_usd);

    // Banner
    println!("{}", BANNER.bold().yellow());
    println!(
        "{} {}",
        "Objective:".bold().cyan(),
        objective.bold().white()
    );
    println!(
        "{} {}",
        "Max turns:".bold().cyan(),
        max_turns.to_string().white()
    );
    println!(
        "{} {}",
        "Cost budget:".bold().cyan(),
        format!("${:.2}", max_budget).white()
    );
    println!(
        "{} {}",
        "Model:".bold().cyan(),
        config.llm.model.white()
    );
    println!(
        "{} {}",
        "Sandbox:".bold().cyan(),
        if config.sandbox.enabled { "enabled".green() } else { "disabled".red() }
    );
    println!("{}", "─".repeat(60).dimmed());

    // Build cost tracker, tool policy, hook manager
    let cost_tracker = CostTracker::new(max_budget, config.cost.warn_at_usd)
        .with_pricing(config.cost.prompt_cost_per_million, config.cost.completion_cost_per_million);

    let tool_policy = ToolPolicy::from_config(config.tools.blocked_commands, config.tools.agents);
    let hook_manager = HookManager::from_config(&config.hooks);

    let initial_history = resume_checkpoint.map(|cp| cp.messages).unwrap_or_default();
    let runner = LoopRunner::new(client, objective.clone())
        .with_max_turns(max_turns)
        .with_cost_tracker(cost_tracker)
        .with_tool_policy(tool_policy)
        .with_hook_manager(hook_manager)
        .with_history(initial_history)
        .with_cache(potato_cli::engine::CacheManager::new(None, true))
        .with_brain(potato_cli::engine::Brain::new(None));

    match runner.run_session().await {
        Ok((summary, messages)) => {
            let session_dir = potato_cli::engine::get_session_dir();
            let checkpoint = potato_cli::engine::SessionCheckpoint::new(
                potato_cli::engine::generate_session_id(),
                objective,
                messages,
                1,
                0,
            );
            if let Ok(saved) = checkpoint.save(&session_dir.to_string_lossy()) {
                println!("{} Session saved: {}", "💾".cyan(), saved.display().to_string().dimmed());
            }

            println!("\n{}", "═".repeat(60).green());
            println!("{}", "🎉 MISSION COMPLETE".bold().green());
            println!("{}", "═".repeat(60).green());
            println!("{}", summary);
            Ok(())
        }
        Err(e) => {
            eprintln!("\n{}", "═".repeat(60).red());
            eprintln!("{} {}", "❌ MISSION FAILED:".bold().red(), e);
            eprintln!("{}", "═".repeat(60).red());
            Err(e)
        }
    }
}

const BANNER: &str = REPL_BANNER;
