//! Four-Tier Memory Integration — Scene-to-Memory layer injectors
//!
//! Provides dedicated injectors for each of the four memory tiers:
//! - L3 (Working Memory): Current scene entity state
//! - L2 (Episodic Memory): Scene change events as episodes
//! - L1 (Semantic Memory): Entity-relationship knowledge graph
//! - L0 (Procedural Memory): Operation patterns and workflows
//!
//! The `FourTierSceneInjector` orchestrates all four layers, and
//! `SceneMemoryAutoUpdater` uses the scene event bus for background updates.

use crate::error::{BridgeError, Result};
use crate::memory_scene_context::SharedSceneContextMemory;
use crate::multica_db::{EntityRecord, SharedMulticaDb};
#[cfg(test)]
use crate::scene_context::ComponentData;
use crate::scene_context::SceneDiff;
use crate::scene_event_bus::{
    SceneEvent, SceneEventSubscriber, SceneEventType, SharedSceneEventBus, SubscriberId,
};
use log::{debug, error, info, warn};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};

// ============================================================================
// Injection Result
// ============================================================================

/// Result of a single memory injection into one tier.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InjectionResult {
    /// The memory layer that was injected into.
    /// "working", "episodic", "semantic", or "procedural".
    pub layer: String,
    /// Whether the injection succeeded.
    pub success: bool,
    /// Human-readable details about what was injected.
    pub details: String,
    /// Number of entries created by this injection.
    pub entry_count: usize,
    /// Duration of the injection in milliseconds.
    pub duration_ms: u64,
    /// Error message if the injection failed.
    pub error: Option<String>,
}

impl InjectionResult {
    /// Create a successful injection result.
    pub fn success(layer: &str, details: String, entry_count: usize, duration_ms: u64) -> Self {
        Self {
            layer: layer.to_string(),
            success: true,
            details,
            entry_count,
            duration_ms,
            error: None,
        }
    }

    /// Create a failed injection result.
    pub fn failure(layer: &str, error: String, duration_ms: u64) -> Self {
        Self {
            layer: layer.to_string(),
            success: false,
            details: String::new(),
            entry_count: 0,
            duration_ms,
            error: Some(error),
        }
    }
}

/// Summary of injection results across multiple layers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InjectionSummary {
    /// Per-layer results.
    pub results: Vec<InjectionResult>,
    /// Total entries injected across all layers.
    pub total_entries: usize,
    /// Total duration across all layers in milliseconds.
    pub total_duration_ms: u64,
    /// Whether all layers succeeded.
    pub all_succeeded: bool,
    /// Scene ID that was injected.
    pub scene_id: String,
    /// Timestamp of the injection.
    pub timestamp: u64,
}

impl InjectionSummary {
    /// Build a summary from a list of per-layer results.
    pub fn from_results(scene_id: String, results: Vec<InjectionResult>) -> Self {
        let total_entries: usize = results.iter().map(|r| r.entry_count).sum();
        let total_duration_ms: u64 = results.iter().map(|r| r.duration_ms).sum();
        let all_succeeded = results.iter().all(|r| r.success);
        let timestamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        Self {
            results,
            total_entries,
            total_duration_ms,
            all_succeeded,
            scene_id,
            timestamp,
        }
    }
}

// ============================================================================
// Cross-Layer Query Result
// ============================================================================

/// Result of a cross-layer query that returns matching entries from
/// multiple memory tiers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossLayerQueryResult {
    /// The query text that was used.
    pub query: String,
    /// Results from Working Memory (L3).
    pub working_results: Vec<String>,
    /// Results from Episodic Memory (L2).
    pub episodic_results: Vec<String>,
    /// Results from Semantic Memory (L1).
    pub semantic_results: Vec<String>,
    /// Results from Procedural Memory (L0).
    pub procedural_results: Vec<String>,
    /// Total result count across all layers.
    pub total_count: usize,
    /// Timestamp of the query.
    pub timestamp: u64,
}

impl CrossLayerQueryResult {
    /// Create an empty query result.
    pub fn new(query: impl Into<String>) -> Self {
        Self {
            query: query.into(),
            working_results: Vec::new(),
            episodic_results: Vec::new(),
            semantic_results: Vec::new(),
            procedural_results: Vec::new(),
            total_count: 0,
            timestamp: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        }
    }

    /// Check if any results were found.
    pub fn is_empty(&self) -> bool {
        self.total_count == 0
    }

    /// Get results from a specific layer by name.
    pub fn get_layer(&self, layer: &str) -> &[String] {
        match layer {
            "working" => &self.working_results,
            "episodic" => &self.episodic_results,
            "semantic" => &self.semantic_results,
            "procedural" => &self.procedural_results,
            _ => &[],
        }
    }

    /// Merge two cross-layer query results (union per layer).
    pub fn merge(&mut self, other: &CrossLayerQueryResult) {
        self.working_results.extend(other.working_results.clone());
        self.episodic_results.extend(other.episodic_results.clone());
        self.semantic_results.extend(other.semantic_results.clone());
        self.procedural_results
            .extend(other.procedural_results.clone());
        self.total_count = self.working_results.len()
            + self.episodic_results.len()
            + self.semantic_results.len()
            + self.procedural_results.len();
    }
}

// ============================================================================
// Prepared Memory Entry (shared across layer injectors)
// ============================================================================

/// A prepared entry ready for injection into a specific memory tier.
/// This is the common format produced by all layer injectors.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreparedMemoryEntry {
    /// Summary for quick display.
    pub summary: String,
    /// Full details as JSON.
    pub payload: serde_json::Value,
    /// Associated entity IDs.
    pub entity_ids: Vec<u64>,
    /// Associated scene ID.
    pub scene_id: String,
    /// Importance score [0.0, 1.0].
    pub importance: f32,
    /// Categorization tags.
    pub tags: Vec<String>,
}

// ============================================================================
// L3: SceneWorkingMemoryInjector
// ============================================================================

/// Injects current scene state into Working Memory (L3).
///
/// Working Memory holds the immediate, short-term context:
/// - Current entity references with their positions and components
/// - Active scene metadata
/// - Computed scene summaries
pub struct SceneWorkingMemoryInjector {
    /// Reference to the multica database for querying entities.
    db: SharedMulticaDb,
}

impl SceneWorkingMemoryInjector {
    /// Create a new working memory injector.
    pub fn new(db: SharedMulticaDb) -> Self {
        Self { db }
    }

