use crate::fts::SearchResult;

/// Reciprocal Rank Fusion (RRF) to merge FTS and semantic search results.
/// `score = 1/(k + rank_fts) + 1/(k + rank_semantic)`, k=60.
pub fn fuse_rrf(
    fts_results: &[SearchResult],
    semantic_results: &[(String, f32)],
    k: usize,
) -> Vec<SearchResult> {
    use std::collections::HashMap;

    let mut scores: HashMap<String, f64> = HashMap::new();
    let mut result_map: HashMap<String, SearchResult> = HashMap::new();

    // Score from FTS ranking
    for (rank, result) in fts_results.iter().enumerate() {
        let id = result.id.to_string();
        *scores.entry(id.clone()).or_default() += 1.0 / (k as f64 + rank as f64 + 1.0);
        result_map.insert(id, result.clone());
    }

    // Score from semantic ranking
    for (rank, (note_id, _sim)) in semantic_results.iter().enumerate() {
        *scores.entry(note_id.clone()).or_default() += 1.0 / (k as f64 + rank as f64 + 1.0);
        // If not in result_map from FTS, create a placeholder SearchResult
        if !result_map.contains_key(note_id) {
            if let Ok(nid) = note_id.parse() {
                result_map.insert(
                    note_id.clone(),
                    SearchResult {
                        id: nid,
                        title: String::new(),
                        snippet: String::new(),
                        score: 0.0,
                    },
                );
            }
        }
    }

    // Sort by RRF score descending
    let mut ranked: Vec<(String, f64)> = scores.into_iter().collect();
    ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    ranked
        .into_iter()
        .filter_map(|(id, rrf_score)| {
            result_map.remove(&id).map(|mut r| {
                r.score = rrf_score as f32;
                r
            })
        })
        .collect()
}
