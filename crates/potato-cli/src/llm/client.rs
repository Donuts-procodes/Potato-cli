use anyhow::{anyhow, Context, Result};
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use serde::{Deserialize, Serialize};
use std::time::Instant;
use tracing::{debug, error, info, warn};

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
                .connect_timeout(std::time::Duration::from_secs(15))
                .tcp_nodelay(true)
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
        let mut backoff = std::time::Duration::from_millis(800);
        let request_timeout = std::time::Duration::from_secs(75);

        for attempt in 1..=max_retries {
            let start_time = Instant::now();
            let post_future = self
                .client
                .post(&endpoint)
                .headers(headers.clone())
                .json(&payload)
                .send();

            let res = tokio::time::timeout(request_timeout, post_future).await;

            match res {
                Ok(Ok(response)) => {
                    let status = response.status();
                    if status.is_success() {
                        let body_future = response.text();
                        let body_res = tokio::time::timeout(std::time::Duration::from_secs(30), body_future).await;

                        let body_text = match body_res {
                            Ok(Ok(text)) => text,
                            Ok(Err(e)) => {
                                error!(attempt, error = %e, "Failed reading LLM response body text");
                                if attempt == max_retries {
                                    return Err(anyhow!("Failed reading LLM response body: {}", e));
                                }
                                tokio::time::sleep(backoff).await;
                                backoff *= 2;
                                continue;
                            }
                            Err(_) => {
                                warn!(attempt, "LLM response body stream stalled for >30s; retrying");
                                if attempt == max_retries {
                                    return Err(anyhow!("LLM response body stream stalled/buffered indefinitely"));
                                }
                                tokio::time::sleep(backoff).await;
                                backoff *= 2;
                                continue;
                            }
                        };

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
                        let is_retryable = status.as_u16() == 429
                            || status.as_u16() == 502
                            || status.as_u16() == 503
                            || status.as_u16() == 504;

                        warn!(
                            status = %status,
                            attempt,
                            retryable = is_retryable,
                            error = %err_text,
                            "LLM request returned non-success status"
                        );

                        if attempt == max_retries || !is_retryable {
                            return Err(anyhow!(
                                "LLM request failed (status {}): {}\nTip: Check API quota or run `/model` to switch models.",
                                status,
                                err_text
                            ));
                        }
                    }
                }
                Ok(Err(err)) => {
                    warn!(attempt, error = %err, "HTTP transport connection error during LLM request");
                    if attempt == max_retries {
                        return Err(anyhow!(
                            "HTTP transport error ({} attempts): {}\nTip: Check network connectivity or proxy settings.",
                            max_retries,
                            err
                        ));
                    }
                }
                Err(_) => {
                    warn!(attempt, max_retries, "LLM request stalled or hung in buffer for >75s; triggering retry");
                    if attempt == max_retries {
                        return Err(anyhow!(
                            "LLM request timed out while buffering from provider ({} attempts x 75s).\n\
                             The server is unresponsive. Check internet connection or switch to another model with `/model`.",
                            max_retries
                        ));
                    }
                }
            }

            tokio::time::sleep(backoff).await;
            backoff *= 2;
        }

        Err(anyhow!("Exhausted retry attempts for LLM call due to buffering or connection stalls"))
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
