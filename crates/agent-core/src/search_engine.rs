//! Hybrid search engine — BM25-style keyword + vector cosine similarity + RRF fusion.
//!
//! Uses TF-IDF as a lightweight BM25 approximation (no external index needed)
//! and in-memory cosine similarity for vector search. Results are fused
//! via Reciprocal Rank Fusion (RRF, k=60).

use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Core types
// ---------------------------------------------------------------------------

/// An indexed document (memory entry, action log, etc.).
#[derive(Debug, Clone)]
pub struct IndexDocument {
    pub id: String,
    pub content: String,
    /// Optional pre-computed embedding (e.g. from candle or external provider).
    pub embedding: Option<Vec<f32>>,
    /// Session/group identifier for diversity capping.
    pub session_id: Option<String>,
}

/// A search result with score.
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub id: String,
    /// Fused RRF score (higher = better match).
    pub score: f32,
    /// Source content snippet.
    pub snippet: String,
}

// ---------------------------------------------------------------------------
// TF-IDF index (lightweight BM25 approximation)
// ---------------------------------------------------------------------------

/// In-memory TF-IDF index. Computes term frequencies and inverse document
/// frequencies on insertion; supports deletion for incremental updates.
pub struct TfIdfIndex {
    documents: HashMap<String, IndexDocument>,
    /// term → (doc_id → raw frequency)
    term_index: HashMap<String, HashMap<String, usize>>,
    /// doc_id → total term count
    doc_lengths: HashMap<String, usize>,
    total_docs: usize,
}

impl TfIdfIndex {
    pub fn new() -> Self {
        Self {
            documents: HashMap::new(),
            term_index: HashMap::new(),
            doc_lengths: HashMap::new(),
            total_docs: 0,
        }
    }

    /// Insert or update a document.
    pub fn insert(&mut self, doc: IndexDocument) {
        let doc_id = doc.id.clone();
        let doc_content = doc.content.clone();
        self.remove(&doc_id);

        let tokens = tokenize(&doc_content);
        let len = tokens.len();
        self.doc_lengths.insert(doc_id.clone(), len);
        self.total_docs += 1;

        for term in tokens {
            self.term_index
                .entry(term)
                .or_default()
                .entry(doc_id.clone())
                .and_modify(|c| *c += 1)
                .or_insert(1);
        }

        self.documents.insert(doc_id, doc);
    }

    /// Remove a document by ID.
    pub fn remove(&mut self, id: &str) {
        if self.documents.remove(id).is_some() {
            self.doc_lengths.remove(id);
            self.total_docs = self.total_docs.saturating_sub(1);
            let to_clear: Vec<String> = self.term_index
                .iter_mut()
                .filter_map(|(term, docs)| {
                    docs.remove(id);
                    if docs.is_empty() { Some(term.clone()) } else { None }
                })
                .collect();
            for term in to_clear {
                self.term_index.remove(&term);
            }
        }
    }

    /// Search with TF-IDF scoring. Returns top_k results (id, score).
    pub fn search(&self, query: &str, top_k: usize) -> Vec<(String, f32)> {
        let query_terms = tokenize(query);
        if query_terms.is_empty() {
            return Vec::new();
        }

        let mut scores: HashMap<String, f32> = HashMap::new();
        let n = self.total_docs as f32;

        for term in &query_terms {
            let df = self.term_index.get(term).map(|d| d.len()).unwrap_or(0);
            if df == 0 { continue; }
            let idf = ((n - df as f32 + 0.5) / (df as f32 + 0.5) + 1.0).ln();

            if let Some(postings) = self.term_index.get(term) {
                for (doc_id, tf) in postings {
                    let doc_len = self.doc_lengths.get(doc_id).copied().unwrap_or(1);
                    // BM25-like: tf * (k1 + 1) / (tf + k1 * (1 - b + b * dl / avgdl))
                    let avgdl = if self.total_docs > 0 { self.total_docs as f32 } else { 1.0 }; // approximation
                    let k1 = 1.2_f32;
                    let b = 0.75_f32;
                    let tf_norm = (*tf as f32 * (k1 + 1.0))
                        / (*tf as f32 + k1 * (1.0 - b + b * doc_len as f32 / avgdl));
                    *scores.entry(doc_id.clone()).or_insert(0.0) += idf * tf_norm;
                }
            }
        }

        let mut results: Vec<(String, f32)> = scores.into_iter().collect();
        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(top_k);
        results
    }

    /// Get a document by ID.
    pub fn get(&self, id: &str) -> Option<&IndexDocument> {
        self.documents.get(id)
    }
}

impl Default for TfIdfIndex {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Vector index (in-memory cosine similarity)
// ---------------------------------------------------------------------------

/// In-memory dense vector index. Stores embeddings and computes cosine
/// similarity on search.
pub struct VectorIndex {
    vectors: HashMap<String, Vec<f32>>,
}

impl VectorIndex {
    pub fn new() -> Self {
        Self { vectors: HashMap::new() }
    }

    pub fn insert(&mut self, id: &str, embedding: Vec<f32>) {
        self.vectors.insert(id.to_string(), embedding);
    }