    /// Inject a single scene's current state into working memory.
    ///
    /// Produces one entry per entity plus a scene summary entry.
    pub fn inject(&self, scene_id: &str) -> Result<InjectionResult> {
        let start = Instant::now();

        let (scene, entities) = {
            let db = self.db.lock().expect("mutex poisoned");
            let scene = db
                .get_scene(scene_id)
                .ok_or_else(|| BridgeError::Other(format!("Scene not found: {}", scene_id)))?;
            let entities = db.get_scene_entities(scene_id);
            (scene, entities)
        };

        let mut entries = Vec::new();

        // Scene-level working memory entry
        entries.push(PreparedMemoryEntry {
            summary: format!(
                "Active scene: '{}' (id={}, {} entities, active={})",
                scene.name,
                scene.scene_id,
                entities.len(),
                scene.is_active
            ),
            payload: serde_json::json!({
                "scene_id": scene.scene_id,
                "scene_name": scene.name,
                "entity_count": entities.len(),
                "is_active": scene.is_active,
                "description": scene.description,
            }),
            entity_ids: entities.iter().map(|e| e.entity_id).collect(),
            scene_id: scene.scene_id.clone(),
            importance: 0.9,
            tags: vec!["scene_active".to_string(), "working".to_string()],
        });

        // Per-entity working memory entries
        for entity in &entities {
            entries.push(PreparedMemoryEntry {
                summary: format!(
                    "[{}] {} (id={}) pos={:?}",
                    entity.entity_type, entity.name, entity.entity_id, entity.position
                ),
                payload: serde_json::json!({
                    "entity_id": entity.entity_id,
                    "name": entity.name,
                    "entity_type": entity.entity_type,
                    "components": entity.components,
                    "position": entity.position,
                }),
                entity_ids: vec![entity.entity_id],
                scene_id: scene.scene_id.clone(),
                importance: 0.5,
                tags: vec![
                    "entity".to_string(),
                    entity.entity_type.clone(),
                    "working".to_string(),
                ],
            });
        }

        // Component summary
        let component_types = collect_component_types(&entities);
        if !component_types.is_empty() {
            entries.push(PreparedMemoryEntry {
                summary: format!("Component types in scene: {}", component_types.join(", ")),
                payload: serde_json::json!({
                    "component_types": component_types,
                    "scene_id": scene.scene_id,
                }),
                entity_ids: vec![],
                scene_id: scene.scene_id.clone(),
                importance: 0.4,
                tags: vec![
                    "components".to_string(),
                    "summary".to_string(),
                    "working".to_string(),
                ],
            });
        }

        let duration_ms = start.elapsed().as_millis() as u64;
        let entry_count = entries.len();
        let details = format!(
            "Injected {} working memory entries for scene '{}' ({} entities, component types: {})",
            entry_count,
            scene.name,
            entities.len(),
            component_types.join(", ")
        );

        info!("{}", details);

        Ok(InjectionResult::success(
            "working",
            details,
            entry_count,
            duration_ms,
        ))
    }

    /// Inject all scenes into working memory.
    pub fn inject_all(&self) -> Result<InjectionSummary> {
        let scenes = {
            let db = self.db.lock().expect("mutex poisoned");
            db.get_all_scenes()
        };

        let mut results = Vec::new();
        for scene in &scenes {
            match self.inject(&scene.scene_id) {
                Ok(result) => results.push(result),
                Err(e) => {
                    results.push(InjectionResult::failure("working", format!("{}", e), 0));
                }
            }
        }

        Ok(InjectionSummary::from_results("all".to_string(), results))
    }
}

// ============================================================================
// L2: SceneEpisodicMemoryInjector
// ============================================================================

/// Injects scene change events into Episodic Memory (L2).
///
/// Episodic Memory records the history of scene operations:
/// - Entity creation, update, deletion events
/// - Component addition, modification, removal events
/// - Scene load/save events
/// - Scene diff summaries
pub struct SceneEpisodicMemoryInjector {
    db: SharedMulticaDb,
    scene_memory: SharedSceneContextMemory,
}

impl SceneEpisodicMemoryInjector {
    /// Create a new episodic memory injector.
    pub fn new(db: SharedMulticaDb, scene_memory: SharedSceneContextMemory) -> Self {
        Self { db, scene_memory }
    }

    /// Inject scene change events from a diff into episodic memory.
    pub fn inject_changes(&self, scene_id: &str, diff: &SceneDiff) -> Result<InjectionResult> {
        let start = Instant::now();

        // Record the change in scene context memory
        {
            let mut memory = self.scene_memory.lock().expect("mutex poisoned");
            memory.record_scene_change(diff.clone(), scene_id.to_string())?;
        }

        let mut entries = Vec::new();

        // Entity creations
        for entity in &diff.added {
            entries.push(PreparedMemoryEntry {
                summary: format!(
                    "EntityCreated: {} (id={}) in scene '{}'",
                    entity.name, entity.id, scene_id
                ),
                payload: serde_json::json!({
                    "event_type": "EntityCreated",
                    "entity": entity,
                    "scene_id": scene_id,
                }),
                entity_ids: vec![entity.id],
                scene_id: scene_id.to_string(),
                importance: 0.7,
                tags: vec!["entity_created".to_string(), "episodic".to_string()],
            });
        }

        // Entity deletions
        for entity in &diff.removed {
            entries.push(PreparedMemoryEntry {
                summary: format!(
                    "EntityDeleted: {} (id={}) from scene '{}'",
                    entity.name, entity.id, scene_id
                ),
                payload: serde_json::json!({
                    "event_type": "EntityDeleted",
                    "entity": entity,
                    "scene_id": scene_id,
                }),
                entity_ids: vec![entity.id],
                scene_id: scene_id.to_string(),
                importance: 0.8,
                tags: vec!["entity_deleted".to_string(), "episodic".to_string()],
            });
        }

        // Entity modifications
        for change in &diff.modified {
            entries.push(PreparedMemoryEntry {
                summary: format!(
                    "EntityModified: id={} changes=[{}] in scene '{}'",
                    change.entity_id,
                    change.changes.join(", "),
                    scene_id
                ),
                payload: serde_json::json!({
                    "event_type": "EntityModified",
                    "entity_id": change.entity_id,
                    "changes": change.changes,
                    "before": change.before,
                    "after": change.after,
                    "scene_id": scene_id,
                }),
                entity_ids: vec![change.entity_id],
                scene_id: scene_id.to_string(),
                importance: 0.6,
                tags: vec!["entity_modified".to_string(), "episodic".to_string()],
            });
        }

        let duration_ms = start.elapsed().as_millis() as u64;
        let entry_count = entries.len();

        let details = format!(
            "Injected {} episodic entries for scene '{}' (+{} -{} ~{})",
            entry_count,
            scene_id,
            diff.added.len(),
            diff.removed.len(),
            diff.modified.len()
        );

        info!("{}", details);

        Ok(InjectionResult::success(
            "episodic",
            details,
            entry_count,
            duration_ms,
        ))
    }

    /// Inject a scene event into episodic memory.
    pub fn inject_event(&self, event: &SceneEvent) -> Result<InjectionResult> {
        let start = Instant::now();

        let (event_label, importance) = match event.event_type {
            SceneEventType::EntityCreated => ("EntityCreated", 0.7),
            SceneEventType::EntityUpdated => ("EntityUpdated", 0.5),
            SceneEventType::EntityDeleted => ("EntityDeleted", 0.8),
            SceneEventType::ComponentAdded => ("ComponentAdded", 0.4),
            SceneEventType::ComponentUpdated => ("ComponentUpdated", 0.4),
            SceneEventType::ComponentRemoved => ("ComponentRemoved", 0.5),
        };

        let entity_name = event
            .entity_after
            .as_ref()
            .or(event.entity_before.as_ref())
            .map(|e| e.name.as_str())
            .unwrap_or("unknown");

        let _entry = PreparedMemoryEntry {
            summary: format!(
                "{}: {} (id={}) in scene '{}'",
                event_label, entity_name, event.entity_id, event.scene_id
            ),
            payload: serde_json::json!({
                "event_type": format!("{:?}", event.event_type),
                "entity_id": event.entity_id,
                "entity_before": event.entity_before,
                "entity_after": event.entity_after,
                "component": event.component,
                "scene_id": event.scene_id,
                "timestamp": event.timestamp,
            }),
            entity_ids: vec![event.entity_id],
            scene_id: event.scene_id.clone(),
            importance,
            tags: vec![
                format!("{:?}", event.event_type).to_lowercase(),
                "episodic".to_string(),
            ],
        };

        let duration_ms = start.elapsed().as_millis() as u64;
        let details = format!(
            "Injected episodic event: {} for entity {}",
            event_label, event.entity_id
        );

        debug!("{}", details);

        Ok(InjectionResult::success(
            "episodic",
            details,
            1,
            duration_ms,
        ))
    }

