use anyhow::{anyhow, Context, Result};
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use serde::{Deserialize, Serialize};
use std::time::Instant;
use tracing::{debug, error, info};

use crate::types::AgentTurnResponse;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatCompletionRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    pub temperature: f32,
    pub response_format: Option<ResponseFormat>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseFormat {
    #[serde(rename = "type")]
    pub format_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatCompletionResponse {
    pub id: String,
    pub choices: Vec<Choice>,
    pub usage: Option<Usage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Choice {
    pub message: ChatMessage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

pub struct LlmClient {
    client: reqwest::Client,
    api_base: String,
    api_key: String,
    model: String,
}

impl LlmClient {
    pub fn new(api_base: String, api_key: String, model: String) -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(180))
                .build()
                .expect("Failed to initialize reqwest client"),
            api_base,
            api_key,
            model,
        }
    }

    pub fn model(&self) -> &str {
        &self.model
    }

    pub fn from_env() -> Result<Self> {
        let api_key = std::env::var("OPENAI_API_KEY")
            .or_else(|_| std::env::var("POTATO_API_KEY"))
            .context("Missing OPENAI_API_KEY or POTATO_API_KEY environment variable")?;

        let api_base = std::env::var("OPENAI_BASE_URL")
            .or_else(|_| std::env::var("POTATO_API_BASE"))
            .unwrap_or_else(|_| "https://api.openai.com/v1".to_string());

        let model = std::env::var("POTATO_MODEL")
            .unwrap_or_else(|_| "gpt-4o".to_string());

        Ok(Self::new(api_base, api_key, model))
    }

    pub async fn send_turn(&self, messages: &[ChatMessage]) -> Result<AgentTurnResponse> {
        let (turn, _) = self.send_turn_with_usage(messages).await?;
        Ok(turn)
    }

    pub async fn send_turn_with_usage(&self, messages: &[ChatMessage]) -> Result<(AgentTurnResponse, Usage)> {
        let endpoint = format!("{}/chat/completions", self.api_base.trim_end_matches('/'));

        let mut headers = HeaderMap::new();
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", self.api_key))?,
        );
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

        let payload = ChatCompletionRequest {
            model: self.model.clone(),
            messages: messages.to_vec(),
            temperature: 0.1,
            response_format: Some(ResponseFormat {
                format_type: "json_object".to_string(),
            }),
        };

        let max_retries = 3;
        let mut backoff = std::time::Duration::from_millis(500);

        for attempt in 1..=max_retries {
            let start_time = Instant::now();
            let res = self
                .client
                .post(&endpoint)
                .headers(headers.clone())
                .json(&payload)
                .send()
                .await;

            match res {
                Ok(response) => {
                    let status = response.status();
                    if status.is_success() {
                        let body_text = response.text().await?;
                        let completion: ChatCompletionResponse = serde_json::from_str(&body_text)
                            .with_context(|| format!("Failed to parse completion body: {}", body_text))?;

                        let duration = start_time.elapsed();
                        let usage = completion.usage.unwrap_or(Usage {
                            prompt_tokens: 0,
                            completion_tokens: 0,
                            total_tokens: 0,
                        });

                        info!(
                            model = %self.model,
                            prompt_tokens = usage.prompt_tokens,
                            completion_tokens = usage.completion_tokens,
                            total_tokens = usage.total_tokens,
                            latency_ms = duration.as_millis(),
                            "LLM call completed successfully"
                        );

                        let raw_content = completion
                            .choices
                            .first()
                            .ok_or_else(|| anyhow!("No choices returned from LLM"))?
                            .message
                            .content
                            .trim();

                        debug!("Raw LLM Output:\n{}", raw_content);

                        let cleaned_json = extract_json_payload(raw_content);
                        let turn: AgentTurnResponse = serde_json::from_str(&cleaned_json)
                            .with_context(|| format!("Failed to parse AgentTurnResponse JSON: {}", cleaned_json))?;

                        return Ok((turn, usage));
                    } else {
                        let err_text = response.text().await.unwrap_or_default();
                        error!(
                            status = %status,
                            attempt,
                            error = %err_text,
                            "LLM request returned error status"
                        );
                        if attempt == max_retries {
                            return Err(anyhow!("LLM request failed after {} attempts: {}", max_retries, err_text));
                        }
                    }
                }
                Err(err) => {
                    error!(attempt, error = %err, "HTTP transport error during LLM request");
                    if attempt == max_retries {
                        return Err(anyhow!("HTTP transport error: {}", err));
                    }
                }
            }

            tokio::time::sleep(backoff).await;
            backoff *= 2;
        }

        Err(anyhow!("Exhausted retry attempts for LLM call"))
    }
}

fn extract_json_payload(raw: &str) -> String {
    let trimmed = raw.trim();
    if let Some(start) = trimmed.find('{') {
        if let Some(end) = trimmed.rfind('}') {
            if end >= start {
                return trimmed[start..=end].to_string();
            }
        }
    }
    trimmed.to_string()
}
