//! MemoryInjector — bridges scene_event_bus events into the four-tier memory system
//!
//! Watches scene events (entity create/update/delete, component add/update/remove)
//! and automatically injects relevant data into Working (L3), Episodic (L2),
//! Semantic (L1), and Procedural (L0) memory tiers.
//!
//! # Usage
//! ```text
//! let mut injector = MemoryInjector::new(memory_system, snapshot_store);
//!
//! // Process raw scene events as they arrive
//! injector.process_batch(&scene_events);
//!
//! // Periodically take full snapshots for diff-based injection
//! injector.take_snapshot(&current_entities);
//! ```

use crate::memory::scene_context::{
    SceneEntityData, SceneEventData, SceneEventKind, SceneSnapshotStore,
};
use crate::memory::system::MemorySystem;
use crate::memory::MemoryEntryId;

#[derive(Debug, Clone, Default)]
pub struct InjectorStats {
    pub events_processed: u64,
    pub entities_injected: u64,
    pub episodes_recorded: u64,
    pub semantic_nodes_built: u64,
    pub workflows_learned: u64,
    pub snapshots_taken: u64,
}

/// The injector wraps MemorySystem and SceneSnapshotStore to provide
/// automatic event-to-memory bridging with configurable batch behavior.
pub struct MemoryInjector {
    pub memory: MemorySystem,
    pub snapshot_store: SceneSnapshotStore,
    pub stats: InjectorStats,

    /// When true, auto-learn workflows after N episodes in a session
    pub auto_learn: bool,
    pub auto_learn_threshold: u32,

    /// Session episode counter for auto-learn trigger
    session_episodes: u32,
}

impl MemoryInjector {
    pub fn new(memory: MemorySystem, snapshot_store: SceneSnapshotStore) -> Self {
        Self {
            memory,
            snapshot_store,
            stats: InjectorStats::default(),
            auto_learn: true,
            auto_learn_threshold: 10,
            session_episodes: 0,
        }
    }

    // ================================================================
    // Event Processing
    // ================================================================

    /// Process a single scene event through all relevant memory tiers.
    pub fn process_event(&mut self, event: &SceneEventData) -> Vec<MemoryEntryId> {
        self.stats.events_processed += 1;
        let mut ids = Vec::new();

        let working_id = self.inject_to_working(event);
        ids.push(working_id);

        let episodic_id = self.inject_to_episodic(event);
        ids.push(episodic_id);

        self.stats.episodes_recorded += 1;
        self.session_episodes += 1;

        if self.auto_learn && self.session_episodes >= self.auto_learn_threshold {
            let learned = self.memory.auto_learn_from_scene_events();
            self.stats.workflows_learned += learned as u64;
            self.session_episodes = 0;
        }

        ids
    }

    /// Process a batch of scene events efficiently.
    pub fn process_batch(&mut self, events: &[SceneEventData]) -> Vec<MemoryEntryId> {
        let mut all_ids = Vec::with_capacity(events.len() * 2);

        if events.is_empty() {
            return all_ids;
        }

        let mut entities_to_add = Vec::new();
        let mut entities_to_remove = Vec::new();
        let mut entities_to_update = Vec::new();

        for event in events {
            self.stats.events_processed += 1;
            self.stats.episodes_recorded += 1;
            self.session_episodes += 1;

            match event.event_kind {
                SceneEventKind::EntityCreated => {
                    if let Some(ref entity) = event.entity_after {
                        let wid = self.memory.inject_entity_working(entity);
                        self.stats.entities_injected += 1;
                        all_ids.push(wid);

                        let eid = self.memory.record_scene_event(event);
                        all_ids.push(eid);

                        entities_to_add.push(entity.clone());
                    }
                }
                SceneEventKind::EntityDeleted => {
                    let eid = self.memory.record_scene_event(event);
                    all_ids.push(eid);

                    if let Some(ref entity) = event.entity_before {
                        entities_to_remove.push(entity.clone());
                    }
                }
                SceneEventKind::EntityUpdated => {
                    let eid = self.memory.record_scene_event(event);
                    all_ids.push(eid);

                    if let Some(ref entity) = event.entity_after {
                        entities_to_update.push(entity.clone());
                    }
                }
                SceneEventKind::ComponentAdded
                | SceneEventKind::ComponentUpdated
                | SceneEventKind::ComponentRemoved => {
                    let eid = self.memory.record_scene_event(event);
                    all_ids.push(eid);

                    if let Some(ref entity) = event.entity_after {
                        entities_to_update.push(entity.clone());
                    }
                }
            }
        }

        if !entities_to_add.is_empty() || !entities_to_update.is_empty() {
            let mut all_relevant: Vec<SceneEntityData> = Vec::new();
            all_relevant.extend(entities_to_add);
            all_relevant.extend(entities_to_update);
            let built = self.memory.build_entity_semantic_graph(&all_relevant);
            self.stats.semantic_nodes_built += built as u64;
        }

        if self.auto_learn && self.session_episodes >= self.auto_learn_threshold {
            let learned = self.memory.auto_learn_from_scene_events();
            self.stats.workflows_learned += learned as u64;
            self.session_episodes = 0;
        }

        all_ids
    }

