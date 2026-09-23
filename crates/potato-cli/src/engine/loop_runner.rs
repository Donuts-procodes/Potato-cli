use anyhow::{Context, Result};
use colored::Colorize;
use tracing::{info, warn};

use crate::engine::executor;
use crate::llm::{build_system_prompt, ChatMessage, LlmClient};
use crate::types::Action;

const MAX_TURNS: usize = 200;
const MAX_CONSECUTIVE_FAILURES: usize = 5;

pub struct LoopRunner {
    client: LlmClient,
    objective: String,
    max_turns: usize,
}

impl LoopRunner {
    pub fn new(client: LlmClient, objective: String) -> Self {
        Self {
            client,
            objective,
            max_turns: MAX_TURNS,
        }
    }

    pub fn with_max_turns(mut self, turns: usize) -> Self {
        self.max_turns = turns;
        self
    }

    pub async fn run(&self) -> Result<String> {
        let system_prompt = build_system_prompt();
        let mut messages: Vec<ChatMessage> = vec![
            ChatMessage {
                role: "system".to_string(),
                content: system_prompt,
            },
            ChatMessage {
                role: "user".to_string(),
                content: format!(
                    "## OBJECTIVE\n{}\n\n## INITIAL STATE\nWorking directory is the current directory. Begin with PHASE 1: SPECIFICATION & ARCHITECTURE.",
                    self.objective
                ),
            },
        ];

        let mut consecutive_failures: usize = 0;

        for turn in 1..=self.max_turns {
            println!(
                "\n{} {}",
                format!("━━━ Turn {}/{}", turn, self.max_turns).bold().cyan(),
                "━".repeat(50).dimmed()
            );

            let agent_response = self
                .client
                .send_turn(&messages)
                .await
                .with_context(|| format!("LLM call failed on turn {}", turn))?;

            // Print thought
            println!(
                "{} {}",
                "💭 Thought:".bold().yellow(),
                truncate_display(&agent_response.thought, 200)
            );
            println!(
                "{} {:?}",
                "📍 Phase:".bold().magenta(),
                agent_response.phase
            );
            println!(
                "{} {}",
                "🔧 Action:".bold().green(),
                action_summary(&agent_response.action)
            );

            // Check for finish
            if let Action::Finish { ref summary } = agent_response.action {
                println!("\n{}", "✅ AGENT COMPLETE".bold().green());
                println!("{}", summary);

                messages.push(ChatMessage {
                    role: "assistant".to_string(),
                    content: serde_json::to_string(&agent_response)?,
                });

                return Ok(summary.clone());
            }

            // Execute the action
            let result = executor::dispatch(&agent_response.action);

            // Display result
            if result.success {
                println!(
                    "{} exit=0 | {}",
                    "  ✓".green(),
                    truncate_display(&result.stdout, 300)
                );
                consecutive_failures = 0;
            } else {
                println!(
                    "{} exit={} | {}",
                    "  ✗".red(),
                    result.exit_code,
                    truncate_display(&result.stderr, 300)
                );
                consecutive_failures += 1;
            }

            // Anti-oscillation guard
            if consecutive_failures >= MAX_CONSECUTIVE_FAILURES {
                warn!(
                    consecutive_failures,
                    "Anti-oscillation: too many consecutive failures, injecting rollback hint"
                );
                println!(
                    "{}",
                    format!(
                        "⚠️  {} consecutive failures detected — injecting rollback hint",
                        consecutive_failures
                    )
                    .bold()
                    .red()
                );
                consecutive_failures = 0;

                messages.push(ChatMessage {
                    role: "assistant".to_string(),
                    content: serde_json::to_string(&agent_response)?,
                });
                messages.push(ChatMessage {
                    role: "user".to_string(),
                    content: format!(
                        "SYSTEM ALERT: {} consecutive action failures detected. \
                         You MUST invoke git_checkpoint with action=rollback now, \
                         then re-evaluate the architecture. \
                         Previous result: exit_code={}, stderr={}",
                        MAX_CONSECUTIVE_FAILURES, result.exit_code, result.stderr
                    ),
                });
                continue;
            }

            // Append assistant turn + tool result to context
            messages.push(ChatMessage {
                role: "assistant".to_string(),
                content: serde_json::to_string(&agent_response)?,
            });
            messages.push(ChatMessage {
                role: "user".to_string(),
                content: format!(
                    "TOOL RESULT:\nexit_code: {}\nstdout:\n{}\nstderr:\n{}",
                    result.exit_code, result.stdout, result.stderr
                ),
            });

            // Context window management: trim oldest non-system messages if too many
            if messages.len() > 80 {
                let system_msg = messages[0].clone();
                let keep_count = 40;
                let drain_end = messages.len() - keep_count;
                messages.drain(1..drain_end);
                messages[0] = system_msg;
                info!("Trimmed context window to {} messages", messages.len());
            }
        }

        Err(anyhow::anyhow!(
            "Agent did not converge within {} turns",
            self.max_turns
        ))
    }
}

fn truncate_display(s: &str, max_chars: usize) -> String {
    let cleaned = s.trim();
    if cleaned.len() > max_chars {
        format!("{}…", &cleaned[..max_chars])
    } else {
        cleaned.to_string()
    }
}

fn action_summary(action: &Action) -> String {
    match action {
        Action::ReadFile { path, start_line, end_line } => {
            format!("read_file({}, L{}-{})", path, start_line, end_line)
        }
        Action::WriteFile { path, content } => {
            format!("write_file({}, {} bytes)", path, content.len())
        }
        Action::ApplyPatch { path, .. } => {
            format!("apply_patch({})", path)
        }
        Action::ListDir { path } => {
            format!("list_dir({})", path)
        }
        Action::ExecCommand { command, timeout_seconds } => {
            format!("exec_command(\"{}\", {}s)", truncate_display(command, 60), timeout_seconds)
        }
        Action::GitCheckpoint { action, message } => {
            format!("git_checkpoint({:?}, {:?})", action, message)
        }
        Action::Finish { .. } => "finish()".to_string(),
    }
}
