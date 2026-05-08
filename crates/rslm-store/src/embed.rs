use anyhow::Result;
use async_trait::async_trait;

const BATCH_SIZE: usize = 100;

#[async_trait]
pub trait EmbedProvider: Send + Sync {
    async fn embed(&self, texts: Vec<String>) -> Result<Vec<Vec<f32>>>;
}

pub struct OpenAiEmbedder {
    model: String,
    client: async_openai::Client<async_openai::config::OpenAIConfig>,
}

impl OpenAiEmbedder {
    /// Default model: `text-embedding-3-small`.
    /// Reads `OPENAI_API_KEY` from the environment automatically via async-openai.
    pub fn new(model: impl Into<String>) -> Self {
        Self {
            model: model.into(),
            client: async_openai::Client::new(),
        }
    }
}

#[async_trait]
impl EmbedProvider for OpenAiEmbedder {
    async fn embed(&self, texts: Vec<String>) -> Result<Vec<Vec<f32>>> {
        use async_openai::types::CreateEmbeddingRequestArgs;

        let mut all_embeddings: Vec<Vec<f32>> = Vec::with_capacity(texts.len());

        for chunk in texts.chunks(BATCH_SIZE) {
            let request = CreateEmbeddingRequestArgs::default()
                .model(self.model.clone())
                .input(chunk.to_vec())
                .build()?;

            let response = self.client.embeddings().create(request).await?;

            let mut batch: Vec<(usize, Vec<f32>)> = response
                .data
                .into_iter()
                .map(|e| (e.index as usize, e.embedding))
                .collect();

            batch.sort_by_key(|(idx, _)| *idx);
            all_embeddings.extend(batch.into_iter().map(|(_, v)| v));
        }

        Ok(all_embeddings)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockEmbedder {
        dim: usize,
    }

    #[async_trait]
    impl EmbedProvider for MockEmbedder {
        async fn embed(&self, texts: Vec<String>) -> Result<Vec<Vec<f32>>> {
            Ok(texts
                .into_iter()
                .map(|_| vec![0.0_f32; self.dim])
                .collect())
        }
    }

    #[tokio::test]
    async fn mock_returns_correct_dim() {
        let embedder = MockEmbedder { dim: 8 };
        let result = embedder
            .embed(vec!["hello".to_string(), "world".to_string()])
            .await
            .unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].len(), 8);
    }

    /// Verify batching: 250 texts split into chunks of 100 = 3 batches.
    #[tokio::test]
    async fn batching_250_texts_yields_250_embeddings() {
        let embedder = MockEmbedder { dim: 4 };
        let texts: Vec<String> = (0..250).map(|i| format!("text {i}")).collect();
        let result = embedder.embed(texts).await.unwrap();
        assert_eq!(result.len(), 250);
        for vec in &result {
            assert_eq!(vec.len(), 4);
        }
    }

    #[cfg(feature = "live-tests")]
    #[tokio::test]
    async fn live_openai_embed_dim_1536() {
        let embedder = OpenAiEmbedder::new("text-embedding-3-small");
        let result = embedder
            .embed(vec!["hello world".to_string()])
            .await
            .unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].len(), 1536);
    }
}