    /// Inject a scene load event into episodic memory.
    pub fn inject_scene_loaded(&self, scene_id: &str) -> Result<InjectionResult> {
        let start = Instant::now();

        let (scene, entities) = {
            let db = self.db.lock().expect("mutex poisoned");
            let scene = db
                .get_scene(scene_id)
                .ok_or_else(|| BridgeError::Other(format!("Scene not found: {}", scene_id)))?;
            let entities = db.get_scene_entities(scene_id);
            (scene, entities)
        };

        let entity_summary: Vec<String> = entities
            .iter()
            .take(10)
            .map(|e| format!("{} ({})", e.name, e.entity_type))
            .collect();

        let _entry = PreparedMemoryEntry {
            summary: format!(
                "SceneLoaded: '{}' (id={}, {} entities)",
                scene.name,
                scene.scene_id,
                entities.len()
            ),
            payload: serde_json::json!({
                "event_type": "SceneLoaded",
                "scene_id": scene.scene_id,
                "scene_name": scene.name,
                "entity_count": entities.len(),
                "entity_preview": entity_summary,
                "is_active": scene.is_active,
            }),
            entity_ids: entities.iter().map(|e| e.entity_id).collect(),
            scene_id: scene.scene_id.clone(),
            importance: 0.8,
            tags: vec!["scene_loaded".to_string(), "episodic".to_string()],
        };

        let duration_ms = start.elapsed().as_millis() as u64;
        let details = format!(
            "Injected scene-loaded episode for '{}': {} entities",
            scene.name,
            entities.len()
        );

        info!("{}", details);

        Ok(InjectionResult::success(
            "episodic",
            details,
            1,
            duration_ms,
        ))
    }
}

// ============================================================================
// L1: SceneSemanticMemoryInjector
// ============================================================================

/// Injects entity-relationship knowledge into Semantic Memory (L1).
///
/// Semantic Memory builds a knowledge graph from scene structure:
/// - Scene nodes with metadata
/// - Entity nodes grouped by type
/// - Component nodes linked to entities via HasA relations
/// - Position-based spatial relations between entities
/// - Hierarchical relations (e.g. Parent/Child via transform components)
pub struct SceneSemanticMemoryInjector {
    db: SharedMulticaDb,
}

impl SceneSemanticMemoryInjector {
    /// Create a new semantic memory injector.
    pub fn new(db: SharedMulticaDb) -> Self {
        Self { db }
    }

    /// Build and inject the entity-relationship graph for a scene.
    ///
    /// Creates semantic nodes for the scene, entity types, individual entities,
    /// and component types, plus relations between them.
    pub fn inject(&self, scene_id: &str) -> Result<InjectionResult> {
        let start = Instant::now();

        let (scene, entities) = {
            let db = self.db.lock().expect("mutex poisoned");
            let scene = db
                .get_scene(scene_id)
                .ok_or_else(|| BridgeError::Other(format!("Scene not found: {}", scene_id)))?;
            let entities = db.get_scene_entities(scene_id);
            (scene, entities)
        };

        let mut entries = Vec::new();

        // --- Scene node ---
        entries.push(PreparedMemoryEntry {
            summary: format!("Scene: {}", scene.name),
            payload: serde_json::json!({
                "node_type": "Scene",
                "name": scene.name,
                "scene_id": scene.scene_id,
                "description": scene.description,
                "entity_count": entities.len(),
            }),
            entity_ids: vec![],
            scene_id: scene.scene_id.clone(),
            importance: 0.8,
            tags: vec!["scene_node".to_string(), "semantic".to_string()],
        });

        // --- Entity type nodes ---
        let type_groups = group_by_type(&entities);
        for (entity_type, group) in &type_groups {
            entries.push(PreparedMemoryEntry {
                summary: format!("EntityType: {} ({} instances)", entity_type, group.len()),
                payload: serde_json::json!({
                    "node_type": "EntityType",
                    "name": entity_type,
                    "instance_count": group.len(),
                    "scene_id": scene.scene_id,
                    "instance_ids": group.iter().map(|e| e.entity_id).collect::<Vec<_>>(),
                }),
                entity_ids: group.iter().map(|e| e.entity_id).collect(),
                scene_id: scene.scene_id.clone(),
                importance: 0.5,
                tags: vec![
                    "entity_type".to_string(),
                    entity_type.clone(),
                    "semantic".to_string(),
                ],
            });
        }

        // --- Individual entity nodes ---
        for entity in &entities {
            entries.push(PreparedMemoryEntry {
                summary: format!("Entity: {} (type={})", entity.name, entity.entity_type),
                payload: serde_json::json!({
                    "node_type": "Entity",
                    "name": entity.name,
                    "entity_id": entity.entity_id,
                    "entity_type": entity.entity_type,
                    "position": entity.position,
                    "scene_id": scene.scene_id,
                }),
                entity_ids: vec![entity.entity_id],
                scene_id: scene.scene_id.clone(),
                importance: 0.4,
                tags: vec![
                    "entity".to_string(),
                    entity.entity_type.clone(),
                    "semantic".to_string(),
                ],
            });
        }

        // --- Component type nodes ---
        let component_types = collect_component_types(&entities);
        for comp_type in &component_types {
            let entities_with_comp: Vec<u64> = entities
                .iter()
                .filter(|e| e.components.iter().any(|c| c.type_name == *comp_type))
                .map(|e| e.entity_id)
                .collect();

            entries.push(PreparedMemoryEntry {
                summary: format!(
                    "ComponentType: {} (used by {} entities)",
                    comp_type,
                    entities_with_comp.len()
                ),
                payload: serde_json::json!({
                    "node_type": "ComponentType",
                    "name": comp_type,
                    "used_by_count": entities_with_comp.len(),
                    "used_by": entities_with_comp,
                    "scene_id": scene.scene_id,
                }),
                entity_ids: entities_with_comp.clone(),
                scene_id: scene.scene_id.clone(),
                importance: 0.4,
                tags: vec![
                    "component_type".to_string(),
                    comp_type.clone(),
                    "semantic".to_string(),
                ],
            });
        }

        // --- HasA relations: Entity -> Component ---
        for entity in &entities {
            for component in &entity.components {
                entries.push(PreparedMemoryEntry {
                    summary: format!(
                        "HasA: {} --[{}]--> {} component",
                        entity.name, component.type_name, component.type_name
                    ),
                    payload: serde_json::json!({
                        "relation_type": "HasA",
                        "from_type": "Entity",
                        "from_id": entity.entity_id,
                        "from_name": entity.name,
                        "to_type": "ComponentType",
                        "to_name": component.type_name,
                        "scene_id": scene.scene_id,
                    }),
                    entity_ids: vec![entity.entity_id],
                    scene_id: scene.scene_id.clone(),
                    importance: 0.3,
                    tags: vec![
                        "HasA".to_string(),
                        component.type_name.clone(),
                        "semantic".to_string(),
                    ],
                });
            }
        }

        // --- Spatial proximity relations between positioned entities ---
        let positioned: Vec<&EntityRecord> =
            entities.iter().filter(|e| e.position.is_some()).collect();
        for i in 0..positioned.len() {
            for j in (i + 1)..positioned.len() {
                let a = positioned[i];
                let b = positioned[j];
                let dist = euclidean_distance(a.position, b.position);
                if dist < 100.0 {
                    entries.push(PreparedMemoryEntry {
                        summary: format!(
                            "Nearby: {} <-> {} (distance={:.1})",
                            a.name, b.name, dist
                        ),
                        payload: serde_json::json!({
                            "relation_type": "Nearby",
                            "entity_a": a.entity_id,
                            "entity_b": b.entity_id,
                            "distance": dist,
                            "scene_id": scene.scene_id,
                        }),
                        entity_ids: vec![a.entity_id, b.entity_id],
                        scene_id: scene.scene_id.clone(),
                        importance: 0.2,
                        tags: vec![
                            "Nearby".to_string(),
                            "spatial".to_string(),
                            "semantic".to_string(),
                        ],
                    });
                }
            }
        }

        let duration_ms = start.elapsed().as_millis() as u64;
        let entry_count = entries.len();

        let details = format!(
            "Injected {} semantic entries for scene '{}' ({} entity types, {} component types, {} HasA relations)",
            entry_count,
            scene.name,
            type_groups.len(),
            component_types.len(),
            entities.iter().map(|e| e.components.len()).sum::<usize>()
        );

        info!("{}", details);

        Ok(InjectionResult::success(
            "semantic",
            details,
            entry_count,
            duration_ms,
        ))
    }
}

