use crate::memory::{MemoryConfig, MemoryStats, MemorySystem};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Unique identifier for an agent — mirrors registry::AgentId without coupling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AgentMemoryId(pub u64);

impl From<u64> for AgentMemoryId {
    fn from(id: u64) -> Self {
        Self(id)
    }
}

/// Per-agent memory wrapper that associates a MemorySystem with agent metadata.
#[derive(Debug, Clone)]
pub struct AgentMemoryEntry {
    pub system: MemorySystem,
    pub created_at: u64,
    pub last_active: u64,
}

impl AgentMemoryEntry {
    pub fn new() -> Self {
        let now = crate::types::current_timestamp();
        Self {
            system: MemorySystem::new(),
            created_at: now,
            last_active: now,
        }
    }

    pub fn with_config(config: MemoryConfig) -> Self {
        let now = crate::types::current_timestamp();
        Self {
            system: MemorySystem::with_config(config),
            created_at: now,
            last_active: now,
        }
    }

    pub fn touch(&mut self) {
        self.last_active = crate::types::current_timestamp();
    }

    /// Seconds since this memory was last accessed.
    pub fn idle_seconds(&self) -> u64 {
        let now = crate::types::current_timestamp();
        now.saturating_sub(self.last_active)
    }
}

impl Default for AgentMemoryEntry {
    fn default() -> Self {
        Self::new()
    }
}

/// Registry of per-agent memory systems.
///
/// Each agent gets its own isolated four-tier memory (Working, Episodic,
/// Semantic, Procedural). Memory can be transferred or merged between
/// agents during task handoffs.
///
/// # ADR-001 Phase 2
///
/// Before: `DirectorRuntime` owned a single `MemorySystem` shared by all agents.
/// After: Each agent ID has its own `AgentMemoryEntry`, accessed through this
/// registry. The `get_mut(agent_id)` method auto-creates memory on first access.
#[derive(Debug, Clone)]
pub struct MemorySystemRegistry {
    entries: HashMap<AgentMemoryId, AgentMemoryEntry>,
    default_config: Option<MemoryConfig>,
    next_id: u64,
}

