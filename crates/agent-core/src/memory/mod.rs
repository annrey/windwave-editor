//! Memory System - Four-layer memory architecture with hybrid retrieval
//!
//! Inspired by agentmemory's four-tier design:
//! 1. Working Memory (L3) - Short-term, immediate context
//! 2. Episodic Memory (L2) - Event history with BM25 retrieval
//! 3. Semantic Memory (L1) - Knowledge graph with vector similarity
//! 4. Procedural Memory (L0) - Workflow templates and decision patterns
//!
//! Plus: Three-stream hybrid retrieval (BM25 + Vector + Recency RRF fusion)

pub mod working;
pub mod episodic;
pub mod semantic;
pub mod procedural;
pub mod retrieval;
pub mod lifecycle;
pub mod system;

pub use working::{WorkingMemory, WorkingMemoryEntry, EntryType as WorkingEntryType};
pub use episodic::{EpisodicMemory, Episode, EpisodeType, EpisodeSearchResult};
pub use semantic::{SemanticMemory, SemanticNode, SemanticRelation, RelationType};
pub use procedural::{ProceduralMemory, WorkflowTemplate, WorkflowStep, DecisionPattern};
pub use retrieval::{
    HybridRetriever, RetrievalQuery, RetrievalResult, RetrievalStream,
    Bm25Scorer, VectorScorer, RrfFusion,
};
pub use lifecycle::{MemoryLifecycle, DecayConfig, MemoryImportance};
pub use system::{MemorySystem, MemoryConfig, MemoryQuery, MemoryContext, MemoryStats};

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