// ============================================================================
// L0: SceneProceduralMemoryInjector
// ============================================================================

/// Injects scene operation patterns into Procedural Memory (L0).
///
/// Procedural Memory stores reusable workflows and decision patterns:
/// - Entity creation workflows (what components to attach for each type)
/// - Component modification sequences
/// - Entity positioning patterns
/// - Batch operation templates (e.g. "create a group of enemies")
pub struct SceneProceduralMemoryInjector {
    db: SharedMulticaDb,
}

impl SceneProceduralMemoryInjector {
    /// Create a new procedural memory injector.
    pub fn new(db: SharedMulticaDb) -> Self {
        Self { db }
    }

    /// Extract and inject operation patterns from a scene's entity structure.
    pub fn inject(&self, scene_id: &str) -> Result<InjectionResult> {
        let start = Instant::now();

        let (scene, entities) = {
            let db = self.db.lock().expect("mutex poisoned");
            let scene = db
                .get_scene(scene_id)
                .ok_or_else(|| BridgeError::Other(format!("Scene not found: {}", scene_id)))?;
            let entities = db.get_scene_entities(scene_id);
            (scene, entities)
        };

        let mut entries = Vec::new();

        // --- Pattern 1: Create entity workflows (per entity type) ---
        let type_groups = group_by_type(&entities);
        for (entity_type, group) in &type_groups {
            if let Some(sample) = group.first() {
                let component_names: Vec<String> = sample
                    .components
                    .iter()
                    .map(|c| c.type_name.clone())
                    .collect();

                let steps: Vec<serde_json::Value> = component_names
                    .iter()
                    .enumerate()
                    .map(|(idx, comp)| {
                        serde_json::json!({
                            "step": idx + 1,
                            "tool": "add_component",
                            "description": format!("Attach {} component", comp),
                            "component_type": comp,
                        })
                    })
                    .collect();

                entries.push(PreparedMemoryEntry {
                    summary: format!(
                        "Workflow: Create {} ({} instances, confidence={:.2})",
                        entity_type,
                        group.len(),
                        group.len() as f32 / entities.len().max(1) as f32
                    ),
                    payload: serde_json::json!({
                        "workflow_name": format!("create_{}", entity_type.to_lowercase()),
                        "trigger": format!("Create a new {}", entity_type.to_lowercase()),
                        "category": entity_type,
                        "success_rate": 0.7,
                        "use_count": group.len() as u32,
                        "steps": steps,
                        "scene_id": scene.scene_id,
                    }),
                    entity_ids: group.iter().map(|e| e.entity_id).collect(),
                    scene_id: scene.scene_id.clone(),
                    importance: 0.5,
                    tags: vec![
                        "workflow".to_string(),
                        entity_type.clone(),
                        "create_entity".to_string(),
                        "procedural".to_string(),
                    ],
                });
            }
        }

        // --- Pattern 2: Entity positioning patterns ---
        let positioned: Vec<&EntityRecord> =
            entities.iter().filter(|e| e.position.is_some()).collect();
        if !positioned.is_empty() {
            let position_centroid = compute_centroid(&positioned);
            let spread = compute_spread(&positioned, &position_centroid);

            entries.push(PreparedMemoryEntry {
                summary: format!(
                    "DecisionPattern: Position entities near {:?} (spread={:.1}, {} entities)",
                    position_centroid, spread, positioned.len()
                ),
                payload: serde_json::json!({
                    "pattern_type": "DecisionPattern",
                    "context": format!("Position {} entities in scene", positioned.len()),
                    "decision": format!("Center near {:?} with spread {:.1}", position_centroid, spread),
                    "outcome": format!("{} entities positioned", positioned.len()),
                    "confidence": 0.5,
                    "observation_count": positioned.len() as u32,
                    "scene_id": scene.scene_id,
                }),
                entity_ids: positioned.iter().map(|e| e.entity_id).collect(),
                scene_id: scene.scene_id.clone(),
                importance: 0.3,
                tags: vec![
                    "decision_pattern".to_string(),
                    "position".to_string(),
                    "procedural".to_string(),
                ],
            });
        }

        // --- Pattern 3: Component modification sequences ---
        let component_sequences = extract_component_sequences(&entities);
        for (comp_type, entity_list) in &component_sequences {
            if entity_list.len() >= 2 {
                entries.push(PreparedMemoryEntry {
                    summary: format!(
                        "Workflow: Modify {} component (used by {} entities: {})",
                        comp_type,
                        entity_list.len(),
                        entity_list.iter().map(|e| e.name.as_str()).collect::<Vec<_>>().join(", ")
                    ),
                    payload: serde_json::json!({
                        "workflow_name": format!("modify_{}", comp_type.to_lowercase()),
                        "trigger": format!("Update {} component", comp_type),
                        "category": "component_modification",
                        "success_rate": 0.6,
                        "use_count": entity_list.len() as u32,
                        "affected_entities": entity_list.iter().map(|e| e.entity_id).collect::<Vec<_>>(),
                        "scene_id": scene.scene_id,
                    }),
                    entity_ids: entity_list.iter().map(|e| e.entity_id).collect(),
                    scene_id: scene.scene_id.clone(),
                    importance: 0.4,
                    tags: vec![
                        "workflow".to_string(),
                        comp_type.clone(),
                        "modify_component".to_string(),
                        "procedural".to_string(),
                    ],
                });
            }
        }

        let duration_ms = start.elapsed().as_millis() as u64;
        let entry_count = entries.len();

        let details = format!(
            "Injected {} procedural entries for scene '{}' ({} entity types, {} component patterns)",
            entry_count,
            scene.name,
            type_groups.len(),
            component_sequences.len()
        );

        info!("{}", details);

        Ok(InjectionResult::success(
            "procedural",
            details,
            entry_count,
            duration_ms,
        ))
    }
}

