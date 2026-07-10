//! Memory System - Unified management layer for all memory tiers
//!
//! Coordinates Working, Episodic, Semantic, and Procedural memory.
//! Provides hybrid retrieval, lifecycle management, and context building.

use crate::memory::*;
use crate::types::{EntityId, Message};
use serde::{Deserialize, Serialize};

/// Unified Memory System
///
/// Manages all four memory tiers and provides:
/// - Unified storage interface
/// - Hybrid retrieval across tiers
/// - Lifecycle management (decay, cleanup)
/// - Context building for LLM prompts
/// - Token budget-aware truncation
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
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

    pub fn record_user_request(
        &mut self,
        request: &str,
        context: Option<serde_json::Value>,
    ) -> MemoryEntryId {
        self.episodic.record_user_request(request, context)
    }

    pub fn record_tool_call(
        &mut self,
        tool_name: &str,
        params: serde_json::Value,
        result: Option<serde_json::Value>,
        success: bool,
    ) -> MemoryEntryId {
        self.episodic
            .record_tool_call(tool_name, params, result, success)
    }

    pub fn record_error(
        &mut self,
        error: &str,
        context: Option<serde_json::Value>,
    ) -> MemoryEntryId {
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
        self.episodic
            .record_step(step_title, result, success, duration_ms)
    }

    pub fn record_user_preference(
        &mut self,
        key: impl Into<String>,
        value: impl Into<String>,
        source: impl Into<String>,
    ) -> MemoryEntryId {
        self.episodic.record_user_preference(key, value, source)
    }

    pub fn get_preference(&self, key: &str) -> Option<&str> {
        self.episodic.get_preference(key)
    }

    pub fn build_preference_summary(&self) -> String {
        self.episodic.build_preference_summary()
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
        self.semantic
            .add_relation(from_id, to_id, relation_type, strength, description);
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
        self.procedural
            .observe_decision(context, decision, outcome, success)
    }

    /// Auto-learn: promote high-confidence decision patterns to workflows.
    pub fn auto_learn(&mut self, min_confidence: f32, min_observations: u32) -> usize {
        self.procedural
            .auto_learn_workflows(min_confidence, min_observations)
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
                working_entries.push((MemoryEntryId(0), format!("Working context: {}", summary)));
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
            let workflows = self
                .procedural
                .find_matching(&query.text, query.max_results);
            for wf in workflows {
                procedural_results.push((wf.metadata.id, format!("{}: {}", wf.name, wf.trigger)));
            }
        }

        // Build retrieval query tiers based on enabled flags
        let mut tiers = Vec::with_capacity(4);
        if query.include_working {
            tiers.push(MemoryTier::Working);
        }
        if query.include_episodic {
            tiers.push(MemoryTier::Episodic);
        }
        if query.include_semantic {
            tiers.push(MemoryTier::Semantic);
        }
        if query.include_procedural {
            tiers.push(MemoryTier::Procedural);
        }

        let retrieval_query = RetrievalQuery::new(&query.text)
            .with_max_results(query.max_results)
            .with_tiers(tiers);

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

        // Include user preferences in episodic context
        let preference_summary = self.episodic.build_preference_summary();
        if !preference_summary.is_empty() {
            episodic_parts.insert(0, preference_summary);
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
    pub fn build_context_with_budget(
        &mut self,
        query: &MemoryQuery,
        budget_chars: usize,
    ) -> MemoryContext {
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

    /// Build a layered context with explicit L0-L3 tier labels.
    ///
    /// Formats the context with tier headers, priority ordering, and
    /// compact summaries. Designed for LLM prompt injection.
    ///
    /// Layer order: L3 (highest priority, most recent) → L0 (foundational).
    pub fn build_layered_context(&mut self, query: &MemoryQuery) -> String {
        let ctx = self.build_context(query);

        let mut parts: Vec<String> = Vec::new();

        parts.push("[Memory Context]".to_string());

        if !ctx.working_context.is_empty() {
            parts.push(format!(
                "[L3 - Working Memory (immediate)]\n{}",
                ctx.working_context
            ));
        }

        if !ctx.episodic_context.is_empty() {
            parts.push(format!(
                "[L2 - Episodic Memory (recent history)]\n{}",
                ctx.episodic_context
            ));
        }

        if !ctx.semantic_context.is_empty() {
            parts.push(format!(
                "[L1 - Semantic Memory (knowledge)]\n{}",
                ctx.semantic_context
            ));
        }

        if !ctx.procedural_context.is_empty() {
            parts.push(format!(
                "[L0 - Procedural Memory (patterns)]\n{}",
                ctx.procedural_context
            ));
        }

        if parts.len() == 1 {
            "No relevant memory context found.".to_string()
        } else {
            parts.join("\n\n")
        }
    }

    /// Build a layered context within a character budget.
    ///
    /// Truncates the full layered context to fit within the budget,
    /// preserving L3 (working) context first, then L2, L1, L0.
    pub fn build_layered_context_with_budget(
        &mut self,
        query: &MemoryQuery,
        budget_chars: usize,
    ) -> String {
        let ctx = self.build_context_with_budget(query, budget_chars);

        let mut parts: Vec<String> = Vec::new();

        parts.push("[Memory Context]".to_string());

        if !ctx.working_context.is_empty() {
            parts.push(format!(
                "[L3 - Working Memory (immediate)]\n{}",
                ctx.working_context
            ));
        }

        if !ctx.episodic_context.is_empty() {
            parts.push(format!(
                "[L2 - Episodic Memory (recent history)]\n{}",
                ctx.episodic_context
            ));
        }

        if !ctx.semantic_context.is_empty() {
            parts.push(format!(
                "[L1 - Semantic Memory (knowledge)]\n{}",
                ctx.semantic_context
            ));
        }

        if !ctx.procedural_context.is_empty() {
            parts.push(format!(
                "[L0 - Procedural Memory (patterns)]\n{}",
                ctx.procedural_context
            ));
        }

        if parts.len() == 1 {
            "No relevant memory context found.".to_string()
        } else {
            parts.join("\n\n")
        }
    }

    // =================================================================
    // Lifecycle Management
    // =================================================================

    /// Run cleanup on all tiers with importance-based selection
    pub fn cleanup(&mut self) {
        if !self.config.enable_decay {
            return;
        }

        // Working memory: cleanup expired entries
        self.working.cleanup_expired();

        // Episodic memory: remove low-importance entries if over capacity
        let excess = self
            .lifecycle
            .excess_count(MemoryTier::Episodic, self.episodic.len());
        if excess > 0 {
            self.cleanup_episodic_by_importance(excess);
        }

        // Semantic memory: remove least important nodes
        let excess = self
            .lifecycle
            .excess_count(MemoryTier::Semantic, self.semantic.node_count());
        if excess > 0 {
            self.cleanup_semantic_by_importance(excess);
        }

        // Procedural memory: remove least used workflows
        let excess = self
            .lifecycle
            .excess_count(MemoryTier::Procedural, self.procedural.workflow_count());
        if excess > 0 {
            self.cleanup_procedural_by_usage(excess);
        }
    }

    /// Remove lowest-importance episodic entries
    fn cleanup_episodic_by_importance(&mut self, count: usize) {
        // Score each episode by: recency * importance * access_frequency
        let mut scored: Vec<(usize, f32)> = self
            .episodic
            .iter()
            .enumerate()
            .map(|(i, ep)| {
                let age_factor = 1.0
                    / (1.0
                        + (crate::types::current_timestamp() - ep.metadata.created_at) as f32
                            / 3600.0); // Decay over hours
                let importance = ep.metadata.importance;
                let access_factor = 1.0 + (ep.metadata.access_count as f32 * 0.1);

                (i, age_factor * importance * access_factor)
            })
            .collect();

        // Sort by score ascending (lowest first)
        scored.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

        // Remove lowest-scoring entries
        let to_remove: Vec<usize> = scored.into_iter().take(count).map(|(i, _)| i).collect();
        self.episodic.remove_by_indices(&to_remove);

        log::debug!(
            "Cleaned up {} episodic entries (removed oldest/least important)",
            to_remove.len()
        );
    }

    /// Remove least important semantic nodes
    fn cleanup_semantic_by_importance(&mut self, count: usize) {
        // Use existing semantic memory cleanup if available, otherwise skip
        // Semantic nodes are knowledge, be conservative
        if self.semantic.node_count() > 1000 {
            // Only clean if very large
            self.semantic.prune_least_important(count);
            log::debug!("Pruned {} semantic nodes", count);
        }
    }

    /// Remove least-used procedural workflows
    fn cleanup_procedural_by_usage(&mut self, count: usize) {
        // Sort by usage frequency and success rate
        let removed = self.procedural.remove_least_used(count);
        log::debug!(
            "Removed {} procedural workflows (low usage/success rate)",
            removed
        );
    }

    /// Clear all memory
    pub fn clear_all(&mut self) {
        self.working.clear();
        self.episodic.clear();
        // Semantic and procedural are knowledge bases, don't clear lightly
    }

    // =================================================================
    // Persistence Operations
    // =================================================================

    /// Compress long conversation history from working memory into episodic memory.
    ///
    /// When conversation turns exceed the configured threshold, older turns
    /// are summarized and moved to episodic memory, keeping only the most
    /// recent turns in working memory for LLM context.
    pub fn compress(&mut self) -> CompressionSummary {
        let conv_indices = self.working.conversation_indices();
        let before = conv_indices.len();

        if before <= self.config.compress_threshold {
            return CompressionSummary {
                working_compressed: 0,
                episodic_compressed: 0,
                summary_snippet: String::new(),
                conversation_turns_before: before,
                conversation_turns_after: before,
            };
        }

        let keep_count = self.config.compress_threshold / 2;
        let compress_count = before - keep_count;

        let mut compressed: Vec<WorkingMemoryEntry> = Vec::with_capacity(compress_count);
        let mut ids_to_remove: Vec<u64> = Vec::with_capacity(compress_count);

        for (i, &idx) in conv_indices.iter().enumerate() {
            if i < compress_count {
                if let Some(entry) = self.working.get_entry_by_index(idx) {
                    compressed.push(entry.clone());
                    ids_to_remove.push(entry.metadata.id.0);
                }
            }
        }

        let mut summary_parts: Vec<String> = Vec::new();
        for entry in &compressed {
            let content = &entry.content;
            if content.len() > 200 {
                summary_parts.push(format!(
                    "{}...",
                    content.chars().take(200).collect::<String>()
                ));
            } else {
                summary_parts.push(content.clone());
            }
        }

        let summary = summary_parts.join("\n");
        let summary_snippet = if summary.len() > 500 {
            format!("{}...", summary.chars().take(500).collect::<String>())
        } else {
            summary.clone()
        };

        let new_id = crate::types::current_timestamp();

        let episode = Episode::new(
            new_id,
            EpisodeType::Summary,
            format!("Compressed {} conversation turns", compress_count),
            serde_json::json!({
                "type": "conversation_compression",
                "compressed_turns": compress_count,
                "summary": summary_snippet,
            }),
        );
        self.episodic.record(episode);
        let removed = self.working.remove_by_ids(&ids_to_remove);

        CompressionSummary {
            working_compressed: removed,
            episodic_compressed: 1,
            summary_snippet,
            conversation_turns_before: before,
            conversation_turns_after: before - removed,
        }
    }

    /// Check if compression is needed and compress if so.
    pub fn compress_if_needed(&mut self) -> CompressionSummary {
        let conv_count = self.working.conversation_indices().len();
        if conv_count > self.config.compress_threshold {
            self.compress()
        } else {
            CompressionSummary {
                working_compressed: 0,
                episodic_compressed: 0,
                summary_snippet: String::new(),
                conversation_turns_before: conv_count,
                conversation_turns_after: conv_count,
            }
        }
    }
}

/// Truncate a string to max_len characters, adding "..." if truncated
fn truncate_chars(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
}