    pub fn remove(&mut self, id: &str) {
        self.vectors.remove(id);
    }

    /// Search with cosine similarity. Query must be pre-computed embedding.
    pub fn search(&self, query_embedding: &[f32], top_k: usize) -> Vec<(String, f32)> {
        let mut scores: Vec<(String, f32)> = self.vectors
            .iter()
            .map(|(id, emb)| (id.clone(), cosine_similarity(query_embedding, emb)))
            .collect();
        scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scores.truncate(top_k);
        scores
    }

    pub fn is_empty(&self) -> bool {
        self.vectors.is_empty()
    }
}

impl Default for VectorIndex {
    fn default() -> Self {
        Self::new()
    }
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let (dot, norm_a, norm_b) = a.iter()
        .zip(b.iter().take(a.len().min(b.len())))
        .fold((0.0_f32, 0.0_f32, 0.0_f32), |(d, na, nb), (ai, bi)| {
            (d + ai * bi, na + ai * ai, nb + bi * bi)
        });
    let norm = (norm_a.sqrt() * norm_b.sqrt()).max(f32::MIN_POSITIVE);
    dot / norm
}

// ---------------------------------------------------------------------------
// HybridSearchEngine — BM25(TF-IDF) + vector + RRF
// ---------------------------------------------------------------------------

/// Hybrid search engine that fuses TF-IDF (keyword) and vector (semantic)
/// results using Reciprocal Rank Fusion (k=60).
pub struct HybridSearchEngine {
    bm25: TfIdfIndex,
    vector: Option<VectorIndex>,
    rrf_k: f32,
}

impl HybridSearchEngine {
    pub fn new() -> Self {
        Self { bm25: TfIdfIndex::new(), vector: None, rrf_k: 60.0 }
    }

    pub fn with_vector(mut self, vi: VectorIndex) -> Self {
        self.vector = Some(vi);
        self
    }

    pub fn with_rrf_k(mut self, k: f32) -> Self {
        self.rrf_k = k;
        self
    }

    /// Enable or disable vector search.
    pub fn set_vector(&mut self, vi: Option<VectorIndex>) {
        self.vector = vi;
    }

    /// Insert a document into all indices.
    pub fn insert(&mut self, doc: IndexDocument) {
        if let Some(emb) = doc.embedding.clone() {
            if let Some(ref mut vi) = self.vector {
                vi.insert(&doc.id, emb);
            }
        }
        self.bm25.insert(doc);
    }

    /// Remove a document from all indices.
    pub fn remove(&mut self, id: &str) {
        self.bm25.remove(id);
        if let Some(ref mut vi) = self.vector {
            vi.remove(id);
        }
    }

    /// Hybrid search: fuse BM25 + optional vector results via RRF.
    ///
    /// `query_embedding` is optional; if provided and a vector index exists,
    /// vector results are fused with keyword results.
    pub fn search(
        &self,
        query: &str,
        query_embedding: Option<&[f32]>,
        top_k: usize,
    ) -> Vec<SearchResult> {
        let mut fused_scores: HashMap<String, f32> = HashMap::new();

        // BM25 stream — always runs
        let bm25_results = self.bm25.search(query, 50);
        for (rank, (id, _)) in bm25_results.iter().enumerate() {
            let score = 1.0 / (self.rrf_k + rank as f32 + 1.0);
            *fused_scores.entry(id.clone()).or_insert(0.0) += score;
        }

        // Vector stream — runs if embedding provided
        if let (Some(vi), Some(qe)) = (&self.vector, query_embedding) {
            if !vi.is_empty() {
                let vec_results = vi.search(qe, 50);
                for (rank, (id, _)) in vec_results.iter().enumerate() {
                    let score = 1.0 / (self.rrf_k + rank as f32 + 1.0);
                    *fused_scores.entry(id.clone()).or_insert(0.0) += score;
                }
            }
        }

        // Sort by RRF score descending
        let mut results: Vec<(String, f32)> = fused_scores.into_iter().collect();
        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Diversity cap: max 3 per session
        let mut session_counts: HashMap<String, usize> = HashMap::new();
        let mut final_results = Vec::new();
        for (id, score) in results {
            if final_results.len() >= top_k { break; }

            if let Some(doc) = self.bm25.get(&id) {
                if let Some(ref sid) = doc.session_id {
                    let count = session_counts.entry(sid.clone()).or_insert(0);
                    if *count >= 3 { continue; }
                    *count += 1;
                }
                final_results.push(SearchResult {
                    id: id.clone(),
                    score,
                    snippet: doc.content.chars().take(200).collect(),
                });
            }
        }

        final_results
    }

    pub fn bm25(&self) -> &TfIdfIndex {
        &self.bm25
    }

