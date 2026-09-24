use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use tracing::warn;

struct CostTrackerInner {
    total_prompt_tokens: AtomicU64,
    total_completion_tokens: AtomicU64,
    total_turns: AtomicU64,
    last_turn_prompt_tokens: AtomicU32,
    last_turn_completion_tokens: AtomicU32,
    max_cost_usd: Mutex<f64>,
    warn_at_usd: Mutex<f64>,
    max_tokens: AtomicU64,
    cost_per_prompt_token: f64,
    cost_per_completion_token: f64,
    warned: Mutex<bool>,
}

/// Tracks cumulative token usage and estimated cost across the entire session.
/// Thread-safe and cloneable for concurrent or shared access.
#[derive(Clone)]
pub struct CostTracker {
    inner: Arc<CostTrackerInner>,
}

impl CostTracker {
    /// Creates a new cost tracker with budget limits.
    /// Default pricing is GPT-4o: $2.50/1M prompt, $10.00/1M completion.
    /// If `max_cost_usd <= 0.0`, cost budget is unlimited.
    pub fn new(max_cost_usd: f64, warn_at_usd: f64) -> Self {
        Self {
            inner: Arc::new(CostTrackerInner {
                total_prompt_tokens: AtomicU64::new(0),
                total_completion_tokens: AtomicU64::new(0),
                total_turns: AtomicU64::new(0),
                last_turn_prompt_tokens: AtomicU32::new(0),
                last_turn_completion_tokens: AtomicU32::new(0),
                max_cost_usd: Mutex::new(max_cost_usd),
                warn_at_usd: Mutex::new(warn_at_usd),
                max_tokens: AtomicU64::new(0),
                cost_per_prompt_token: 2.50 / 1_000_000.0,
                cost_per_completion_token: 10.00 / 1_000_000.0,
                warned: Mutex::new(false),
            }),
        }
    }

    /// Configures a maximum session token limit (0 = unlimited).
    pub fn with_token_limit(self, max_tokens: u64) -> Self {
        self.inner.max_tokens.store(max_tokens, Ordering::Relaxed);
        self
    }

    /// Sets custom pricing per token (for non-GPT-4o models).
    pub fn with_pricing(mut self, prompt_per_million: f64, completion_per_million: f64) -> Self {
        if let Some(inner) = Arc::get_mut(&mut self.inner) {
            inner.cost_per_prompt_token = prompt_per_million / 1_000_000.0;
            inner.cost_per_completion_token = completion_per_million / 1_000_000.0;
        } else {
            let total_prompt = self.inner.total_prompt_tokens.load(Ordering::Relaxed);
            let total_completion = self.inner.total_completion_tokens.load(Ordering::Relaxed);
            let total_turns = self.inner.total_turns.load(Ordering::Relaxed);
            let last_turn_prompt = self.inner.last_turn_prompt_tokens.load(Ordering::Relaxed);
            let last_turn_completion = self.inner.last_turn_completion_tokens.load(Ordering::Relaxed);
            let max_cost = *self.inner.max_cost_usd.lock().unwrap();
            let warn_at = *self.inner.warn_at_usd.lock().unwrap();
            let max_tokens = self.inner.max_tokens.load(Ordering::Relaxed);

            self.inner = Arc::new(CostTrackerInner {
                total_prompt_tokens: AtomicU64::new(total_prompt),
                total_completion_tokens: AtomicU64::new(total_completion),
                total_turns: AtomicU64::new(total_turns),
                last_turn_prompt_tokens: AtomicU32::new(last_turn_prompt),
                last_turn_completion_tokens: AtomicU32::new(last_turn_completion),
                max_cost_usd: Mutex::new(max_cost),
                warn_at_usd: Mutex::new(warn_at),
                max_tokens: AtomicU64::new(max_tokens),
                cost_per_prompt_token: prompt_per_million / 1_000_000.0,
                cost_per_completion_token: completion_per_million / 1_000_000.0,
                warned: Mutex::new(false),
            });
        }
        self
    }

