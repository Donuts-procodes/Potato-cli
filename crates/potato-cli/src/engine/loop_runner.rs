use anyhow::{Context, Result};
use colored::Colorize;
use tracing::{info, warn};

use crate::engine::cost_tracker::CostTracker;
use crate::engine::executor;
use crate::engine::hooks::{HookContext, HookManager, HookType};
use crate::engine::signal::is_shutdown_requested;
use crate::engine::tool_policy::ToolPolicy;
use crate::llm::{build_system_prompt, ChatMessage, LlmClient};
use crate::tools::diff_display::unified_diff;
use crate::types::Action;

const MAX_TURNS: usize = 200;
const MAX_CONSECUTIVE_FAILURES: usize = 5;

pub struct LoopRunner {
    client: LlmClient,
    objective: String,
    max_turns: usize,
    cost_tracker: Option<CostTracker>,
    tool_policy: Option<ToolPolicy>,
    hook_manager: Option<HookManager>,
}

impl LoopRunner {
    pub fn new(client: LlmClient, objective: String) -> Self {
        Self {
            client,
            objective,
            max_turns: MAX_TURNS,
            cost_tracker: None,
            tool_policy: None,
            hook_manager: None,
        }
    }

    pub fn with_max_turns(mut self, turns: usize) -> Self {
        self.max_turns = turns;
        self
    }

    pub fn with_cost_tracker(mut self, tracker: CostTracker) -> Self {
        self.cost_tracker = Some(tracker);
        self
    }

    pub fn with_tool_policy(mut self, policy: ToolPolicy) -> Self {
        self.tool_policy = Some(policy);
        self
    }

    pub fn with_hook_manager(mut self, hooks: HookManager) -> Self {
        self.hook_manager = Some(hooks);
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
            // Graceful shutdown check
            if is_shutdown_requested() {
                println!("\n{}", "🛑 Graceful shutdown — committing checkpoint...".bold().yellow());
                if let Some(ref hooks) = self.hook_manager {
                    let ctx = HookContext {
                        turn_number: Some(turn),
                        ..HookContext::empty()
                    };
                    hooks.run_hooks(&HookType::OnShutdown, &ctx);
                }
                let _ = crate::tools::git_tools::git_checkpoint(
                    &crate::types::GitAction::Commit,
                    Some("chore: graceful shutdown checkpoint"),
                );
                return Err(anyhow::anyhow!("Shutdown requested by user (Ctrl+C)"));
            }

            println!(
                "\n{} {}",
                format!("━━━ Turn {}/{}", turn, self.max_turns).bold().cyan(),
                "━".repeat(50).dimmed()
            );

            let (agent_response, usage) = self
                .client
                .send_turn_with_usage(&messages)
                .await
                .with_context(|| format!("LLM call failed on turn {}", turn))?;

            // Track cost and tokens
            if let Some(ref tracker) = self.cost_tracker {
                if let Err(exceeded) = tracker.record(usage.prompt_tokens, usage.completion_tokens) {
                    println!("\n{}", format!("🛑 BUDGET EXCEEDED: {}", exceeded).bold().red());
                    return Err(anyhow::anyhow!(exceeded));
                }
                println!("   {}", tracker.summary().dimmed());
            }

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

                if let Some(ref hooks) = self.hook_manager {
                    let ctx = HookContext {
                        turn_number: Some(turn),
                        ..HookContext::empty()
                    };
                    hooks.run_hooks(&HookType::OnComplete, &ctx);
                }

                messages.push(ChatMessage {
                    role: "assistant".to_string(),
                    content: serde_json::to_string(&agent_response)?,
                });

                return Ok(summary.clone());
            }

            // Check ToolPolicy permissions
            let action_name = action_variant_name(&agent_response.action);
            if let Some(ref policy) = self.tool_policy {
                if !policy.is_action_allowed("default", action_name) {
                    let denial = policy.denial_message("default", action_name);
                    println!("{} {}", "  ⛔ BLOCKED:".bold().red(), denial);
                    messages.push(ChatMessage {
                        role: "assistant".to_string(),
                        content: serde_json::to_string(&agent_response)?,
                    });
                    messages.push(ChatMessage {
                        role: "user".to_string(),
                        content: format!("TOOL RESULT:\nexit_code: 1\nstdout:\n\nstderr:\n{}", denial),
                    });
                    consecutive_failures += 1;
                    continue;
                }

                if let Action::ExecCommand { ref command, .. } = agent_response.action {
                    if !policy.is_command_allowed("default", command) {
                        let denial = policy.denial_message("default", command);
                        println!("{} {}", "  ⛔ BLOCKED:".bold().red(), denial);
                        messages.push(ChatMessage {
                            role: "assistant".to_string(),
                            content: serde_json::to_string(&agent_response)?,
                        });
                        messages.push(ChatMessage {
                            role: "user".to_string(),
                            content: format!("TOOL RESULT:\nexit_code: 1\nstdout:\n\nstderr:\n{}", denial),
                        });
                        consecutive_failures += 1;
                        continue;
                    }
                }
            }

