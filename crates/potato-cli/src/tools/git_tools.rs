use crate::types::{ExecutionResult, GitAction};
use tracing::info;

pub fn git_checkpoint(action: &GitAction, message: Option<&str>) -> ExecutionResult {
    match action {
        GitAction::Commit => {
            let msg = message.unwrap_or("chore: auto checkpoint");
            info!(message = msg, "Git commit checkpoint");

            let add_result = super::exec_tools::exec_command("git add -A", 30);
            if !add_result.success {
                return ExecutionResult::err(
                    add_result.exit_code,
                    format!("git add failed: {}", add_result.stderr),
                );
            }

            let commit_cmd = format!("git commit -m \"{}\" --allow-empty", msg.replace('"', "\\\""));
            let commit_result = super::exec_tools::exec_command(&commit_cmd, 30);
            if !commit_result.success {
                return ExecutionResult::err(
                    commit_result.exit_code,
                    format!("git commit failed: {}", commit_result.stderr),
                );
            }

            ExecutionResult::ok(format!("Committed checkpoint: {}", msg))
        }
        GitAction::Rollback => {
            info!("Rolling back to last committed state");

            let reset_result = super::exec_tools::exec_command("git checkout -- .", 30);
            if !reset_result.success {
                let hard_reset = super::exec_tools::exec_command("git reset --hard HEAD", 30);
                if !hard_reset.success {
                    return ExecutionResult::err(
                        hard_reset.exit_code,
                        format!("git rollback failed: {}", hard_reset.stderr),
                    );
                }
            }

            ExecutionResult::ok("Rolled back to last committed state".to_string())
        }
    }
}
