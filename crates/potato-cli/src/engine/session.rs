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
