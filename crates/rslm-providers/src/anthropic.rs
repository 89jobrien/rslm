use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::json;

use crate::provider::{LlmProvider, Message, Role};

pub struct AnthropicProvider {
    client: reqwest::Client,
    model: String,
    api_key: String,
}

impl AnthropicProvider {
    pub fn new(model: impl Into<String>) -> Result<Self> {
        let api_key = std::env::var("ANTHROPIC_API_KEY").context("ANTHROPIC_API_KEY not set")?;
        Ok(Self {
            client: reqwest::Client::new(),
            model: model.into(),
            api_key,
        })
    }
}

#[derive(Deserialize)]
struct AnthropicResponse {
    content: Vec<AnthropicContent>,
}

#[derive(Deserialize)]
struct AnthropicContent {
    text: String,
}

#[async_trait]
impl LlmProvider for AnthropicProvider {
    async fn complete(&self, messages: Vec<Message>) -> Result<String> {
        // Split system message from conversation
        let mut system_content = String::new();
        let mut conv: Vec<serde_json::Value> = Vec::new();

        for m in messages {
            match m.role {
                Role::System => {
                    system_content = m.content;
                }
                Role::User => {
                    conv.push(json!({ "role": "user", "content": m.content }));
                }
                Role::Assistant => {
                    conv.push(json!({ "role": "assistant", "content": m.content }));
                }
            }
        }

        let mut body = json!({
            "model": self.model,
            "max_tokens": 4096,
            "messages": conv,
        });

        if !system_content.is_empty() {
            body["system"] = json!(system_content);
        }

        let resp = self
            .client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await
            .context("Anthropic HTTP request")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            bail!("Anthropic API error {status}: {text}");
        }

        let parsed: AnthropicResponse = resp.json().await.context("parse Anthropic response")?;
        let text = parsed
            .content
            .into_iter()
            .next()
            .map(|c| c.text)
            .context("empty Anthropic response")?;

        Ok(text)
    }

    fn model_id(&self) -> &str {
        &self.model
    }
}
