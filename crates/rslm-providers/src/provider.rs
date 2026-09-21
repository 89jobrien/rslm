//! Provider-neutral chat messages and asynchronous completion interface.

use anyhow::Result;
use async_trait::async_trait;

#[derive(Debug, Clone)]
pub enum Role {
    System,
    User,
    Assistant,
}

#[derive(Debug, Clone)]
pub struct Message {
    pub role: Role,
    pub content: String,
}

impl Message {
    /// Creates a system instruction message.
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: Role::System,
            content: content.into(),
        }
    }

    /// Creates a user message.
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            content: content.into(),
        }
    }

    /// Creates an assistant message.
    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: Role::Assistant,
            content: content.into(),
        }
    }
}

#[async_trait]
pub trait LlmProvider: Send + Sync {
    /// Returns the assistant content for the supplied conversation.
    async fn complete(&self, messages: Vec<Message>) -> Result<String>;
    /// Returns the provider-specific model identifier.
    fn model_id(&self) -> &str;
}
