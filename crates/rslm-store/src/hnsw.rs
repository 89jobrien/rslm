use instant_distance::{Builder, HnswMap, Point as InstantPoint, Search};

/// Newtype implementing cosine distance for instant-distance.
#[derive(Clone)]
struct EmbedPoint(Vec<f32>);

impl InstantPoint for EmbedPoint {
    fn distance(&self, other: &Self) -> f32 {
        let dot: f32 = self.0.iter().zip(&other.0).map(|(a, b)| a * b).sum();
        let na: f32 = self.0.iter().map(|x| x * x).sum::<f32>().sqrt();
        let nb: f32 = other.0.iter().map(|x| x * x).sum::<f32>().sqrt();
        1.0 - dot / (na * nb + f32::EPSILON)
    }
}

pub struct HnswIndex {
    inner: HnswMap<EmbedPoint, usize>,
}

impl HnswIndex {
    /// Build from (chunk_id, embedding) pairs.
    pub fn build(embeddings: &[(usize, Vec<f32>)]) -> Self {
        let points: Vec<EmbedPoint> = embeddings.iter().map(|(_, v)| EmbedPoint(v.clone())).collect();
        let values: Vec<usize> = embeddings.iter().map(|(id, _)| *id).collect();
        let inner = Builder::default().build(points, values);
        Self { inner }
    }

    /// Return top-k (chunk_id, distance) sorted ascending by distance.
    pub fn search(&self, query: &[f32], k: usize) -> Vec<(usize, f32)> {
        let q = EmbedPoint(query.to_vec());
        let mut search = Search::default();
        let mut results: Vec<(usize, f32)> = self
            .inner
            .search(&q, &mut search)
            .take(k)
            .map(|item| (*item.value, item.distance))
            .collect();
        results.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unit_vec(dim: usize, seed: u64) -> Vec<f32> {
        // Simple deterministic pseudo-random unit vector via LCG
        let mut state = seed.wrapping_add(1);
        let mut v: Vec<f32> = (0..dim)
            .map(|_| {
                state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                // Map high 32 bits to [-1, 1]
                let hi = (state >> 32) as u32;
                (hi as f32 / u32::MAX as f32) * 2.0 - 1.0
            })
            .collect();
        let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > f32::EPSILON {
            v.iter_mut().for_each(|x| *x /= norm);
        }
        v
    }

    #[test]
    fn test_hnsw_exact_match_ranks_first() {
        let dim = 128;
        let n = 20;
        let embeddings: Vec<(usize, Vec<f32>)> = (0..n).map(|i| (i, unit_vec(dim, i as u64 + 1))).collect();
        let index = HnswIndex::build(&embeddings);

        // Query with the 5th vector — exact match should rank first with distance ≈ 0
        let query = embeddings[5].1.clone();
        let results = index.search(&query, 3);

        assert_eq!(results.len(), 3, "should return exactly 3 results");
        let (top_id, top_dist) = results[0];
        assert_eq!(top_id, 5, "exact match should rank first");
        assert!(top_dist < 1e-4, "distance to self should be ≈ 0, got {top_dist}");
    }

    #[test]
    fn test_hnsw_results_sorted_ascending() {
        let dim = 128;
        let n = 20;
        let embeddings: Vec<(usize, Vec<f32>)> = (0..n).map(|i| (i, unit_vec(dim, i as u64 + 1))).collect();
        let index = HnswIndex::build(&embeddings);

        let query = unit_vec(dim, 999);
        let results = index.search(&query, 5);

        assert!(!results.is_empty());
        for w in results.windows(2) {
            assert!(w[0].1 <= w[1].1, "results must be sorted ascending by distance");
        }
    }

    #[test]
    fn test_hnsw_k_results() {
        let dim = 128;
        let n = 20;
        let embeddings: Vec<(usize, Vec<f32>)> = (0..n).map(|i| (i, unit_vec(dim, i as u64 + 1))).collect();
        let index = HnswIndex::build(&embeddings);

        let query = embeddings[10].1.clone();
        let results = index.search(&query, 3);
        assert_eq!(results.len(), 3);
    }
}
