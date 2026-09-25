use colored::Colorize;
use serde::{Deserialize, Serialize};
use std::io::{self, BufRead, Write};

use crate::types::Action;

/// Controls how autonomous the agent is allowed to be when executing actions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionMode {
    /// Fully autonomous execution. Executes actions without prompting (respecting blocklists).
    #[default]
    Autonomous,
    /// Human-in-the-loop (like Claude Code & Roo Code). Prompts user before executing
    /// shell commands, file writes, or file modifications.
    Supervised,
    /// Strictly read-only analysis. Mutating actions (writes, patches, commands) are prohibited.
    ReadOnly,
}

impl std::fmt::Display for ExecutionMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Autonomous => write!(f, "Autonomous (Yolo)"),
            Self::Supervised => write!(f, "Supervised (Human-in-the-loop)"),
            Self::ReadOnly => write!(f, "Read-Only (Analysis)"),
        }
    }
}

/// The outcome of an interactive approval request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApprovalDecision {
    /// Action is approved for this turn.
    Approved,
    /// Action is approved, and all future actions of this category are allowed for this session.
    AlwaysAllowCategory(String),
    /// User rejected the action and provided explanatory guidance to steer the agent.
    Rejected(String),
    /// User requested an immediate abort of the agent loop.
    Abort,
}

/// Manages interactive action approval, policy enforcement, and mid-loop steering.
#[derive(Debug, Clone)]
pub struct ApprovalGate {
    pub mode: ExecutionMode,
    always_allowed_actions: Vec<String>,
}

impl Default for ApprovalGate {
    fn default() -> Self {
        Self::new(ExecutionMode::Autonomous)
    }
}

impl ApprovalGate {
    pub fn new(mode: ExecutionMode) -> Self {
        Self {
            mode,
            always_allowed_actions: Vec::new(),
        }
    }

    /// Checks whether an action requires approval or is blocked by read-only mode.
    pub fn request_approval(&mut self, action: &Action) -> Result<ApprovalDecision, io::Error> {
        let action_name = match action {
            Action::ReadFile { .. } => "read_file",
            Action::WriteFile { .. } => "write_file",
            Action::ApplyPatch { .. } => "apply_patch",
            Action::ListDir { .. } => "list_dir",
            Action::ExecCommand { .. } => "exec_command",
            Action::GitCheckpoint { .. } => "git_checkpoint",
            Action::Finish { .. } => "finish",
        };

        // Read-only actions (read_file, list_dir, finish) never need interactive prompting
        let is_read_only = matches!(
            action,
            Action::ReadFile { .. } | Action::ListDir { .. } | Action::Finish { .. }
        );

        if is_read_only {
            return Ok(ApprovalDecision::Approved);
        }

        // In ReadOnly mode, mutating actions are hard blocked
        if self.mode == ExecutionMode::ReadOnly {
            return Ok(ApprovalDecision::Rejected(
                "Execution mode is currently set to ReadOnly. File writes, patches, and command executions are prohibited."
                    .to_string(),
            ));
        }

        // In Autonomous mode, proceed automatically
        if self.mode == ExecutionMode::Autonomous {
            return Ok(ApprovalDecision::Approved);
        }

        // In Supervised mode, check if user already approved this action category for the session
        if self.always_allowed_actions.iter().any(|a| a == action_name) {
            return Ok(ApprovalDecision::Approved);
        }

        // Prompt user interactively
        self.prompt_user(action, action_name)
    }

