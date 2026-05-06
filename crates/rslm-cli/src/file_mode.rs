use std::sync::Arc;

use anyhow::{bail, Context, Result};
use rslm_core::Rlm;
use rslm_providers::LlmProvider;

pub fn resolve_context(context_file: Option<String>, context: Option<String>) -> Result<String> {
    match (context_file, context) {
        (Some(path), _) => {
            std::fs::read_to_string(&path).with_context(|| format!("read context file: {path}"))
        }
        (_, Some(s)) => Ok(s),
        (None, None) => bail!("provide --context-file or --context"),
    }
}

pub async fn run(
    provider: Arc<dyn LlmProvider>,
    query: &str,
    ctx: &str,
    max_depth: usize,
    max_iterations: usize,
    verbose: bool,
) -> Result<()> {
    let rlm = Rlm::new(provider, max_depth, max_iterations, verbose);
    let answer = rlm.run(query, ctx).await?;
    println!("{answer}");
    Ok(())
}
