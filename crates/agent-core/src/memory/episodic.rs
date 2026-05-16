//! Episodic Memory (L2) - Event history with BM25 text retrieval
//!
//! Stores agent action episodes as retrievable documents.
//! Uses a pure-Rust BM25 scorer for keyword-based relevance ranking.

use crate::memory::{MemoryEntryId, MemoryMetadata, MemoryTier};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Types of episodes that can be recorded
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum EpisodeType {
    UserRequest,
    PlanCreated,
    StepExecuted,
    ToolCalled,
    ToolResult,
    ErrorOccurred,
    UserApproved,
    UserRejected,
    StateChanged,
    Observation,
    Reflection,
    Summary,
}

/// A single episode (event) in the agent's history
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Episode {
    pub metadata: MemoryMetadata,
    pub episode_type: EpisodeType,
    /// Human-readable summary
    pub summary: String,
    /// Full details (JSON)
    pub details: serde_json::Value,
    /// Associated entities
    pub entity_ids: Vec<u64>,
    /// Success/failure
    pub success: Option<bool>,
    /// Duration in milliseconds
    pub duration_ms: Option<u64>,
}

impl Episode {
    pub fn new(
        id: u64,
        episode_type: EpisodeType,
        summary: impl Into<String>,
        details: serde_json::Value,
    ) -> Self {
        Self {
            metadata: MemoryMetadata::new(id, MemoryTier::Episodic),
            episode_type,
            summary: summary.into(),
            details,
            entity_ids: Vec::new(),
            success: None,
            duration_ms: None,
        }
    }

    pub fn with_entity(mut self, entity_id: u64) -> Self {
        self.entity_ids.push(entity_id);
        self
    }

    pub fn with_success(mut self, success: bool) -> Self {
        self.success = Some(success);
        self
    }

    pub fn with_duration(mut self, ms: u64) -> Self {
        self.duration_ms = Some(ms);
        self
    }

    /// Full text for indexing
    pub fn full_text(&self) -> String {
        format!(
            "{} {} {}",
            self.summary,
            self.details.to_string(),
            format!("{:?}", self.episode_type)
        )
    }
}

/// BM25 scoring parameters
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Bm25Params {
    pub k1: f32,
    pub b: f32,
}

impl Default for Bm25Params {
    fn default() -> Self {
        Self { k1: 1.2, b: 0.75 }
    }
}

/// Search result from episodic memory
#[derive(Debug, Clone)]
pub struct EpisodeSearchResult {
    pub episode: Episode,
    pub bm25_score: f32,
    pub recency_score: f32,
    pub combined_score: f32,
}

/// Episodic Memory - L2 event history
///
/// Design principles:
/// - Append-only log of agent actions and events
/// - BM25 text retrieval for keyword-based search
/// - Recency boosting for temporal relevance
/// - Entity-based filtering
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EpisodicMemory {
    episodes: Vec<Episode>,
    next_id: u64,
    params: Bm25Params,
    /// BM25 document frequency cache
    #[serde(skip)]
    df_cache: HashMap<String, usize>,
    #[serde(skip)]
    avg_dl: f32,
    #[serde(skip)]
    cache_valid: bool,
}

impl EpisodicMemory {
    pub fn new() -> Self {
        Self {
            episodes: Vec::new(),
            next_id: 1,
            params: Bm25Params::default(),
            df_cache: HashMap::new(),
            avg_dl: 0.0,
            cache_valid: false,
        }
    }

    /// Record a new episode
    pub fn record(&mut self, episode: Episode) -> MemoryEntryId {
        let id = episode.metadata.id;
        self.episodes.push(episode);
        self.cache_valid = false;
        id
    }

    /// Convenience: record a user request episode
    pub fn record_user_request(&mut self, request: &str, context: Option<serde_json::Value>) -> MemoryEntryId {
        let id = self.next_id();
        let details = context.unwrap_or_else(|| serde_json::json!({"request": request}));
        let episode = Episode::new(id.0, EpisodeType::UserRequest, request, details);
        self.record(episode)
    }

    /// Convenience: record a tool call episode
    pub fn record_tool_call(
        &mut self,
        tool_name: &str,
        params: serde_json::Value,
        result: Option<serde_json::Value>,
        success: bool,
    ) -> MemoryEntryId {
        let id = self.next_id();
        let summary = format!("Tool {} called", tool_name);
        let details = serde_json::json!({
            "tool": tool_name,
            "params": params,
            "result": result,
            "success": success,
        });
        let episode = Episode::new(id.0, EpisodeType::ToolCalled, summary, details)
            .with_success(success);
        self.record(episode)
    }

