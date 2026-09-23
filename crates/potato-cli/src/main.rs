use anyhow::Result;
use clap::Parser;
use colored::Colorize;
use tracing_subscriber::EnvFilter;

use potato_cli::agents::Coordinator;
use potato_cli::config::PotatoConfig;
use potato_cli::engine::{
    cost_tracker::CostTracker, hooks::HookManager, install_signal_handler,
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
    /// The software objective to accomplish (e.g., "Build a REST API in Go with SQLite")
    #[arg(index = 1)]
    objective: Option<String>,

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

    // Load layered config
    let config = PotatoConfig::load()?;

    // Handle --list-agents
    if cli.list_agents {
        println!("{}", BANNER.bold().yellow());
        println!("{}", "🤖 REGISTERED SUBAGENTS:".bold().cyan());
        println!("{}", "─".repeat(50).dimmed());

        let coordinator = Coordinator::new();
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

    let objective = match cli.objective {
        Some(obj) if !obj.trim().is_empty() => obj,
        _ => {
            eprintln!("{}", "Error: Objective is required. Run 'potato --help' for usage or 'potato --list-agents' to view agents.".red());
            std::process::exit(1);
        }
    };

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

    let api_key = config.llm.api_key.ok_or_else(|| {
        anyhow::anyhow!(
            "No API key found. Set OPENAI_API_KEY or POTATO_API_KEY, or add api_key to potato.toml"
        )
    })?;

    // Build cost tracker, tool policy, hook manager
    let cost_tracker = CostTracker::new(max_budget, config.cost.warn_at_usd)
        .with_pricing(config.cost.prompt_cost_per_million, config.cost.completion_cost_per_million);

    let tool_policy = ToolPolicy::from_config(config.tools.blocked_commands, config.tools.agents);
    let hook_manager = HookManager::from_config(&config.hooks);

    let client = LlmClient::new(config.llm.api_base, api_key, config.llm.model);
    let runner = LoopRunner::new(client, objective)
        .with_max_turns(max_turns)
        .with_cost_tracker(cost_tracker)
        .with_tool_policy(tool_policy)
        .with_hook_manager(hook_manager);

    match runner.run().await {
        Ok(summary) => {
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

const BANNER: &str = r#"
  ╔══════════════════════════════════════════════════╗
  ║       🥔  P O T A T O   C L I  🥔               ║
  ║     Autonomous Super Loop Agent Engine           ║
  ║     20 Specialist & Meta Subagents • Sandboxed   ║
  ╚══════════════════════════════════════════════════╝
"#;