    /// Updates the max budget dynamically. (0.0 = unlimited)
    pub fn set_max_cost_usd(&self, limit: f64) {
        let mut budget = self.inner.max_cost_usd.lock().unwrap();
        *budget = limit;
    }

    /// Updates the session token limit dynamically. (0 = unlimited)
    pub fn set_max_tokens(&self, limit: u64) {
        self.inner.max_tokens.store(limit, Ordering::Relaxed);
    }

    /// Returns the active max cost limit in USD.
    pub fn max_cost_usd(&self) -> f64 {
        *self.inner.max_cost_usd.lock().unwrap()
    }

    /// Returns the active token limit (0 = unlimited).
    pub fn max_tokens(&self) -> u64 {
        self.inner.max_tokens.load(Ordering::Relaxed)
    }

    /// Records token usage from a single LLM call.
    /// Returns `Err` if the cost or token budget is exceeded.
    pub fn record(&self, prompt_tokens: u32, completion_tokens: u32) -> Result<(), CostTrackerError> {
        self.inner.total_prompt_tokens.fetch_add(prompt_tokens as u64, Ordering::Relaxed);
        self.inner.total_completion_tokens.fetch_add(completion_tokens as u64, Ordering::Relaxed);
        self.inner.total_turns.fetch_add(1, Ordering::Relaxed);
        self.inner.last_turn_prompt_tokens.store(prompt_tokens, Ordering::Relaxed);
        self.inner.last_turn_completion_tokens.store(completion_tokens, Ordering::Relaxed);

        let current_cost = self.current_cost_usd();
        let max_cost = *self.inner.max_cost_usd.lock().unwrap();
        let warn_at = *self.inner.warn_at_usd.lock().unwrap();

        // Warn threshold
        if max_cost > 0.0 && warn_at > 0.0 && current_cost >= warn_at {
            let mut warned = self.inner.warned.lock().unwrap();
            if !*warned {
                warn!(
                    cost = format!("${:.4}", current_cost),
                    limit = format!("${:.2}", max_cost),
                    "Cost warning threshold reached"
                );
                *warned = true;
            }
        }

        // Hard cost limit
        if max_cost > 0.0 && current_cost >= max_cost {
            return Err(CostTrackerError::CostLimitExceeded {
                current_cost,
                limit: max_cost,
            });
        }

        // Hard token limit
        let token_limit = self.inner.max_tokens.load(Ordering::Relaxed);
        let total_tokens = self.total_tokens();
        if token_limit > 0 && total_tokens >= token_limit {
            return Err(CostTrackerError::TokenLimitExceeded {
                current_tokens: total_tokens,
                limit: token_limit,
            });
        }

        Ok(())
    }

    /// Returns the current estimated cost in USD.
    pub fn current_cost_usd(&self) -> f64 {
        let prompt = self.inner.total_prompt_tokens.load(Ordering::Relaxed) as f64;
        let completion = self.inner.total_completion_tokens.load(Ordering::Relaxed) as f64;
        prompt * self.inner.cost_per_prompt_token + completion * self.inner.cost_per_completion_token
    }

    /// Returns estimated cost for the last recorded turn in USD.
    pub fn last_turn_cost_usd(&self) -> f64 {
        let prompt = self.inner.last_turn_prompt_tokens.load(Ordering::Relaxed) as f64;
        let completion = self.inner.last_turn_completion_tokens.load(Ordering::Relaxed) as f64;
        prompt * self.inner.cost_per_prompt_token + completion * self.inner.cost_per_completion_token
    }

    /// Returns prompt tokens consumed in the last turn.
    pub fn last_turn_prompt_tokens(&self) -> u32 {
        self.inner.last_turn_prompt_tokens.load(Ordering::Relaxed)
    }

    /// Returns completion tokens consumed in the last turn.
    pub fn last_turn_completion_tokens(&self) -> u32 {
        self.inner.last_turn_completion_tokens.load(Ordering::Relaxed)
    }

    /// Returns total prompt tokens consumed.
    pub fn prompt_tokens(&self) -> u64 {
        self.inner.total_prompt_tokens.load(Ordering::Relaxed)
    }

