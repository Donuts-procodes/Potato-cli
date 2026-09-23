use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use tracing::warn;

/// Tracks cumulative token usage and estimated cost across the entire session.
/// Thread-safe for concurrent access.
pub struct CostTracker {
    total_prompt_tokens: AtomicU64,
    total_completion_tokens: AtomicU64,
    total_turns: AtomicU64,
    max_cost_usd: f64,
    warn_at_usd: f64,
    cost_per_prompt_token: f64,
    cost_per_completion_token: f64,
    warned: Mutex<bool>,
}

impl CostTracker {
    /// Creates a new cost tracker with budget limits.
    /// Default pricing is GPT-4o: $2.50/1M prompt, $10.00/1M completion.
    pub fn new(max_cost_usd: f64, warn_at_usd: f64) -> Self {
        Self {
            total_prompt_tokens: AtomicU64::new(0),
            total_completion_tokens: AtomicU64::new(0),
            total_turns: AtomicU64::new(0),
            max_cost_usd,
            warn_at_usd,
            cost_per_prompt_token: 2.50 / 1_000_000.0,
            cost_per_completion_token: 10.00 / 1_000_000.0,
            warned: Mutex::new(false),
        }
    }

    /// Sets custom pricing per token (for non-GPT-4o models).
    pub fn with_pricing(mut self, prompt_per_million: f64, completion_per_million: f64) -> Self {
        self.cost_per_prompt_token = prompt_per_million / 1_000_000.0;
        self.cost_per_completion_token = completion_per_million / 1_000_000.0;
        self
    }

    /// Records token usage from a single LLM call.
    /// Returns `Err` if the budget is exceeded.
    pub fn record(&self, prompt_tokens: u32, completion_tokens: u32) -> Result<(), CostLimitExceeded> {
        self.total_prompt_tokens.fetch_add(prompt_tokens as u64, Ordering::Relaxed);
        self.total_completion_tokens.fetch_add(completion_tokens as u64, Ordering::Relaxed);
        self.total_turns.fetch_add(1, Ordering::Relaxed);

        let current_cost = self.current_cost_usd();

        // Warn threshold
        if current_cost >= self.warn_at_usd && self.warn_at_usd > 0.0 {
            let mut warned = self.warned.lock().unwrap();
            if !*warned {
                warn!(
                    cost = format!("${:.4}", current_cost),
                    limit = format!("${:.2}", self.max_cost_usd),
                    "Cost warning threshold reached"
                );
                *warned = true;
            }
        }

        // Hard limit
        if current_cost >= self.max_cost_usd && self.max_cost_usd > 0.0 {
            return Err(CostLimitExceeded {
                current_cost,
                limit: self.max_cost_usd,
            });
        }

        Ok(())
    }

    /// Returns the current estimated cost in USD.
    pub fn current_cost_usd(&self) -> f64 {
        let prompt = self.total_prompt_tokens.load(Ordering::Relaxed) as f64;
        let completion = self.total_completion_tokens.load(Ordering::Relaxed) as f64;
        prompt * self.cost_per_prompt_token + completion * self.cost_per_completion_token
    }

    /// Returns total prompt tokens consumed.
    pub fn prompt_tokens(&self) -> u64 {
        self.total_prompt_tokens.load(Ordering::Relaxed)
    }

    /// Returns total completion tokens consumed.
    pub fn completion_tokens(&self) -> u64 {
        self.total_completion_tokens.load(Ordering::Relaxed)
    }

    /// Returns total turns (LLM calls) made.
    pub fn turns(&self) -> u64 {
        self.total_turns.load(Ordering::Relaxed)
    }

    /// Returns a formatted summary string for display.
    pub fn summary(&self) -> String {
        format!(
            "Tokens: {} prompt + {} completion | Turns: {} | Cost: ${:.4} / ${:.2}",
            self.prompt_tokens(),
            self.completion_tokens(),
            self.turns(),
            self.current_cost_usd(),
            self.max_cost_usd,
        )
    }
}

#[derive(Debug, thiserror::Error)]
#[error("Cost budget exceeded: ${current_cost:.4} >= ${limit:.2} limit")]
pub struct CostLimitExceeded {
    pub current_cost: f64,
    pub limit: f64,
}
