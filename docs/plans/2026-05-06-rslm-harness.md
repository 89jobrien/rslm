# Plan: rslm-harness

## Goal

Add a deterministic test harness crate that drives `Rlm` with scripted provider responses and
captures the full execution trace, enabling integration tests and benchmarks without a real LLM.

## Architecture

- Crates affected: new `crates/rslm-harness`; `Cargo.toml` workspace members updated
- New traits/types:
  - `HarnessProvider` (`rslm-harness/src/provider.rs`) — `LlmProvider` impl backed by a vec of
    canned response strings; records every `complete()` call's input messages
  - `HarnessRun` (`rslm-harness/src/lib.rs`) — result type holding `answer`, `notebook`,
    `iterations`, and `messages_received`
  - `Harness` (`rslm-harness/src/lib.rs`) — builder/runner: takes query, ctx, responses vec,
    returns `HarnessRun`
- Data flow:
  `Harness::run(query, ctx, responses)` → builds `HarnessProvider` + `Rlm` → calls `rlm.run()`
  → returns `HarnessRun { answer, notebook, iterations, messages_received }`

## Tech Stack

- Rust edition 2021 (workspace)
- New crate deps: `rslm-core`, `rslm-providers`, `tokio` (full), `async-trait`, `anyhow`
  (all `workspace = true`)

## Tasks

### Task 1: Add workspace member and crate skeleton

**Crate**: `rslm-harness`
**File(s)**: `Cargo.toml`, `crates/rslm-harness/Cargo.toml`, `crates/rslm-harness/src/lib.rs`
**Run**: `cargo check --workspace`

1. Add `"crates/rslm-harness"` to `[workspace] members` in root `Cargo.toml`.

2. Create `crates/rslm-harness/Cargo.toml`:

   ```toml
   [package]
   name = "rslm-harness"
   version.workspace = true
   edition.workspace = true
   license.workspace = true
   authors.workspace = true

   [dependencies]
   rslm-core.workspace = true
   rslm-providers.workspace = true
   tokio.workspace = true
   async-trait.workspace = true
   anyhow.workspace = true
   ```

3. Create `crates/rslm-harness/src/lib.rs` with empty public items to satisfy `cargo check`:

   ```rust
   pub mod provider;

   pub use provider::HarnessProvider;

   /// Collected result of a single harness run.
   pub struct HarnessRun {
       pub answer: String,
       pub iterations: usize,
       pub messages_received: Vec<Vec<rslm_providers::Message>>,
   }
   ```

4. Create `crates/rslm-harness/src/provider.rs`:

   ```rust
   use std::sync::Mutex;

   use anyhow::Result;
   use async_trait::async_trait;
   use rslm_providers::{LlmProvider, Message};

   /// A deterministic provider that replays scripted responses in order.
   /// After responses are exhausted it returns `final_answer("no more responses")`.
   pub struct HarnessProvider {
       responses: Mutex<Vec<String>>,
       pub received: Mutex<Vec<Vec<Message>>>,
   }

   impl HarnessProvider {
       pub fn new(responses: Vec<impl Into<String>>) -> Self {
           Self {
               responses: Mutex::new(responses.into_iter().map(Into::into).collect()),
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
   ```

5. Verify:
   ```
   cargo check --workspace   → zero errors
   ```

6. Run: `git branch --show-current`
   Commit: `git commit -m "feat(rslm-harness): add crate skeleton and HarnessProvider"`

### Task 2: Implement Harness runner and HarnessRun

**Crate**: `rslm-harness`
**File(s)**: `crates/rslm-harness/src/lib.rs`
**Run**: `cargo nextest run -p rslm-harness`

1. Write failing test at the bottom of `crates/rslm-harness/src/lib.rs`:

   ```rust
   #[cfg(test)]
   mod tests {
       use std::sync::Arc;
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
           let run = Harness::run(
               "q",
               "ctx",
               vec![r#"final_answer("ok")"#],
           )
           .await
           .unwrap();
           // provider.complete() called once → one message batch recorded
           assert_eq!(run.messages_received.len(), 1);
       }

       #[tokio::test]
       async fn harness_multi_step() {
           // First response does not call final_answer; second does.
           let run = Harness::run(
               "q",
               "ctx",
               vec!["ctx_len()", r#"final_answer("done")"#],
           )
           .await
           .unwrap();
           assert_eq!(run.answer, "done");
           assert_eq!(run.iterations, 2);
       }
   }
   ```

   Run: `cargo nextest run -p rslm-harness`
   Expected: FAIL (no `Harness` type yet)

