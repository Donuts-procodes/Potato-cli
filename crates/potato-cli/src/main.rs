use anyhow::Result;
use clap::Parser;
use colored::Colorize;
use tracing_subscriber::EnvFilter;

use potato_cli::config::PotatoConfig;
use potato_cli::engine::{install_signal_handler, LoopRunner};
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
    objective: String,

    /// Maximum number of ReAct turns before aborting
    #[arg(long, env = "POTATO_MAX_TURNS")]
    max_turns: Option<usize>,

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

    let max_turns = cli.max_turns.unwrap_or(config.agent.max_turns);

    // Banner
    println!("{}", BANNER.bold().yellow());
    println!(
        "{} {}",
        "Objective:".bold().cyan(),
        cli.objective.bold().white()
    );
    println!(
        "{} {}",
        "Max turns:".bold().cyan(),
        max_turns.to_string().white()
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

    let client = LlmClient::new(config.llm.api_base, api_key, config.llm.model);
    let runner = LoopRunner::new(client, cli.objective).with_max_turns(max_turns);

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
  ║       🥔  P O T A T O   C L I  🥔              ║
  ║     Autonomous Super Loop Agent Engine           ║
  ║     7 Specialist Subagents • Anti-Oscillation    ║
  ╚══════════════════════════════════════════════════╝
"#;
