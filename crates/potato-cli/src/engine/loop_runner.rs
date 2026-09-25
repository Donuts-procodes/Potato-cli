use anyhow::Result;
use colored::Colorize;
use tracing::{info, warn};

use crate::engine::arcade::{format_game_over, format_stage_clear, format_stage_header};
use crate::engine::brain::Brain;
use crate::engine::cache::CacheManager;
use crate::engine::cost_tracker::CostTracker;
use crate::engine::executor;
use crate::engine::hooks::{HookContext, HookManager, HookType};
use crate::engine::signal::is_shutdown_requested;
use crate::engine::spinner::Spinner;
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
    history: Vec<ChatMessage>,
    cache_manager: Option<CacheManager>,
    brain: Option<Brain>,
    rules: Option<crate::engine::rules::RulesEngine>,
    approval_gate: crate::engine::approval::ApprovalGate,
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
            history: Vec::new(),
            cache_manager: None,
            brain: None,
            rules: None,
            approval_gate: crate::engine::approval::ApprovalGate::default(),
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

    pub fn with_history(mut self, history: Vec<ChatMessage>) -> Self {
        self.history = history;
        self
    }

    pub fn with_cache(mut self, cache: CacheManager) -> Self {
        self.cache_manager = Some(cache);
        self
    }

    pub fn with_brain(mut self, brain: Brain) -> Self {
        self.brain = Some(brain);
        self
    }

    pub fn with_rules(mut self, rules: crate::engine::rules::RulesEngine) -> Self {
        self.rules = Some(rules);
        self
    }

    pub fn with_approval_gate(mut self, gate: crate::engine::approval::ApprovalGate) -> Self {
        self.approval_gate = gate;
        self
    }

    pub fn with_execution_mode(mut self, mode: crate::engine::approval::ExecutionMode) -> Self {
        self.approval_gate.mode = mode;
        self
    }

    pub async fn run(&self) -> Result<String> {
        let (summary, _) = self.run_session().await?;
        Ok(summary)
    }

    pub async fn run_session(&self) -> Result<(String, Vec<ChatMessage>)> {
        let mut approval_gate = self.approval_gate.clone();
        let mut messages: Vec<ChatMessage> = if self.history.is_empty() {
            let mut system_prompt = build_system_prompt();

            // Inject custom user rules and personality directives (e.g. POTATO.md / GEMINI.md)
            let rules = self.rules.clone().unwrap_or_else(crate::engine::rules::RulesEngine::load);
            if !rules.is_empty() {
                system_prompt.push_str(&rules.formatted_prompt_block());
            }

            let mut msgs = vec![ChatMessage {
                role: "system".to_string(),
                content: system_prompt,
            }];

            if let Some(ref brain) = self.brain {
                let brain_ctx = brain.assemble_context();
                if !brain_ctx.is_empty() {
                    msgs.push(ChatMessage {
                        role: "user".to_string(),
                        content: brain_ctx,
                    });
                }
            }

            msgs.push(ChatMessage {
                role: "user".to_string(),
                content: format!(
                    "## OBJECTIVE\n{}\n\n## INITIAL STATE\nWorking directory is the current directory. Begin with PHASE 1: SPECIFICATION & ARCHITECTURE.",
                    self.objective
                ),
            });
            msgs
        } else {
            let mut msgs = self.history.clone();
            msgs.push(ChatMessage {
                role: "user".to_string(),
                content: format!(
                    "## SUBSEQUENT OBJECTIVE (CONTINUING ACTIVE SESSION)\n{}\n\nContinue building upon the existing architectural state, touched files, and implementation.",
                    self.objective
                ),
            });
            msgs
        };

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

            let current_score = ((turn - 1) as u64 * 100)
                + self.cost_tracker.as_ref().map(|t| t.total_tokens() / 10).unwrap_or(0);
            println!("{}", format_stage_header(turn, self.max_turns, current_score));

            // Check cache or execute LLM call with 8-bit animated loader
            let cached_response = if let Some(ref cache) = self.cache_manager {
                cache.get(self.client.model(), &messages)
            } else {
                None
            };

            let (agent_response, usage) = if let Some(resp) = cached_response {
                println!("   {}", "⚡ Cache Hit: Loaded response from disk cache (0 tokens)".dimmed());
                (resp, crate::llm::Usage { prompt_tokens: 0, completion_tokens: 0, total_tokens: 0 })
            } else {
                let mut spinner = Spinner::start(format!("🕹️  ARCADE THINKING... [{}]", self.client.model()));
                let res = self.client.send_turn_with_usage(&messages).await;
                spinner.stop();

                let (resp, usage) = match res {
                    Ok(val) => val,
                    Err(err) => {
                        warn!(error = %err, turn, "Turn encountered buffering stall or network error");
                        println!(
                            "\n{} {}",
                            "⚠️  LLM BUFFERING / STALL DETECTED:".bold().yellow(),
                            err
                        );
                        println!("{}", "🔄 Auto-recovering: attempting retry in 2 seconds...".cyan());
                        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

                        let mut retry_spinner = Spinner::start(format!("🕹️  RECOVERING TURN... [{}]", self.client.model()));
                        let retry_res = self.client.send_turn_with_usage(&messages).await;
                        retry_spinner.stop();

                        match retry_res {
                            Ok(recovered) => {
                                println!("{}", "  ✓ Recovered successfully from buffering stall!".green().bold());
                                recovered
                            }
                            Err(fatal) => {
                                return Err(anyhow::anyhow!("Turn {} failed after recovery retry: {}", turn, fatal));
                            }
                        }
                    }
                };

                if let Some(ref cache) = self.cache_manager {
                    let _ = cache.set(self.client.model(), &messages, &resp);
                }

                (resp, usage)
            };

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
                "💭 [8-BIT THOUGHT]:".bold().yellow(),
                truncate_display(&agent_response.thought, 200)
            );
            println!(
                "{} {:?}",
                "📍 [STAGE PHASE]:".bold().magenta(),
                agent_response.phase
            );
            println!(
                "{} {}",
                "🔧 [ACTION COMBO]:".bold().green(),
                action_summary(&agent_response.action)
            );

            // Check for finish
            if let Action::Finish { ref summary } = agent_response.action {
                println!("{}", format_stage_clear(summary));

                if let Some(ref tracker) = self.cost_tracker {
                    println!("{}", tracker.format_turn_report());
                }

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

                // Auto-update Brain
                if let Some(ref brain) = self.brain {
                    let _ = brain.auto_update(&self.objective, &messages, summary, true);
                }

                return Ok((summary.clone(), messages));
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

            // Interactive human-in-the-loop approval gate
            match approval_gate.request_approval(&agent_response.action)? {
                crate::engine::approval::ApprovalDecision::Approved
                | crate::engine::approval::ApprovalDecision::AlwaysAllowCategory(_) => {
                    // Approved to proceed
                }
                crate::engine::approval::ApprovalDecision::Rejected(reason) => {
                    println!("{} {}", "  🛑 ACTION REJECTED BY USER:".bold().yellow(), reason);
                    messages.push(ChatMessage {
                        role: "assistant".to_string(),
                        content: serde_json::to_string(&agent_response)?,
                    });
                    messages.push(ChatMessage {
                        role: "user".to_string(),
                        content: format!("TOOL RESULT:\nexit_code: 1\nstdout:\n\nstderr:\n{}", reason),
                    });
                    consecutive_failures += 1;
                    continue;
                }
                crate::engine::approval::ApprovalDecision::Abort => {
                    return Err(anyhow::anyhow!("Super Loop execution aborted by user"));
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

            // Execute the action with 8-bit animated loader
            let mut spinner = Spinner::start(format!("⚡ EXECUTING COMBO: {}", action_summary(&agent_response.action)));
            let result = executor::dispatch(&agent_response.action);
            spinner.stop();

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
                    format_game_over(&format!(
                        "⚠️  {} consecutive failures detected — injecting rollback hint",
                        consecutive_failures
                    ))
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
