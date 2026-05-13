//! Memory System - Unified management layer for all memory tiers
//!
//! Coordinates Working, Episodic, Semantic, and Procedural memory.
//! Provides hybrid retrieval, lifecycle management, and context building.

use crate::memory::*;
use crate::types::{Message, EntityId};
use serde::{Deserialize, Serialize};

/// Configuration for the memory system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryConfig {
    pub working_capacity: usize,
    pub max_retrieval_results: usize,
    pub context_budget_chars: usize,
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
            parts.push(format!("### Suggested Workflows\n{}", self.procedural_context));
        }

        parts.join("\n\n")
    }

    pub fn to_compact_string(&self) -> String {
        self.to_prompt_section()
    }
}

/// Unified Memory System
///
/// Manages all four memory tiers and provides:
/// - Unified storage interface
/// - Hybrid retrieval across tiers
/// - Lifecycle management (decay, cleanup)
/// - Context building for LLM prompts
/// - Token budget-aware truncation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemorySystem {
    pub working: WorkingMemory,
    pub episodic: EpisodicMemory,
    pub semantic: SemanticMemory,
    pub procedural: ProceduralMemory,
    pub config: MemoryConfig,
    #[serde(skip)]
    pub lifecycle: MemoryLifecycle,
    #[serde(skip)]
    pub retriever: HybridRetriever,
}

impl MemorySystem {
    pub fn new() -> Self {
        Self::with_config(MemoryConfig::default())
    }

    pub fn with_config(config: MemoryConfig) -> Self {
        let lifecycle = MemoryLifecycle::with_config(config.decay_config);
        Self {
            working: WorkingMemory::new(config.working_capacity),
            episodic: EpisodicMemory::new(),
            semantic: SemanticMemory::new(),
            procedural: ProceduralMemory::new(),
            config,
            lifecycle,
            retriever: HybridRetriever::new(),
        }
    }

    // =================================================================
    // Working Memory Operations
    // =================================================================

    pub fn add_message(&mut self, message: Message) {
        self.working.add_message(message);
    }

    pub fn register_entity(&mut self, name: &str, id: EntityId) {
        self.working.register_entity(name, id);
    }

    pub fn set_value(&mut self, key: &str, value: serde_json::Value) {
        self.working.set_value(key, value);
    }

    pub fn set_intent(&mut self, intent: &str) {
        self.working.set_intent(intent);
    }

    pub fn add_hint(&mut self, hint: &str) {
        self.working.add_hint(hint);
    }

    // =================================================================
    // Episodic Memory Operations
    // =================================================================

    pub fn record_episode(&mut self, episode: Episode) -> MemoryEntryId {
        self.episodic.record(episode)
    }

    pub fn record_user_request(&mut self, request: &str, context: Option<serde_json::Value>) -> MemoryEntryId {
        self.episodic.record_user_request(request, context)
    }

    pub fn record_tool_call(
        &mut self,
        tool_name: &str,
        params: serde_json::Value,
        result: Option<serde_json::Value>,
        success: bool,
    ) -> MemoryEntryId {
        self.episodic.record_tool_call(tool_name, params, result, success)
    }

    pub fn record_error(&mut self, error: &str, context: Option<serde_json::Value>) -> MemoryEntryId {
        self.episodic.record_error(error, context)
    }

    pub fn record_plan(&mut self, plan_title: &str, steps_count: usize) -> MemoryEntryId {
        self.episodic.record_plan(plan_title, steps_count)
    }

    pub fn record_step(
        &mut self,
        step_title: &str,
        result: &str,
        success: bool,
        duration_ms: u64,
    ) -> MemoryEntryId {
        self.episodic.record_step(step_title, result, success, duration_ms)
    }

    // =================================================================
    // Semantic Memory Operations
    // =================================================================

    pub fn add_semantic_node(&mut self, node: SemanticNode) -> MemoryEntryId {
        self.semantic.add_node(node)
    }

    pub fn create_semantic_node(
        &mut self,
        name: impl Into<String>,
        node_type: impl Into<String>,
        description: impl Into<String>,
    ) -> MemoryEntryId {
        self.semantic.create_node(name, node_type, description)
    }

