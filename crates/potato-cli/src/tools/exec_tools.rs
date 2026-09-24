use crate::types::ExecutionResult;
use std::io::Read;
use std::process::{Command, Stdio};
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

/// Max memory buffer per stream (2MB) to prevent OOM on infinite command output
const MAX_OUTPUT_BYTES: usize = 2 * 1024 * 1024;

/// Executes a shell command with pipe-deadlock prevention, stdin nullification,
/// and comprehensive timeout handling in case of buffering or hanging processes.
pub fn exec_command(command: &str, timeout_seconds: u64) -> ExecutionResult {
    let timeout_secs = if timeout_seconds == 0 { 60 } else { timeout_seconds.min(300) };
    debug!(command, timeout_secs, "Executing shell command with async buffer streaming");

    let mut cmd = shell_command(command);
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = match cmd.spawn() {
        Ok(child) => child,
        Err(e) => {
            return ExecutionResult::err(1, format!("Failed to spawn command: {}", e));
        }
    };

    // Asynchronously drain stdout and stderr in background threads to avoid OS pipe buffer deadlocks
    let mut stdout_pipe = child.stdout.take();
    let stdout_handle = std::thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(mut out) = stdout_pipe.take() {
            let mut chunk = [0u8; 8192];
            while let Ok(n) = out.read(&mut chunk) {
                if n == 0 {
                    break;
                }
                if buf.len() < MAX_OUTPUT_BYTES {
                    buf.extend_from_slice(&chunk[..n]);
                }
            }
        }
        String::from_utf8_lossy(&buf).to_string()
    });

    let mut stderr_pipe = child.stderr.take();
    let stderr_handle = std::thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(mut err) = stderr_pipe.take() {
            let mut chunk = [0u8; 8192];
            while let Ok(n) = err.read(&mut chunk) {
                if n == 0 {
                    break;
                }
                if buf.len() < MAX_OUTPUT_BYTES {
                    buf.extend_from_slice(&chunk[..n]);
                }
            }
        }
        String::from_utf8_lossy(&buf).to_string()
    });

    let timeout = Duration::from_secs(timeout_secs);
    let start = std::time::Instant::now();

    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let stdout = stdout_handle.join().unwrap_or_default();
                let stderr = stderr_handle.join().unwrap_or_default();
                let code = status.code().unwrap_or(-1);
                return ExecutionResult {
                    success: code == 0,
                    exit_code: code,
                    stdout,
                    stderr,
                };
            }
            Ok(None) => {
                if start.elapsed() > timeout {
                    warn!(command, timeout_secs, "Command timed out / stuck, terminating process tree");
                    #[cfg(target_os = "windows")]
                    {
                        // On Windows, child.kill() only kills cmd.exe, leaving spawned grandchildren running.
                        // taskkill /F /T terminates the entire process tree.
                        let pid = child.id();
                        let _ = Command::new("taskkill")
                            .args(["/F", "/T", "/PID", &pid.to_string()])
                            .output();
                    }
                    let _ = child.kill();

                    let stdout = stdout_handle.join().unwrap_or_default();
                    let stderr = stderr_handle.join().unwrap_or_default();
                    return ExecutionResult {
                        success: false,
                        exit_code: 124,
                        stdout,
                        stderr: format!(
                            "⚠️ Command timed out / stuck after {}s. Process tree killed.\nPartial output collected before stall:\n{}",
                            timeout_secs, stderr
                        ),
                    };
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(e) => {
                return ExecutionResult::err(1, format!("Failed to poll child process: {}", e));
            }
        }
    }
}