    // ================================================================
    // Snapshot Management
    // ================================================================

    /// Take a full entity snapshot and inject diffs into all memory tiers.
    pub fn take_snapshot(&mut self, entities: &[SceneEntityData]) {
        let prev_snapshot = self.snapshot_store.latest().cloned();

        self.snapshot_store.take_snapshot(entities);
        self.stats.snapshots_taken += 1;

        self.memory.inject_active_scene(entities);
        let built = self.memory.build_entity_semantic_graph(entities);
        self.stats.semantic_nodes_built += built as u64;

        if let Some(ref prev) = prev_snapshot {
            if let Some(diff) = self
                .snapshot_store
                .diff(prev.version, self.snapshot_store.current_version)
            {
                self.memory.record_scene_diff(&diff);
                self.stats.episodes_recorded +=
                    (diff.added.len() + diff.removed.len() + diff.modified.len()) as u64;
            }
        }
    }

    // ================================================================
    // Batch Snapshot Processing
    // ================================================================

    /// Sync memory with current scene state: snapshot → diff → inject all tiers.
    pub fn sync_scene(&mut self, entities: &[SceneEntityData]) -> InjectorStats {
        self.take_snapshot(entities);

        if self.auto_learn && self.session_episodes >= self.auto_learn_threshold {
            let learned = self.memory.auto_learn_from_scene_events();
            self.stats.workflows_learned += learned as u64;
            self.session_episodes = 0;
        }

        self.stats.clone()
    }

    // ================================================================
    // Context Building
    // ================================================================

    /// Build a memory context suitable for LLM prompt injection based on scene state.
    pub fn build_scene_context(&mut self, query: &str) -> String {
        let entities = self.memory.query_scene_entities(query);
        let stats = self.memory.scene_memory_stats(&self.snapshot_store);

        let mut parts = vec![
            format!("[Scene] {} active entities", stats.active_entities),
            format!("[Events] {} recorded", stats.recorded_events),
            format!("[Knowledge] {} concepts", stats.semantic_nodes),
            format!("[Workflows] {} learned patterns", stats.learned_workflows),
        ];

        if !entities.is_empty() {
            parts.push(format!("[Entities] {}", entities.join(", ")));
        }

        parts.join("\n")
    }

    // ================================================================
    // Private helpers
    // ================================================================

    fn inject_to_working(&mut self, event: &SceneEventData) -> MemoryEntryId {
        let entity = event.entity_after.as_ref().or(event.entity_before.as_ref());
        if let Some(entity) = entity {
            self.stats.entities_injected += 1;
            self.memory.inject_entity_working(entity)
        } else {
            MemoryEntryId(0)
        }
    }

