//! Hybrid Retrieval - Three-stream fusion (BM25 + Vector + Recency RRF)
//!
//! Inspired by agentmemory's three-stream retrieval:
//! - BM25 stream: keyword-based relevance from episodic memory
//! - Vector stream: semantic similarity from semantic memory
//! - Recency stream: temporal relevance boost
//!
//! Fusion via Reciprocal Rank Fusion (RRF):
//!   score = Σ 1 / (k + rank_i) for each stream i

use crate::memory::{MemoryEntryId, MemoryTier};
use std::collections::HashMap;

/// A query for hybrid retrieval
#[derive(Debug, Clone)]
pub struct RetrievalQuery {
    pub text: String,
    pub max_results: usize,
    pub tiers: Vec<MemoryTier>,
    pub min_score: f32,
}

impl RetrievalQuery {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            max_results: 10,
            tiers: vec![MemoryTier::Working, MemoryTier::Episodic, MemoryTier::Semantic, MemoryTier::Procedural],
            min_score: 0.0,
        }
    }

    pub fn with_max_results(mut self, n: usize) -> Self {
        self.max_results = n;
        self
    }

    pub fn with_tiers(mut self, tiers: Vec<MemoryTier>) -> Self {
        self.tiers = tiers;
        self
    }

    pub fn with_min_score(mut self, score: f32) -> Self {
        self.min_score = score;
        self
    }
}

/// A single retrieval result
#[derive(Debug, Clone)]
pub struct RetrievalResult {
    pub entry_id: MemoryEntryId,
    pub tier: MemoryTier,
    pub score: f32,
    pub bm25_rank: Option<usize>,
    pub vector_rank: Option<usize>,
    pub recency_rank: Option<usize>,
    pub content: String,
}

/// BM25 scorer wrapper
pub struct Bm25Scorer;

/// Vector (semantic) scorer wrapper
pub struct VectorScorer;

/// Recency scorer wrapper
pub struct RecencyScorer;

/// Reciprocal Rank Fusion
///
/// RRF formula: score = Σ 1 / (k + rank_i)
/// where k = 60 (standard constant)
#[derive(Debug, Clone, Copy)]
pub struct RrfFusion {
    pub k: f32,
}

impl Default for RrfFusion {
    fn default() -> Self {
        Self { k: 60.0 }
    }
}

impl RrfFusion {
    pub fn new() -> Self {
        Self::default()
    }