impl MemorySystemRegistry {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
            default_config: None,
            next_id: 0,
        }
    }

    pub fn with_default_config(config: MemoryConfig) -> Self {
        Self {
            entries: HashMap::new(),
            default_config: Some(config),
            next_id: 0,
        }
    }

    /// Register a new agent with an empty memory system.
    /// Returns the assigned AgentMemoryId.
    pub fn register(&mut self, agent_id: impl Into<AgentMemoryId>) -> AgentMemoryId {
        let id = agent_id.into();
        let entry = match &self.default_config {
            Some(config) => AgentMemoryEntry::with_config(config.clone()),
            None => AgentMemoryEntry::new(),
        };
        self.entries.insert(id, entry);
        id
    }

    /// Get mutable access to an agent's memory system.
    /// Auto-creates a new memory system if the agent is not yet registered.
    pub fn get_mut(&mut self, agent_id: impl Into<AgentMemoryId>) -> &mut AgentMemoryEntry {
        let id = agent_id.into();
        if !self.entries.contains_key(&id) {
            self.register(id);
        }
        let entry = self.entries.get_mut(&id).unwrap();
        entry.touch();
        entry
    }

    /// Read-only access to an agent's memory entry.
    pub fn get(&self, agent_id: impl Into<AgentMemoryId>) -> Option<&AgentMemoryEntry> {
        let id = agent_id.into();
        self.entries.get(&id)
    }

    /// Read-only access to the underlying MemorySystem.
    pub fn get_system(&self, agent_id: impl Into<AgentMemoryId>) -> Option<&MemorySystem> {
        self.get(agent_id).map(|e| &e.system)
    }

    /// Remove an agent's memory entirely.
    pub fn remove(&mut self, agent_id: impl Into<AgentMemoryId>) -> bool {
        self.entries.remove(&agent_id.into()).is_some()
    }

    /// Transfer memory from one agent to another (task handoff).
    /// Source agent's memory entry is removed.
    pub fn transfer(
        &mut self,
        from: impl Into<AgentMemoryId>,
        to: impl Into<AgentMemoryId>,
    ) -> bool {
        let from_id = from.into();
        let to_id = to.into();
        if let Some(entry) = self.entries.remove(&from_id) {
            self.entries.insert(to_id, entry);
            true
        } else {
            false
        }
    }

    /// Merge source agent's memory into target agent's memory.
    /// Preserves both memory systems (source is NOT removed).
    /// Merges episodic events and procedural workflows.
    pub fn merge(&mut self, source: impl Into<AgentMemoryId>, target: impl Into<AgentMemoryId>) {
        let source_id = source.into();
        let target_id = target.into();
        if source_id == target_id {
            return;
        }
        let mut source_system = match self.entries.get(&source_id).map(|e| &e.system) {
            Some(s) => s.clone(),
            None => return,
        };
        let target_entry = self.get_mut(target_id);
        // Merge working memory intent: append source intent to target
        if let Some(source_intent) = source_system.working.current_intent() {
            let combined = match target_entry.system.working.current_intent() {
                Some(existing) => format!("{} / {}", existing, source_intent),
                None => source_intent,
            };
            target_entry.system.set_intent(&combined);
        }
        // Merge episodic episodes
        for episode in source_system.episodic.get_all_episodes() {
            target_entry
                .system
                .episodic
                .restore_episode(episode.clone());
        }
        // Merge semantic nodes
        for node in source_system.semantic.export_nodes() {
            target_entry.system.semantic.import_node(node);
        }
        // Merge procedural workflows
        for wf in source_system.procedural.export_workflows() {
            target_entry.system.procedural.import_workflow(wf);
        }
    }

    /// Check if an agent has registered memory.
    pub fn has(&self, agent_id: impl Into<AgentMemoryId>) -> bool {
        self.entries.contains_key(&agent_id.into())
    }

    /// Number of agents with registered memory.
    pub fn count(&self) -> usize {
        self.entries.len()
    }

    /// List all registered agent IDs.
    pub fn agent_ids(&self) -> Vec<AgentMemoryId> {
        self.entries.keys().copied().collect()
    }

    /// Clear memory for a single agent.
    pub fn clear_agent(&mut self, agent_id: impl Into<AgentMemoryId>) {
        if let Some(entry) = self.entries.get_mut(&agent_id.into()) {
            entry.system.clear_all();
        }
    }

    /// Clear all agents' memory (does not remove agents).
    pub fn clear_all(&mut self) {
        for entry in self.entries.values_mut() {
            entry.system.clear_all();
        }
    }

    /// Statistics for all agents.
    pub fn stats(&self) -> HashMap<AgentMemoryId, MemoryStats> {
        self.entries
            .iter()
            .map(|(id, entry)| (*id, entry.system.stats()))
            .collect()
    }

    /// Allocate a new unique AgentMemoryId.
    pub fn allocate_id(&mut self) -> AgentMemoryId {
        let id = self.next_id;
        self.next_id += 1;
        AgentMemoryId(id)
    }

    // =================================================================
    // Persistence
    // =================================================================

    /// Save all agent memory systems to a directory.
    /// Each agent gets its own JSON file: `{dir}/agent_{id}.json`.
    pub fn save_all_to_dir(&self, dir: &std::path::Path) -> std::io::Result<usize> {
        std::fs::create_dir_all(dir)?;
        let mut saved = 0;
        for (id, entry) in &self.entries {
            let filename = format!("agent_{}.json", id.0);
            let path = dir.join(&filename);
            entry
                .system
                .save_to_file(path.to_str().unwrap_or("memory.json"))
                .map_err(std::io::Error::other)?;
            saved += 1;
        }
        Ok(saved)
    }

    /// Load all agent memory systems from a directory.
    /// Returns the number of agents loaded.
    pub fn load_all_from_dir(&mut self, dir: &std::path::Path) -> std::io::Result<usize> {
        if !dir.exists() {
            return Ok(0);
        }
        let mut loaded = 0;
        for entry_result in std::fs::read_dir(dir)? {
            let entry = entry_result?;
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "json") {
                if let Some(filename) = path.file_stem().and_then(|s| s.to_str()) {
                    if let Some(id_str) = filename.strip_prefix("agent_") {
                        if let Ok(id_num) = id_str.parse::<u64>() {
                            let id = AgentMemoryId(id_num);
                            let agent_entry = self.get_mut(id);
                            match agent_entry
                                .system
                                .load_from_file(path.to_str().unwrap_or(""))
                            {
                                Ok(result) => {
                                    log::info!(
                                        "Loaded memory for agent {} ({} working, {} episodic, {} semantic, {} procedural)",
                                        id_num,
                                        result.working_restored,
                                        result.episodic_restored,
                                        result.semantic_restored,
                                        result.procedural_restored,
                                    );
                                    loaded += 1;
                                }
                                Err(e) => {
                                    log::warn!("Failed to load memory for agent {}: {}", id_num, e);
                                }
                            }
                        }
                    }
                }
            }
        }
        Ok(loaded)
    }
}

