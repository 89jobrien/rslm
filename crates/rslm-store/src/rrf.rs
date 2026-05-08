/// Reciprocal Rank Fusion of two ranked lists.
///
/// Each list is `Vec<(chunk_id, score)>` sorted **descending** by score.
/// Returns merged top-k sorted **descending** by RRF score.
///
/// RRF(d) = sum_r 1 / (k_const + rank_r(d)),  k_const = 60
pub fn rrf_merge(
    bm25: &[(usize, f32)],
    semantic: &[(usize, f32)],
    top_k: usize,
) -> Vec<(usize, f32)> {
    const K_CONST: f32 = 60.0;

    let mut scores: std::collections::HashMap<usize, f32> = std::collections::HashMap::new();

    for (rank, (id, _score)) in bm25.iter().enumerate() {
        *scores.entry(*id).or_insert(0.0) += 1.0 / (K_CONST + rank as f32 + 1.0);
    }
    for (rank, (id, _score)) in semantic.iter().enumerate() {
        *scores.entry(*id).or_insert(0.0) += 1.0 / (K_CONST + rank as f32 + 1.0);
    }

    let mut merged: Vec<(usize, f32)> = scores.into_iter().collect();
    merged.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    merged.truncate(top_k);
    merged
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rrf_merge_overlap_ranks_higher() {
        // chunk 1 appears in both lists, chunks 0/2 appear in only one
        let bm25 = vec![(0usize, 1.0f32), (1, 0.8), (2, 0.5)];
        let semantic = vec![(1usize, 1.0f32), (3, 0.7), (4, 0.3)];
        let merged = rrf_merge(&bm25, &semantic, 5);

        // chunk 1 must rank first (appears in both lists)
        assert_eq!(
            merged[0].0, 1,
            "chunk present in both lists should rank first"
        );

        // Scores must be sorted descending
        for w in merged.windows(2) {
            assert!(w[0].1 >= w[1].1);
        }
    }

    #[test]
    fn test_rrf_merge_top_k_respected() {
        let bm25: Vec<(usize, f32)> = (0..10).map(|i| (i, 10.0 - i as f32)).collect();
        let semantic: Vec<(usize, f32)> = (5..15).map(|i| (i, 15.0 - i as f32)).collect();
        let merged = rrf_merge(&bm25, &semantic, 3);
        assert_eq!(merged.len(), 3);
    }

    #[test]
    fn test_rrf_empty_lists() {
        let merged = rrf_merge(&[], &[], 5);
        assert!(merged.is_empty());
    }
}
