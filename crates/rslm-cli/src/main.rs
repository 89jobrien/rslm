mod file_mode;
mod ingest_mode;
mod interactive;
#[cfg(test)]
mod tests;

use std::sync::Arc;

use anyhow::{bail, Context, Result};
use clap::{CommandFactory, Parser, Subcommand};
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

    /// Path to SQLite chunk store (enables ctx_chunks/ctx_search/ctx_hybrid Rhai functions)
    #[arg(long, global = true)]
    store: Option<String>,

    /// Document ID in the store (defaults to context file path)
    #[arg(long, global = true)]
    doc_id: Option<String>,

    /// Embedding model for semantic search (default: text-embedding-3-small)
    #[arg(long, global = true, default_value = "text-embedding-3-small")]
    embed_model: String,
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
    /// Ingest a document into the chunk store
    Ingest {
        /// Context file to ingest
        #[arg(long)]
        context_file: String,
        /// Chunking strategy: fixed, paragraph, line
        #[arg(long, default_value = "paragraph")]
        strategy: String,
        /// Document ID (defaults to file path)
        #[arg(long)]
        doc_id: Option<String>,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    if std::env::args().nth(1).as_deref() == Some("completions") {
        clap_complete::generate(
            clap_complete_nushell::Nushell,
            &mut Cli::command(),
            "rslm",
            &mut std::io::stdout(),
        );
        return Ok(());
    }

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
            let ctx = file_mode::resolve_context(context_file.clone(), context)?;

            if let Some(ref store_path) = cli.store {
                let store = Arc::new(rslm_store::ChunkStore::open(store_path)?);
                let doc_id = cli
                    .doc_id
                    .clone()
                    .or(context_file)
                    .unwrap_or_else(|| "default".to_string());

                // Auto-ingest if no chunks present for this doc
                if store.get_chunks(&doc_id)?.is_empty() {
                    store
                        .ingest(&doc_id, &ctx, &rslm_store::ChunkStrategy::Paragraph, None)
                        .await?;
                }

                let embedder: Option<Arc<dyn rslm_store::EmbedProvider>> =
                    if std::env::var("OPENAI_API_KEY").is_ok() {
                        Some(Arc::new(rslm_store::embed::OpenAiEmbedder::new(
                            &cli.embed_model,
                        )))
                    } else {
                        None
                    };

                let rlm =
                    rslm_core::Rlm::new(provider, cli.max_depth, cli.max_iterations, cli.verbose)
                        .with_store(Arc::clone(&store), embedder, doc_id);
                let answer = rlm.run(&query, &ctx).await?;
                println!("{answer}");
                Ok(())
            } else {
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
        }
        Command::Interactive => {
            interactive::run(provider, cli.max_depth, cli.max_iterations, cli.verbose).await
        }
        Command::Ingest {
            context_file,
            strategy,
            doc_id,
        } => {
            let store_path = cli
                .store
                .as_deref()
                .ok_or_else(|| anyhow::anyhow!("--store is required for ingest"))?;
            ingest_mode::run(
                store_path,
                &context_file,
                &strategy,
                doc_id.as_deref(),
                &cli.embed_model,
            )
            .await
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
