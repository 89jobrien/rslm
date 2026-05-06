use std::io::{self, BufRead, Write};
use std::sync::Arc;

use anyhow::Result;
use rslm_core::Rlm;
use rslm_providers::LlmProvider;

pub async fn run(
    provider: Arc<dyn LlmProvider>,
    max_depth: usize,
    max_iterations: usize,
    verbose: bool,
) -> Result<()> {
    let stdin = io::stdin();
    let stdout = io::stdout();

    print!("Query: ");
    stdout.lock().flush()?;
    let mut query = String::new();
    stdin.lock().read_line(&mut query)?;
    let query = query.trim().to_string();

    print!("Context file path (or press Enter to type inline): ");
    stdout.lock().flush()?;
    let mut path_line = String::new();
    stdin.lock().read_line(&mut path_line)?;
    let path_line = path_line.trim().to_string();

    let ctx = if path_line.is_empty() {
        println!("Paste context (end with a line containing only '---'):");
        let mut lines = Vec::new();
        for line in stdin.lock().lines() {
            let line = line?;
            if line == "---" {
                break;
            }
            lines.push(line);
        }
        lines.join("\n")
    } else {
        std::fs::read_to_string(&path_line).map_err(|e| anyhow::anyhow!("read {path_line}: {e}"))?
    };

    let rlm = Rlm::new(provider, max_depth, max_iterations, verbose);
    let answer = rlm.run(&query, &ctx).await?;
    println!("\nAnswer: {answer}");
    Ok(())
}