    pub fn add_relation(
        &mut self,
        from_id: MemoryEntryId,
        to_id: MemoryEntryId,
        relation_type: RelationType,
        strength: f32,
        description: impl Into<String>,
    ) {
        self.semantic.add_relation(from_id, to_id, relation_type, strength, description);
    }

    // =================================================================
    // Procedural Memory Operations
    // =================================================================

    pub fn add_workflow(&mut self, workflow: WorkflowTemplate) -> MemoryEntryId {
        self.procedural.add_workflow(workflow)
    }

    pub fn create_workflow(
        &mut self,
        name: impl Into<String>,
        trigger: impl Into<String>,
        category: impl Into<String>,
    ) -> MemoryEntryId {
        self.procedural.create_workflow(name, trigger, category)
    }

    pub fn record_workflow_use(&mut self, name: &str, success: bool) {
        self.procedural.record_use(name, success);
    }

    pub fn observe_decision(
        &mut self,
        context: &str,
        decision: &str,
        outcome: &str,
        success: bool,
    ) -> MemoryEntryId {
        self.procedural.observe_decision(context, decision, outcome, success)
    }

    // =================================================================
    // Hybrid Retrieval
    // =================================================================

    /// Retrieve relevant memories across all tiers
    pub fn retrieve(&mut self, query: &MemoryQuery) -> Vec<RetrievalResult> {
        if !self.config.enable_hybrid_retrieval {
            return Vec::new();
        }

        let mut working_entries = Vec::new();
        let mut episodic_results = Vec::new();
        let mut semantic_results = Vec::new();
        let mut procedural_results = Vec::new();

        // Working memory: get summary entries
        if query.include_working {
            let summary = self.working.build_summary();
            if !summary.is_empty() {
                working_entries.push((
                    MemoryEntryId(0),
                    format!("Working context: {}", summary),
                ));
            }
        }

        // Episodic memory: BM25 search
        if query.include_episodic {
            let results = self.episodic.search(&query.text, query.max_results);
            for result in results {
                episodic_results.push((
                    result.episode.metadata.id,
                    result.combined_score,
                    result.episode.summary.clone(),
                ));
            }
        }

        // Semantic memory: vector search
        if query.include_semantic {
            let results = self.semantic.search(&query.text, query.max_results);
            for (node_id, score) in results {
                if let Some(node) = self.semantic.find_by_id(node_id) {
                    semantic_results.push((
                        node_id,
                        score,
                        format!("{}: {}", node.name, node.description),
                    ));
                }
            }
        }

        // Procedural memory: keyword matching
        if query.include_procedural {
            let workflows = self.procedural.find_matching(&query.text, query.max_results);
            for wf in workflows {
                procedural_results.push((
                    wf.metadata.id,
                    format!("{}: {}", wf.name, wf.trigger),
                ));
            }
        }

        // Build retrieval query
        let retrieval_query = RetrievalQuery::new(&query.text)
            .with_max_results(query.max_results)
            .with_tiers(vec![
                if query.include_working { MemoryTier::Working } else { MemoryTier::Episodic },
                if query.include_episodic { MemoryTier::Episodic } else { MemoryTier::Working },
                if query.include_semantic { MemoryTier::Semantic } else { MemoryTier::Working },
                if query.include_procedural { MemoryTier::Procedural } else { MemoryTier::Working },
            ].into_iter().collect::<std::collections::HashSet<_>>().into_iter().collect());

        self.retriever.retrieve(
            &retrieval_query,
            working_entries,
            episodic_results,
            semantic_results,
            procedural_results,
        )
    }

    /// Build memory context for LLM prompt injection
    pub fn build_context(&mut self, query: &MemoryQuery) -> MemoryContext {
        let results = self.retrieve(query);

        let mut working_parts = Vec::new();
        let mut episodic_parts = Vec::new();
        let mut semantic_parts = Vec::new();
        let mut procedural_parts = Vec::new();

        for result in results {
            match result.tier {
                MemoryTier::Working => {
                    working_parts.push(result.content);
                }
                MemoryTier::Episodic => {
                    episodic_parts.push(result.content);
                }
                MemoryTier::Semantic => {
                    semantic_parts.push(result.content);
                }
                MemoryTier::Procedural => {
                    procedural_parts.push(result.content);
                }
            }
        }

        // Also include working memory summary
        let working_summary = self.working.build_summary();
        if !working_summary.is_empty() {
            working_parts.insert(0, working_summary);
        }

        let working_context = working_parts.join("\n");
        let episodic_context = episodic_parts.join("\n");
        let semantic_context = semantic_parts.join("\n");
        let procedural_context = procedural_parts.join("\n");

        let total_chars = working_context.len()
            + episodic_context.len()
            + semantic_context.len()
            + procedural_context.len();

        MemoryContext {
            working_context,
            episodic_context,
            semantic_context,
            procedural_context,
            total_chars,
        }
    }