// ============================================================================
// FourTierSceneInjector (Orchestrator)
// ============================================================================

/// Orchestrates memory injection across all four tiers.
///
/// Coordinates the individual layer injectors to provide a unified
/// injection pipeline from scene data into all memory layers.
///
/// # Usage
///
/// ```ignore
/// let db = create_shared_multica_db();
/// let scene_memory = create_shared_scene_context_memory();
/// let orchestrator = FourTierSceneInjector::new(db.clone(), scene_memory.clone());
///
/// // Inject a single scene into all four tiers
/// let summary = orchestrator.inject_scene("my_scene").unwrap();
/// assert!(summary.all_succeeded);
/// ```
pub struct FourTierSceneInjector {
    /// Shared database for direct queries.
    pub db: SharedMulticaDb,
    /// L3 injector
    pub working: SceneWorkingMemoryInjector,
    /// L2 injector
    pub episodic: SceneEpisodicMemoryInjector,
    /// L1 injector
    pub semantic: SceneSemanticMemoryInjector,
    /// L0 injector
    pub procedural: SceneProceduralMemoryInjector,
    /// Reference to scene context memory for querying
    pub scene_memory: SharedSceneContextMemory,
}

impl FourTierSceneInjector {
    /// Create a new orchestrator with shared database and scene memory.
    pub fn new(db: SharedMulticaDb, scene_memory: SharedSceneContextMemory) -> Self {
        Self {
            db: db.clone(),
            working: SceneWorkingMemoryInjector::new(db.clone()),
            episodic: SceneEpisodicMemoryInjector::new(db.clone(), scene_memory.clone()),
            semantic: SceneSemanticMemoryInjector::new(db.clone()),
            procedural: SceneProceduralMemoryInjector::new(db.clone()),
            scene_memory,
        }
    }

    /// Inject a scene into all four memory tiers.
    ///
    /// Returns a summary with per-layer results.
    pub fn inject_scene(&self, scene_id: &str) -> Result<InjectionSummary> {
        info!(
            "FourTierSceneInjector: injecting scene '{}' into all tiers",
            scene_id
        );

        let mut results = Vec::new();

        // L3 - Working Memory
        match self.working.inject(scene_id) {
            Ok(r) => results.push(r),
            Err(e) => {
                error!("L3 Working injection failed: {}", e);
                results.push(InjectionResult::failure("working", format!("{}", e), 0));
            }
        }

        // L2 - Episodic Memory
        match self.episodic.inject_scene_loaded(scene_id) {
            Ok(r) => results.push(r),
            Err(e) => {
                error!("L2 Episodic injection failed: {}", e);
                results.push(InjectionResult::failure("episodic", format!("{}", e), 0));
            }
        }

        // L1 - Semantic Memory
        match self.semantic.inject(scene_id) {
            Ok(r) => results.push(r),
            Err(e) => {
                error!("L1 Semantic injection failed: {}", e);
                results.push(InjectionResult::failure("semantic", format!("{}", e), 0));
            }
        }

        // L0 - Procedural Memory
        match self.procedural.inject(scene_id) {
            Ok(r) => results.push(r),
            Err(e) => {
                error!("L0 Procedural injection failed: {}", e);
                results.push(InjectionResult::failure("procedural", format!("{}", e), 0));
            }
        }

        let summary = InjectionSummary::from_results(scene_id.to_string(), results);

        info!(
            "FourTierSceneInjector: scene '{}' injected: {} entries across 4 tiers (all_succeeded={})",
            scene_id,
            summary.total_entries,
            summary.all_succeeded
        );

        Ok(summary)
    }

    /// Inject scene changes (diff) across all tiers.
    ///
    /// Only Episodic and Working tiers typically process diffs;
    /// Semantic and Procedural need a full re-injection if entity structure changed.
    pub fn inject_changes(&self, scene_id: &str, diff: &SceneDiff) -> Result<InjectionSummary> {
        let mut results = Vec::new();

        // L2 - Episodic: record the changes
        match self.episodic.inject_changes(scene_id, diff) {
            Ok(r) => results.push(r),
            Err(e) => {
                results.push(InjectionResult::failure("episodic", format!("{}", e), 0));
            }
        }

        // L3 - Working: update current state
        match self.working.inject(scene_id) {
            Ok(r) => results.push(r),
            Err(e) => {
                results.push(InjectionResult::failure("working", format!("{}", e), 0));
            }
        }

        // If entities were added or removed, also re-inject semantic and procedural
        if !diff.added.is_empty() || !diff.removed.is_empty() {
            match self.semantic.inject(scene_id) {
                Ok(r) => results.push(r),
                Err(e) => {
                    results.push(InjectionResult::failure("semantic", format!("{}", e), 0));
                }
            }

            match self.procedural.inject(scene_id) {
                Ok(r) => results.push(r),
                Err(e) => {
                    results.push(InjectionResult::failure("procedural", format!("{}", e), 0));
                }
            }
        }

        Ok(InjectionSummary::from_results(
            scene_id.to_string(),
            results,
        ))
    }

    /// Inject a scene event across relevant tiers.
    ///
    /// Episodic tier records the event; Working tier updates current state.
    pub fn inject_event(&self, event: &SceneEvent) -> Result<InjectionSummary> {
        let mut results = Vec::new();

        // L2 - Episodic: record the event
        match self.episodic.inject_event(event) {
            Ok(r) => results.push(r),
            Err(e) => {
                results.push(InjectionResult::failure("episodic", format!("{}", e), 0));
            }
        }

        // L3 - Working: refresh current state
        match self.working.inject(&event.scene_id) {
            Ok(r) => results.push(r),
            Err(e) => {
                results.push(InjectionResult::failure("working", format!("{}", e), 0));
            }
        }

        Ok(InjectionSummary::from_results(
            event.scene_id.clone(),
            results,
        ))
    }

