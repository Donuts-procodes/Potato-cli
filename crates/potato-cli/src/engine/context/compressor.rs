use crate::llm::ChatMessage;

/// Compresses a sequence of ChatMessages into a dense contextual summary,
/// preserving system directives, critical error traces, and architectural decisions.
pub struct ContextCompressor;

impl ContextCompressor {
    /// Compresses conversation messages older than keep_recent.
    /// Retains the system message at index 0 and the most recent keep_recent messages,
    /// inserting a condensed recap message of the purged turns.
    pub fn compress(messages: &[ChatMessage], keep_recent: usize) -> Vec<ChatMessage> {
        if messages.len() <= keep_recent + 1 {
            return messages.to_vec();
        }

        let mut result = Vec::new();

        // 1. Preserve initial system prompt
        if let Some(system_msg) = messages.first() {
            result.push(system_msg.clone());
        }

        let purge_end = messages.len() - keep_recent;
        let mut key_decisions = Vec::new();
        let mut touched_files = Vec::new();

        for msg in &messages[1..purge_end] {
            if msg.role == "assistant" {
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&msg.content) {
                    if let Some(thought) = val.get("thought").and_then(|t| t.as_str()) {
                        let trimmed = thought.trim();
                        if !trimmed.is_empty() {
                            let snippet = if trimmed.len() > 120 {
                                format!("{}...", &trimmed[..120])
                            } else {
                                trimmed.to_string()
                            };
                            key_decisions.push(snippet);
                        }
                    }
                    if let Some(action) = val.get("action") {
                        if let Some(path) = action.get("path").and_then(|p| p.as_str()) {
                            if !touched_files.contains(&path.to_string()) {
                                touched_files.push(path.to_string());
                            }
                        }
                    }
                }
            }
        }

        // 2. Build dense compression block
        let mut recap = String::from("## COMPRESSED HISTORICAL CONTEXT (AUTOMATICALLY CONDENSED)\n");
        if !touched_files.is_empty() {
            recap.push_str(&format!("- Mutated files: {}\n", touched_files.join(", ")));
        }
        if !key_decisions.is_empty() {
            recap.push_str("- Historical decisions:\n");
            for decision in key_decisions.iter().take(8) {
                recap.push_str(&format!("  * {}\n", decision));
            }
        }

        result.push(ChatMessage {
            role: "user".to_string(),
            content: recap,
        });

        // 3. Append recent turns
        result.extend_from_slice(&messages[purge_end..]);

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compressor_preserves_short_history() {
        let messages = vec![
            ChatMessage {
                role: "system".to_string(),
                content: "sys".to_string(),
            },
            ChatMessage {
                role: "user".to_string(),
                content: "u1".to_string(),
            },
        ];
        let compressed = ContextCompressor::compress(&messages, 5);
        assert_eq!(compressed.len(), 2);
    }

    #[test]
    fn test_compressor_condenses_old_turns() {
        let mut messages = vec![
            ChatMessage {
                role: "system".to_string(),
                content: "system prompt".to_string(),
            },
        ];

        for i in 1..=10 {
            messages.push(ChatMessage {
                role: "assistant".to_string(),
                content: format!(r#"{{"thought": "Decided step {}", "action": {{"type": "write_file", "path": "file_{}.rs"}}}}"#, i, i),
            });
            messages.push(ChatMessage {
                role: "user".to_string(),
                content: format!("Result of step {}", i),
            });
        }

        // Keep only 4 recent messages + system + 1 compression summary
        let compressed = ContextCompressor::compress(&messages, 4);
        assert_eq!(compressed.len(), 6); // 1 system + 1 summary + 4 recent
        assert_eq!(compressed[0].content, "system prompt");
        assert!(compressed[1].content.contains("COMPRESSED HISTORICAL CONTEXT"));
        assert!(compressed[1].content.contains("file_1.rs"));
    }
}
