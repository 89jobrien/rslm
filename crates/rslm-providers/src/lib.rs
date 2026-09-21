//! LLM provider interface and OpenAI and Anthropic adapters.

// TODO(bamlish): another copy of the anthropic.rs + openai.rs adapter pair already present in
// naptrace-llm, minibox-llm and disyn-neural. rslm drives its own recursive loop, so looprs is
// not the fit — but nothing in this provider layer is rslm-specific, and it could come from a
// shared provider crate or from bamlish's client pools instead.

pub mod anthropic;
pub mod openai;
pub mod provider;

pub use anthropic::AnthropicProvider;
pub use openai::OpenAiProvider;
pub use provider::{LlmProvider, Message, Role};

#[cfg(all(test, feature = "live-tests"))]
mod live_tests {
    use super::*;

    /// Real OpenAI call — requires OPENAI_API_KEY in env.
    /// Run with: cargo test -p rslm-providers --features live-tests -- live_tests
    #[tokio::test]
    async fn openai_completes() {
        let provider = OpenAiProvider::new("gpt-4o-mini");
        let messages = vec![
            Message::system("You are a helpful assistant."),
            Message::user("Reply with exactly the word: pong"),
        ];
        let resp = provider.complete(messages).await.unwrap();
        assert!(
            resp.to_lowercase().contains("pong"),
            "unexpected response: {resp}"
        );
    }
}