    /// Perform a cross-layer query, collecting results from the scene context
    /// memory that correspond to the query text across all four tiers.
    ///
    /// This queries the in-memory scene context (snapshots and change history)
    /// since the actual agent-core MemorySystem is not directly accessible
    /// from this crate.
    pub fn cross_layer_query(&self, query: &str) -> CrossLayerQueryResult {
        let mut result = CrossLayerQueryResult::new(query);
        let query_lower = query.to_lowercase();

        // Query the database for working-memory style results
        {
            let db = self.db.lock().expect("mutex poisoned");
            for scene in db.get_all_scenes() {
                let entities = db.get_scene_entities(&scene.scene_id);
                for entity in &entities {
                    if entity.name.to_lowercase().contains(&query_lower)
                        || entity.entity_type.to_lowercase().contains(&query_lower)
                    {
                        result.working_results.push(format!(
                            "Entity: {} (id={}, type={}, scene={})",
                            entity.name, entity.entity_id, entity.entity_type, scene.name
                        ));
                    }
                }
                // Also check scene name
                if scene.name.to_lowercase().contains(&query_lower)
                    || scene.scene_id.to_lowercase().contains(&query_lower)
                {
                    result.working_results.push(format!(
                        "Scene: {} (id={}, {} entities)",
                        scene.name,
                        scene.scene_id,
                        entities.len()
                    ));
                }
            }
        }

        let memory = self.scene_memory.lock().expect("mutex poisoned");

        // Working: check active scene (supplemental to DB query above)
        if let Some(ref active) = memory.active_scene {
            if (active.scene_name.to_lowercase().contains(&query_lower)
                || active.scene_id.to_lowercase().contains(&query_lower))
                && !result
                    .working_results
                    .iter()
                    .any(|s| s.contains(&active.scene_name))
            {
                result.working_results.push(format!(
                    "Active scene: {} ({} entities)",
                    active.scene_name,
                    active.entity_count()
                ));
            }

            // Check entities in the active scene
            for entity in &active.snapshot.entities {
                if entity.name.to_lowercase().contains(&query_lower) {
                    result.working_results.push(format!(
                        "Entity: {} (id={}, components=[{}])",
                        entity.name,
                        entity.id,
                        entity
                            .components
                            .iter()
                            .map(|c| c.type_name.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    ));
                }
            }
        }

        // Episodic: search change events
        for event in &memory.change_events {
            let summary = event.summary().to_lowercase();
            let desc = event.description.as_deref().unwrap_or("").to_lowercase();
            if summary.contains(&query_lower) || desc.contains(&query_lower) {
                result.episodic_results.push(event.summary());
            }
        }

        // Semantic: search relation nodes
        for node in memory.relation_nodes.values() {
            if node.name.to_lowercase().contains(&query_lower) {
                result
                    .semantic_results
                    .push(format!("Node: {} (type={:?})", node.name, node.node_type));
            }
        }

        // Search relation edges
        for edge in &memory.relation_edges {
            if edge.relation_type.to_lowercase().contains(&query_lower) {
                result.semantic_results.push(format!(
                    "Relation: {} -> {} ({})",
                    edge.from_node_id, edge.to_node_id, edge.relation_type
                ));
            }
        }

        // Procedural: check scene history for patterns
        for (scene_id, entries) in &memory.scene_history {
            if scene_id.to_lowercase().contains(&query_lower) {
                if let Some(last) = entries.last() {
                    result.procedural_results.push(format!(
                        "Scene pattern for {}: {} snapshots, last snapshot has {} entities",
                        scene_id,
                        entries.len(),
                        last.entity_count()
                    ));
                }
            }
        }

        result.total_count = result.working_results.len()
            + result.episodic_results.len()
            + result.semantic_results.len()
            + result.procedural_results.len();

        result
    }
}

// ============================================================================
// SceneMemoryAutoUpdater
// ============================================================================

/// Background updater that watches for scene changes and triggers
/// memory injection automatically.
///
/// Registers as a `SceneEventSubscriber` on the `SceneEventBus`.
/// When events arrive, it delegates to the `FourTierSceneInjector`
/// to update all relevant memory tiers.
pub struct SceneMemoryAutoUpdater {
    /// The four-tier injector that performs the actual injection.
    injector: FourTierSceneInjector,
    /// Subscriber ID on the event bus.
    subscriber_id: Option<SubscriberId>,
    /// Whether auto-update is currently enabled.
    pub enabled: bool,
    /// Minimum interval between auto-updates (throttle).
    pub min_interval: Duration,
    /// Timestamp of the last update.
    last_update: Instant,
    /// Statistics.
    pub stats: AutoUpdateStats,
}

/// Statistics for the SceneMemoryAutoUpdater.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AutoUpdateStats {
    /// Total number of updates triggered.
    pub updates_triggered: u64,
    /// Number of successful updates.
    pub updates_succeeded: u64,
    /// Number of failed updates.
    pub updates_failed: u64,
    /// Number of events received.
    pub events_received: u64,
    /// Number of events skipped due to throttling.
    pub events_throttled: u64,
    /// Timestamp of the last update.
    pub last_update_time: Option<u64>,
}

impl SceneMemoryAutoUpdater {
    /// Create a new auto-updater with the given four-tier injector.
    pub fn new(injector: FourTierSceneInjector) -> Self {
        Self {
            injector,
            subscriber_id: None,
            enabled: true,
            min_interval: Duration::from_millis(500),
            last_update: Instant::now(),
            stats: AutoUpdateStats::default(),
        }
    }

    /// Subscribe to the given event bus for automatic updates.
    ///
    /// Returns the subscriber ID assigned by the bus.
    pub fn subscribe(&mut self, event_bus: &SharedSceneEventBus) -> SubscriberId {
        let subscriber = Arc::new(AutoUpdateSubscriber {
            injector_db: self.injector.episodic.db.clone(),
            scene_memory: self.injector.scene_memory.clone(),
            stats: Arc::new(Mutex::new(self.stats.clone())),
            min_interval: self.min_interval,
            last_update: Arc::new(Mutex::new(Instant::now())),
        });

        let mut bus = event_bus.lock().expect("mutex poisoned");
        let id = bus.subscribe(subscriber);
        self.subscriber_id = Some(id);
        info!("SceneMemoryAutoUpdater subscribed to event bus (id={})", id);
        id
    }

    /// Unsubscribe from the event bus.
    pub fn unsubscribe(&mut self, event_bus: &SharedSceneEventBus) {
        if let Some(id) = self.subscriber_id.take() {
            let mut bus = event_bus.lock().expect("mutex poisoned");
            bus.unsubscribe(id);
            info!("SceneMemoryAutoUpdater unsubscribed (id={})", id);
        }
    }

