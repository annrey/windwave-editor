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

    /// Run cleanup on all tiers with importance-based selection
    pub fn cleanup(&mut self) {
        if !self.config.enable_decay {
            return;
        }

        // Working memory: cleanup expired entries
        self.working.cleanup_expired();

        // Episodic memory: remove low-importance entries if over capacity
        let excess = self.lifecycle.excess_count(MemoryTier::Episodic, self.episodic.len());
        if excess > 0 {
            self.cleanup_episodic_by_importance(excess);
        }

        // Semantic memory: remove least important nodes
        let excess = self.lifecycle.excess_count(MemoryTier::Semantic, self.semantic.node_count());
        if excess > 0 {
            self.cleanup_semantic_by_importance(excess);
        }

        // Procedural memory: remove least used workflows
        let excess = self.lifecycle.excess_count(MemoryTier::Procedural, self.procedural.workflow_count());
        if excess > 0 {
            self.cleanup_procedural_by_usage(excess);
        }
    }

    /// Remove lowest-importance episodic entries
    fn cleanup_episodic_by_importance(&mut self, count: usize) {
        // Score each episode by: recency * importance * access_frequency
        let mut scored: Vec<(usize, f32)> = self.episodic.iter()
            .enumerate()
            .map(|(i, ep)| {
                let age_factor = 1.0 / (1.0 + (crate::types::current_timestamp() - ep.metadata.created_at) as f32 / 3600.0); // Decay over hours
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
        if self.semantic.node_count() > 1000 {  // Only clean if very large
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

    /// Save memory state to disk (JSON format)
    ///
    /// # Arguments
    /// * `path` - File path to save to (e.g., "data/memory.json")
    ///
    /// # Returns
    /// Result with statistics about saved data
    pub fn save_to_file(&self, path: &str) -> Result<MemoryPersistenceInfo, String> {
        let dir = std::path::Path::new(path).parent()
            .unwrap_or(std::path::Path::new("."));

        std::fs::create_dir_all(dir)
            .map_err(|e| format!("Failed to create directory {:?}: {}", dir, e))?;

        let data = MemoryPersistedData {
            working_entries: self.working.get_entries_for_persistence(),
            episodes: self.episodic.get_all_episodes().into_iter().filter(|e| {
                matches!(e.episode_type, crate::memory::EpisodeType::UserRequest |
                                       crate::memory::EpisodeType::ToolCalled |
                                       crate::memory::EpisodeType::ErrorOccurred |
                                       crate::memory::EpisodeType::Summary)
            }).collect(),
            semantic_nodes: self.semantic.export_nodes(),
            procedural_workflows: self.procedural.export_workflows(),
            config: self.config.clone(),
            saved_at: chrono::Utc::now().to_rfc3339(),
            version: "1.0".to_string(),
        };

        let json = serde_json::to_string_pretty(&data)
            .map_err(|e| format!("Serialization failed: {}", e))?;

        let total_bytes = json.len();

        std::fs::write(path, &json)
            .map_err(|e| format!("Failed to write to {}: {}", path, e))?;

        Ok(MemoryPersistenceInfo {
            file_path: path.to_string(),
            total_bytes,
            working_count: data.working_entries.len(),
            episodic_count: data.episodes.len(),
            semantic_count: data.semantic_nodes.len(),
            procedural_count: data.procedural_workflows.len(),
        })
    }

    /// Load memory state from disk
    ///
    /// # Arguments
    /// * `path` - File path to load from
    ///
    /// # Returns
    /// Result with loaded data statistics
    pub fn load_from_file(&mut self, path: &str) -> Result<MemoryLoadResult, String> {
        if !std::path::Path::new(path).exists() {
            return Err(format!("File not found: {}", path));
        }

        let json = std::fs::read_to_string(path)
            .map_err(|e| format!("Failed to read {}: {}", path, e))?;

        let data: MemoryPersistedData = serde_json::from_str(&json)
            .map_err(|e| format!("Deserialization failed: {}", e))?;

        // Save counts before moving data
        let working_count = data.working_entries.len();
        let episodic_count = data.episodes.len();
        let semantic_count = data.semantic_nodes.len();
        let procedural_count = data.procedural_workflows.len();

        // Restore working memory
        for entry in data.working_entries {
            self.working.restore_entry(entry);
        }

        // Restore episodic memory
        for episode in data.episodes {
            self.episodic.restore_episode(episode);
        }

        // Restore semantic memory
        for node in data.semantic_nodes {
            self.semantic.import_node(node);
        }

        // Restore procedural memory
        for workflow in data.procedural_workflows {
            self.procedural.import_workflow(workflow);
        }

        Ok(MemoryLoadResult {
            file_path: path.to_string(),
            working_restored: working_count,
            episodic_restored: episodic_count,
            semantic_restored: semantic_count,
            procedural_restored: procedural_count,
            saved_at: data.saved_at,
        })
    }

    /// Auto-save with backup rotation
    /// Keeps last N backups (default: 5)
    pub fn auto_save(&self, base_path: &str, max_backups: usize) -> Result<Vec<String>, String> {
        let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
        let current_path = format!("{}_{}.json", base_path, timestamp);

        self.save_to_file(&current_path)?;

        // Create symlink to latest
        let latest_path = format!("{}_latest.json", base_path);
        let _ = std::fs::remove_file(&latest_path);  // Remove old symlink
        #[cfg(unix)]
        {
            let _ = std::os::unix::fs::symlink(
                std::path::Path::new(&current_path).file_name().unwrap(),
                &latest_path,
            );
        }

        // Cleanup old backups (keep only max_backups most recent)
        Self::cleanup_old_backups(base_path, max_backups)?;

        Ok(vec![current_path])
    }

    fn cleanup_old_backups(base_path: &str, keep: usize) -> Result<(), String> {
        let pattern = format!("{}_*.json", base_path);

        let mut files: Vec<std::path::PathBuf> = glob::glob(&pattern)
            .map_err(|e| format!("Glob error: {}", e))?
            .filter_map(Result::ok)
            .collect();

        files.sort();
        files.reverse();  // Newest first

        for old_file in files.into_iter().skip(keep) {
            std::fs::remove_file(&old_file)
                .map_err(|e| format!("Failed to remove old backup {:?}: {}", old_file, e))?;
        }

        Ok(())
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

    // =================================================================
    // Memory Compression
    // =================================================================

    /// Compress episodic memory using LLM summarization
    ///
    /// When episodic memory exceeds threshold, use LLM to generate
    /// a concise summary preserving key information.
    ///
    /// # Arguments
    /// * `max_episodes` - Maximum number of episodes before triggering compression (default: 50)
    /// * `llm_client` - Optional LLM client for summarization (None = rule-based compression)
    ///
    /// # Returns
    /// Number of episodes compressed
    pub async fn compress_episodic_memory(
        &mut self,
        max_episodes: usize,
        llm_client: Option<&dyn crate::llm::LlmClient>,
    ) -> usize {
        if self.episodic.len() <= max_episodes {
            return 0;
        }

        let episodes_to_compress = self.episodic.len() - max_episodes;

        // Group episodes into chunks for batch processing
        let old_episodes: Vec<Episode> = self.episodic.drain_old_episodes(episodes_to_compress);

        if old_episodes.is_empty() {
            return 0;
        }

        // If LLM available, use intelligent summarization
        if let Some(client) = llm_client {
            let summary = Self::generate_llm_summary(&old_episodes, client).await;

            // Create compressed episode from summary
            let compressed = Episode {
                metadata: crate::memory::MemoryMetadata::new(
                    self.episodic.next_id_counter(),
                    crate::memory::MemoryTier::Episodic,
                ),
                episode_type: crate::memory::EpisodeType::Summary,
                summary: format!("[Compressed {} episodes] {}", old_episodes.len(), summary),
                details: serde_json::json!({
                    "compressed_count": old_episodes.len(),
                    "original_ids": old_episodes.iter().map(|e| e.metadata.id.0).collect::<Vec<_>>(),
                }),
                entity_ids: Vec::new(),
                success: None,
                duration_ms: None,
            };
            self.episodic.record_compressed(compressed);
        } else {
            // Rule-based compression: extract key information
            let summary = Self::generate_rule_based_summary(&old_episodes);

            let compressed = Episode {
                metadata: crate::memory::MemoryMetadata::new(
                    self.episodic.next_id_counter(),
                    crate::memory::MemoryTier::Episodic,
                ),
                episode_type: crate::memory::EpisodeType::Summary,
                summary: format!("[Auto-compressed {} episodes] {}", old_episodes.len(), summary),
                details: serde_json::json!({
                    "compressed_count": old_episodes.len(),
                    "method": "rule_based",
                }),
                entity_ids: Vec::new(),
                success: None,
                duration_ms: None,
            };
            self.episodic.record_compressed(compressed);
        }

        episodes_to_compress
    }

    /// Generate summary using LLM
    async fn generate_llm_summary(episodes: &[Episode], client: &dyn crate::llm::LlmClient) -> String {
        let episodes_text: String = episodes
            .iter()
            .enumerate()
            .map(|(i, ep)| format!("{}. [{}] {}", i + 1, format_episode_short(ep), ep.summary))
            .collect::<Vec<_>>()
            .join("\n");

        let prompt = format!(
            "Summarize the following conversation history into key points. \
             Preserve important decisions, user preferences, and errors encountered.\n\n\
             ---\n{}\n---\n\n\
             Summary:",
            episodes_text
        );

        match client.chat(crate::llm::LlmRequest {
            model: "gpt-4o-mini".to_string(),
            messages: vec![crate::llm::LlmMessage {
                role: crate::llm::Role::User,
                content: prompt,
            }],
            max_tokens: Some(500),
            temperature: Some(0.3),
            tools: None,
        }).await {
            Ok(response) => response.content,
            Err(_) => Self::generate_rule_based_summary(episodes), // Fallback to rule-based
        }
    }

    /// Generate summary using rules (no LLM needed)
    fn generate_rule_based_summary(episodes: &[Episode]) -> String {
        let mut key_points = Vec::new();
        let mut user_requests = Vec::new();
        let mut errors_encountered = Vec::new();

        for ep in episodes {
            match ep.episode_type {
                crate::memory::EpisodeType::UserRequest => {
                    user_requests.push(truncate_str(&ep.summary, 100));
                }
                crate::memory::EpisodeType::ErrorOccurred => {
                    errors_encountered.push(truncate_str(&ep.summary, 80));
                }
                _ => {
                    if key_points.len() < 5 {
                        // Keep top 5 other events
                        key_points.push(format!("• {}", truncate_str(&ep.summary, 80)));
                    }
                }
            }
        }

        let mut parts = Vec::new();

        if !user_requests.is_empty() {
            parts.push(format!("User requests: {}", user_requests.join("; ")));
        }
        if !errors_encountered.is_empty() {
            parts.push(format!("Issues: {}", errors_encountered.join("; ")));
        }
        if !key_points.is_empty() {
            parts.push(format!("Key events: {}", key_points.join(" ")));
        }

        if parts.is_empty() {
            "[No significant events]".to_string()
        } else {
            parts.join(" | ")
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

// Helper: format episode type as short tag
fn format_episode_short(ep: &Episode) -> &'static str {
    match ep.episode_type {
        crate::memory::EpisodeType::UserRequest => "USER",
        crate::memory::EpisodeType::ToolCalled => "TOOL",
        crate::memory::EpisodeType::Observation => "OBS",
        crate::memory::EpisodeType::ErrorOccurred => "ERR",
        crate::memory::EpisodeType::Summary => "SUM",
        _ => "???",
    }
}

// Helper: truncate string to max length with ellipsis
fn truncate_str(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
}

// ============================================================================
// Persistence Data Structures
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MemoryPersistedData {
    working_entries: Vec<serde_json::Value>,
    episodes: Vec<crate::memory::Episode>,
    semantic_nodes: Vec<crate::memory::SemanticNode>,
    procedural_workflows: Vec<crate::memory::WorkflowTemplate>,
    config: MemoryConfig,
    saved_at: String,
    version: String,
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
