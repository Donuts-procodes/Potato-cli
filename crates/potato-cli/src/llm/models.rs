use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: &'static str,
    pub provider: &'static str,
    pub description: &'static str,
    pub prompt_cost_per_m: f64,
    pub completion_cost_per_m: f64,
    pub context_tokens: usize,
}

pub struct ModelRegistry;

impl ModelRegistry {
    pub fn all() -> &'static [ModelInfo] {
        &[
            // --- Anthropic ---
            ModelInfo {
                id: "claude-3-7-sonnet",
                provider: "Anthropic",
                description: "Hybrid reasoning & premier coding model",
                prompt_cost_per_m: 3.00,
                completion_cost_per_m: 15.00,
                context_tokens: 200_000,
            },
            ModelInfo {
                id: "claude-3-5-sonnet",
                provider: "Anthropic",
                description: "Established high-performance coding agent model",
                prompt_cost_per_m: 3.00,
                completion_cost_per_m: 15.00,
                context_tokens: 200_000,
            },
            ModelInfo {
                id: "claude-3-5-haiku",
                provider: "Anthropic",
                description: "Ultra-fast, low-latency review and triage model",
                prompt_cost_per_m: 0.80,
                completion_cost_per_m: 4.00,
                context_tokens: 200_000,
            },

            // --- OpenAI ---
            ModelInfo {
                id: "gpt-4o",
                provider: "OpenAI",
                description: "Omnimodal flagship generalist default",
                prompt_cost_per_m: 2.50,
                completion_cost_per_m: 10.00,
                context_tokens: 128_000,
            },
            ModelInfo {
                id: "gpt-4o-mini",
                provider: "OpenAI",
                description: "Cost-efficient, high-speed lightweight model",
                prompt_cost_per_m: 0.15,
                completion_cost_per_m: 0.60,
                context_tokens: 128_000,
            },
            ModelInfo {
                id: "o3-mini",
                provider: "OpenAI",
                description: "High-reasoning math, logic & algorithmic patch model",
                prompt_cost_per_m: 1.10,
                completion_cost_per_m: 4.40,
                context_tokens: 200_000,
            },
            ModelInfo {
                id: "o1",
                provider: "OpenAI",
                description: "Deep reasoning frontier model for hard architecture",
                prompt_cost_per_m: 15.00,
                completion_cost_per_m: 60.00,
                context_tokens: 200_000,
            },

            // --- DeepSeek ---
            ModelInfo {
                id: "deepseek-chat",
                provider: "DeepSeek",
                description: "DeepSeek-V3 general coding & instruction model",
                prompt_cost_per_m: 0.14,
                completion_cost_per_m: 0.28,
                context_tokens: 64_000,
            },
            ModelInfo {
                id: "deepseek-reasoner",
                provider: "DeepSeek",
                description: "DeepSeek-R1 chain-of-thought reasoning model",
                prompt_cost_per_m: 0.55,
                completion_cost_per_m: 2.19,
                context_tokens: 64_000,
            },

            // --- Google Gemini ---
            ModelInfo {
                id: "gemini-2.0-flash",
                provider: "Google",
                description: "Real-time multimodal speed with 1M context",
                prompt_cost_per_m: 0.10,
                completion_cost_per_m: 0.40,
                context_tokens: 1_000_000,
            },
            ModelInfo {
                id: "gemini-2.0-pro-exp",
                provider: "Google",
                description: "Frontier reasoning and complex coding agent model",
                prompt_cost_per_m: 1.25,
                completion_cost_per_m: 5.00,
                context_tokens: 1_000_000,
            },

            // --- Local Ollama & Open Weights ---
            ModelInfo {
                id: "qwen2.5-coder:32b",
                provider: "Ollama (Local)",
                description: "Top-tier open-weight local coding model",
                prompt_cost_per_m: 0.0,
                completion_cost_per_m: 0.0,
                context_tokens: 32_000,
            },
            ModelInfo {
                id: "llama3.3:70b",
                provider: "Ollama (Local)",
                description: "Llama 3.3 70B open instruction model",
                prompt_cost_per_m: 0.0,
                completion_cost_per_m: 0.0,
                context_tokens: 128_000,
            },
        ]
    }

    pub fn find(model_id: &str) -> Option<&'static ModelInfo> {
        Self::all().iter().find(|m| m.id.eq_ignore_ascii_case(model_id))
    }

    pub fn by_index(idx: usize) -> Option<&'static ModelInfo> {
        if idx > 0 && idx <= Self::all().len() {
            Some(&Self::all()[idx - 1])
        } else {
            None
        }
    }
}
