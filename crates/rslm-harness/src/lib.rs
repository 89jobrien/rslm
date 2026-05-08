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

/// Conformance suite: verifies every Rhai API function advertised in the system prompt.
#[cfg(test)]
mod conformance {
    use std::sync::Arc;

    use rslm_core::Rlm;
    use rslm_providers::{LlmProvider, Role};
    use serial_test::serial;

    use super::*;

    // #10 — ctx_len() returns correct byte length
    #[tokio::test]
    #[serial]
    async fn ctx_len_returns_byte_length() {
        let ctx = "hello world"; // 11 bytes
        let run = Harness::run(
            "q",
            ctx,
            vec![
                r#"
                let n = ctx_len();
                final_answer(n.to_string())
            "#,
            ],
        )
        .await
        .unwrap();
        assert_eq!(run.answer, "11");
    }

    // #11 — ctx_slice(start, end) returns expected substring
    #[tokio::test]
    #[serial]
    async fn ctx_slice_returns_substring() {
        let run = Harness::run("q", "abcdef", vec![r#"final_answer(ctx_slice(2, 5))"#])
            .await
            .unwrap();
        assert_eq!(run.answer, "cde");
    }

    // #12 — ctx_slice out-of-bounds: no panic, returns empty or truncated
    #[tokio::test]
    #[serial]
    async fn ctx_slice_out_of_bounds_no_panic() {
        let run = Harness::run("q", "abc", vec![r#"final_answer(ctx_slice(100, 200))"#])
            .await
            .unwrap();
        // Should not panic; result is empty
        assert_eq!(run.answer, "");
    }

    // #13 — ctx_grep returns matching lines
    #[tokio::test]
    #[serial]
    async fn ctx_grep_returns_matching_lines() {
        let ctx = "foo bar\nbaz qux\nfoo baz";
        let run = Harness::run("q", ctx, vec![r#"final_answer(ctx_grep("^foo"))"#])
            .await
            .unwrap();
        assert_eq!(run.answer, "foo bar\nfoo baz");
    }

    // #14 — ctx_grep no match returns empty string
    #[tokio::test]
    #[serial]
    async fn ctx_grep_no_match_returns_empty() {
        let run = Harness::run("q", "hello world", vec![r#"final_answer(ctx_grep("zzz"))"#])
            .await
            .unwrap();
        assert_eq!(run.answer, "");
    }

    // #15 — print_cell output appears in loop cell output
    #[tokio::test]
    #[serial]
    async fn print_cell_output_captured() {
        // Two-step: first step calls print_cell, second step calls final_answer.
        // The cell output from step 1 is fed back in the "Cell output:" user message,
        // which the harness records in messages_received[1].
        let run = Harness::run(
            "q",
            "ctx",
            vec![r#"print_cell("logged")"#, r#"final_answer("done")"#],
        )
        .await
        .unwrap();
        assert_eq!(run.answer, "done");
        // The second message pair should contain "logged" in the user turn
        // The conversation grows: find the *last* user message in the second call's message list,
        // which will be the "Cell output: logged" turn.
        let second_user = run.messages_received[1]
            .iter()
            .rev()
            .find(|m| matches!(m.role, Role::User))
            .map(|m| m.content.as_str())
            .unwrap_or("");
        assert!(
            second_user.contains("logged"),
            "expected 'logged' in cell output, got: {second_user}"
        );
    }

    // #16 — rlm_call delegates to child RLM and returns its answer
    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    #[serial]
    async fn rlm_call_delegates_to_child() {
        // Step 1: call rlm_call; the child provider is the same HarnessProvider which
        // has one response left — it will answer "child_answer".
        // Step 2: receive child result and call final_answer with it.
        let run = Harness::run(
            "q",
            "parent ctx",
            vec![
                // parent step 1: delegate
                r#"let ans = rlm_call("sub query", "child ctx"); final_answer(ans)"#,
                // child step 1 (consumed from same queue): final answer
                r#"final_answer("child_answer")"#,
            ],
        )
        .await
        .unwrap();
        assert_eq!(run.answer, "child_answer");
    }

    // #17 — final_answer works with a Rhai variable (not just a literal)
    #[tokio::test]
    #[serial]
    async fn final_answer_from_variable() {
        let run = Harness::run(
            "q",
            "ctx",
            vec![
                r#"
                let result = "computed";
                final_answer(result)
            "#,
            ],
        )
        .await
        .unwrap();
        assert_eq!(run.answer, "computed");
    }

    // #18 — script error feeds back to model; model corrects and produces final answer
    #[tokio::test]
    #[serial]
    async fn script_error_triggers_self_correction() {
        let run = Harness::run(
            "q",
            "ctx",
            vec![
                // Step 1: invalid Rhai — will produce a script error
                "this is not valid rhai @@@@",
                // Step 2: model "corrects" and answers
                r#"final_answer("corrected")"#,
            ],
        )
        .await
        .unwrap();
        // Self-correction succeeded; the error was fed back and the next step answered
        assert_eq!(run.answer, "corrected");
        // Two provider calls were made: one for the bad script, one for the correction
        assert_eq!(run.messages_received.len(), 2);
        // The second call's user message should contain "Script error"
        let second_user = run.messages_received[1]
            .iter()
            .rev()
            .find(|m| matches!(m.role, Role::User))
            .map(|m| m.content.as_str())
            .unwrap_or("");
        assert!(
            second_user.contains("Script error"),
            "expected 'Script error' in feedback message, got: {second_user}"
        );
    }

    // #19 — rlm_call at max depth returns MaxDepthExceeded error string (not panic)
    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    #[serial]
    async fn rlm_call_max_depth_guard() {
        // Create an Rlm with max_depth=1 so any rlm_call from depth=0 will hit depth=1 >= max.
        let provider = Arc::new(HarnessProvider::new(vec![
            // parent: tries to call rlm_call
            r#"let ans = rlm_call("sub", "ctx"); final_answer(ans)"#.to_string(),
        ]));
        let rlm = Rlm::new(Arc::clone(&provider) as Arc<dyn LlmProvider>, 1, 5, false);
        let result = rlm.run("q", "ctx").await.unwrap();
        // rlm_call returns "rlm_call error: max depth 1 exceeded" as a string
        assert!(
            result.contains("max depth"),
            "expected max depth error in rlm_call result, got: {result}"
        );
    }
}
