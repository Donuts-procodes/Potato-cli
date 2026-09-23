use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "name", content = "params")]
pub enum Action {
    #[serde(rename = "read_file")]
    ReadFile {
        path: String,
        start_line: usize,
        end_line: usize,
    },
    #[serde(rename = "write_file")]
    WriteFile {
        path: String,
        content: String,
    },
    #[serde(rename = "apply_patch")]
    ApplyPatch {
        path: String,
        search: String,
        replace: String,
    },
    #[serde(rename = "list_dir")]
    ListDir {
        path: String,
    },
    #[serde(rename = "exec_command")]
    ExecCommand {
        command: String,
        #[serde(default = "default_timeout")]
        timeout_seconds: u64,
    },
    #[serde(rename = "git_checkpoint")]
    GitCheckpoint {
        action: GitAction,
        message: Option<String>,
    },
    #[serde(rename = "finish")]
    Finish {
        summary: String,
    },
}

fn default_timeout() -> u64 {
    120
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum GitAction {
    Commit,
    Rollback,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    pub success: bool,
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
}

impl ExecutionResult {
    pub fn ok(output: String) -> Self {
        Self {
            success: true,
            exit_code: 0,
            stdout: output,
            stderr: String::new(),
        }
    }

    pub fn err(code: i32, err: String) -> Self {
        Self {
            success: false,
            exit_code: code,
            stdout: String::new(),
            stderr: err,
        }
    }
}
