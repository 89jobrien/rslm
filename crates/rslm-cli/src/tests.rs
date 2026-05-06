#[cfg(test)]
mod file_mode_tests {
    use std::sync::Arc;

    use rslm_harness::HarnessProvider;
    use rslm_providers::LlmProvider;

    use crate::file_mode;

    // --- resolve_context ---

    #[test]
    fn resolve_context_inline() {
        let ctx = file_mode::resolve_context(None, Some("hello world".into())).unwrap();
        assert_eq!(ctx, "hello world");
    }

    #[test]
    fn resolve_context_file() {
        let mut f = tempfile::NamedTempFile::new().unwrap();
        std::io::Write::write_all(&mut f, b"file content").unwrap();
        let path = f.path().to_str().unwrap().to_string();
        let ctx = file_mode::resolve_context(Some(path), None).unwrap();
        assert_eq!(ctx, "file content");
    }

    #[test]
    fn resolve_context_file_takes_priority_over_inline() {
        let mut f = tempfile::NamedTempFile::new().unwrap();
        std::io::Write::write_all(&mut f, b"from file").unwrap();
        let path = f.path().to_str().unwrap().to_string();
        let ctx = file_mode::resolve_context(Some(path), Some("inline".into())).unwrap();
        assert_eq!(ctx, "from file");
    }

    #[test]
    fn resolve_context_neither_errors() {
        let result = file_mode::resolve_context(None, None);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("--context"));
    }

    #[test]
    fn resolve_context_missing_file_errors() {
        let result = file_mode::resolve_context(Some("/nonexistent/path/ctx.txt".into()), None);
        assert!(result.is_err());
    }

    // --- file_mode::run ---

    #[tokio::test]
    async fn file_mode_run_prints_answer() {
        let provider: Arc<dyn LlmProvider> = Arc::new(HarnessProvider::new(vec![
            r#"final_answer("42")"#.to_string(),
        ]));
        // run() prints to stdout and returns Ok — we just verify it doesn't error.
        let result = file_mode::run(provider, "what is the answer?", "ctx", 3, 20, false).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn file_mode_run_propagates_rlm_error() {
        // Provider never calls final_answer and max_iterations=1 → error propagates.
        let provider: Arc<dyn LlmProvider> =
            Arc::new(HarnessProvider::new(vec!["ctx_len()".to_string()]));
        let result = file_mode::run(provider, "q", "ctx", 3, 1, false).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("max iterations"));
    }
}
