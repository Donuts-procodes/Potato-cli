use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use anyhow::{Context, Result};
use chrono::Utc;
use tracing::info;

use crate::llm::ChatMessage;

/// A serializable checkpoint of a full agent session.
/// Enables crash recovery and session resume.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionCheckpoint {
    /// Unique session identifier
    pub session_id: String,
    /// The original user objective
    pub objective: String,
    /// Full message history at checkpoint time
    pub messages: Vec<ChatMessage>,
    /// Current turn number
    pub turn: usize,
    /// ISO-8601 timestamp of checkpoint
    pub timestamp: String,
    /// Consecutive failure counter at checkpoint
    pub consecutive_failures: usize,
}

impl SessionCheckpoint {
    pub fn new(
        session_id: String,
        objective: String,
        messages: Vec<ChatMessage>,
        turn: usize,
        consecutive_failures: usize,
    ) -> Self {
        Self {
            session_id,
            objective,
            messages,
            turn,
            timestamp: Utc::now().to_rfc3339(),
            consecutive_failures,
        }
    }

    /// Saves the checkpoint to a JSON file in the given directory.
    pub fn save(&self, session_dir: &str) -> Result<PathBuf> {
        let dir = PathBuf::from(session_dir);
        std::fs::create_dir_all(&dir)?;

        let filename = format!("session_{}.json", self.session_id);
        let path = dir.join(&filename);

        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, content)
            .with_context(|| format!("Failed to save session to {}", path.display()))?;

        info!(path = %path.display(), turn = self.turn, "Session checkpoint saved");
        Ok(path)
    }

    /// Loads the most recent session checkpoint from the directory.
    pub fn load_latest(session_dir: &str) -> Result<Option<Self>> {
        let dir = PathBuf::from(session_dir);
        if !dir.exists() {
            return Ok(None);
        }

        let mut latest: Option<(Self, std::time::SystemTime)> = None;

        for entry in std::fs::read_dir(&dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().map(|e| e == "json").unwrap_or(false) {
                if let Ok(metadata) = entry.metadata() {
                    if let Ok(modified) = metadata.modified() {
                        let content = std::fs::read_to_string(&path)?;
                        if let Ok(checkpoint) = serde_json::from_str::<Self>(&content) {
                            match &latest {
                                Some((_, latest_time)) if modified > *latest_time => {
                                    latest = Some((checkpoint, modified));
                                }
                                None => {
                                    latest = Some((checkpoint, modified));
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
        }

        Ok(latest.map(|(cp, _)| cp))
    }

    /// Loads a specific session by ID.
    pub fn load_by_id(session_dir: &str, session_id: &str) -> Result<Option<Self>> {
        let path = PathBuf::from(session_dir).join(format!("session_{}.json", session_id));
        if !path.exists() {
            return Ok(None);
        }

        let content = std::fs::read_to_string(&path)?;
        let checkpoint: Self = serde_json::from_str(&content)?;
        Ok(Some(checkpoint))
    }
}

/// Metadata summary of a stored session without loading full message history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSummary {
    pub session_id: String,
    pub objective: String,
    pub turn: usize,
    pub timestamp: String,
    pub message_count: usize,
    pub path: PathBuf,
}

/// Resolves the active session directory.
/// Checks local `.potato/sessions` first, then falls back to global user config.
pub fn get_session_dir() -> PathBuf {
    let local = PathBuf::from(".potato").join("sessions");
    if local.exists() || std::fs::create_dir_all(&local).is_ok() {
        return local;
    }

    if let Some(config_dir) = dirs::config_dir() {
        let global = config_dir.join("potato").join("sessions");
        let _ = std::fs::create_dir_all(&global);
        return global;
    }

    local
}

/// Generates a human-readable, chronologically sortable session ID.
pub fn generate_session_id() -> String {
    let now = Utc::now();
    let millis = now.timestamp_subsec_millis();
    format!("{}_{:03}", now.format("%Y%m%d_%H%M%S"), millis)
}

/// Lists all saved session checkpoints in the given directory, ordered newest to oldest.
pub fn list_sessions(session_dir: &std::path::Path) -> Result<Vec<SessionSummary>> {
    if !session_dir.exists() {
        return Ok(Vec::new());
    }

    let mut summaries = Vec::new();

    for entry in std::fs::read_dir(session_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().map(|e| e == "json").unwrap_or(false) {
            let filename = path.file_name().and_then(|f| f.to_str()).unwrap_or("");
            if filename.starts_with("session_") {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    if let Ok(cp) = serde_json::from_str::<SessionCheckpoint>(&content) {
                        summaries.push(SessionSummary {
                            session_id: cp.session_id,
                            objective: cp.objective,
                            turn: cp.turn,
                            timestamp: cp.timestamp,
                            message_count: cp.messages.len(),
                            path: path.clone(),
                        });
                    }
                }
            }
        }
    }

    // Sort descending by timestamp
    summaries.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    Ok(summaries)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_session_save_and_load_roundtrip() {
        let dir = tempdir().unwrap();
        let dir_str = dir.path().to_str().unwrap();

        let msgs = vec![
            ChatMessage {
                role: "system".to_string(),
                content: "sys".to_string(),
            },
            ChatMessage {
                role: "user".to_string(),
                content: "build a web server".to_string(),
            },
        ];

        let cp = SessionCheckpoint::new(
            "test_sess_001".to_string(),
            "build a web server".to_string(),
            msgs.clone(),
            3,
            0,
        );

        let saved_path = cp.save(dir_str).unwrap();
        assert!(saved_path.exists());

        // Load by ID
        let loaded = SessionCheckpoint::load_by_id(dir_str, "test_sess_001")
            .unwrap()
            .expect("Session should be found");
        assert_eq!(loaded.session_id, "test_sess_001");
        assert_eq!(loaded.turn, 3);
        assert_eq!(loaded.messages.len(), 2);

        // Load latest
        let latest = SessionCheckpoint::load_latest(dir_str)
            .unwrap()
            .expect("Latest session should be found");
        assert_eq!(latest.session_id, "test_sess_001");

        // List sessions
        let list = list_sessions(dir.path()).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].session_id, "test_sess_001");
        assert_eq!(list[0].message_count, 2);
    }
}