            // Capture pre-action state for diff display & hooks
            let (old_content, target_path) = match &agent_response.action {
                Action::WriteFile { path, .. } | Action::ApplyPatch { path, .. } => {
                    let old = std::fs::read_to_string(path).unwrap_or_default();
                    if let Some(ref hooks) = self.hook_manager {
                        let ctx = HookContext {
                            file_path: Some(path.clone()),
                            action_type: Some(action_name.to_string()),
                            turn_number: Some(turn),
                            agent_name: Some("default".to_string()),
                            exit_code: None,
                        };
                        hooks.run_hooks(&HookType::PreWrite, &ctx);
                    }
                    (Some(old), Some(path.clone()))
                }
                Action::ExecCommand { command, .. } => {
                    if let Some(ref hooks) = self.hook_manager {
                        let ctx = HookContext {
                            action_type: Some(format!("exec: {}", command)),
                            turn_number: Some(turn),
                            agent_name: Some("default".to_string()),
                            file_path: None,
                            exit_code: None,
                        };
                        hooks.run_hooks(&HookType::PreExec, &ctx);
                    }
                    (None, None)
                }
                _ => (None, None),
            };

            // Pre-write AST validation check
            if let Action::WriteFile { ref path, ref content } = agent_response.action {
                if let Err(ast_err) = crate::tools::AstValidator::validate(path, content) {
                    println!("{} {}", "  ⚠️ AST SYNTAX GATEKEEPER DENIAL:".bold().yellow(), ast_err);
                    messages.push(ChatMessage {
                        role: "assistant".to_string(),
                        content: serde_json::to_string(&agent_response)?,
                    });
                    messages.push(ChatMessage {
                        role: "user".to_string(),
                        content: format!("PRE-WRITE AST SYNTAX ERROR:\nFile: {}\nError: {}\nYou must fix syntax errors before writing.", path, ast_err),
                    });
                    consecutive_failures += 1;
                    continue;
                }
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

                // Show unified diff for file modifications
                if let (Some(ref old), Some(ref path)) = (old_content, target_path) {
                    let new = std::fs::read_to_string(path).unwrap_or_default();
                    let diff = unified_diff(path, old, &new);
                    if !diff.is_empty() {
                        println!("\n{}\n", diff);
                    }
                    if let Some(ref hooks) = self.hook_manager {
                        let ctx = HookContext {
                            file_path: Some(path.clone()),
                            action_type: Some(action_name.to_string()),
                            exit_code: Some(0),
                            turn_number: Some(turn),
                            agent_name: Some("default".to_string()),
                        };
                        hooks.run_hooks(&HookType::PostWrite, &ctx);
                    }
                }

                // Post-commit hook
                if let Action::GitCheckpoint { .. } = &agent_response.action {
                    if let Some(ref hooks) = self.hook_manager {
                        let ctx = HookContext {
                            action_type: Some("git_checkpoint".to_string()),
                            exit_code: Some(0),
                            turn_number: Some(turn),
                            agent_name: Some("default".to_string()),
                            file_path: None,
                        };
                        hooks.run_hooks(&HookType::PostCommit, &ctx);
                    }
                }

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

                if let Some(ref hooks) = self.hook_manager {
                    let ctx = HookContext {
                        turn_number: Some(turn),
                        exit_code: Some(result.exit_code),
                        agent_name: Some("default".to_string()),
                        file_path: None,
                        action_type: None,
                    };
                    hooks.run_hooks(&HookType::OnFailure, &ctx);
                }

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

            // Context window management: compress historical context when exceeding limit
            if messages.len() > 80 {
                messages = crate::engine::context::ContextCompressor::compress(&messages, 40);
                info!("Compressed context window to {} messages", messages.len());
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

fn action_variant_name(action: &Action) -> &'static str {
    match action {
        Action::ReadFile { .. } => "read_file",
        Action::WriteFile { .. } => "write_file",
        Action::ApplyPatch { .. } => "apply_patch",
        Action::ListDir { .. } => "list_dir",
        Action::ExecCommand { .. } => "exec_command",
        Action::GitCheckpoint { .. } => "git_checkpoint",
        Action::Finish { .. } => "finish",
    }
}