    fn inject_to_episodic(&mut self, event: &SceneEventData) -> MemoryEntryId {
        self.memory.record_scene_event(event)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::scene_context::{ComponentData, SceneEntityData, SceneEventKind};
    use std::collections::HashMap;

    fn make_test_entity(id: u64, name: &str) -> SceneEntityData {
        SceneEntityData {
            id,
            name: name.to_string(),
            components: vec![ComponentData {
                type_name: "Transform".to_string(),
                properties: HashMap::from([("x".to_string(), serde_json::json!(0.0))]),
            }],
            position: Some([0.0, 0.0, 0.0]),
        }
    }

    fn make_create_event(entity_id: u64, name: &str) -> SceneEventData {
        SceneEventData {
            event_kind: SceneEventKind::EntityCreated,
            entity_id,
            entity_before: None,
            entity_after: Some(make_test_entity(entity_id, name)),
            component: None,
            timestamp: "now".to_string(),
        }
    }

    fn make_delete_event(entity_id: u64, name: &str) -> SceneEventData {
        SceneEventData {
            event_kind: SceneEventKind::EntityDeleted,
            entity_id,
            entity_before: Some(make_test_entity(entity_id, name)),
            entity_after: None,
            component: None,
            timestamp: "now".to_string(),
        }
    }

    // ---- Construction ----

    #[test]
    fn test_injector_creation() {
        let memory = MemorySystem::new();
        let store = SceneSnapshotStore::new(10);
        let injector = MemoryInjector::new(memory, store);
        assert_eq!(injector.stats.events_processed, 0);
        assert!(injector.auto_learn);
        assert_eq!(injector.auto_learn_threshold, 10);
    }

    #[test]
    fn test_injector_defaults() {
        let memory = MemorySystem::new();
        let store = SceneSnapshotStore::new(5);
        let mut injector = MemoryInjector::new(memory, store);
        injector.auto_learn = false;
        assert!(!injector.auto_learn);
    }

    // ---- Event Processing ----

    #[test]
    fn test_process_single_create_event() {
        let memory = MemorySystem::new();
        let store = SceneSnapshotStore::new(10);
        let mut injector = MemoryInjector::new(memory, store);
        injector.auto_learn = false;

        let event = make_create_event(1, "Player");
        let ids = injector.process_event(&event);

        assert_eq!(ids.len(), 2);
        assert_eq!(injector.stats.events_processed, 1);
        assert_eq!(injector.stats.entities_injected, 1);
        assert_eq!(injector.stats.episodes_recorded, 1);
    }

    #[test]
    fn test_process_single_delete_event() {
        let memory = MemorySystem::new();
        let store = SceneSnapshotStore::new(10);
        let mut injector = MemoryInjector::new(memory, store);
        injector.auto_learn = false;

        let event = make_delete_event(1, "OldEntity");
        let ids = injector.process_event(&event);

        assert_eq!(ids.len(), 2);
        assert_eq!(injector.stats.events_processed, 1);
        assert_eq!(injector.stats.episodes_recorded, 1);
    }

    #[test]
    fn test_process_batch_create() {
        let memory = MemorySystem::new();
        let store = SceneSnapshotStore::new(10);
        let mut injector = MemoryInjector::new(memory, store);
        injector.auto_learn = false;

        let events = vec![
            make_create_event(1, "A"),
            make_create_event(2, "B"),
            make_create_event(3, "C"),
        ];
        let ids = injector.process_batch(&events);

        assert_eq!(ids.len(), 3 * 2);
        assert_eq!(injector.stats.events_processed, 3);
        assert_eq!(injector.stats.entities_injected, 3);
        assert_eq!(injector.stats.episodes_recorded, 3);

        let entity_count = injector
            .memory
            .working
            .entries
            .iter()
            .filter(|e| {
                matches!(
                    e.entry_type,
                    crate::memory::WorkingEntryType::EntityReference
                )
            })
            .count();
        assert_eq!(entity_count, 3);
    }

    #[test]
    fn test_process_batch_mixed() {
        let memory = MemorySystem::new();
        let store = SceneSnapshotStore::new(10);
        let mut injector = MemoryInjector::new(memory, store);
        injector.auto_learn = false;

        let events = vec![
            make_create_event(1, "Spawned"),
            make_delete_event(2, "Removed"),
            make_create_event(3, "Created"),
        ];
        let ids = injector.process_batch(&events);
        assert_eq!(ids.len(), 5); // create=2 + delete(no entity_after)=1 + create=2
        assert_eq!(injector.stats.events_processed, 3);
    }

    #[test]
    fn test_process_empty_batch() {
        let memory = MemorySystem::new();
        let store = SceneSnapshotStore::new(10);
        let mut injector = MemoryInjector::new(memory, store);

        let ids = injector.process_batch(&[]);
        assert!(ids.is_empty());
        assert_eq!(injector.stats.events_processed, 0);
    }

    // ---- Snapshot + Sync ----

    #[test]
    fn test_take_snapshot_injects() {
        let memory = MemorySystem::new();
        let store = SceneSnapshotStore::new(10);
        let mut injector = MemoryInjector::new(memory, store);
        injector.auto_learn = false;

        let entities = vec![make_test_entity(1, "Player"), make_test_entity(2, "Enemy")];
        injector.take_snapshot(&entities);

        assert_eq!(injector.stats.snapshots_taken, 1);
        assert_eq!(injector.snapshot_store.len(), 1);

        let entity_count = injector
            .memory
            .working
            .entries
            .iter()
            .filter(|e| {
                matches!(
                    e.entry_type,
                    crate::memory::WorkingEntryType::EntityReference
                )
            })
            .count();
        assert_eq!(entity_count, 2);
    }

    #[test]
    fn test_sync_scene_returns_stats() {
        let memory = MemorySystem::new();
        let store = SceneSnapshotStore::new(10);
        let mut injector = MemoryInjector::new(memory, store);
        injector.auto_learn = false;

        let entities = vec![make_test_entity(1, "Player")];
        let stats = injector.sync_scene(&entities);

        assert_eq!(stats.snapshots_taken, 1);
        assert!(stats.semantic_nodes_built > 0);
    }

    #[test]
    fn test_second_snapshot_captures_diff() {
        let memory = MemorySystem::new();
        let store = SceneSnapshotStore::new(10);
        let mut injector = MemoryInjector::new(memory, store);
        injector.auto_learn = false;

        let initial = vec![make_test_entity(1, "A")];
        injector.take_snapshot(&initial);

        let updated = vec![
            make_test_entity(1, "A_renamed"),
            make_test_entity(2, "B_added"),
        ];
        injector.take_snapshot(&updated);

        assert_eq!(injector.snapshot_store.len(), 2);
        assert!(injector.stats.episodes_recorded > 0);
    }

    // ---- Context Building ----

    #[test]
    fn test_build_scene_context() {
        let memory = MemorySystem::new();
        let store = SceneSnapshotStore::new(10);
        let mut injector = MemoryInjector::new(memory, store);
        injector.auto_learn = false;

        let entities = vec![make_test_entity(1, "Player")];
        injector.sync_scene(&entities);

        let ctx = injector.build_scene_context("Player");
        assert!(ctx.contains("Player"));
        assert!(ctx.contains("active entities"));
    }

    // ---- Auto-learn ----

    #[test]
    fn test_auto_learn_triggered() {
        let memory = MemorySystem::new();
        let store = SceneSnapshotStore::new(10);
        let mut injector = MemoryInjector::new(memory, store);
        injector.auto_learn_threshold = 3;

        for _ in 1..=4 {
            injector.memory.observe_decision(
                "creating entities",
                "create entity",
                "entity successfully created",
                true,
            );
        }

        let events: Vec<_> = (1..=4)
            .map(|i| make_create_event(i, &format!("Entity_{}", i)))
            .collect();
        injector.process_batch(&events);

        assert!(
            injector.stats.workflows_learned > 0,
            "auto_learn should produce workflows"
        );
    }

    #[test]
    fn test_auto_learn_disabled() {
        let memory = MemorySystem::new();
        let store = SceneSnapshotStore::new(10);
        let mut injector = MemoryInjector::new(memory, store);
        injector.auto_learn = false;

        let events: Vec<_> = (1..=15)
            .map(|i| make_create_event(i, &format!("Entity_{}", i)))
            .collect();

        injector.process_batch(&events);
        assert_eq!(injector.stats.workflows_learned, 0);
    }

    // ---- Integration ----

    #[test]
    fn test_full_injector_pipeline() {
        let memory = MemorySystem::new();
        let store = SceneSnapshotStore::new(10);
        let mut injector = MemoryInjector::new(memory, store);
        injector.auto_learn_threshold = 5;

        // Phase 1: Create initial scene
        let entities = vec![make_test_entity(1, "Player"), make_test_entity(2, "Enemy")];
        injector.sync_scene(&entities);

        assert_eq!(injector.stats.snapshots_taken, 1);
        assert_eq!(injector.snapshot_store.len(), 1);

        // Phase 2: Simulate events
        let create_event = make_create_event(3, "Coin");
        let delete_event = make_delete_event(2, "Enemy");
        injector.process_batch(&[create_event, delete_event]);

        assert_eq!(injector.stats.events_processed, 2);
        assert!(injector.stats.entities_injected >= 1);

        // Phase 3: Take new snapshot
        let updated = vec![make_test_entity(1, "Player"), make_test_entity(3, "Coin")];
        injector.take_snapshot(&updated);

        assert_eq!(injector.snapshot_store.len(), 2);
        assert!(injector.stats.episodes_recorded > 0);

        // Phase 4: Build context for LLM
        let ctx = injector.build_scene_context("Player");
        assert!(ctx.contains("Player"));
        assert!(ctx.contains("active entities"));

        // Phase 5: Verify stats
        let stats = injector.memory.scene_memory_stats(&injector.snapshot_store);
        assert!(stats.active_entities > 0, "should have active entities");
        assert!(stats.semantic_nodes > 0, "should have semantic nodes");
        assert_eq!(stats.snapshots_stored, 2);
    }
}