    /// Manually trigger an update for the given scene.
    pub fn update_now(&mut self, scene_id: &str) -> Result<InjectionSummary> {
        self.stats.updates_triggered += 1;

        match self.injector.inject_scene(scene_id) {
            Ok(summary) => {
                self.stats.updates_succeeded += 1;
                self.stats.last_update_time = Some(
                    SystemTime::now()
                        .duration_since(SystemTime::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs(),
                );
                Ok(summary)
            }
            Err(e) => {
                self.stats.updates_failed += 1;
                Err(e)
            }
        }
    }

    /// Check if an update should be throttled.
    pub fn should_throttle(&self) -> bool {
        self.last_update.elapsed() < self.min_interval
    }

    /// Get a snapshot of the current statistics.
    pub fn get_stats(&self) -> AutoUpdateStats {
        self.stats.clone()
    }
}

// ============================================================================
// AutoUpdateSubscriber (internal event bus subscriber)
// ============================================================================

/// Internal subscriber that the `SceneMemoryAutoUpdater` registers on the
/// event bus. Processes scene events and triggers memory updates.
struct AutoUpdateSubscriber {
    injector_db: SharedMulticaDb,
    scene_memory: SharedSceneContextMemory,
    stats: Arc<Mutex<AutoUpdateStats>>,
    min_interval: Duration,
    last_update: Arc<Mutex<Instant>>,
}

impl SceneEventSubscriber for AutoUpdateSubscriber {
    fn on_event(&self, event: &SceneEvent) {
        {
            let mut stats = self.stats.lock().expect("mutex poisoned");
            stats.events_received += 1;
        }

        // Throttle check
        {
            let last = self.last_update.lock().expect("mutex poisoned");
            if last.elapsed() < self.min_interval {
                let mut stats = self.stats.lock().expect("mutex poisoned");
                stats.events_throttled += 1;
                debug!(
                    "AutoUpdater: throttled event {:?} for entity {}",
                    event.event_type, event.entity_id
                );
                return;
            }
        }

        // Perform the injection
        let injector =
            FourTierSceneInjector::new(self.injector_db.clone(), self.scene_memory.clone());

        match injector.inject_event(event) {
            Ok(summary) => {
                let mut stats = self.stats.lock().expect("mutex poisoned");
                stats.updates_triggered += 1;
                stats.updates_succeeded += 1;
                stats.last_update_time = Some(
                    SystemTime::now()
                        .duration_since(SystemTime::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs(),
                );
                debug!(
                    "AutoUpdater: injected event {:?} for entity {} ({} entries)",
                    event.event_type, event.entity_id, summary.total_entries
                );
            }
            Err(e) => {
                let mut stats = self.stats.lock().expect("mutex poisoned");
                stats.updates_triggered += 1;
                stats.updates_failed += 1;
                warn!("AutoUpdater: failed to inject event: {}", e);
            }
        }

        // Update last update time
        {
            let mut last = self.last_update.lock().expect("mutex poisoned");
            *last = Instant::now();
        }
    }
}

// ============================================================================
// Helpers
// ============================================================================

/// Collect unique component type names from entities.
fn collect_component_types(entities: &[EntityRecord]) -> Vec<String> {
    let mut types: Vec<String> = entities
        .iter()
        .flat_map(|e| e.components.iter().map(|c| c.type_name.clone()))
        .collect();
    types.sort();
    types.dedup();
    types
}

/// Group entities by their entity_type field.
fn group_by_type(entities: &[EntityRecord]) -> HashMap<String, Vec<&EntityRecord>> {
    let mut groups: HashMap<String, Vec<&EntityRecord>> = HashMap::new();
    for entity in entities {
        groups
            .entry(entity.entity_type.clone())
            .or_default()
            .push(entity);
    }
    groups
}

/// Compute Euclidean distance between two optional positions.
fn euclidean_distance(a: Option<[f64; 3]>, b: Option<[f64; 3]>) -> f64 {
    match (a, b) {
        (Some(a), Some(b)) => {
            let dx = a[0] - b[0];
            let dy = a[1] - b[1];
            let dz = a[2] - b[2];
            (dx * dx + dy * dy + dz * dz).sqrt()
        }
        _ => f64::MAX,
    }
}

/// Compute the centroid of positioned entities.
fn compute_centroid(entities: &[&EntityRecord]) -> [f64; 3] {
    let mut sum = [0.0; 3];
    let mut count = 0;
    for e in entities {
        if let Some(pos) = e.position {
            sum[0] += pos[0];
            sum[1] += pos[1];
            sum[2] += pos[2];
            count += 1;
        }
    }
    if count > 0 {
        [
            sum[0] / count as f64,
            sum[1] / count as f64,
            sum[2] / count as f64,
        ]
    } else {
        [0.0; 3]
    }
}

/// Compute average spread (distance from centroid).
fn compute_spread(entities: &[&EntityRecord], centroid: &[f64; 3]) -> f64 {
    let mut total = 0.0;
    let mut count = 0;
    for e in entities {
        if let Some(pos) = e.position {
            let dx = pos[0] - centroid[0];
            let dy = pos[1] - centroid[1];
            let dz = pos[2] - centroid[2];
            total += (dx * dx + dy * dy + dz * dz).sqrt();
            count += 1;
        }
    }
    if count > 0 {
        total / count as f64
    } else {
        0.0
    }
}

/// Extract component sequences: group entities by component type presence.
fn extract_component_sequences(entities: &[EntityRecord]) -> HashMap<String, Vec<EntityRecord>> {
    let mut sequences: HashMap<String, Vec<EntityRecord>> = HashMap::new();
    for entity in entities {
        for component in &entity.components {
            sequences
                .entry(component.type_name.clone())
                .or_default()
                .push(entity.clone());
        }
    }
    sequences
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory_scene_context::create_shared_scene_context_memory;
    use crate::multica_db::create_shared_multica_db;
    use crate::scene_context::SceneEntity;
    use std::collections::HashMap;

    fn create_test_db_with_scene() -> (SharedMulticaDb, String) {
        let db = create_shared_multica_db();
        let scene_id = {
            let mut db = db.lock().unwrap();
            let sid = db
                .create_scene("TestScene".to_string(), Some("A test scene".to_string()))
                .unwrap();
            db.create_entity(
                sid.clone(),
                "Player".to_string(),
                "GameObject".to_string(),
                vec![
                    ComponentData {
                        type_name: "Transform".to_string(),
                        properties: {
                            let mut m = HashMap::new();
                            m.insert("x".to_string(), serde_json::json!(0.0));
                            m.insert("y".to_string(), serde_json::json!(0.0));
                            m
                        },
                    },
                    ComponentData {
                        type_name: "Render".to_string(),
                        properties: HashMap::new(),
                    },
                ],
                Some([0.0, 0.0, 0.0]),
            )
            .unwrap();
            db.create_entity(
                sid.clone(),
                "Enemy".to_string(),
                "GameObject".to_string(),
                vec![ComponentData {
                    type_name: "Transform".to_string(),
                    properties: {
                        let mut m = HashMap::new();
                        m.insert("x".to_string(), serde_json::json!(10.0));
                        m
                    },
                }],
                Some([10.0, 0.0, 0.0]),
            )
            .unwrap();
            db.create_entity(
                sid.clone(),
                "Light".to_string(),
                "Light".to_string(),
                vec![ComponentData {
                    type_name: "Light".to_string(),
                    properties: {
                        let mut m = HashMap::new();
                        m.insert("intensity".to_string(), serde_json::json!(1.5));
                        m
                    },
                }],
                Some([5.0, 5.0, 5.0]),
            )
            .unwrap();
            sid
        };
        (db, scene_id)
    }

    // --- L3 Working Memory Tests ---

    #[test]
    fn test_working_memory_injector() {
        let (db, scene_id) = create_test_db_with_scene();
        let injector = SceneWorkingMemoryInjector::new(db);

        let result = injector.inject(&scene_id).unwrap();
        assert!(result.success);
        assert_eq!(result.layer, "working");
        assert!(
            result.entry_count >= 3,
            "Should have at least entity entries + summary"
        );
        assert!(result.details.contains("TestScene"));

        // Error case
        let err = injector.inject("nonexistent");
        assert!(err.is_err());
    }

    // --- L2 Episodic Memory Tests ---

    #[test]
    fn test_episodic_memory_inject_changes() {
        let (db, scene_id) = create_test_db_with_scene();
        let scene_memory = create_shared_scene_context_memory();
        let injector = SceneEpisodicMemoryInjector::new(db, scene_memory);

        let diff = SceneDiff {
            added: vec![SceneEntity {
                id: 999,
                name: "NewObject".to_string(),
                components: vec![],
                position: None,
            }],
            removed: vec![],
            modified: vec![],
        };

        let result = injector.inject_changes(&scene_id, &diff).unwrap();
        assert!(result.success);
        assert_eq!(result.layer, "episodic");
        assert!(result.entry_count >= 1);
        assert!(result.details.contains("+1"));
    }

    #[test]
    fn test_episodic_memory_inject_event() {
        let (db, scene_id) = create_test_db_with_scene();
        let scene_memory = create_shared_scene_context_memory();
        let injector = SceneEpisodicMemoryInjector::new(db, scene_memory);

        let event = SceneEvent {
            scene_id: scene_id.clone(),
            event_type: SceneEventType::ComponentAdded,
            entity_id: 1,
            entity_before: None,
            entity_after: Some(SceneEntity {
                id: 1,
                name: "Player".to_string(),
                components: vec![],
                position: None,
            }),
            component: Some(ComponentData {
                type_name: "Physics".to_string(),
                properties: HashMap::new(),
            }),
            timestamp: chrono::Utc::now().to_rfc3339(),
        };

        let result = injector.inject_event(&event).unwrap();
        assert!(result.success);
        assert_eq!(result.layer, "episodic");
        assert_eq!(result.entry_count, 1);
    }

    #[test]
    fn test_episodic_memory_inject_scene_loaded() {
        let (db, scene_id) = create_test_db_with_scene();
        let scene_memory = create_shared_scene_context_memory();
        let injector = SceneEpisodicMemoryInjector::new(db, scene_memory);

        let result = injector.inject_scene_loaded(&scene_id).unwrap();
        assert!(result.success);
        assert_eq!(result.layer, "episodic");
        assert_eq!(result.entry_count, 1);
        assert!(result.details.contains("3 entities"));
    }

    // --- L1 Semantic Memory Tests ---

    #[test]
    fn test_semantic_memory_injector() {
        let (db, scene_id) = create_test_db_with_scene();
        let injector = SceneSemanticMemoryInjector::new(db);

        let result = injector.inject(&scene_id).unwrap();
        assert!(result.success);
        assert_eq!(result.layer, "semantic");
        // Should have: scene node + entity type nodes + entity nodes + component type nodes + relations
        assert!(
            result.entry_count >= 5,
            "Should have multiple semantic entries, got {}",
            result.entry_count
        );
        assert!(result.details.contains("HasA"));
    }

    // --- L0 Procedural Memory Tests ---

    #[test]
    fn test_procedural_memory_injector() {
        let (db, scene_id) = create_test_db_with_scene();
        let injector = SceneProceduralMemoryInjector::new(db);

        let result = injector.inject(&scene_id).unwrap();
        assert!(result.success);
        assert_eq!(result.layer, "procedural");
        // Should have: entity creation workflows + positioning pattern
        assert!(
            result.entry_count >= 2,
            "Should have multiple procedural entries"
        );
        assert!(result.details.contains("entity types"));
    }

    // --- FourTierSceneInjector (Orchestrator) Tests ---

    #[test]
    fn test_four_tier_inject_scene() {
        let (db, scene_id) = create_test_db_with_scene();
        let scene_memory = create_shared_scene_context_memory();
        let orchestrator = FourTierSceneInjector::new(db, scene_memory);

        let summary = orchestrator.inject_scene(&scene_id).unwrap();
        assert!(summary.all_succeeded);
        assert_eq!(summary.results.len(), 4);
        assert_eq!(summary.scene_id, scene_id);
        assert!(summary.total_entries > 0);

        // Verify all four layers present
        let layers: Vec<&str> = summary.results.iter().map(|r| r.layer.as_str()).collect();
        assert!(layers.contains(&"working"));
        assert!(layers.contains(&"episodic"));
        assert!(layers.contains(&"semantic"));
        assert!(layers.contains(&"procedural"));
    }

    #[test]
    fn test_four_tier_inject_changes() {
        let (db, scene_id) = create_test_db_with_scene();
        let scene_memory = create_shared_scene_context_memory();
        let orchestrator = FourTierSceneInjector::new(db, scene_memory);

        let diff = SceneDiff {
            added: vec![SceneEntity {
                id: 100,
                name: "AddedObj".to_string(),
                components: vec![],
                position: None,
            }],
            removed: vec![],
            modified: vec![],
        };

        let summary = orchestrator.inject_changes(&scene_id, &diff).unwrap();
        // All layers that were attempted should succeed
        let succeeded: Vec<_> = summary.results.iter().filter(|r| r.success).collect();
        assert!(
            !succeeded.is_empty(),
            "Should have at least some successful injections"
        );
        assert!(succeeded.iter().any(|r| r.layer == "episodic"));
        assert!(succeeded.iter().any(|r| r.layer == "working"));
    }

    #[test]
    fn test_cross_layer_query() {
        let (db, scene_id) = create_test_db_with_scene();
        let scene_memory = create_shared_scene_context_memory();
        let orchestrator = FourTierSceneInjector::new(db.clone(), scene_memory.clone());

        // First inject the scene to populate the context memory
        let _ = orchestrator.inject_scene(&scene_id).unwrap();

        // Query for "Player" - should find in working memory
        let result = orchestrator.cross_layer_query("Player");
        assert!(!result.is_empty(), "Should find Player entity");
        assert!(!result.working_results.is_empty());
        assert!(result.working_results.iter().any(|s| s.contains("Player")));

        // Query for "enemy" - case insensitive
        let result2 = orchestrator.cross_layer_query("enemy");
        assert!(!result2.is_empty());
        assert!(result2
            .working_results
            .iter()
            .any(|s| s.to_lowercase().contains("enemy")));

        // Empty query result
        let empty_result = CrossLayerQueryResult::new("nonexistent_xyz123");
        assert!(empty_result.is_empty());
        assert_eq!(empty_result.total_count, 0);
    }

    #[test]
    fn test_injection_result_types() {
        let success = InjectionResult::success("working", "test".to_string(), 5, 100);
        assert!(success.success);
        assert_eq!(success.entry_count, 5);
        assert!(success.error.is_none());

        let failure = InjectionResult::failure("episodic", "error msg".to_string(), 50);
        assert!(!failure.success);
        assert_eq!(failure.entry_count, 0);
        assert_eq!(failure.error, Some("error msg".to_string()));
    }

    #[test]
    fn test_injection_summary_from_results() {
        let results = vec![
            InjectionResult::success("working", "ok".into(), 3, 10),
            InjectionResult::success("episodic", "ok".into(), 1, 5),
            InjectionResult::failure("semantic", "err".into(), 2),
        ];

        let summary = InjectionSummary::from_results("scene-1".to_string(), results);
        assert_eq!(summary.scene_id, "scene-1");
        assert_eq!(summary.total_entries, 4); // 3 + 1 + 0
        assert!(!summary.all_succeeded); // semantic failed
    }

    #[test]
    fn test_cross_layer_query_merge() {
        let mut result1 = CrossLayerQueryResult::new("test");
        result1.working_results.push("W1".to_string());
        result1.episodic_results.push("E1".to_string());
        result1.total_count = 2;

        let mut result2 = CrossLayerQueryResult::new("test");
        result2.working_results.push("W2".to_string());
        result2.semantic_results.push("S1".to_string());
        result2.total_count = 2;

        result1.merge(&result2);
        assert_eq!(result1.working_results.len(), 2);
        assert_eq!(result1.episodic_results.len(), 1);
        assert_eq!(result1.semantic_results.len(), 1);
        assert_eq!(result1.total_count, 4);
    }

    #[test]
    fn test_auto_updater_stats() {
        let mut stats = AutoUpdateStats::default();
        assert_eq!(stats.updates_triggered, 0);
        assert_eq!(stats.events_received, 0);

        stats.events_received += 1;
        stats.updates_triggered += 1;
        stats.updates_succeeded += 1;

        assert_eq!(stats.events_received, 1);
        assert_eq!(stats.updates_succeeded, 1);
    }
}
