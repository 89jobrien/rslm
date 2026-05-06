use std::sync::{Arc, Mutex};

use anyhow::Result;
use rslm_providers::{LlmProvider, Message};
use tracing::{debug, info};

use crate::{
    env::{build_engine, run_script, CallState},
    protocol::{Notebook, RlmError},
};

const SYSTEM_PROMPT: &str = r#"You are an RLM (Recursive Language Model) agent. You never see the full context directly.
Instead, you interact with it through a Rhai scripting environment. Each response you produce must be
a valid Rhai script that uses the following registered functions:

  ctx_len() -> int               — total byte length of the context
  ctx_slice(start, end) -> String — byte-range slice of context
  ctx_grep(pattern) -> String    — regex search; returns matching lines joined by newline
  rlm_call(query, ctx) -> String — spawn a child RLM with a sub-context; blocks until done
  print_cell(msg)                — log a message to this cell's output
  final_answer(answer)           — signal your final answer and exit the loop

Rules:
- Your entire response must be a Rhai script (no markdown fences, no prose).
- When you have enough information to answer the query, call final_answer("your answer").
- You may call rlm_call to delegate sub-questions with focused context slices.
- Each script runs in an isolated scope; no state persists between scripts.
- If a script errors, you will receive the error message and can correct it.
"#;

pub struct Rlm {
    pub(crate) provider: Arc<dyn LlmProvider>,
    pub(crate) depth: usize,
    pub(crate) max_depth: usize,
    pub(crate) max_iterations: usize,
    pub(crate) verbose: bool,
}

impl Rlm {
    pub fn new(
        provider: Arc<dyn LlmProvider>,
        max_depth: usize,
        max_iterations: usize,
        verbose: bool,
    ) -> Self {
        Self {
            provider,
            depth: 0,
            max_depth,
            max_iterations,
            verbose,
        }
    }

    pub async fn run(&self, query: &str, ctx: &str) -> Result<String, RlmError> {
        if self.depth >= self.max_depth {
            return Err(RlmError::MaxDepthExceeded(self.max_depth));
        }

        let state: Arc<Mutex<CallState>> = Arc::new(Mutex::new(CallState::default()));
        let mut notebook = Notebook::default();

        // Build the rlm_call callback — spawns a blocking child Rlm
        let child_provider = Arc::clone(&self.provider);
        let child_depth = self.depth + 1;
        let child_max_depth = self.max_depth;
        let child_max_iter = self.max_iterations;
        let child_verbose = self.verbose;

        let rlm_call_fn = move |q: String, sub_ctx: String| -> String {
            let provider = Arc::clone(&child_provider);
            let child = Rlm {
                provider,
                depth: child_depth,
                max_depth: child_max_depth,
                max_iterations: child_max_iter,
                verbose: child_verbose,
            };
            // Bridge async -> sync via a new tokio runtime
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("build child runtime");
            match rt.block_on(child.run(&q, &sub_ctx)) {
                Ok(ans) => ans,
                Err(e) => format!("rlm_call error: {e}"),
            }
        };

        let engine = build_engine(ctx.to_string(), Arc::clone(&state), rlm_call_fn);

        let mut messages: Vec<Message> = vec![
            Message::system(SYSTEM_PROMPT),
            Message::user(format!("Query: {query}")),
        ];

        for iteration in 0..self.max_iterations {
            debug!(depth = self.depth, iteration, "RLM step");

            let script = self
                .provider
                .complete(messages.clone())
                .await
                .map_err(RlmError::ProviderError)?;

            let script = strip_code_fences(&script);

            if self.verbose {
                println!("[depth={}] >> {}", self.depth, script.trim());
            }

            // Execute the script; on error feed it back to the model for self-correction.
            let output = match run_script(&engine, &script, &state) {
                Ok(out) => out,
                Err(err) => {
                    tracing::warn!(depth = self.depth, iteration, error = %err, "script error, feeding back to model");
                    messages.push(Message::assistant(script));
                    messages.push(Message::user(format!(
                        "Script error:\n{err}\n\nFix the script and try again, or call final_answer()."
                    )));
                    continue;
                }
            };

            if self.verbose {
                println!("[depth={}] << {}", self.depth, output.trim());
            }

            // Check for final answer
            let final_answer = state.lock().ok().and_then(|s| s.final_answer.clone());
            if let Some(answer) = final_answer {
                if self.verbose {
                    println!("[depth={}] FINAL: {}", self.depth, answer);
                }
                info!(depth = self.depth, "RLM produced final answer");
                return Ok(answer);
            }

            notebook.push(script.clone(), output.clone());

            // Build next message: append assistant script + user (cell output)
            messages.push(Message::assistant(script));
            messages.push(Message::user(format!(
                "Cell output:\n{output}\n\nContinue toward answering the query, or call final_answer()."
            )));

            let _ = iteration; // suppress unused warning
        }

        Err(RlmError::MaxIterationsExceeded(self.max_iterations))
    }
}

pub(crate) fn strip_code_fences(s: &str) -> String {
    let s = s.trim();
    if let Some(inner) = s.strip_prefix("```rhai") {
        if let Some(inner) = inner.strip_suffix("```") {
            return inner.trim().to_string();
        }
    }
    if let Some(inner) = s.strip_prefix("```") {
        if let Some(inner) = inner.strip_suffix("```") {
            return inner.trim().to_string();
        }
    }
    s.to_string()
}
