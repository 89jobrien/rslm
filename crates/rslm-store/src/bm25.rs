use std::collections::HashMap;

const K1: f32 = 1.5;
const B: f32 = 0.75;

pub struct Bm25Index {
    chunks: Vec<(usize, Vec<String>)>,
    avgdl: f32,
    idf: HashMap<String, f32>,
}

impl Bm25Index {
    /// Build index from (chunk_id, text) pairs.
    pub fn build(chunks: &[(usize, &str)]) -> Self {
        if chunks.is_empty() {
            return Self {
                chunks: vec![],
                avgdl: 0.0,
                idf: HashMap::new(),
            };
        }

        let tokenized: Vec<(usize, Vec<String>)> = chunks
            .iter()
            .map(|(id, text)| (*id, tokenize(text)))
            .collect();

        let n = tokenized.len() as f32;
        let avgdl = tokenized.iter().map(|(_, t)| t.len() as f32).sum::<f32>() / n;

        // Count document frequency per term
        let mut df: HashMap<String, usize> = HashMap::new();
        for (_, tokens) in &tokenized {
            let unique: std::collections::HashSet<&String> = tokens.iter().collect();
            for term in unique {
                *df.entry(term.clone()).or_insert(0) += 1;
            }
        }

        // Compute IDF using BM25 formula: ln((N - df + 0.5) / (df + 0.5) + 1)
        let idf = df
            .into_iter()
            .map(|(term, freq)| {
                let score = ((n - freq as f32 + 0.5) / (freq as f32 + 0.5) + 1.0).ln();
                (term, score)
            })
            .collect();

        Self {
            chunks: tokenized,
            avgdl,
            idf,
        }
    }

    /// Return top-k (chunk_id, score) sorted descending.
    pub fn search(&self, query: &str, k: usize) -> Vec<(usize, f32)> {
        let query_terms = tokenize(query);
        if query_terms.is_empty() || k == 0 {
            return vec![];
        }

        let mut scores: Vec<(usize, f32)> = self
            .chunks
            .iter()
            .map(|(chunk_id, tokens)| {
                let dl = tokens.len() as f32;
                let score = query_terms
                    .iter()
                    .map(|term| {
                        let idf = self.idf.get(term).copied().unwrap_or(0.0);
                        let tf = tokens.iter().filter(|t| *t == term).count() as f32;
                        let numerator = tf * (K1 + 1.0);
                        let denominator = tf + K1 * (1.0 - B + B * dl / self.avgdl);
                        idf * numerator / denominator
                    })
                    .sum::<f32>();
                (*chunk_id, score)
            })
            .collect();

        scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scores.truncate(k);
        scores
    }
}

fn tokenize(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build_test_index() -> Bm25Index {
        Bm25Index::build(&[
            (0, "the quick brown fox jumps over the lazy dog"),
            (1, "rust programming language systems programming"),
            (2, "bm25 information retrieval ranking algorithm scoring"),
        ])
    }

    #[test]
    fn test_relevant_chunk_ranks_first() {
        let index = build_test_index();
        let results = index.search("rust programming", 3);
        assert!(!results.is_empty());
        assert_eq!(results[0].0, 1, "chunk 1 (rust programming) should rank first");
    }

    #[test]
    fn test_empty_query_returns_empty() {
        let index = build_test_index();
        let results = index.search("", 10);
        assert!(results.is_empty());
    }

    #[test]
    fn test_k1_returns_at_most_one_result() {
        let index = build_test_index();
        let results = index.search("programming", 1);
        assert!(results.len() <= 1);
    }

    #[test]
    fn test_k0_returns_empty() {
        let index = build_test_index();
        let results = index.search("rust", 0);
        assert!(results.is_empty());
    }

    #[test]
    fn test_scores_sorted_descending() {
        let index = build_test_index();
        let results = index.search("programming language", 3);
        for i in 1..results.len() {
            assert!(
                results[i - 1].1 >= results[i].1,
                "results must be sorted descending by score"
            );
        }
    }

    #[test]
    fn test_exact_term_match_scores_higher() {
        let index = build_test_index();
        // "ranking" only appears in chunk 2
        let results = index.search("ranking", 3);
        assert!(!results.is_empty());
        assert_eq!(results[0].0, 2);
    }

    #[test]
    fn test_empty_corpus_returns_empty() {
        let index = Bm25Index::build(&[]);
        let results = index.search("anything", 5);
        assert!(results.is_empty());
    }

    #[test]
    fn test_query_term_not_in_corpus() {
        let index = build_test_index();
        // All scores will be 0 for an unknown term — results may be empty or all zeros
        let results = index.search("xyzzy", 3);
        for (_, score) in &results {
            assert_eq!(*score, 0.0, "unknown term should produce zero score");
        }
    }
}
