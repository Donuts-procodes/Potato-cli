use anyhow::{anyhow, Result};
use std::process::Command;
use tracing::{info, warn};

/// Orchestrates parallel speculative branches in Git to explore divergent repair solutions.
pub struct SpeculativeExecutor;

impl SpeculativeExecutor {
    /// Creates an isolated temporary git branch for a speculative agent turn.
    pub fn create_branch(branch_name: &str) -> Result<()> {
        let status = Command::new("git")
            .args(["checkout", "-b", branch_name])
            .status()?;

        if !status.success() {
            return Err(anyhow!("Failed to create speculative branch: {}", branch_name));
        }
        info!(branch = branch_name, "Spawned speculative branch");
        Ok(())
    }

    /// Evaluates candidate branch verification suite.
    pub fn verify_branch(test_command: &str) -> bool {
        #[cfg(target_os = "windows")]
        let status = Command::new("powershell")
            .args(["-NoProfile", "-Command", test_command])
            .status();

        #[cfg(not(target_os = "windows"))]
        let status = Command::new("bash")
            .args(["-c", test_command])
            .status();

        match status {
            Ok(s) => s.success(),
            Err(_) => false,
        }
    }

    /// Merges the winning branch back into the main working tree.
    pub fn merge_winner(target_branch: &str, winning_branch: &str) -> Result<()> {
        info!(winner = winning_branch, target = target_branch, "Merging winning branch");

        let checkout = Command::new("git").args(["checkout", target_branch]).status()?;
        if !checkout.success() {
            return Err(anyhow!("Failed to checkout target branch {}", target_branch));
        }

        let merge = Command::new("git").args(["merge", "--ff-only", winning_branch]).status()?;
        if !merge.success() {
            warn!("Fast-forward merge failed, falling back to standard merge");
            let fallback = Command::new("git").args(["merge", winning_branch, "-m", "chore: merge winning speculative branch"]).status()?;
            if !fallback.success() {
                return Err(anyhow!("Merge failed for winning branch {}", winning_branch));
            }
        }

        // Cleanup speculative branch
        let _ = Command::new("git").args(["branch", "-D", winning_branch]).status();
        Ok(())
    }

    /// Prunes and rolls back an unsuccessful candidate branch.
    pub fn prune_branch(base_branch: &str, failing_branch: &str) -> Result<()> {
        info!(failing_branch, "Pruning failed speculative branch");
        let _ = Command::new("git").args(["checkout", base_branch]).status();
        let _ = Command::new("git").args(["branch", "-D", failing_branch]).status();
        Ok(())
    }
}