    /// Convenience: record an error episode
    pub fn record_error(&mut self, error: &str, context: Option<serde_json::Value>) -> MemoryEntryId {
        let id = self.next_id();
        let details = context.unwrap_or_else(|| serde_json::json!({"error": error}));
        let episode = Episode::new(id.0, EpisodeType::ErrorOccurred, error, details)
            .with_success(false);
        self.record(episode)
    }

    /// Convenience: record a plan creation episode
    pub fn record_plan(&mut self, plan_title: &str, steps_count: usize) -> MemoryEntryId {
        let id = self.next_id();
        let summary = format!("Plan created: {} ({} steps)", plan_title, steps_count);
        let details = serde_json::json!({
            "title": plan_title,
            "steps_count": steps_count,
        });
        let episode = Episode::new(id.0, EpisodeType::PlanCreated, summary, details);
        self.record(episode)
    }

    /// Convenience: record a step execution episode
    pub fn record_step(
        &mut self,
        step_title: &str,
        result: &str,
        success: bool,
        duration_ms: u64,
    ) -> MemoryEntryId {
        let id = self.next_id();
        let summary = format!("Step '{}' {}", step_title, if success { "succeeded" } else { "failed" });
        let details = serde_json::json!({
            "step": step_title,
            "result": result,
        });
        let episode = Episode::new(id.0, EpisodeType::StepExecuted, summary, details)
            .with_success(success)
            .with_duration(duration_ms);
        self.record(episode)
    }

