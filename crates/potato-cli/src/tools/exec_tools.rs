use crate::types::ExecutionResult;
use std::io::Read;
use std::process::Command;
use std::time::Duration;
use tracing::{debug, warn};

#[cfg(target_os = "windows")]
fn shell_command(cmd_str: &str) -> Command {
    let mut cmd = Command::new("cmd");
    cmd.args(["/C", cmd_str]);
    cmd
}

#[cfg(not(target_os = "windows"))]
fn shell_command(cmd_str: &str) -> Command {
    let mut cmd = Command::new("sh");
    cmd.args(["-c", cmd_str]);
    cmd
}

pub fn exec_command(command: &str, timeout_seconds: u64) -> ExecutionResult {
    debug!(command, timeout_seconds, "Executing shell command");

    let mut child = match shell_command(command)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(e) => {
            return ExecutionResult::err(1, format!("Failed to spawn command: {}", e));
        }
    };

    let timeout = Duration::from_secs(timeout_seconds);
    let start = std::time::Instant::now();

    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let mut stdout_buf = String::new();
                if let Some(mut out) = child.stdout.take() {
                    let _ = out.read_to_string(&mut stdout_buf);
                }

                let mut stderr_buf = String::new();
                if let Some(mut err) = child.stderr.take() {
                    let _ = err.read_to_string(&mut stderr_buf);
                }

                let code = status.code().unwrap_or(-1);
                return ExecutionResult {
                    success: code == 0,
                    exit_code: code,
                    stdout: stdout_buf,
                    stderr: stderr_buf,
                };
            }
            Ok(None) => {
                if start.elapsed() > timeout {
                    let _ = child.kill();
                    warn!(command, "Command timed out, killed process");
                    return ExecutionResult::err(
                        124,
                        format!("Command timed out after {} seconds", timeout_seconds),
                    );
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(e) => {
                return ExecutionResult::err(1, format!("Failed to poll child process: {}", e));
            }
        }
    }
}
