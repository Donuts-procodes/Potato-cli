use tracing::info;

use crate::tools::{exec_tools, fs_tools, git_tools};
use crate::types::{Action, ExecutionResult};

/// Dispatches a single parsed `Action` to the appropriate filesystem/shell/git tool.
/// Returns an `ExecutionResult` that gets fed back into the LLM context window.
pub fn dispatch(action: &Action) -> ExecutionResult {
    match action {
        Action::ReadFile {
            path,
            start_line,
            end_line,
        } => {
            info!(path, start_line, end_line, "ACTION: read_file");
            fs_tools::read_file(path, *start_line, *end_line)
        }
        Action::WriteFile { path, content } => {
            info!(path, bytes = content.len(), "ACTION: write_file");
            fs_tools::write_file(path, content)
        }
        Action::ApplyPatch {
            path,
            search,
            replace,
        } => {
            info!(path, "ACTION: apply_patch");
            fs_tools::apply_patch(path, search, replace)
        }
        Action::ListDir { path } => {
            info!(path, "ACTION: list_dir");
            fs_tools::list_dir(path)
        }
        Action::ExecCommand {
            command,
            timeout_seconds,
        } => {
            info!(command, timeout_seconds, "ACTION: exec_command");
            exec_tools::exec_command(command, *timeout_seconds)
        }
        Action::GitCheckpoint { action, message } => {
            info!(?action, "ACTION: git_checkpoint");
            git_tools::git_checkpoint(action, message.as_deref())
        }
        Action::Finish { summary } => {
            info!("ACTION: finish");
            ExecutionResult::ok(summary.clone())
        }
    }
}
