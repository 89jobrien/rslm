mod file_mode;
mod interactive;
#[cfg(test)]
mod tests;

use std::sync::Arc;

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use rslm_providers::{AnthropicProvider, LlmProvider, OpenAiProvider};
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(name = "rslm", about = "Recursive Language Model inference")]
struct Cli {
    #[command(subcommand)]
    command: Command,

    /// Override provider (openai | anthropic); default: RSLM_PROVIDER env or openai
    #[arg(long, global = true)]
    provider: Option<String>,

    /// Override model ID; default: RSLM_MODEL env or provider default
    #[arg(long, global = true)]
    model: Option<String>,

    /// Maximum recursion depth
    #[arg(long, global = true, default_value = "5")]
    max_depth: usize,

    /// Maximum loop iterations per RLM instance
    #[arg(long, global = true, default_value = "20")]
    max_iterations: usize,

    /// Stream Rhai cells and outputs to terminal
    #[arg(long, global = true)]
    verbose: bool,
}

#[derive(Subcommand)]
enum Command {
    /// Run a single query with context
    Query {
        query: String,
        /// Read context from file
        #[arg(long)]
        context_file: Option<String>,
        /// Inline context string
        #[arg(long)]
        context: Option<String>,
    },
    /// Interactive mode: prompts for query and context
    Interactive,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .init();

    let provider = build_provider(cli.provider.as_deref(), cli.model.as_deref())?;

    match cli.command {
        Command::Query {
            query,
            context_file,
            context,
        } => {
            let ctx = file_mode::resolve_context(context_file, context)?;
            file_mode::run(
                provider,
                &query,
                &ctx,
                cli.max_depth,
                cli.max_iterations,
                cli.verbose,
            )
            .await
        }
        Command::Interactive => {
            interactive::run(provider, cli.max_depth, cli.max_iterations, cli.verbose).await
        }
    }
}

fn build_provider(provider: Option<&str>, model: Option<&str>) -> Result<Arc<dyn LlmProvider>> {
    let provider_name = provider
        .map(str::to_string)
        .or_else(|| std::env::var("RSLM_PROVIDER").ok())
        .unwrap_or_else(|| "openai".to_string());

    let model_id = model
        .map(str::to_string)
        .or_else(|| std::env::var("RSLM_MODEL").ok());

    match provider_name.as_str() {
        "openai" => {
            let m = model_id.unwrap_or_else(|| "gpt-4o".to_string());
            Ok(Arc::new(OpenAiProvider::new(m)))
        }
        "anthropic" => {
            let m = model_id.unwrap_or_else(|| "claude-sonnet-4-6".to_string());
            Ok(Arc::new(
                AnthropicProvider::new(m).context("init Anthropic provider")?,
            ))
        }
        other => bail!("unknown provider: {other}; use openai or anthropic"),
    }
}
