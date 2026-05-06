use std::sync::Mutex;

use anyhow::Result;
use async_trait::async_trait;
use rslm_providers::{LlmProvider, Message};

/// A deterministic provider that replays scripted responses in order.
/// After responses are exhausted it returns `final_answer("no more responses")`.
pub struct HarnessProvider {
    pub(crate) responses: Mutex<Vec<String>>,
    pub received: Mutex<Vec<Vec<Message>>>,
}

impl HarnessProvider {
    pub fn new(responses: Vec<String>) -> Self {
        Self {
            responses: Mutex::new(responses),
            received: Mutex::new(Vec::new()),
        }
    }
}

#[async_trait]
impl LlmProvider for HarnessProvider {
    async fn complete(&self, messages: Vec<Message>) -> Result<String> {
        self.received.lock().unwrap().push(messages);
        let mut q = self.responses.lock().unwrap();
        if q.is_empty() {
            Ok(r#"final_answer("no more responses")"#.to_string())
        } else {
            Ok(q.remove(0))
        }
    }

    fn model_id(&self) -> &str {
        "harness"
    }
}