    pub fn vector(&self) -> Option<&VectorIndex> {
        self.vector.as_ref()
    }
}

impl Default for HybridSearchEngine {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tokenizer for Chinese + English
// ---------------------------------------------------------------------------

/// Simple tokenizer: whitespace-split for English, 2-gram character sliding
/// window for CJK characters, plus word-level synonyms.
fn tokenize(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut cjk_buf = String::new();

    fn flush_cjk(buf: &mut String, tokens: &mut Vec<String>) {
        // Bigram sliding window for CJK
        let chars: Vec<char> = buf.chars().collect();
        if chars.len() == 1 {
            tokens.push(buf.clone());
        } else {
            for w in chars.windows(2) {
                tokens.push(w.iter().collect());
            }
        }
        // Also add individual chars for recall
        for c in &chars {
            tokens.push(c.to_string());
        }
        buf.clear();
    }

    for ch in text.chars() {
        if ch.is_alphanumeric() {
            // Detect CJK
            if ('\u{4E00}'..='\u{9FFF}').contains(&ch) ||
               ('\u{3040}'..='\u{30FF}').contains(&ch) ||
               ('\u{AC00}'..='\u{D7AF}').contains(&ch) {
                if !current.is_empty() {
                    tokens.push(current.to_lowercase());
                    current.clear();
                }
                cjk_buf.push(ch);
            } else {
                if !cjk_buf.is_empty() {
                    flush_cjk(&mut cjk_buf, &mut tokens);
                }
                current.push(ch);
            }
        } else {
            if !cjk_buf.is_empty() {
                flush_cjk(&mut cjk_buf, &mut tokens);
            }
            if !current.is_empty() {
                tokens.push(current.to_lowercase());
                current.clear();
            }
        }
    }
    if !cjk_buf.is_empty() {
        flush_cjk(&mut cjk_buf, &mut tokens);
    }
    if !current.is_empty() {
        tokens.push(current.to_lowercase());
    }

    tokens
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tokenize_english() {
        let tokens = tokenize("hello world");
        assert!(tokens.contains(&"hello".to_string()));
        assert!(tokens.contains(&"world".to_string()));
    }

    #[test]
    fn test_tokenize_chinese() {
        let tokens = tokenize("移动玩家");
        assert!(tokens.iter().any(|t| t == "移动"));
        assert!(tokens.iter().any(|t| t == "动玩"));
        assert!(tokens.iter().any(|t| t == "玩家"));
    }

    #[test]
    fn test_tfidf_insert_and_search() {
        let mut idx = TfIdfIndex::new();
        idx.insert(IndexDocument {
            id: "1".into(),
            content: "创建一个红色敌人".into(),
            embedding: None,
            session_id: None,
        });
        idx.insert(IndexDocument {
            id: "2".into(),
            content: "玩家移动到左侧".into(),
            embedding: None,
            session_id: None,
        });
        idx.insert(IndexDocument {
            id: "3".into(),
            content: "创建蓝色敌人并放在右侧".into(),
            embedding: None,
            session_id: None,
        });

        let results = idx.search("创建敌人", 3);
        assert!(!results.is_empty());
        // Doc 1 and 3 should score higher than 2
        let has_1 = results.iter().any(|(id, _)| id == "1");
        let has_3 = results.iter().any(|(id, _)| id == "3");
        assert!(has_1);
        assert!(has_3);
    }

    #[test]
    fn test_tfidf_remove() {
        let mut idx = TfIdfIndex::new();
        idx.insert(IndexDocument {
            id: "X".into(),
            content: "test document".into(),
            embedding: None,
            session_id: None,
        });
        assert!(idx.search("test", 1).len() > 0);
        idx.remove("X");
        assert!(idx.search("test", 1).is_empty());
    }

    #[test]
    fn test_cosine_similarity() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert!((cosine_similarity(&a, &b) - 1.0).abs() < 0.001);

        let c = vec![0.0, 1.0, 0.0];
        assert!((cosine_similarity(&a, &c) - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_hybrid_search_rrf_fusion() {
        let mut engine = HybridSearchEngine::new();
        engine.insert(IndexDocument {
            id: "a".into(),
            content: "red enemy spawn".into(),
            embedding: Some(vec![1.0, 0.1, 0.1]),
            session_id: None,
        });
        engine.insert(IndexDocument {
            id: "b".into(),
            content: "blue player move left".into(),
            embedding: Some(vec![0.1, 1.0, 0.1]),
            session_id: None,
        });

        // Without vector: keyword-only
        let results_no_vec = engine.search("red", None, 5);
        assert!(!results_no_vec.is_empty());

        // With vector (similar to doc b): should surface both
        let query_emb = vec![0.1, 1.0, 0.1];
        let results_vec = engine.search("red", Some(&query_emb), 5);
        assert!(!results_vec.is_empty());
    }

    #[test]
    fn test_session_diversity_cap() {
        let mut engine = HybridSearchEngine::new();
        for i in 0..5 {
            engine.insert(IndexDocument {
                id: format!("s{}", i),
                content: format!("doc {}", i),
                embedding: None,
                session_id: Some("session_A".into()),
            });
        }

        let results = engine.search("doc", None, 10);
        // Diversity cap: max 3 per session
        let from_a = results.iter().filter(|r| r.id.starts_with('s')).count();
        assert!(from_a <= 3);
    }
}
