use std::sync::Arc;

use anyhow::Result;

pub async fn run(
    store_path: &str,
    context_file: &str,
    strategy_str: &str,
    doc_id: Option<&str>,
    embed_model: &str,
) -> Result<()> {
    let text = std::fs::read_to_string(context_file)?;
    let doc_id = doc_id.unwrap_or(context_file);
    let strategy = match strategy_str {
        "fixed" => rslm_store::ChunkStrategy::Fixed {
            size: 1000,
            overlap: 100,
        },
        "line" => rslm_store::ChunkStrategy::Line { count: 50 },
        _ => rslm_store::ChunkStrategy::Paragraph,
    };
    let store = rslm_store::ChunkStore::open(store_path)?;
    let embedder: Option<Arc<dyn rslm_store::EmbedProvider>> =
        if std::env::var("OPENAI_API_KEY").is_ok() {
            Some(Arc::new(rslm_store::embed::OpenAiEmbedder::new(
                embed_model,
            )))
        } else {
            None
        };
    let count = store
        .ingest(doc_id, &text, &strategy, embedder.as_deref())
        .await?;
    println!("Ingested {count} chunks for doc '{doc_id}' into {store_path}");
    Ok(())
}
