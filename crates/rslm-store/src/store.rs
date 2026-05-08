use std::sync::{Arc, Mutex, RwLock};

use anyhow::Result;
use rusqlite::{params, Connection};

use crate::{
    bm25::Bm25Index,
    chunk::{chunk_text, ChunkStrategy},
    embed::EmbedProvider,
    hnsw::HnswIndex,
    rrf::rrf_merge,
};

pub struct ChunkStore {
    conn: Mutex<Connection>,
    hnsw: Arc<RwLock<Option<HnswIndex>>>,
}

impl ChunkStore {
    /// Open or create a SQLite store at `path`.
    pub fn open(path: &str) -> Result<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS chunks (
                id        INTEGER PRIMARY KEY,
                doc_id    TEXT NOT NULL,
                idx       INTEGER NOT NULL,
                text      TEXT NOT NULL,
                embedding BLOB,
                UNIQUE(doc_id, idx)
            );
            CREATE INDEX IF NOT EXISTS chunks_doc ON chunks(doc_id);",
        )?;
        Ok(Self {
            conn: Mutex::new(conn),
            hnsw: Arc::new(RwLock::new(None)),
        })
    }

    /// Ingest a document: chunk, embed (if embedder provided), store.
    /// Idempotent: uses INSERT OR REPLACE on (doc_id, idx).
    /// Returns chunk count.
    pub async fn ingest(
        &self,
        doc_id: &str,
        text: &str,
        strategy: &ChunkStrategy,
        embedder: Option<&dyn EmbedProvider>,
    ) -> Result<usize> {
        let texts = chunk_text(text, strategy);
        let n = texts.len();

        let embeddings: Vec<Option<Vec<f32>>> = if let Some(emb) = embedder {
            let vecs = emb.embed(texts.clone()).await?;
            vecs.into_iter().map(Some).collect()
        } else {
            vec![None; n]
        };

        let conn = self.conn.lock().unwrap();
        for (idx, (chunk_text, embedding)) in texts.iter().zip(embeddings.iter()).enumerate() {
            let blob: Option<Vec<u8>> = embedding
                .as_ref()
                .map(|v| v.iter().flat_map(|f| f.to_le_bytes()).collect());
            conn.execute(
                "INSERT OR REPLACE INTO chunks (doc_id, idx, text, embedding)
                 VALUES (?1, ?2, ?3, ?4)",
                params![doc_id, idx as i64, chunk_text, blob],
            )?;
        }

        Ok(n)
    }

    /// Load all chunks for a doc, sorted by idx.
    pub fn get_chunks(&self, doc_id: &str) -> Result<Vec<(usize, String)>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt =
            conn.prepare("SELECT idx, text FROM chunks WHERE doc_id = ?1 ORDER BY idx")?;
        let rows = stmt.query_map(params![doc_id], |row| {
            let idx: i64 = row.get(0)?;
            let text: String = row.get(1)?;
            Ok((idx as usize, text))
        })?;
        let mut result = Vec::new();
        for row in rows {
            result.push(row?);
        }
        Ok(result)
    }

    /// BM25 keyword search.
    pub fn search_bm25(&self, doc_id: &str, query: &str, k: usize) -> Result<Vec<(usize, f32)>> {
        let chunks = self.get_chunks(doc_id)?;
        if chunks.is_empty() {
            return Ok(vec![]);
        }
        let pairs: Vec<(usize, &str)> = chunks.iter().map(|(i, t)| (*i, t.as_str())).collect();
        let index = Bm25Index::build(&pairs);
        Ok(index.search(query, k))
    }

    /// Cosine semantic search (requires embeddings in store).
    pub async fn search_semantic(
        &self,
        doc_id: &str,
        query: &[f32],
        k: usize,
    ) -> Result<Vec<(usize, f32)>> {
        let embeddings = self.load_embeddings(doc_id)?;
        if embeddings.is_empty() {
            return Ok(vec![]);
        }
        let index = HnswIndex::build(&embeddings);
        // HNSW returns ascending by distance; convert distance to score = 1 - dist
        let results = index
            .search(query, k)
            .into_iter()
            .map(|(id, dist)| (id, 1.0 - dist))
            .collect();
        Ok(results)
    }

    /// Hybrid RRF search. Returns chunk texts ranked by RRF.
    pub async fn search_hybrid(
        &self,
        doc_id: &str,
        query_text: &str,
        query_vec: &[f32],
        k: usize,
    ) -> Result<Vec<(usize, String)>> {
        let bm25_results = self.search_bm25(doc_id, query_text, k * 2)?;
        let semantic_results = self.search_semantic(doc_id, query_vec, k * 2).await?;

        let merged = rrf_merge(&bm25_results, &semantic_results, k);

        let chunks = self.get_chunks(doc_id)?;
        let chunk_map: std::collections::HashMap<usize, String> = chunks.into_iter().collect();

        let results = merged
            .into_iter()
            .filter_map(|(idx, _score)| chunk_map.get(&idx).map(|t| (idx, t.clone())))
            .collect();
        Ok(results)
    }

    /// Rebuild the in-memory HNSW index from stored embeddings for this doc.
    pub fn rebuild_hnsw(&self, doc_id: &str) -> Result<()> {
        let embeddings = self.load_embeddings(doc_id)?;
        let index = if embeddings.is_empty() {
            None
        } else {
            Some(HnswIndex::build(&embeddings))
        };
        *self.hnsw.write().unwrap() = index;
        Ok(())
    }

    fn load_embeddings(&self, doc_id: &str) -> Result<Vec<(usize, Vec<f32>)>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT idx, embedding FROM chunks WHERE doc_id = ?1 AND embedding IS NOT NULL ORDER BY idx",
        )?;
        let rows = stmt.query_map(params![doc_id], |row| {
            let idx: i64 = row.get(0)?;
            let blob: Vec<u8> = row.get(1)?;
            Ok((idx as usize, blob))
        })?;
        let mut result = Vec::new();
        for row in rows {
            let (idx, blob) = row?;
            let floats: Vec<f32> = blob
                .chunks_exact(4)
                .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
                .collect();
            result.push((idx, floats));
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    fn tmp_store() -> (NamedTempFile, ChunkStore) {
        let f = NamedTempFile::new().unwrap();
        let store = ChunkStore::open(f.path().to_str().unwrap()).unwrap();
        (f, store)
    }

    #[tokio::test]
    async fn test_chunk_fixed() {
        // 500 ASCII chars, Fixed{size:100, overlap:20}
        let text: String = "abcdefghij".repeat(50); // 500 bytes
        let chunks = chunk_text(
            &text,
            &ChunkStrategy::Fixed {
                size: 100,
                overlap: 20,
            },
        );
        // Each chunk should be 100 chars
        for c in &chunks {
            assert!(c.len() <= 100);
        }
        // Consecutive chunks overlap by 20 chars
        for pair in chunks.windows(2) {
            let tail = &pair[0][pair[0].len() - 20..];
            let head = &pair[1][..20];
            assert_eq!(
                tail, head,
                "expected 20-char overlap between consecutive chunks"
            );
        }
    }

    #[tokio::test]
    async fn test_chunk_paragraph() {
        let text = "First paragraph.\n\nSecond paragraph.\n\nThird paragraph.";
        let chunks = chunk_text(text, &ChunkStrategy::Paragraph);
        assert_eq!(chunks.len(), 3);
    }

    #[tokio::test]
    async fn test_rrf_merge() {
        let bm25 = vec![(0usize, 1.0f32), (1, 0.8), (2, 0.5)];
        let semantic = vec![(1usize, 1.0f32), (3, 0.7)];
        let merged = rrf_merge(&bm25, &semantic, 4);
        // chunk 1 appears in both lists — should rank first
        assert_eq!(merged[0].0, 1);
    }

    #[tokio::test]
    async fn test_store_ingest_idempotent() {
        let (_f, store) = tmp_store();
        let text = "Hello world.\n\nSecond paragraph.\n\nThird paragraph.";
        let strategy = ChunkStrategy::Paragraph;
        let count1 = store.ingest("doc1", text, &strategy, None).await.unwrap();
        let count2 = store.ingest("doc1", text, &strategy, None).await.unwrap();
        assert_eq!(count1, count2);
        let chunks = store.get_chunks("doc1").unwrap();
        assert_eq!(chunks.len(), count1);
    }

    #[tokio::test]
    async fn test_store_bm25_search() {
        let (_f, store) = tmp_store();
        let doc = "the quick brown fox\n\nrust systems programming\n\nbm25 ranking algorithm";
        store
            .ingest("doc2", doc, &ChunkStrategy::Paragraph, None)
            .await
            .unwrap();
        let results = store.search_bm25("doc2", "rust programming", 3).unwrap();
        assert!(!results.is_empty());
        // Chunk idx 1 contains "rust systems programming" — should rank first
        assert_eq!(results[0].0, 1);
    }
}