impl Default for MemorySystemRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_and_get() {
        let mut registry = MemorySystemRegistry::new();
        let id = registry.register(AgentMemoryId(1));
        assert_eq!(id, AgentMemoryId(1));
        assert!(registry.has(AgentMemoryId(1)));
        assert!(!registry.has(AgentMemoryId(2)));
        assert_eq!(registry.count(), 1);
    }

    #[test]
    fn test_get_mut_auto_creates() {
        let mut registry = MemorySystemRegistry::new();
        let entry = registry.get_mut(AgentMemoryId(42));
        entry.system.set_intent("test intent");
        assert!(registry.has(AgentMemoryId(42)));
        assert_eq!(registry.count(), 1);
    }

    #[test]
    fn test_isolation_between_agents() {
        let mut registry = MemorySystemRegistry::new();
        registry
            .get_mut(AgentMemoryId(1))
            .system
            .set_intent("intent for agent 1");
        registry
            .get_mut(AgentMemoryId(2))
            .system
            .set_intent("intent for agent 2");

        let ctx1 = registry
            .get_mut(AgentMemoryId(1))
            .system
            .build_context(&crate::memory::MemoryQuery::working_only("intent"));
        assert!(ctx1.working_context.contains("agent 1"));

        let ctx2 = registry
            .get_mut(AgentMemoryId(2))
            .system
            .build_context(&crate::memory::MemoryQuery::working_only("intent"));
        assert!(ctx2.working_context.contains("agent 2"));
    }

    #[test]
    fn test_transfer_memory() {
        let mut registry = MemorySystemRegistry::new();
        registry
            .get_mut(AgentMemoryId(1))
            .system
            .set_intent("project alpha");
        assert!(registry.transfer(AgentMemoryId(1), AgentMemoryId(2)));
        assert!(!registry.has(AgentMemoryId(1)));
        assert!(registry.has(AgentMemoryId(2)));
        let ctx = registry
            .get_mut(AgentMemoryId(2))
            .system
            .build_context(&crate::memory::MemoryQuery::working_only("project"));
        assert!(ctx.working_context.contains("project alpha"));
    }

    #[test]
    fn test_merge_memory() {
        let mut registry = MemorySystemRegistry::new();
        registry
            .get_mut(AgentMemoryId(1))
            .system
            .set_intent("from source");
        registry
            .get_mut(AgentMemoryId(2))
            .system
            .set_intent("from target");

        registry.merge(AgentMemoryId(1), AgentMemoryId(2));

        // Source still exists
        assert!(registry.has(AgentMemoryId(1)));
        // Target has both
        let ctx = registry
            .get_mut(AgentMemoryId(2))
            .system
            .build_context(&crate::memory::MemoryQuery::working_only("from"));
        assert!(ctx.working_context.contains("from source"));
        assert!(ctx.working_context.contains("from target"));
    }

    #[test]
    fn test_clear_all() {
        let mut registry = MemorySystemRegistry::new();
        registry.get_mut(AgentMemoryId(1)).system.set_intent("data");
        registry.get_mut(AgentMemoryId(2)).system.set_intent("data");
        assert_eq!(registry.count(), 2);
        registry.clear_all();
        for entry in registry.entries.values_mut() {
            let ctx = entry
                .system
                .build_context(&crate::memory::MemoryQuery::working_only("data"));
            assert!(ctx.is_empty());
        }
    }

    #[test]
    fn test_remove_agent() {
        let mut registry = MemorySystemRegistry::new();
        registry.get_mut(AgentMemoryId(1));
        assert_eq!(registry.count(), 1);
        assert!(registry.remove(AgentMemoryId(1)));
        assert_eq!(registry.count(), 0);
        assert!(!registry.remove(AgentMemoryId(1))); // already gone
    }

    #[test]
    fn test_idle_tracking() {
        let mut registry = MemorySystemRegistry::new();
        registry.register(AgentMemoryId(1));
        let idle = registry.get(AgentMemoryId(1)).unwrap().idle_seconds();
        // Should be near 0 since we just created it
        assert!(idle < 2);
    }

    #[test]
    fn test_stats() {
        let mut registry = MemorySystemRegistry::new();
        registry
            .get_mut(AgentMemoryId(1))
            .system
            .set_intent("hello");
        registry
            .get_mut(AgentMemoryId(2))
            .system
            .set_intent("world");
        let stats = registry.stats();
        assert_eq!(stats.len(), 2);
        for stat in stats.values() {
            assert!(stat.working_entries > 0);
        }
    }
}
