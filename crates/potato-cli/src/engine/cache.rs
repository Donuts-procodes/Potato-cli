use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use tracing::info;

use crate::llm::ChatMessage;
use crate::types::AgentTurnResponse;

/// Tracks cache performance metrics.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub cached_entries: usize,
    pub cache_dir: String,
}

struct CacheInner {
    enabled: bool,
    cache_dir: PathBuf,
    mem_cache: Mutex<HashMap<String, AgentTurnResponse>>,
    hits: AtomicU64,
    misses: AtomicU64,
}

/// Persistent and in-memory LLM response cache.
/// Prevents redundant token consumption for identical query trajectories and static audits.
#[derive(Clone)]
pub struct CacheManager {
    inner: Arc<CacheInner>,
}

impl CacheManager {
    /// Initializes cache with an optional custom directory.
    /// Falls back to `.potato/cache` or global user cache.
    pub fn new(cache_dir: Option<PathBuf>, enabled: bool) -> Self {
        let dir = cache_dir.unwrap_or_else(get_default_cache_dir);
        if enabled {
            let _ = std::fs::create_dir_all(&dir);
        }

        Self {
            inner: Arc::new(CacheInner {
                enabled,
                cache_dir: dir,
                mem_cache: Mutex::new(HashMap::new()),
                hits: AtomicU64::new(0),
                misses: AtomicU64::new(0),
            }),
        }
    }

    /// Computes a 64-bit deterministic hash key for (model, messages).
    fn compute_key(model: &str, messages: &[ChatMessage]) -> String {
        let mut hasher = DefaultHasher::new();
        model.hash(&mut hasher);
        for msg in messages {
            msg.role.hash(&mut hasher);
            msg.content.hash(&mut hasher);
        }
        format!("{:016x}", hasher.finish())
    }

    /// Retrieves a cached AgentTurnResponse if present in memory or on disk.
    pub fn get(&self, model: &str, messages: &[ChatMessage]) -> Option<AgentTurnResponse> {
        if !self.inner.enabled {
            return None;
        }

        let key = Self::compute_key(model, messages);

        // 1. Check in-memory cache
        {
            let mem = self.inner.mem_cache.lock().unwrap();
            if let Some(resp) = mem.get(&key) {
                self.inner.hits.fetch_add(1, Ordering::Relaxed);
                return Some(resp.clone());
            }
        }

        // 2. Check disk cache
        let file_path = self.inner.cache_dir.join(format!("llm_{}.json", key));
        if file_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&file_path) {
                if let Ok(resp) = serde_json::from_str::<AgentTurnResponse>(&content) {
                    // Populate memory cache
                    let mut mem = self.inner.mem_cache.lock().unwrap();
                    mem.insert(key, resp.clone());
                    self.inner.hits.fetch_add(1, Ordering::Relaxed);
                    return Some(resp);
                }
            }
        }

        self.inner.misses.fetch_add(1, Ordering::Relaxed);
        None
    }

    /// Stores an AgentTurnResponse into memory and disk cache.
    pub fn set(&self, model: &str, messages: &[ChatMessage], response: &AgentTurnResponse) -> Result<()> {
        if !self.inner.enabled {
            return Ok(());
        }

        let key = Self::compute_key(model, messages);

        // 1. Save to memory cache
        {
            let mut mem = self.inner.mem_cache.lock().unwrap();
            mem.insert(key.clone(), response.clone());
        }

        // 2. Save to disk cache
        let file_path = self.inner.cache_dir.join(format!("llm_{}.json", key));
        let content = serde_json::to_string(response)
            .context("Failed to serialize AgentTurnResponse for caching")?;
        let _ = std::fs::write(&file_path, content);
        info!(key = %key, "Cached LLM response to disk");

        Ok(())
    }

    /// Clears all disk and in-memory cache entries. Returns count of deleted files.
    pub fn clear(&self) -> Result<usize> {
        let mut count = 0;
        {
            let mut mem = self.inner.mem_cache.lock().unwrap();
            mem.clear();
        }

        if self.inner.cache_dir.exists() {
            for entry in std::fs::read_dir(&self.inner.cache_dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.extension().map(|e| e == "json").unwrap_or(false)
                    && std::fs::remove_file(path).is_ok()
                {
                    count += 1;
                }
            }
        }

        Ok(count)
    }

    /// Returns performance statistics of the cache.
    pub fn stats(&self) -> CacheStats {
        let mut count = 0;
        if self.inner.cache_dir.exists() {
            if let Ok(entries) = std::fs::read_dir(&self.inner.cache_dir) {
                count = entries
                    .flatten()
                    .filter(|e| e.path().extension().map(|x| x == "json").unwrap_or(false))
                    .count();
            }
        }

        CacheStats {
            hits: self.inner.hits.load(Ordering::Relaxed),
            misses: self.inner.misses.load(Ordering::Relaxed),
            cached_entries: count,
            cache_dir: self.inner.cache_dir.display().to_string(),
        }
    }
}

/// Resolves default cache directory (.potato/cache or global config fallback).
pub fn get_default_cache_dir() -> PathBuf {
    let local = PathBuf::from(".potato").join("cache");
    if local.exists() || std::fs::create_dir_all(&local).is_ok() {
        return local;
    }

    if let Some(config_dir) = dirs::config_dir() {
        let global = config_dir.join("potato").join("cache");
        let _ = std::fs::create_dir_all(&global);
        return global;
    }

    local
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Action, Phase};
    use tempfile::tempdir;

    #[test]
    fn test_cache_miss_then_hit() {
        let dir = tempdir().unwrap();
        let cache = CacheManager::new(Some(dir.path().to_path_buf()), true);

        let msgs = vec![
            ChatMessage { role: "system".to_string(), content: "test system".to_string() },
            ChatMessage { role: "user".to_string(), content: "hello".to_string() },
        ];

        // Initial miss
        assert!(cache.get("gpt-4o", &msgs).is_none());
        assert_eq!(cache.stats().misses, 1);
        assert_eq!(cache.stats().hits, 0);

        // Populate
        let resp = AgentTurnResponse {
            thought: "I will greet the user".to_string(),
            phase: Phase::Specification,
            action: Action::Finish { summary: "Done".to_string() },
        };
        cache.set("gpt-4o", &msgs, &resp).unwrap();

        // Second lookup: Hit
        let cached = cache.get("gpt-4o", &msgs).expect("Should hit cache");
        assert_eq!(cached.thought, "I will greet the user");
        assert_eq!(cache.stats().hits, 1);
    }

    #[test]
    fn test_cache_clear() {
        let dir = tempdir().unwrap();
        let cache = CacheManager::new(Some(dir.path().to_path_buf()), true);

        let msgs = vec![ChatMessage { role: "user".to_string(), content: "foo".to_string() }];
        let resp = AgentTurnResponse {
            thought: "bar".to_string(),
            phase: Phase::Implementation,
            action: Action::Finish { summary: "ok".to_string() },
        };

        cache.set("gpt-4o", &msgs, &resp).unwrap();
        assert_eq!(cache.stats().cached_entries, 1);

        let deleted = cache.clear().unwrap();
        assert_eq!(deleted, 1);
        assert_eq!(cache.stats().cached_entries, 0);
        assert!(cache.get("gpt-4o", &msgs).is_none());
    }
}