    /// Build context within a character budget
    pub fn build_context_with_budget(&mut self, query: &MemoryQuery, budget_chars: usize) -> MemoryContext {
        let mut context = self.build_context(query);

        if context.total_chars <= budget_chars {
            return context;
        }

        // Truncate sections proportionally, keeping working memory highest priority
        let working_budget = (budget_chars as f32 * 0.4) as usize;
        let episodic_budget = (budget_chars as f32 * 0.3) as usize;
        let semantic_budget = (budget_chars as f32 * 0.2) as usize;
        let procedural_budget = budget_chars - working_budget - episodic_budget - semantic_budget;

        context.working_context = truncate_chars(&context.working_context, working_budget);
        context.episodic_context = truncate_chars(&context.episodic_context, episodic_budget);
        context.semantic_context = truncate_chars(&context.semantic_context, semantic_budget);
        context.procedural_context = truncate_chars(&context.procedural_context, procedural_budget);

        context.total_chars = context.working_context.len()
            + context.episodic_context.len()
            + context.semantic_context.len()
            + context.procedural_context.len();

        context
    }

    // =================================================================
    // Lifecycle Management
    // =================================================================

    /// Run cleanup on all tiers
    pub fn cleanup(&mut self) {
        if !self.config.enable_decay {
            return;
        }

        // Working memory: cleanup expired entries
        self.working.cleanup_expired();

        // Episodic memory: remove low-importance entries if over capacity
        let excess = self.lifecycle.excess_count(MemoryTier::Episodic, self.episodic.len());
        if excess > 0 {
            self.episodic.clear(); // TODO: selective removal based on importance
        }

        // Semantic memory
        let excess = self.lifecycle.excess_count(MemoryTier::Semantic, self.semantic.node_count());
        if excess > 0 {
            // TODO: remove least important nodes
        }

        // Procedural memory
        let excess = self.lifecycle.excess_count(MemoryTier::Procedural, self.procedural.workflow_count());
        if excess > 0 {
            // TODO: remove least used workflows
        }
    }

    /// Clear all memory
    pub fn clear_all(&mut self) {
        self.working.clear();
        self.episodic.clear();
        // Semantic and procedural are knowledge bases, don't clear lightly
    }

    /// Get memory statistics
    pub fn stats(&self) -> MemoryStats {
        MemoryStats {
            working_entries: self.working.len(),
            working_capacity: self.config.working_capacity,
            episodic_entries: self.episodic.len(),
            semantic_nodes: self.semantic.node_count(),
            semantic_relations: self.semantic.relation_count(),
            procedural_workflows: self.procedural.workflow_count(),
            procedural_patterns: self.procedural.pattern_count(),
        }
    }

    /// Seed with default knowledge
    pub fn seed_defaults(&mut self) {
        self.semantic.seed_with_defaults();
        self.procedural.seed_with_defaults();
    }
}

impl Default for MemorySystem {
    fn default() -> Self {
        let mut system = Self::new();
        system.seed_defaults();
        system
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
        self.working_entries + self.episodic_entries + self.semantic_nodes + self.procedural_workflows
    }
}

// Helper: truncate string to max characters, preserving whole lines
fn truncate_chars(s: &str, max_chars: usize) -> String {
    if s.len() <= max_chars {
        return s.to_string();
    }

    let mut result = String::with_capacity(max_chars);
    let mut chars = 0;

    for line in s.lines() {
        let line_len = line.len() + 1; // +1 for newline
        if chars + line_len > max_chars {
            result.push_str("\n... (truncated)");
            break;
        }
        if !result.is_empty() {
            result.push('\n');
        }
        result.push_str(line);
        chars += line_len;
    }

    result
}
