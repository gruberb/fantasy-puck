//! OpenRouter `/chat/completions` client shared by every narrative
//! call (Pulse team diagnosis, Insights daily preview, season recap).
//!
//! OpenRouter speaks the OpenAI chat-completions dialect regardless of
//! the upstream model, so the system prompt travels as the first
//! message rather than a top-level field.

use anyhow::{anyhow, Context};
use reqwest::Client;
use secrecy::{ExposeSecret, SecretString};

const OPENROUTER_API_URL: &str = "https://openrouter.ai/api/v1/chat/completions";

pub struct OpenRouterClient {
    api_key: SecretString,
    model: String,
    http: Client,
}

impl OpenRouterClient {
    pub fn new(api_key: SecretString, model: String) -> anyhow::Result<Self> {
        let http = Client::builder()
            .timeout(crate::tuning::http::LLM_TIMEOUT)
            .build()
            .context("failed to build OpenRouter HTTP client")?;
        Ok(Self {
            api_key,
            model,
            http,
        })
    }

    pub fn model(&self) -> &str {
        &self.model
    }

    /// Single-turn completion. Returns the trimmed text of the first choice.
    pub async fn complete(
        &self,
        system: &str,
        user: &str,
        max_tokens: u32,
    ) -> anyhow::Result<String> {
        let body = serde_json::json!({
            "model": self.model,
            "max_tokens": max_tokens,
            "messages": [
                { "role": "system", "content": system },
                { "role": "user", "content": user }
            ]
        });

        let response = self
            .http
            .post(OPENROUTER_API_URL)
            .bearer_auth(self.api_key.expose_secret())
            .header("X-Title", "Fantasy Puck")
            .json(&body)
            .send()
            .await
            .context("OpenRouter request failed")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow!("OpenRouter returned {status}: {body}"));
        }

        let body: serde_json::Value = response
            .json()
            .await
            .context("failed to parse OpenRouter response")?;

        // OpenRouter can surface upstream provider failures as a 200
        // carrying an `error` object instead of `choices`.
        if let Some(err) = body.get("error") {
            return Err(anyhow!("OpenRouter upstream error: {err}"));
        }

        body.get("choices")
            .and_then(|c| c.as_array())
            .and_then(|arr| arr.first())
            .and_then(|choice| choice.get("message"))
            .and_then(|m| m.get("content"))
            .and_then(|t| t.as_str())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .ok_or_else(|| anyhow!("no text content in OpenRouter response: {body}"))
    }
}
