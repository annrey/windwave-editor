//! Memory System - Four-layer memory architecture with hybrid retrieval
//!
//! Inspired by agentmemory's four-tier design:
//! 1. Working Memory (L3) - Short-term, immediate context
//! 2. Episodic Memory (L2) - Event history with BM25 retrieval
//! 3. Semantic Memory (L1) - Knowledge graph with vector similarity
//! 4. Procedural Memory (L0) - Workflow templates and decision patterns
//!
//! Plus: Three-stream hybrid retrieval (BM25 + Vector + Recency RRF fusion)

pub mod compression;
pub mod episodic;
pub mod lifecycle;
pub mod memory_injector;
pub mod persistence;
pub mod preferences;
pub mod procedural;
pub mod registry;
pub mod retrieval;
pub mod scene_context;
pub mod semantic;
pub mod system;
pub mod types;
pub mod working;

pub use episodic::{Episode, EpisodeSearchResult, EpisodeType, EpisodicMemory};
pub use lifecycle::{DecayConfig, MemoryImportance, MemoryLifecycle};
pub use procedural::{DecisionPattern, ProceduralMemory, WorkflowStep, WorkflowTemplate};
pub use retrieval::{
    Bm25Scorer, HybridRetriever, RetrievalQuery, RetrievalResult, RetrievalStream, RrfFusion,
    VectorScorer,
};
pub use semantic::{RelationType, SemanticMemory, SemanticNode, SemanticRelation};
pub use system::MemorySystem;
pub use types::{
    CompressionSummary, MemoryConfig, MemoryContext, MemoryLoadResult, MemoryPersistenceInfo,
    MemoryQuery, MemoryStats,
};
pub use working::{EntryType as WorkingEntryType, WorkingMemory, WorkingMemoryEntry};

/// Shared tokenization utility used by episodic and semantic memory.
/// Lowercases, splits on non-alphanumeric boundaries, filters short tokens.
pub(crate) fn tokenize(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|s| s.len() > 1)
        .map(|s| s.to_string())
        .collect()
}
pub use preferences::{
    InteractionOutcome, PreferenceCategory, PreferenceInteraction, UserPreference, UserPreferences,
};
pub use registry::{AgentMemoryEntry, AgentMemoryId, MemorySystemRegistry};

use serde::{Deserialize, Serialize};

/// Memory tier classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MemoryTier {
    Working,
    Episodic,
    Semantic,
    Procedural,
}

/// Unified memory entry ID
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MemoryEntryId(pub u64);

/// Common metadata for all memory entries
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryMetadata {
    pub id: MemoryEntryId,
    pub tier: MemoryTier,
    pub created_at: u64,
    pub last_accessed: u64,
    pub access_count: u32,
    pub importance: f32, // 0.0 - 1.0
    pub tags: Vec<String>,
}

impl MemoryMetadata {
    pub fn new(id: u64, tier: MemoryTier) -> Self {
        let now = crate::types::current_timestamp();
        Self {
            id: MemoryEntryId(id),
            tier,
            created_at: now,
            last_accessed: now,
            access_count: 0,
            importance: 0.5,
            tags: Vec::new(),
        }
    }

    pub fn touch(&mut self) {
        self.last_accessed = crate::types::current_timestamp();
        self.access_count += 1;
    }
}