    /// Returns total completion tokens consumed.
    pub fn completion_tokens(&self) -> u64 {
        self.inner.total_completion_tokens.load(Ordering::Relaxed)
    }

    /// Returns total prompt and completion tokens combined.
    pub fn total_tokens(&self) -> u64 {
        self.prompt_tokens() + self.completion_tokens()
    }

    /// Returns total turns (LLM calls) made.
    pub fn turns(&self) -> u64 {
        self.inner.total_turns.load(Ordering::Relaxed)
    }

    /// Formats a complete 8-bit boxed turn report after an interaction.
    pub fn format_turn_report(&self) -> String {
        crate::engine::arcade::format_token_report(
            self.last_turn_prompt_tokens(),
            self.last_turn_completion_tokens(),
            self.last_turn_cost_usd(),
            self.prompt_tokens(),
            self.completion_tokens(),
            self.current_cost_usd(),
            self.max_cost_usd(),
            self.max_tokens(),
        )
    }

    /// Returns a formatted summary string for display.
    pub fn summary(&self) -> String {
        let max_cost = self.max_cost_usd();
        let cost_str = if max_cost > 0.0 {
            format!("${:.4} / ${:.2}", self.current_cost_usd(), max_cost)
        } else {
            format!("${:.4} (Unlimited)", self.current_cost_usd())
        };

        let token_limit = self.max_tokens();
        let token_str = if token_limit > 0 {
            format!("{} / {} tokens", self.total_tokens(), token_limit)
        } else {
            format!("{} prompt + {} completion", self.prompt_tokens(), self.completion_tokens())
        };

        format!(
            "Tokens: {} | Turns: {} | Cost: {}",
            token_str,
            self.turns(),
            cost_str,
        )
    }
}

#[derive(Debug, thiserror::Error)]
pub enum CostTrackerError {
    #[error("Cost budget exceeded: ${current_cost:.4} >= ${limit:.2} limit")]
    CostLimitExceeded { current_cost: f64, limit: f64 },

    #[error("Token limit exceeded: {current_tokens} >= {limit} session tokens limit")]
    TokenLimitExceeded { current_tokens: u64, limit: u64 },
}

pub type CostLimitExceeded = CostTrackerError;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unlimited_cost_by_default() {
        let tracker = CostTracker::new(0.0, 0.0);
        // Record 1,000,000 tokens which would cost $12.50
        let res = tracker.record(500_000, 500_000);
        assert!(res.is_ok(), "Unlimited tracker should not fail on large usage");
        assert!(tracker.summary().contains("Unlimited"));
    }

    #[test]
    fn test_cost_limit_enforcement() {
        let tracker = CostTracker::new(1.0, 0.5);
        // GPT-4o: $2.50 / 1M prompt -> 1M prompt is $2.50 > $1.0 limit
        let res = tracker.record(1_000_000, 0);
        assert!(res.is_err(), "Exceeding budget must return error");
        match res.unwrap_err() {
            CostTrackerError::CostLimitExceeded { limit, .. } => assert_eq!(limit, 1.0),
            _ => panic!("Expected CostLimitExceeded"),
        }
    }

    #[test]
    fn test_token_limit_enforcement() {
        let tracker = CostTracker::new(0.0, 0.0).with_token_limit(500);
        assert!(tracker.record(200, 200).is_ok());
        let res = tracker.record(100, 50); // total = 550 >= 500
        assert!(res.is_err(), "Exceeding token limit must return error");
        match res.unwrap_err() {
            CostTrackerError::TokenLimitExceeded { current_tokens, limit } => {
                assert_eq!(current_tokens, 550);
                assert_eq!(limit, 500);
            }
            _ => panic!("Expected TokenLimitExceeded"),
        }
    }

    #[test]
    fn test_turn_report_formatting() {
        let tracker = CostTracker::new(0.0, 0.0);
        tracker.record(150, 50).unwrap();
        let report = tracker.format_turn_report();
        assert!(report.contains("TURN TOKEN USAGE & COST REPORT"));
        assert!(report.contains("150 prompt + 50 completion = 200 tokens"));
    }
}