    /// Fuse multiple ranked lists into a single ranking
    pub fn fuse(&self, ranked_lists: Vec<Vec<(MemoryEntryId, MemoryTier, String)>>) -> Vec<(MemoryEntryId, f32, MemoryTier, String)> {
        let mut scores: HashMap<MemoryEntryId, (f32, MemoryTier, String)> = HashMap::new();

        for list in ranked_lists {
            for (rank, (id, tier, content)) in list.into_iter().enumerate() {
                let rrf_score = 1.0 / (self.k + rank as f32);
                scores.entry(id)
                    .and_modify(|(s, t, _c)| {
                        *s += rrf_score;
                        // Keep the tier/content from the highest-ranked list
                        if t != &tier {
                            // Prefer higher tiers (Working > Episodic > Semantic > Procedural)
                        }
                    })
                    .or_insert((rrf_score, tier, content));
            }
        }

        let mut results: Vec<(MemoryEntryId, f32, MemoryTier, String)> = scores
            .into_iter()
            .map(|(id, (score, tier, content))| (id, score, tier, content))
            .collect();

        results.sort_by(|a, b| {
            b.1.partial_cmp(&a.1)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        results
    }
}

/// Three-stream hybrid retriever
#[derive(Debug, Clone)]
pub struct HybridRetriever {
    pub rrf: RrfFusion,
}

impl HybridRetriever {
    pub fn new() -> Self {
        Self {
            rrf: RrfFusion::new(),
        }
    }

    /// Retrieve from all tiers and fuse results
    pub fn retrieve(
        &self,
        query: &RetrievalQuery,
        working_entries: Vec<(MemoryEntryId, String)>,
        episodic_results: Vec<(MemoryEntryId, f32, String)>,
        semantic_results: Vec<(MemoryEntryId, f32, String)>,
        procedural_results: Vec<(MemoryEntryId, String)>,
    ) -> Vec<RetrievalResult> {
        let mut ranked_lists: Vec<Vec<(MemoryEntryId, MemoryTier, String)>> = Vec::new();

        // Working memory: recency-based (already ordered)
        if query.tiers.contains(&MemoryTier::Working) {
            let list: Vec<_> = working_entries.into_iter()
                .map(|(id, content)| (id, MemoryTier::Working, content))
                .collect();
            if !list.is_empty() {
                ranked_lists.push(list);
            }
        }

        // Episodic memory: BM25 score-based ranking
        if query.tiers.contains(&MemoryTier::Episodic) {
            let mut scored: Vec<_> = episodic_results.into_iter()
                .map(|(id, score, content)| (id, score, MemoryTier::Episodic, content))
                .collect();
            scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            let list: Vec<_> = scored.into_iter()
                .map(|(id, _, tier, content)| (id, tier, content))
                .collect();
            if !list.is_empty() {
                ranked_lists.push(list);
            }
        }

        // Semantic memory: vector similarity-based ranking
        if query.tiers.contains(&MemoryTier::Semantic) {
            let mut scored: Vec<_> = semantic_results.into_iter()
                .map(|(id, score, content)| (id, score, MemoryTier::Semantic, content))
                .collect();
            scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            let list: Vec<_> = scored.into_iter()
                .map(|(id, _, tier, content)| (id, tier, content))
                .collect();
            if !list.is_empty() {
                ranked_lists.push(list);
            }
        }

        // Procedural memory: keyword match ranking
        if query.tiers.contains(&MemoryTier::Procedural) {
            let list: Vec<_> = procedural_results.into_iter()
                .map(|(id, content)| (id, MemoryTier::Procedural, content))
                .collect();
            if !list.is_empty() {
                ranked_lists.push(list);
            }
        }

        // Fuse all ranked lists
        let fused = self.rrf.fuse(ranked_lists);

        fused.into_iter()
            .take(query.max_results)
            .map(|(id, score, tier, content)| RetrievalResult {
                entry_id: id,
                tier,
                score,
                bm25_rank: None,
                vector_rank: None,
                recency_rank: None,
                content,
            })
            .collect()
    }
}

impl Default for HybridRetriever {
    fn default() -> Self {
        Self::new()
    }
}

/// Retrieval stream for async/concurrent retrieval
pub struct RetrievalStream {
    pub query: RetrievalQuery,
    pub results: Vec<RetrievalResult>,
}

impl RetrievalStream {
    pub fn new(query: RetrievalQuery) -> Self {
        Self {
            query,
            results: Vec::new(),
        }
    }

    pub fn add_results(&mut self, results: Vec<RetrievalResult>) {
        self.results.extend(results);
        // Re-sort by score
        self.results.sort_by(|a, b| {
            b.score.partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        // Deduplicate by entry_id
        let mut seen = std::collections::HashSet::new();
        self.results.retain(|r| seen.insert(r.entry_id.0));
        // Trim to max_results
        if self.results.len() > self.query.max_results {
            self.results.truncate(self.query.max_results);
        }
    }

    pub fn to_context_string(&self) -> String {
        if self.results.is_empty() {
            return String::new();
        }

        let mut parts = vec!["## Retrieved Memory".to_string()];
        for result in &self.results {
            let tier_name = format!("{:?}", result.tier).replace("MemoryTier::", "");
            parts.push(format!(
                "[{} | score: {:.3}] {}",
                tier_name,
                result.score,
                result.content.lines().next().unwrap_or("").to_string()
            ));
        }
        parts.join("\n")
    }
}