    /// Search episodes using BM25 + recency scoring
    pub fn search(&mut self, query: &str, top_k: usize) -> Vec<EpisodeSearchResult> {
        if self.episodes.is_empty() || query.trim().is_empty() {
            return Vec::new();
        }

        self.ensure_cache();

        let query_tokens = tokenize(query);
        let now = crate::types::current_timestamp() as f32;

        let mut scored: Vec<EpisodeSearchResult> = self.episodes.iter()
            .map(|ep| {
                let bm25 = self.bm25_score(&query_tokens, ep);
                let recency = self.recency_score(ep, now);
                let combined = bm25 * 0.7 + recency * 0.3;

                EpisodeSearchResult {
                    episode: ep.clone(),
                    bm25_score: bm25,
                    recency_score: recency,
                    combined_score: combined,
                }
            })
            .filter(|r| r.combined_score > 0.0)
            .collect();

        scored.sort_by(|a, b| {
            b.combined_score.partial_cmp(&a.combined_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Mark accessed
        for result in scored.iter_mut().take(top_k) {
            if let Some(ep) = self.episodes.iter_mut()
                .find(|e| e.metadata.id == result.episode.metadata.id) {
                ep.metadata.touch();
            }
        }

        scored.into_iter().take(top_k).collect()
    }

    /// Search by episode type
    pub fn search_by_type(&self, episode_type: &EpisodeType, limit: usize) -> Vec<&Episode> {
        self.episodes.iter()
            .rev()
            .filter(|ep| &ep.episode_type == episode_type)
            .take(limit)
            .collect()
    }

    /// Search by entity association
    pub fn search_by_entity(&self, entity_id: u64, limit: usize) -> Vec<&Episode> {
        self.episodes.iter()
            .rev()
            .filter(|ep| ep.entity_ids.contains(&entity_id))
            .take(limit)
            .collect()
    }

    /// Get recent episodes
    pub fn recent(&self, n: usize) -> Vec<&Episode> {
        self.episodes.iter().rev().take(n).collect()
    }

    /// Get all episodes
    pub fn all(&self) -> &[Episode] {
        &self.episodes
    }

    /// Get episode count
    pub fn len(&self) -> usize {
        self.episodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.episodes.is_empty()
    }

    /// Clear all episodes
    pub fn clear(&mut self) {
        self.episodes.clear();
        self.df_cache.clear();
        self.cache_valid = false;
    }

    /// Iterate over episodes
    pub fn iter(&self) -> impl Iterator<Item = &Episode> {
        self.episodes.iter()
    }

    /// Remove episodes by indices (for cleanup)
    pub fn remove_by_indices(&mut self, indices: &[usize]) {
        let to_remove: std::collections::HashSet<usize> = indices.iter().copied().collect();
        let mut new_episodes = Vec::with_capacity(self.episodes.len() - to_remove.len());
        for (i, ep) in self.episodes.drain(..).enumerate() {
            if !to_remove.contains(&i) {
                new_episodes.push(ep);
            }
        }
        self.episodes = new_episodes;
        self.cache_valid = false;
    }

    /// Drain old episodes (oldest first) for compression
    pub fn drain_old_episodes(&mut self, count: usize) -> Vec<Episode> {
        let drain_count = count.min(self.episodes.len());
        let mut drained = Vec::with_capacity(drain_count);
        for _ in 0..drain_count {
            if !self.episodes.is_empty() {
                let ep = self.episodes.remove(0);
                drained.push(ep);
            }
        }
        if !drained.is_empty() {
            self.cache_valid = false;
        }
        drained
    }

    /// Record a compressed summary episode
    pub fn record_compressed(&mut self, compressed: Episode) -> MemoryEntryId {
        self.record(compressed)
    }

    /// Get next ID counter value (without incrementing)
    pub fn next_id_counter(&self) -> u64 {
        self.next_id
    }

    /// Build a summary string for LLM context
    pub fn build_summary(&mut self, query: &str, max_entries: usize) -> String {
        let results = self.search(query, max_entries);
        if results.is_empty() {
            return String::new();
        }

        let mut parts = vec!["## Past Episodes".to_string()];
        for result in results {
            let ep = &result.episode;
            let status = match ep.success {
                Some(true) => "✓",
                Some(false) => "✗",
                None => "○",
            };
            parts.push(format!(
                "- [{}] {}: {}",
                status,
                format!("{:?}", ep.episode_type).replace("EpisodeType::", ""),
                ep.summary
            ));
        }

        parts.join("\n")
    }

    // ------------------------------------------------------------------
    // BM25 scoring
    // ------------------------------------------------------------------

    fn ensure_cache(&mut self) {
        if self.cache_valid {
            return;
        }

        self.df_cache.clear();
        let mut total_dl = 0usize;

        for ep in &self.episodes {
            let text = ep.full_text();
            let tokens = tokenize(&text);
            total_dl += tokens.len();

            let mut seen = std::collections::HashSet::new();
            for token in tokens {
                if seen.insert(token.clone()) {
                    *self.df_cache.entry(token).or_insert(0) += 1;
                }
            }
        }

        self.avg_dl = if self.episodes.is_empty() {
            1.0
        } else {
            total_dl as f32 / self.episodes.len() as f32
        };

        self.cache_valid = true;
    }

    fn bm25_score(&self, query_tokens: &[String], episode: &Episode) -> f32 {
        let text = episode.full_text();
        let doc_tokens = tokenize(&text);
        let doc_len = doc_tokens.len() as f32;
        let avg_dl = self.avg_dl.max(1.0);

        let mut score = 0.0;
        for token in query_tokens {
            let tf = doc_tokens.iter().filter(|t| **t == *token).count() as f32;
            let df = *self.df_cache.get(token).unwrap_or(&0) as f32;
            let n = self.episodes.len() as f32;

            let idf = ((n - df + 0.5) / (df + 0.5) + 1.0).ln();
            let tf_norm = tf * (self.params.k1 + 1.0)
                / (tf + self.params.k1 * (1.0 - self.params.b + self.params.b * doc_len / avg_dl));

            score += idf.max(0.0) * tf_norm;
        }

        score
    }

    fn recency_score(&self, episode: &Episode, now: f32) -> f32 {
        let age = now - episode.metadata.created_at as f32;
        let half_life = 86400.0; // 1 day half-life
        (-age / half_life).exp()
    }

    fn next_id(&mut self) -> MemoryEntryId {
        let id = MemoryEntryId(self.next_id);
        self.next_id += 1;
        id
    }

    // =================================================================
    // Persistence Operations
    // =================================================================

    /// Get all episodes for serialization
    pub fn get_all_episodes(&self) -> Vec<Episode> {
        self.episodes.clone()
    }

    /// Restore an episode from serialized data
    pub fn restore_episode(&mut self, episode: Episode) {
        if self.next_id <= episode.metadata.id.0 {
            self.next_id = episode.metadata.id.0 + 1;
        }
        self.episodes.push(episode);
        self.cache_valid = false;
    }
}

impl Default for EpisodicMemory {
    fn default() -> Self {
        Self::new()
    }
}

// ------------------------------------------------------------------
// Helpers
// ------------------------------------------------------------------

fn tokenize(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|s| s.len() > 1)
        .map(|s| s.to_string())
        .collect()
}