2. Implement `Harness` in `crates/rslm-harness/src/lib.rs`:

   ```rust
   pub mod provider;

   use std::sync::Arc;

   use anyhow::Result;
   pub use provider::HarnessProvider;
   use rslm_core::Rlm;
   use rslm_providers::Message;

   /// Collected result of a single harness run.
   pub struct HarnessRun {
       pub answer: String,
       pub iterations: usize,
       pub messages_received: Vec<Vec<Message>>,
   }

   /// Synchronous-friendly test driver for `Rlm`.
   pub struct Harness;

   impl Harness {
       /// Run the RLM loop with scripted `responses` and return the trace.
       ///
       /// - `max_depth = 3`, `max_iterations = responses.len().max(20)`
       /// - Panics if the provider errors (responses are always `Ok`).
       pub async fn run(
           query: &str,
           ctx: &str,
           responses: Vec<impl Into<String>>,
       ) -> Result<HarnessRun> {
           let provider = Arc::new(HarnessProvider::new(responses));
           let max_iter = provider.responses.lock().unwrap().len().max(20);
           let rlm = Rlm::new(Arc::clone(&provider) as Arc<dyn rslm_providers::LlmProvider>, 3,
               max_iter, false);
           let answer = rlm.run(query, ctx).await?;
           let received = provider.received.lock().unwrap().clone();
           let iterations = received.len();
           Ok(HarnessRun { answer, iterations, messages_received: received })
       }
   }
   ```

   > Note: `provider.responses` is private — expose it via a `len()` helper or compute
   > `max_iter` before constructing the provider. Fix by storing the length before building:

   Final implementation (resolves the field visibility issue):

   ```rust
   pub async fn run(
       query: &str,
       ctx: &str,
       responses: Vec<impl Into<String>>,
   ) -> Result<HarnessRun> {
       let responses: Vec<String> = responses.into_iter().map(Into::into).collect();
       let max_iter = responses.len().max(20);
       let provider = Arc::new(HarnessProvider::new(responses));
       let rlm = Rlm::new(
           Arc::clone(&provider) as Arc<dyn rslm_providers::LlmProvider>,
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
   ```

3. Verify:
   ```
   cargo nextest run -p rslm-harness   → all green
   cargo clippy -p rslm-harness -- -D warnings  → zero warnings
   ```

4. Run: `git branch --show-current`
   Commit: `git commit -m "feat(rslm-harness): implement Harness runner and HarnessRun"`

### Task 3: Integration smoke test from rslm-core perspective

**Crate**: `rslm-harness`
**File(s)**: `crates/rslm-harness/src/lib.rs` (tests section)
**Run**: `cargo nextest run -p rslm-harness`

1. Add depth-limit and max-iterations error tests:

   ```rust
   #[tokio::test]
   async fn harness_propagates_max_iterations_error() {
       use rslm_core::RlmError;
       // Provide only no-op scripts — never calls final_answer.
       // max_iter will be max(1, 20) = 20; use a tiny override by constructing manually.
       let provider = Arc::new(HarnessProvider::new(vec!["ctx_len()"]));
       let rlm = rslm_core::Rlm::new(
           Arc::clone(&provider) as Arc<dyn rslm_providers::LlmProvider>,
           3,
           1, // max_iterations = 1
           false,
       );
       let result = rlm.run("q", "ctx").await;
       assert!(matches!(result, Err(RlmError::MaxIterationsExceeded(1))));
   }
   ```

2. Verify:
   ```
   cargo nextest run -p rslm-harness   → all green
   cargo clippy -p rslm-harness -- -D warnings  → zero warnings
   cargo check --workspace             → zero errors
   ```

3. Run: `git branch --show-current`
   Commit: `git commit -m "test(rslm-harness): add error-path integration tests"`

## Quality Rules

- No placeholders — all code above is copy-paste ready
- `HarnessProvider::received` is `pub` so callers can inspect raw message history
- `Harness::run` is `async`; callers use `#[tokio::test]` or `tokio::runtime::Runtime::block_on`
- `HarnessProvider` and `Harness` are both `pub` from the crate root
