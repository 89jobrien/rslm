use anyhow::{Context, Result};
use async_openai::{
    types::{
        ChatCompletionRequestAssistantMessageArgs, ChatCompletionRequestMessage,
        ChatCompletionRequestSystemMessageArgs, ChatCompletionRequestUserMessageArgs,
        CreateChatCompletionRequestArgs,
    },
    Client,
};
use async_trait::async_trait;

use crate::provider::{LlmProvider, Message, Role};

pub struct OpenAiProvider {
    client: Client<async_openai::config::OpenAIConfig>,
    model: String,
}

impl OpenAiProvider {
    pub fn new(model: impl Into<String>) -> Self {
        Self {
            client: Client::new(),
            model: model.into(),
        }
    }
}

#[async_trait]
impl LlmProvider for OpenAiProvider {
    async fn complete(&self, messages: Vec<Message>) -> Result<String> {
        let msgs: Vec<ChatCompletionRequestMessage> = messages
            .into_iter()
            .map(|m| match m.role {
                Role::System => ChatCompletionRequestSystemMessageArgs::default()
                    .content(m.content)
                    .build()
                    .unwrap()
                    .into(),
                Role::User => ChatCompletionRequestUserMessageArgs::default()
                    .content(m.content)
                    .build()
                    .unwrap()
                    .into(),
                Role::Assistant => ChatCompletionRequestAssistantMessageArgs::default()
                    .content(m.content)
                    .build()
                    .unwrap()
                    .into(),
            })
            .collect();

        let req = CreateChatCompletionRequestArgs::default()
            .model(&self.model)
            .messages(msgs)
            .build()
            .context("build OpenAI request")?;

        let resp = self
            .client
            .chat()
            .create(req)
            .await
            .context("OpenAI API call")?;

        let content = resp
            .choices
            .into_iter()
            .next()
            .and_then(|c| c.message.content)
            .context("no content in OpenAI response")?;

        Ok(content)
    }

    fn model_id(&self) -> &str {
        &self.model
    }
}