    fn prompt_user(&mut self, action: &Action, action_name: &str) -> Result<ApprovalDecision, io::Error> {
        let separator = "─".repeat(60).dimmed();
        println!("\n{}", separator);
        println!("{}", "🛡️  ACTION APPROVAL REQUIRED (Supervised Mode)".bold().yellow());
        match action {
            Action::ExecCommand { command, timeout_seconds } => {
                println!("  Action  : {}", "Execute Shell Command".bold().red());
                println!("  Command : {}", command.bold().white());
                println!("  Timeout : {}s", timeout_seconds);
            }
            Action::WriteFile { path, content } => {
                println!("  Action  : {}", "Create New File".bold().green());
                println!("  Path    : {}", path.bold().white());
                println!("  Bytes   : {} bytes ({} lines)", content.len(), content.lines().count());
            }
            Action::ApplyPatch { path, search, replace } => {
                println!("  Action  : {}", "Surgically Patch File".bold().cyan());
                println!("  Path    : {}", path.bold().white());
                println!("  Search  : {} chars", search.len());
                println!("  Replace : {} chars", replace.len());
            }
            Action::GitCheckpoint { action, message } => {
                println!("  Action  : {}", "Git Checkpoint Mutation".bold().magenta());
                println!("  Type    : {:?}", action);
                if let Some(msg) = message {
                    println!("  Message : {}", msg);
                }
            }
            _ => {
                println!("  Action  : {}", action_name.bold().white());
            }
        }
        println!("{}", separator);
        println!(
            "  {} [y] Approve once | [a] Always allow '{}' | [r] Reject with feedback | [q] Abort",
            "Options:".bold().cyan(),
            action_name
        );
        print!("{}", "  Decision [y/a/r/q] ❯ ".bold().yellow());
        io::stdout().flush()?;

        let mut input = String::new();
        let stdin = io::stdin();
        let mut reader = stdin.lock();
        reader.read_line(&mut input)?;

        match input.trim().to_lowercase().as_str() {
            "y" | "yes" | "" => {
                println!("{}", "  ✓ Action approved.".green());
                Ok(ApprovalDecision::Approved)
            }
            "a" | "always" => {
                self.always_allowed_actions.push(action_name.to_string());
                println!("{} Always allowing '{}' for the rest of this session.", "✓".green(), action_name);
                Ok(ApprovalDecision::AlwaysAllowCategory(action_name.to_string()))
            }
            "r" | "reject" | "n" | "no" => {
                print!("{}", "  Enter guidance / reason for rejection (or press Enter): ".bold().yellow());
                io::stdout().flush()?;
                let mut reason = String::new();
                reader.read_line(&mut reason)?;
                let trimmed = reason.trim();
                let final_reason = if trimmed.is_empty() {
                    format!("User rejected action '{}'. Try an alternative approach.", action_name)
                } else {
                    format!("User rejected action with guidance: {}", trimmed)
                };
                println!("{} Action rejected. Feeding guidance back to agent.", "⛔".red());
                Ok(ApprovalDecision::Rejected(final_reason))
            }
            "q" | "abort" | "exit" => {
                println!("{}", "  🛑 Super Loop session aborted by user.".red().bold());
                Ok(ApprovalDecision::Abort)
            }
            _ => {
                println!("{}", "  Unrecognized input. Defaulting to rejection.".yellow());
                Ok(ApprovalDecision::Rejected("Unrecognized input from user; action rejected.".to_string()))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_autonomous_mode_allows_all() {
        let mut gate = ApprovalGate::new(ExecutionMode::Autonomous);
        let action = Action::ExecCommand {
            command: "cargo test".to_string(),
            timeout_seconds: 60,
        };
        assert_eq!(gate.request_approval(&action).unwrap(), ApprovalDecision::Approved);
    }

    #[test]
    fn test_read_only_mode_blocks_mutations() {
        let mut gate = ApprovalGate::new(ExecutionMode::ReadOnly);
        let write_action = Action::WriteFile {
            path: "test.txt".to_string(),
            content: "hello".to_string(),
        };
        let res = gate.request_approval(&write_action).unwrap();
        match res {
            ApprovalDecision::Rejected(msg) => assert!(msg.contains("ReadOnly")),
            _ => panic!("Expected rejection in read-only mode"),
        }

        // Read actions must be allowed
        let read_action = Action::ReadFile {
            path: "test.txt".to_string(),
            start_line: 1,
            end_line: 10,
        };
        assert_eq!(gate.request_approval(&read_action).unwrap(), ApprovalDecision::Approved);
    }
}
