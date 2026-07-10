//! Memory system types — Configuration, query, context, and result types

use crate::memory::DecayConfig;
use serde::{Deserialize, Serialize};

/// Configuration for the memory system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryConfig {
    pub working_capacity: usize,
    pub max_retrieval_results: usize,
    pub context_budget_chars: usize,
    pub compress_threshold: usize,
    pub enable_decay: bool,
    pub enable_hybrid_retrieval: bool,
    pub decay_config: DecayConfig,
}

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            working_capacity: 50,
            max_retrieval_results: 15,
            context_budget_chars: 4000,
            compress_threshold: 25,
            enable_decay: true,
            enable_hybrid_retrieval: true,
            decay_config: DecayConfig::default(),
        }
    }
}

/// Query interface for the memory system
#[derive(Debug, Clone)]
pub struct MemoryQuery {
    pub text: String,
    pub max_results: usize,
    pub include_working: bool,
    pub include_episodic: bool,
    pub include_semantic: bool,
    pub include_procedural: bool,
}

impl MemoryQuery {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            max_results: 10,
            include_working: true,
            include_episodic: true,
            include_semantic: true,
            include_procedural: true,
        }
    }

    pub fn working_only(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            max_results: 10,
            include_working: true,
            include_episodic: false,
            include_semantic: false,
            include_procedural: false,
        }
    }

    pub fn episodic_only(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            max_results: 10,
            include_working: false,
            include_episodic: true,
            include_semantic: false,
            include_procedural: false,
        }
    }
}

/// Context built from memory for LLM prompt injection
#[derive(Debug, Clone)]
pub struct MemoryContext {
    pub working_context: String,
    pub episodic_context: String,
    pub semantic_context: String,
    pub procedural_context: String,
    pub total_chars: usize,
}

impl MemoryContext {
    pub fn is_empty(&self) -> bool {
        self.working_context.is_empty()
            && self.episodic_context.is_empty()
            && self.semantic_context.is_empty()
            && self.procedural_context.is_empty()
    }

    pub fn to_prompt_section(&self) -> String {
        let mut parts = Vec::new();

        if !self.working_context.is_empty() {
            parts.push(format!("### Working Context\n{}", self.working_context));
        }
        if !self.episodic_context.is_empty() {
            parts.push(format!("### Past Events\n{}", self.episodic_context));
        }
        if !self.semantic_context.is_empty() {
            parts.push(format!("### Related Knowledge\n{}", self.semantic_context));
        }
        if !self.procedural_context.is_empty() {
            parts.push(format!(
                "### Suggested Workflows\n{}",
                self.procedural_context
            ));
        }

        parts.join("\n\n")
    }

    pub fn to_compact_string(&self) -> String {
        self.to_prompt_section()
    }
}

/// Memory statistics
#[derive(Debug, Clone)]
pub struct MemoryStats {
    pub working_entries: usize,
    pub working_capacity: usize,
    pub episodic_entries: usize,
    pub semantic_nodes: usize,
    pub semantic_relations: usize,
    pub procedural_workflows: usize,
    pub procedural_patterns: usize,
}

impl MemoryStats {
    pub fn total_entries(&self) -> usize {
        self.working_entries
            + self.episodic_entries
            + self.semantic_nodes
            + self.procedural_workflows
    }
}

#[derive(Debug, Clone)]
pub struct MemoryPersistenceInfo {
    pub file_path: String,
    pub total_bytes: usize,
    pub working_count: usize,
    pub episodic_count: usize,
    pub semantic_count: usize,
    pub procedural_count: usize,
}

#[derive(Debug, Clone)]
pub struct MemoryLoadResult {
    pub file_path: String,
    pub working_restored: usize,
    pub episodic_restored: usize,
    pub semantic_restored: usize,
    pub procedural_restored: usize,
    pub saved_at: String,
}

/// Summary of a compression run
#[derive(Debug, Clone)]
pub struct CompressionSummary {
    pub working_compressed: usize,
    pub episodic_compressed: usize,
    pub summary_snippet: String,
    pub conversation_turns_before: usize,
    pub conversation_turns_after: usize,
}
