pub mod provider;

use std::sync::Arc;

use anyhow::Result;
pub use provider::HarnessProvider;
use rslm_core::Rlm;
use rslm_providers::{LlmProvider, Message};

/// Collected result of a single harness run.
pub struct HarnessRun {
    pub answer: String,
    pub iterations: usize,
    pub messages_received: Vec<Vec<Message>>,
}

/// Deterministic test driver for [`Rlm`].
pub struct Harness;

impl Harness {
    /// Run the RLM loop with scripted `responses` and return the execution trace.
    ///
    /// `max_depth = 3`, `max_iterations = responses.len().max(20)`.
    pub async fn run(
        query: &str,
        ctx: &str,
        responses: Vec<impl Into<String>>,
    ) -> Result<HarnessRun> {
        let responses: Vec<String> = responses.into_iter().map(Into::into).collect();
        let max_iter = responses.len().max(20);
        let provider = Arc::new(HarnessProvider::new(responses));
        let rlm = Rlm::new(
            Arc::clone(&provider) as Arc<dyn LlmProvider>,
            3,
            max_iter,
            false,
        );
        let answer = rlm.run(query, ctx).await?;
        let received = provider.received.lock().unwrap().clone();
        let iterations = received.len();
        Ok(HarnessRun {
            answer,
            iterations,
            messages_received: received,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use rslm_core::{Rlm, RlmError};
    use rslm_providers::LlmProvider;

    use super::*;

    #[tokio::test]
    async fn harness_returns_final_answer() {
        let run = Harness::run(
            "what is 2+2?",
            "no context needed",
            vec![r#"final_answer("4")"#],
        )
        .await
        .unwrap();
        assert_eq!(run.answer, "4");
        assert_eq!(run.iterations, 1);
    }

    #[tokio::test]
    async fn harness_records_messages_received() {
        let run = Harness::run("q", "ctx", vec![r#"final_answer("ok")"#])
            .await
            .unwrap();
        assert_eq!(run.messages_received.len(), 1);
    }

    #[tokio::test]
    async fn harness_multi_step() {
        let run = Harness::run("q", "ctx", vec!["ctx_len()", r#"final_answer("done")"#])
            .await
            .unwrap();
        assert_eq!(run.answer, "done");
        assert_eq!(run.iterations, 2);
    }

    #[tokio::test]
    async fn harness_propagates_max_iterations_error() {
        let provider = Arc::new(HarnessProvider::new(vec!["ctx_len()".to_string()]));
        let rlm = Rlm::new(Arc::clone(&provider) as Arc<dyn LlmProvider>, 3, 1, false);
        let result = rlm.run("q", "ctx").await;
        assert!(matches!(result, Err(RlmError::MaxIterationsExceeded(1))));
    }
}
