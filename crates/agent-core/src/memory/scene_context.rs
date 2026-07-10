//! Scene Context Memory Bridge
//!
//! Bridges between scene entities/events (from multica-bridge or bevy-adapter)
//! and the four-tier memory system (Working/Episodic/Semantic/Procedural).
//!
//! # Architecture
//! ```text
//! SceneEventBus (multica-bridge)    Bevy ECS (bevy-adapter)
//!         │                                  │
//!         ▼                                  ▼
//!   SceneEventData    ──convert──►   SceneMemoryEntry
//!         │                                  │
//!         ▼                                  ▼
//! ┌─────────────────────────────────────────────┐
//! │         MemorySystem (agent-core)            │
//! │  Working(L3)  Episodic(L2)  Semantic(L1)    │
//! │               Procedural(L0)                │
//! └─────────────────────────────────────────────┘
//! ```
//!
//! Uses generic data types (SceneEntityData, SceneEventData) to avoid
//! depending on multica-bridge or bevy-adapter types directly.
//! The DirectorRuntime integration layer handles type conversion.

use crate::memory::system::MemorySystem;
use crate::memory::{
    Episode, EpisodeType, MemoryEntryId, MemoryMetadata, MemoryQuery, MemoryTier, RelationType,
    SemanticNode, WorkingEntryType, WorkingMemoryEntry,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// Scene Data Types (generic, crate-local)
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SceneEntityData {
    pub id: u64,
    pub name: String,
    pub components: Vec<ComponentData>,
    pub position: Option<[f64; 3]>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ComponentData {
    pub type_name: String,
    pub properties: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SceneEventKind {
    EntityCreated,
    EntityUpdated,
    EntityDeleted,
    ComponentAdded,
    ComponentUpdated,
    ComponentRemoved,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneEventData {
    pub event_kind: SceneEventKind,
    pub entity_id: u64,
    pub entity_before: Option<SceneEntityData>,
    pub entity_after: Option<SceneEntityData>,
    pub component: Option<ComponentData>,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneSnapshot {
    pub entities: Vec<SceneEntityData>,
    pub timestamp: String,
    pub version: u64,
}

// ============================================================================
// SceneMemoryEntry — unified scene-to-memory mapping
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SceneMemoryEntry {
    WorkingEntity {
        entity: SceneEntityData,
        memory_id: MemoryEntryId,
    },
    EpisodicEvent {
        event: SceneEventData,
        memory_id: MemoryEntryId,
    },
    SemanticNode {
        entity: SceneEntityData,
        memory_id: MemoryEntryId,
    },
    ProceduralPattern {
        name: String,
        trigger: String,
        memory_id: MemoryEntryId,
    },
}

impl SceneMemoryEntry {
    pub fn memory_id(&self) -> MemoryEntryId {
        match self {
            SceneMemoryEntry::WorkingEntity { memory_id, .. } => *memory_id,
            SceneMemoryEntry::EpisodicEvent { memory_id, .. } => *memory_id,
            SceneMemoryEntry::SemanticNode { memory_id, .. } => *memory_id,
            SceneMemoryEntry::ProceduralPattern { memory_id, .. } => *memory_id,
        }
    }

    pub fn tier(&self) -> MemoryTier {
        match self {
            SceneMemoryEntry::WorkingEntity { .. } => MemoryTier::Working,
            SceneMemoryEntry::EpisodicEvent { .. } => MemoryTier::Episodic,
            SceneMemoryEntry::SemanticNode { .. } => MemoryTier::Semantic,
            SceneMemoryEntry::ProceduralPattern { .. } => MemoryTier::Procedural,
        }
    }
}

// ============================================================================
// SceneSnapshotStore — manages scene state history
// ============================================================================

#[derive(Debug, Clone)]
pub struct SceneSnapshotStore {
    pub snapshots: Vec<SceneSnapshot>,
    pub max_snapshots: usize,
    pub current_version: u64,
}

impl SceneSnapshotStore {
    pub fn new(max_snapshots: usize) -> Self {
        Self {
            snapshots: Vec::new(),
            max_snapshots,
            current_version: 0,
        }
    }

    pub fn take_snapshot(&mut self, entities: &[SceneEntityData]) -> &SceneSnapshot {
        let timestamp = chrono::Utc::now().to_rfc3339();
        self.current_version += 1;
        let snapshot = SceneSnapshot {
            entities: entities.to_vec(),
            timestamp,
            version: self.current_version,
        };
        self.snapshots.push(snapshot);
        if self.snapshots.len() > self.max_snapshots {
            self.snapshots.remove(0);
        }
        &self.snapshots[self.snapshots.len() - 1]
    }

    pub fn latest(&self) -> Option<&SceneSnapshot> {
        self.snapshots.last()
    }

    pub fn diff(&self, from_version: u64, to_version: u64) -> Option<SceneDiff> {
        let from = self.snapshots.iter().find(|s| s.version == from_version)?;
        let to = self.snapshots.iter().find(|s| s.version == to_version)?;
        Some(Self::compute_diff(from, to))
    }

    fn compute_diff(from: &SceneSnapshot, to: &SceneSnapshot) -> SceneDiff {
        let from_map: HashMap<u64, &SceneEntityData> =
            from.entities.iter().map(|e| (e.id, e)).collect();
        let to_map: HashMap<u64, &SceneEntityData> =
            to.entities.iter().map(|e| (e.id, e)).collect();

        let mut added = Vec::new();
        let mut removed = Vec::new();
        let mut modified = Vec::new();

        for (id, entity) in &to_map {
            if !from_map.contains_key(id) {
                added.push((*entity).clone());
            } else if from_map[id] != *entity {
                let changes = Self::entity_changes(from_map[id], entity);
                modified.push(EntityChange {
                    entity_id: *id,
                    before: Some((*from_map[id]).clone()),
                    after: Some((*entity).clone()),
                    changes,
                });
            }
        }
        for id in from_map.keys() {
            if !to_map.contains_key(id) {
                removed.push(from_map[id].clone());
            }
        }

        SceneDiff {
            added,
            removed,
            modified,
        }
    }

    fn entity_changes(before: &SceneEntityData, after: &SceneEntityData) -> Vec<String> {
        let mut changes = Vec::new();
        if before.name != after.name {
            changes.push(format!("name: {} → {}", before.name, after.name));
        }
        if before.position != after.position {
            changes.push("position changed".to_string());
        }
        let before_comp: HashMap<&str, &ComponentData> = before
            .components
            .iter()
            .map(|c| (c.type_name.as_str(), c))
            .collect();
        let after_comp: HashMap<&str, &ComponentData> = after
            .components
            .iter()
            .map(|c| (c.type_name.as_str(), c))
            .collect();
        for name in before_comp.keys() {
            if !after_comp.contains_key(name) {
                changes.push(format!("component removed: {}", name));
            }
        }
        for name in after_comp.keys() {
            if !before_comp.contains_key(name) {
                changes.push(format!("component added: {}", name));
            } else if before_comp[name] != after_comp[name] {
                changes.push(format!("component updated: {}", name));
            }
        }
        changes
    }

    pub fn len(&self) -> usize {
        self.snapshots.len()
    }

    pub fn is_empty(&self) -> bool {
        self.snapshots.is_empty()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneDiff {
    pub added: Vec<SceneEntityData>,
    pub removed: Vec<SceneEntityData>,
    pub modified: Vec<EntityChange>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityChange {
    pub entity_id: u64,
    pub before: Option<SceneEntityData>,
    pub after: Option<SceneEntityData>,
    pub changes: Vec<String>,
}

// ============================================================================
// SceneMemoryIntegration — inject scene data into all four memory tiers
// ============================================================================

impl MemorySystem {
    // ---- Working Memory (L3) ----

    pub fn inject_entity_working(&mut self, entity: &SceneEntityData) -> MemoryEntryId {
        let id = self.working.entries.len() as u64 + 1000;
        let entry = WorkingMemoryEntry {
            entry_type: WorkingEntryType::EntityReference,
            content: format!(
                "{} (id={}, components={})",
                entity.name,
                entity.id,
                entity
                    .components
                    .iter()
                    .map(|c| c.type_name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            metadata: MemoryMetadata::new(id, MemoryTier::Working),
            source_message: None,
            entity_id: Some(crate::types::EntityId(entity.id)),
            value_json: Some(serde_json::json!(entity)),
            ttl_seconds: 3600,
        };
        self.working.entries.push(entry);
        MemoryEntryId(id)
    }

    pub fn inject_active_scene(&mut self, entities: &[SceneEntityData]) -> usize {
        let mut count = 0;
        for entity in entities {
            self.inject_entity_working(entity);
            count += 1;
        }
        self.set_value(
            "active_scene_entity_count",
            serde_json::json!(entities.len()),
        );
        self.add_hint(&format!("Active scene has {} entities", entities.len()));
        count
    }

    // ---- Episodic Memory (L2) ----

    pub fn record_scene_event(&mut self, event: &SceneEventData) -> MemoryEntryId {
        let (episode_type, summary) = match event.event_kind {
            SceneEventKind::EntityCreated => {
                let name = event
                    .entity_after
                    .as_ref()
                    .map(|e| e.name.as_str())
                    .unwrap_or("unknown");
                (
                    EpisodeType::StateChanged,
                    format!("Entity created: {}", name),
                )
            }
            SceneEventKind::EntityUpdated => {
                let name = event
                    .entity_before
                    .as_ref()
                    .or(event.entity_after.as_ref())
                    .map(|e| e.name.as_str())
                    .unwrap_or("unknown");
                (
                    EpisodeType::StateChanged,
                    format!("Entity updated: {}", name),
                )
            }
            SceneEventKind::EntityDeleted => {
                let name = event
                    .entity_before
                    .as_ref()
                    .map(|e| e.name.as_str())
                    .unwrap_or("unknown");
                (
                    EpisodeType::StateChanged,
                    format!("Entity deleted: {}", name),
                )
            }
            SceneEventKind::ComponentAdded
            | SceneEventKind::ComponentUpdated
            | SceneEventKind::ComponentRemoved => {
                let comp = event
                    .component
                    .as_ref()
                    .map(|c| c.type_name.as_str())
                    .unwrap_or("unknown");
                (
                    EpisodeType::StateChanged,
                    format!(
                        "Component {}: {}",
                        match event.event_kind {
                            SceneEventKind::ComponentAdded => "added",
                            SceneEventKind::ComponentRemoved => "removed",
                            _ => "updated",
                        },
                        comp
                    ),
                )
            }
        };

        let details = serde_json::to_value(event).unwrap_or_default();
        self.record_episode(Episode {
            metadata: MemoryMetadata::new(
                self.episodic.episodes.len() as u64 + 2000,
                MemoryTier::Episodic,
            ),
            episode_type,
            summary,
            details,
            entity_ids: vec![event.entity_id],
            success: Some(true),
            duration_ms: None,
        })
    }

    pub fn record_scene_diff(&mut self, diff: &SceneDiff) -> Vec<MemoryEntryId> {
        let mut ids = Vec::new();
        for entity in &diff.added {
            let event = SceneEventData {
                event_kind: SceneEventKind::EntityCreated,
                entity_id: entity.id,
                entity_before: None,
                entity_after: Some(entity.clone()),
                component: None,
                timestamp: chrono::Utc::now().to_rfc3339(),
            };
            ids.push(self.record_scene_event(&event));
        }
        for entity in &diff.removed {
            let event = SceneEventData {
                event_kind: SceneEventKind::EntityDeleted,
                entity_id: entity.id,
                entity_before: Some(entity.clone()),
                entity_after: None,
                component: None,
                timestamp: chrono::Utc::now().to_rfc3339(),
            };
            ids.push(self.record_scene_event(&event));
        }
        for change in &diff.modified {
            let event = SceneEventData {
                event_kind: SceneEventKind::EntityUpdated,
                entity_id: change.entity_id,
                entity_before: change.before.clone(),
                entity_after: change.after.clone(),
                component: None,
                timestamp: chrono::Utc::now().to_rfc3339(),
            };
            ids.push(self.record_scene_event(&event));
        }
        ids
    }

    // ---- Semantic Memory (L1) ----

    pub fn build_entity_semantic_graph(&mut self, entities: &[SceneEntityData]) -> usize {
        let mut count = 0;

        for entity in entities {
            let node = SemanticNode::new(
                self.semantic.nodes.len() as u64 + 3000,
                &entity.name,
                "entity",
                format!("Scene entity with {} components", entity.components.len()),
            );
            let node_id = self.add_semantic_node(node);
            count += 1;

            for component in &entity.components {
                let comp_node = SemanticNode::new(
                    self.semantic.nodes.len() as u64 + 4000,
                    &component.type_name,
                    "component",
                    format!("Component of {}", entity.name),
                );
                let comp_id = self.add_semantic_node(comp_node);
                self.add_relation(
                    node_id,
                    comp_id,
                    RelationType::HasA,
                    0.8,
                    format!("{} has component {}", entity.name, component.type_name),
                );
                count += 1;
            }
        }

        count
    }

    pub fn query_scene_entities(&mut self, query: &str) -> Vec<String> {
        let memory_query = MemoryQuery {
            text: query.to_string(),
            max_results: 10,
            include_working: true,
            include_episodic: false,
            include_semantic: true,
            include_procedural: false,
        };
        let results = self.retrieve(&memory_query);
        results
            .iter()
            .filter_map(|r| {
                if r.tier == MemoryTier::Semantic || r.tier == MemoryTier::Working {
                    Some(r.content.clone())
                } else {
                    None
                }
            })
            .collect()
    }

    // ---- Procedural Memory (L0) ----

    pub fn learn_scene_workflow(
        &mut self,
        name: &str,
        trigger: &str,
        _steps: Vec<crate::memory::WorkflowStep>,
    ) -> MemoryEntryId {
        self.create_workflow(name, trigger, "scene_manipulation")
    }

    pub fn auto_learn_from_scene_events(&mut self) -> usize {
        self.auto_learn(0.6, 3)
    }
}

// ============================================================================
// SceneMemoryStats
// ============================================================================

#[derive(Debug, Clone)]
pub struct SceneMemoryStats {
    pub active_entities: usize,
    pub recorded_events: usize,
    pub semantic_nodes: usize,
    pub learned_workflows: usize,
    pub snapshots_stored: usize,
}

impl MemorySystem {
    pub fn scene_memory_stats(&self, snapshot_store: &SceneSnapshotStore) -> SceneMemoryStats {
        let active_entities = self
            .working
            .entries
            .iter()
            .filter(|e| matches!(e.entry_type, WorkingEntryType::EntityReference))
            .count();
        let recorded_events = self
            .episodic
            .episodes
            .iter()
            .filter(|e| matches!(e.episode_type, EpisodeType::StateChanged))
            .count();
        let learned_workflows = self
            .procedural
            .workflows
            .iter()
            .filter(|w| w.category == "scene_manipulation")
            .count();

        SceneMemoryStats {
            active_entities,
            recorded_events,
            semantic_nodes: self.semantic.nodes.len(),
            learned_workflows,
            snapshots_stored: snapshot_store.len(),
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_entity(id: u64, name: &str) -> SceneEntityData {
        SceneEntityData {
            id,
            name: name.to_string(),
            components: vec![
                ComponentData {
                    type_name: "Transform".to_string(),
                    properties: HashMap::from([
                        ("x".to_string(), serde_json::json!(0.0)),
                        ("y".to_string(), serde_json::json!(1.0)),
                    ]),
                },
                ComponentData {
                    type_name: "Sprite".to_string(),
                    properties: HashMap::from([(
                        "color".to_string(),
                        serde_json::json!("#ff0000"),
                    )]),
                },
            ],
            position: Some([0.0, 1.0, 0.0]),
        }
    }

    // ---- S1.1: SceneMemoryEntry serialization ----

    #[test]
    fn test_scene_memory_entry_tier() {
        let entity = make_test_entity(1, "Player");
        let entry = SceneMemoryEntry::WorkingEntity {
            entity: entity.clone(),
            memory_id: MemoryEntryId(42),
        };
        assert_eq!(entry.tier(), MemoryTier::Working);
        assert_eq!(entry.memory_id(), MemoryEntryId(42));
    }

    #[test]
    fn test_scene_memory_entry_serialization() {
        let entity = make_test_entity(1, "Player");
        let entry = SceneMemoryEntry::WorkingEntity {
            entity,
            memory_id: MemoryEntryId(1),
        };
        let json = serde_json::to_string(&entry).unwrap();
        let parsed: SceneMemoryEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.memory_id(), MemoryEntryId(1));
        assert_eq!(parsed.tier(), MemoryTier::Working);
    }

    #[test]
    fn test_scene_event_data_serialization() {
        let event = SceneEventData {
            event_kind: SceneEventKind::EntityCreated,
            entity_id: 1,
            entity_before: None,
            entity_after: Some(make_test_entity(1, "Enemy")),
            component: None,
            timestamp: "2026-05-23T00:00:00Z".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        let parsed: SceneEventData = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.entity_id, 1);
        assert_eq!(parsed.event_kind, SceneEventKind::EntityCreated);
    }

    // ---- S1.2: Snapshot + diff ----

    #[test]
    fn test_snapshot_store_basic() {
        let mut store = SceneSnapshotStore::new(5);
        assert!(store.is_empty());

        let entities = vec![make_test_entity(1, "Player")];
        store.take_snapshot(&entities);
        assert_eq!(store.len(), 1);
        assert_eq!(store.current_version, 1);

        let latest = store.latest().unwrap();
        assert_eq!(latest.entities.len(), 1);
        assert_eq!(latest.version, 1);
    }

    #[test]
    fn test_snapshot_capacity_limit() {
        let mut store = SceneSnapshotStore::new(2);
        store.take_snapshot(&[make_test_entity(1, "A")]);
        store.take_snapshot(&[make_test_entity(2, "B")]);
        store.take_snapshot(&[make_test_entity(3, "C")]);
        assert_eq!(store.len(), 2);
        assert_eq!(store.snapshots[0].version, 2);
        assert_eq!(store.snapshots[1].version, 3);
    }

    #[test]
    fn test_scene_diff_added() {
        let mut store = SceneSnapshotStore::new(5);
        store.take_snapshot(&[make_test_entity(1, "A")]);
        store.take_snapshot(&[make_test_entity(1, "A"), make_test_entity(2, "B")]);

        let diff = store.diff(1, 2).unwrap();
        assert_eq!(diff.added.len(), 1);
        assert_eq!(diff.added[0].name, "B");
        assert!(diff.removed.is_empty());
    }

    #[test]
    fn test_scene_diff_removed() {
        let mut store = SceneSnapshotStore::new(5);
        store.take_snapshot(&[make_test_entity(1, "A"), make_test_entity(2, "B")]);
        store.take_snapshot(&[make_test_entity(1, "A")]);

        let diff = store.diff(1, 2).unwrap();
        assert_eq!(diff.removed.len(), 1);
        assert_eq!(diff.removed[0].name, "B");
    }

    // ---- S1.3: Working Memory injection ----

    #[test]
    fn test_inject_entity_working() {
        let mut ms = MemorySystem::new();
        let entity = make_test_entity(1, "Player");
        let id = ms.inject_entity_working(&entity);
        assert!(id.0 > 0);

        let entity_entries: Vec<_> = ms
            .working
            .entries
            .iter()
            .filter(|e| matches!(e.entry_type, WorkingEntryType::EntityReference))
            .collect();
        assert_eq!(entity_entries.len(), 1);
        assert!(entity_entries[0].content.contains("Player"));
    }

    #[test]
    fn test_inject_active_scene() {
        let mut ms = MemorySystem::new();
        let entities = vec![
            make_test_entity(1, "Player"),
            make_test_entity(2, "Enemy"),
            make_test_entity(3, "Coin"),
        ];
        let count = ms.inject_active_scene(&entities);
        assert_eq!(count, 3);

        let entity_entries: Vec<_> = ms
            .working
            .entries
            .iter()
            .filter(|e| matches!(e.entry_type, WorkingEntryType::EntityReference))
            .collect();
        assert_eq!(entity_entries.len(), 3);
    }

    // ---- S1.4: Episodic event recording ----

    #[test]
    fn test_record_scene_event_created() {
        let mut ms = MemorySystem::new();
        let event = SceneEventData {
            event_kind: SceneEventKind::EntityCreated,
            entity_id: 10,
            entity_before: None,
            entity_after: Some(make_test_entity(10, "Spawned")),
            component: None,
            timestamp: "now".to_string(),
        };
        let id = ms.record_scene_event(&event);
        assert!(id.0 > 0);

        let scene_episodes: Vec<_> = ms
            .episodic
            .episodes
            .iter()
            .filter(|e| matches!(e.episode_type, EpisodeType::StateChanged))
            .collect();
        assert_eq!(scene_episodes.len(), 1);
        assert!(scene_episodes[0].summary.contains("Spawned"));
    }

    #[test]
    fn test_record_scene_diff() {
        let mut ms = MemorySystem::new();
        let diff = SceneDiff {
            added: vec![make_test_entity(1, "New")],
            removed: vec![make_test_entity(2, "Old")],
            modified: vec![],
        };
        let ids = ms.record_scene_diff(&diff);
        assert_eq!(ids.len(), 2);
    }

    // ---- S1.5: Semantic graph building ----

    #[test]
    fn test_build_entity_semantic_graph() {
        let mut ms = MemorySystem::new();
        let entities = vec![make_test_entity(1, "Player")];
        let count = ms.build_entity_semantic_graph(&entities);
        // 1 entity + 2 components = 3 nodes, 2 relations
        assert_eq!(count, 3);
        assert!(ms.semantic.nodes.len() >= 2);
    }

    #[test]
    fn test_query_scene_entities() {
        let mut ms = MemorySystem::new();
        let entities = vec![make_test_entity(1, "Player")];
        ms.inject_active_scene(&entities);

        let results = ms.query_scene_entities("Player");
        assert!(!results.is_empty());
    }

    // ---- S1.6: Procedural workflow learning ----

    #[test]
    fn test_learn_scene_workflow() {
        let mut ms = MemorySystem::new();
        let steps = vec![crate::memory::WorkflowStep {
            tool_name: "create_entity".to_string(),
            description: "Create a new entity".to_string(),
            parameters: None,
        }];
        let id = ms.learn_scene_workflow("Spawn Enemy", "spawn enemy at position", steps);
        assert!(id.0 > 0);

        let scene_wfs: Vec<_> = ms
            .procedural
            .workflows
            .iter()
            .filter(|w| w.category == "scene_manipulation")
            .collect();
        assert_eq!(scene_wfs.len(), 1);
    }

    // ---- S1.8: SceneMemoryStats ----

    #[test]
    fn test_scene_memory_stats() {
        let mut ms = MemorySystem::new();
        let store = SceneSnapshotStore::new(5);

        ms.inject_active_scene(&[make_test_entity(1, "Test")]);
        let stats = ms.scene_memory_stats(&store);
        assert_eq!(stats.active_entities, 1);
        assert_eq!(stats.snapshots_stored, 0);
    }

    #[test]
    fn test_full_integration_flow() {
        let mut ms = MemorySystem::new();
        let mut store = SceneSnapshotStore::new(5);

        let entities = vec![make_test_entity(1, "Player"), make_test_entity(2, "Enemy")];

        // Inject into working memory
        ms.inject_active_scene(&entities);

        // Build semantic graph
        ms.build_entity_semantic_graph(&entities);

        // Take snapshot
        store.take_snapshot(&entities);

        // Simulate scene change
        let new_entities = vec![
            make_test_entity(1, "Player"),
            make_test_entity(2, "Enemy"),
            make_test_entity(3, "Coin"),
        ];
        store.take_snapshot(&new_entities);

        // Record diff
        if let Some(diff) = store.diff(1, 2) {
            ms.record_scene_diff(&diff);
        }

        // Learn workflow
        ms.learn_scene_workflow("Add Entity", "add new entity to scene", vec![]);

        // Verify all tiers populated
        let stats = ms.scene_memory_stats(&store);
        assert_eq!(stats.active_entities, 2);
        assert!(stats.recorded_events > 0);
        assert!(stats.semantic_nodes > 0);
        assert_eq!(stats.learned_workflows, 1);
        assert_eq!(stats.snapshots_stored, 2);
    }
}
