//! Memory Injector — Production-grade scene-to-memory injection bridge
//!
//! Bridges multica-db scene data into agent-core's 4-tier memory system.
//! Subscribes to scene events and automatically triggers memory injection
//! when entities are created, updated, or deleted.

use crate::error::{BridgeError, Result};
use crate::memory_scene_context::SharedSceneContextMemory;
use crate::multica_db::{EntityRecord, SceneRecord, SharedMulticaDb};
#[cfg(test)]
use crate::scene_context::ComponentData;
use crate::scene_context::{SceneDiff, SceneEntity, SceneSnapshot};
use crate::scene_event_bus::{
    SceneEvent, SceneEventSubscriber, SceneEventType, SharedSceneEventBus, SubscriberId,
};
use log::{debug, error, info, warn};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};

// ============================================================================
// Configuration
// ============================================================================

/// Configuration for the MulticaMemoryInjector.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryInjectorConfig {
    /// Whether to automatically inject scene data on events.
    pub auto_inject: bool,
    /// Interval between automatic injections (throttle).
    pub inject_interval_secs: u64,
    /// Maximum number of entities to inject per batch.
    pub max_entities_per_injection: usize,
    /// Maximum number of tokens per injection batch (None = unlimited).
    /// When set, entries are trimmed to stay within this budget.
    pub max_tokens_per_injection: Option<usize>,
    /// Whether to inject into Working Memory (L3).
    pub inject_working: bool,
    /// Whether to inject into Episodic Memory (L2).
    pub inject_episodic: bool,
    /// Whether to inject into Semantic Memory (L1).
    pub inject_semantic: bool,
    /// Whether to inject into Procedural Memory (L0).
    pub inject_procedural: bool,
    /// Whether to build entity-relation graphs during injection.
    pub build_relations: bool,
    /// Whether to record scene change events.
    pub record_changes: bool,
}

impl Default for MemoryInjectorConfig {
    fn default() -> Self {
        Self {
            auto_inject: true,
            inject_interval_secs: 1,
            max_entities_per_injection: 1000,
            max_tokens_per_injection: Some(2000),
            inject_working: true,
            inject_episodic: true,
            inject_semantic: true,
            inject_procedural: true,
            build_relations: true,
            record_changes: true,
        }
    }
}

impl MemoryInjectorConfig {
    /// Create a config with auto-injection disabled (manual-only mode).
    pub fn manual_only() -> Self {
        Self {
            auto_inject: false,
            ..Default::default()
        }
    }

    /// Create a config that only injects working + episodic memory.
    pub fn quick_mode() -> Self {
        Self {
            inject_semantic: false,
            inject_procedural: false,
            build_relations: false,
            ..Default::default()
        }
    }
}

// ============================================================================
// Statistics
// ============================================================================

/// Statistics tracking for memory injection operations.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MemoryInjectionStats {
    /// Total number of injection attempts.
    pub injections_count: u64,
    /// Number of failed injections.
    pub errors_count: u64,
    /// Timestamp (unix seconds) of the last injection.
    pub last_injection_time: Option<u64>,
    /// Duration of the last injection in milliseconds.
    pub last_injection_duration_ms: Option<u64>,
    /// Number of entities injected.
    pub entities_injected: u64,
    /// Number of scene changes processed.
    pub changes_processed: u64,
    /// Number of events handled by the event handler.
    pub events_handled: u64,
    /// Whether an injection is currently in progress.
    pub injection_in_progress: bool,
}

impl MemoryInjectionStats {
    /// Record a successful injection.
    pub fn record_success(&mut self, entity_count: usize, duration_ms: u64) {
        self.injections_count += 1;
        self.entities_injected += entity_count as u64;
        self.last_injection_time = Some(
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        );
        self.last_injection_duration_ms = Some(duration_ms);
        self.injection_in_progress = false;
    }

    /// Record a failed injection.
    pub fn record_error(&mut self) {
        self.errors_count += 1;
        self.injection_in_progress = false;
    }

    /// Record a scene change event.
    pub fn record_change(&mut self) {
        self.changes_processed += 1;
    }

    /// Record an event handler invocation.
    pub fn record_event(&mut self) {
        self.events_handled += 1;
    }

    /// Mark injection as in progress.
    pub fn start_injection(&mut self) {
        self.injection_in_progress = true;
    }
}

// ============================================================================
// Injected Memory Entry (output format for each tier)
// ============================================================================

/// A prepared memory entry ready for injection into a specific tier.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InjectedEntry {
    /// Target memory tier: "working", "episodic", "semantic", "procedural"
    pub tier: String,
    /// Human-readable summary
    pub summary: String,
    /// JSON payload with full details
    pub payload: serde_json::Value,
    /// Associated entity IDs
    pub entity_ids: Vec<u64>,
    /// Associated scene ID
    pub scene_id: String,
    /// Importance score [0.0, 1.0]
    pub importance: f32,
    /// Entry tags
    pub tags: Vec<String>,
}

// ============================================================================
// MulticaMemoryInjector
// ============================================================================

/// The main memory injector that bridges multica-db scene data into
/// agent-core's 4-tier memory system.
///
/// This injector:
/// - Reads scene data from `MulticaDb`
/// - Persists snapshots in `SceneContextMemory`
/// - Subscribes to `SceneEventBus` for real-time updates
/// - Transforms scene entities/relations into tier-specific memory entries
/// - Supports both automatic (event-driven) and manual injection triggers
pub struct MulticaMemoryInjector {
    /// Scene context memory for snapshot persistence.
    pub scene_memory: SharedSceneContextMemory,
    /// Multica database for querying scene/entity data.
    pub db: SharedMulticaDb,
    /// Optional event bus subscription for auto-injection.
    pub event_bus: Option<SharedSceneEventBus>,
    /// Our subscriber ID on the event bus (if subscribed).
    subscriber_id: Option<SubscriberId>,
    /// Injection configuration.
    pub config: MemoryInjectorConfig,
    /// Injection statistics.
    pub stats: MemoryInjectionStats,
    /// Time of the last injection (for throttling).
    last_inject_instant: Instant,
}

impl MulticaMemoryInjector {
    /// Create a new memory injector with the given scene memory and database.
    pub fn new(
        scene_memory: SharedSceneContextMemory,
        db: SharedMulticaDb,
        config: MemoryInjectorConfig,
    ) -> Self {
        Self {
            scene_memory,
            db,
            event_bus: None,
            subscriber_id: None,
            config,
            stats: MemoryInjectionStats::default(),
            last_inject_instant: Instant::now(),
        }
    }

    /// Create a new memory injector with an event bus for auto-injection.
    ///
    /// Registers a `SceneMemoryEventHandler` on the bus so that scene
    /// entity changes automatically trigger memory injection.
    ///
    /// Returns an `Arc<Mutex<Self>>` so the handler can hold a proper
    /// reference to the injector for event processing.
    pub fn with_event_bus(
        scene_memory: SharedSceneContextMemory,
        db: SharedMulticaDb,
        event_bus: SharedSceneEventBus,
        config: MemoryInjectorConfig,
    ) -> Arc<Mutex<Self>> {
        let injector = Arc::new(Mutex::new(Self {
            scene_memory,
            db,
            event_bus: Some(event_bus.clone()),
            subscriber_id: None,
            config,
            stats: MemoryInjectionStats::default(),
            last_inject_instant: Instant::now(),
        }));

        if injector.lock().expect("mutex poisoned").config.auto_inject {
            let handler = Arc::new(SceneMemoryEventHandler {
                injector: Some(injector.clone()),
            });
            let mut bus = event_bus.lock().expect("mutex poisoned");
            let id = bus.subscribe(handler);
            injector.lock().expect("mutex poisoned").subscriber_id = Some(id);
        }

        injector
    }

    // ========================================================================
    // Manual Injection Triggers
    // ========================================================================

    /// Manually inject the current scene state from the database into memory.
    ///
    /// Reads all entities for the given scene, creates a snapshot, saves it
    /// to scene context memory, and prepares tier-specific injection entries.
    pub fn inject_scene(&mut self, scene_id: &str) -> Result<Vec<InjectedEntry>> {
        self.stats.start_injection();
        let start = Instant::now();

        let result = self.do_inject_scene(scene_id);
        let duration_ms = start.elapsed().as_millis() as u64;

        match &result {
            Ok(entries) => {
                self.stats.record_success(entries.len(), duration_ms);
                self.last_inject_instant = Instant::now();
                info!(
                    "Injected scene '{}': {} entries in {}ms",
                    scene_id,
                    entries.len(),
                    duration_ms
                );
            }
            Err(e) => {
                self.stats.record_error();
                error!("Failed to inject scene '{}': {}", scene_id, e);
            }
        }

        result
    }

    /// Manually inject all scenes from the database.
    pub fn inject_all_scenes(&mut self) -> Result<Vec<InjectedEntry>> {
        let scenes = {
            let db = self.db.lock().expect("mutex poisoned");
            db.get_all_scenes()
        };

        let mut all_entries = Vec::new();
        for scene in &scenes {
            match self.inject_scene(&scene.scene_id) {
                Ok(entries) => {
                    all_entries.extend(entries);
                }
                Err(e) => {
                    warn!("Skipping scene '{}': {}", scene.scene_id, e);
                }
            }
        }

        info!(
            "Injected {} scenes: {} total entries",
            scenes.len(),
            all_entries.len()
        );

        Ok(all_entries)
    }

    /// Inject changes for a specific scene (diff-based).
    pub fn inject_changes(
        &mut self,
        scene_id: &str,
        diff: &SceneDiff,
    ) -> Result<Vec<InjectedEntry>> {
        self.stats.start_injection();
        let start = Instant::now();

        // Record the change in scene context memory
        {
            let mut memory = self.scene_memory.lock().expect("mutex poisoned");
            memory.record_scene_change(diff.clone(), scene_id.to_string())?;
        }

        let mut entries = Vec::new();

        // Inject added entities as episodic events
        for entity in &diff.added {
            let payload = serde_json::to_value(entity).unwrap_or_default();
            entries.push(InjectedEntry {
                tier: "episodic".to_string(),
                summary: format!("Entity created: {} (id={})", entity.name, entity.id),
                payload,
                entity_ids: vec![entity.id],
                scene_id: scene_id.to_string(),
                importance: 0.6,
                tags: vec!["entity_created".to_string(), "scene_change".to_string()],
            });
        }

        // Inject removed entities as episodic events
        for entity in &diff.removed {
            let payload = serde_json::to_value(entity).unwrap_or_default();
            entries.push(InjectedEntry {
                tier: "episodic".to_string(),
                summary: format!("Entity deleted: {} (id={})", entity.name, entity.id),
                payload,
                entity_ids: vec![entity.id],
                scene_id: scene_id.to_string(),
                importance: 0.7,
                tags: vec!["entity_deleted".to_string(), "scene_change".to_string()],
            });
        }

        // Inject modified entities as episodic events
        for change in &diff.modified {
            let payload = serde_json::json!({
                "entity_id": change.entity_id,
                "changes": change.changes,
                "before": change.before,
                "after": change.after,
            });
            entries.push(InjectedEntry {
                tier: "episodic".to_string(),
                summary: format!(
                    "Entity modified: id={}, changes=[{}]",
                    change.entity_id,
                    change.changes.join(", ")
                ),
                payload,
                entity_ids: vec![change.entity_id],
                scene_id: scene_id.to_string(),
                importance: 0.5,
                tags: vec!["entity_modified".to_string(), "scene_change".to_string()],
            });
        }

        let duration_ms = start.elapsed().as_millis() as u64;

        if entries.is_empty() {
            debug!("No changes to inject for scene '{}'", scene_id);
            self.stats.record_success(0, duration_ms);
        } else {
            self.stats.record_success(entries.len(), duration_ms);
            self.stats.record_change();
            self.last_inject_instant = Instant::now();
            info!(
                "Injected {} change entries for scene '{}' in {}ms",
                entries.len(),
                scene_id,
                duration_ms
            );
        }

        Ok(entries)
    }

    /// Trigger injection for the given scene event.
    ///
    /// Called by the `SceneMemoryEventHandler` when a scene event is published.
    pub fn inject_from_event(&mut self, event: &SceneEvent) -> Result<Vec<InjectedEntry>> {
        let mut entries = Vec::new();

        let entity_json = match &event.entity_after {
            Some(e) => serde_json::to_value(e).unwrap_or_default(),
            None => match &event.entity_before {
                Some(e) => serde_json::to_value(e).unwrap_or_default(),
                None => serde_json::Value::Null,
            },
        };

        let (summary, tags) = match event.event_type {
            SceneEventType::EntityCreated => (
                format!(
                    "Entity created in scene '{}': {} (id={})",
                    event.scene_id,
                    event
                        .entity_after
                        .as_ref()
                        .map(|e| e.name.as_str())
                        .unwrap_or("unknown"),
                    event.entity_id
                ),
                vec!["entity_created".to_string()],
            ),
            SceneEventType::EntityUpdated => (
                format!(
                    "Entity updated in scene '{}': id={}",
                    event.scene_id, event.entity_id
                ),
                vec!["entity_updated".to_string()],
            ),
            SceneEventType::EntityDeleted => (
                format!(
                    "Entity deleted from scene '{}': id={}",
                    event.scene_id, event.entity_id
                ),
                vec!["entity_deleted".to_string()],
            ),
            SceneEventType::ComponentAdded => (
                format!(
                    "Component added to entity {} in scene '{}'",
                    event.entity_id, event.scene_id
                ),
                vec!["component_added".to_string()],
            ),
            SceneEventType::ComponentUpdated => (
                format!(
                    "Component updated on entity {} in scene '{}'",
                    event.entity_id, event.scene_id
                ),
                vec!["component_updated".to_string()],
            ),
            SceneEventType::ComponentRemoved => (
                format!(
                    "Component removed from entity {} in scene '{}'",
                    event.entity_id, event.scene_id
                ),
                vec!["component_removed".to_string()],
            ),
        };

        entries.push(InjectedEntry {
            tier: "episodic".to_string(),
            summary,
            payload: entity_json,
            entity_ids: vec![event.entity_id],
            scene_id: event.scene_id.clone(),
            importance: 0.6,
            tags,
        });

        // Also inject current scene state to working memory
        if self.config.inject_working {
            let entity_entries = self.prepare_working_entries(&event.scene_id)?;
            if entity_entries.is_empty() {
                warn!(
                    "inject_from_event: no entities found for scene '{}' in working memory injection",
                    event.scene_id
                );
            }
            entries.extend(entity_entries);
        }

        self.stats.record_event();
        self.last_inject_instant = Instant::now();
        Ok(entries)
    }

    /// Get a snapshot of the current injection statistics.
    pub fn get_stats(&self) -> MemoryInjectionStats {
        self.stats.clone()
    }

    /// Reset injection statistics.
    pub fn reset_stats(&mut self) {
        self.stats = MemoryInjectionStats::default();
    }

    /// Check if an injection has happened recently enough to throttle.
    #[allow(dead_code)]
    fn should_throttle(&self) -> bool {
        let elapsed = self.last_inject_instant.elapsed();
        elapsed < Duration::from_secs(self.config.inject_interval_secs)
    }

    // ========================================================================
    // Internal injection logic
    // ========================================================================

    /// Core injection logic: read scene from DB, build snapshot, prepare entries.
    fn do_inject_scene(&self, scene_id: &str) -> Result<Vec<InjectedEntry>> {
        let (scene, entities) = {
            let db = self.db.lock().expect("mutex poisoned");
            let scene = db
                .get_scene(scene_id)
                .ok_or_else(|| BridgeError::Other(format!("Scene not found: {}", scene_id)))?;
            let entities = db.get_scene_entities(scene_id);
            (scene, entities)
        };

        // Build and save a scene snapshot
        let snapshot = entities_to_snapshot(&entities);
        {
            let mut memory = self.scene_memory.lock().expect("mutex poisoned");
            memory.save_scene_snapshot(scene.scene_id.clone(), scene.name.clone(), snapshot)?;
        }

        let mut entries = Vec::new();

        // L3 - Working Memory: current scene state
        if self.config.inject_working {
            entries.extend(self.prepare_working_entries(scene_id)?);
        }

        // L2 - Episodic Memory: scene load event
        if self.config.inject_episodic {
            entries.push(InjectedEntry {
                tier: "episodic".to_string(),
                summary: format!(
                    "Scene loaded: '{}' (id={}, {} entities)",
                    scene.name,
                    scene.scene_id,
                    entities.len()
                ),
                payload: serde_json::json!({
                    "scene_id": scene.scene_id,
                    "scene_name": scene.name,
                    "entity_count": entities.len(),
                    "is_active": scene.is_active,
                }),
                entity_ids: entities.iter().map(|e| e.entity_id).collect(),
                scene_id: scene.scene_id.clone(),
                importance: 0.8,
                tags: vec!["scene_loaded".to_string()],
            });
        }

        // L1 - Semantic Memory: entity-relation graph
        if self.config.inject_semantic && self.config.build_relations {
            entries.extend(self.prepare_semantic_entries(&scene, &entities));
        }

        // L0 - Procedural Memory: entity creation patterns
        if self.config.inject_procedural {
            entries.extend(self.prepare_procedural_entries(&scene, &entities));
        }

        Ok(entries)
    }

    /// Prepare working-memory entries: one per entity with current state.
    fn prepare_working_entries(&self, scene_id: &str) -> Result<Vec<InjectedEntry>> {
        let entities = {
            let db = self.db.lock().expect("mutex poisoned");
            db.get_scene_entities(scene_id)
        };

        let entries = entities
            .iter()
            .map(|entity| {
                let payload = serde_json::to_value(entity).unwrap_or_default();
                InjectedEntry {
                    tier: "working".to_string(),
                    summary: format!(
                        "Entity: {} (id={}, type={})",
                        entity.name, entity.entity_id, entity.entity_type
                    ),
                    payload,
                    entity_ids: vec![entity.entity_id],
                    scene_id: scene_id.to_string(),
                    importance: 0.4,
                    tags: vec![
                        "entity".to_string(),
                        entity.entity_type.clone(),
                        "scene_current".to_string(),
                    ],
                }
            })
            .collect();

        Ok(entries)
    }

    /// Prepare semantic-memory entries: entity types, component relations.
    fn prepare_semantic_entries(
        &self,
        scene: &SceneRecord,
        entities: &[EntityRecord],
    ) -> Vec<InjectedEntry> {
        let mut entries = Vec::new();

        // Scene node
        entries.push(InjectedEntry {
            tier: "semantic".to_string(),
            summary: format!("Scene: {}", scene.name),
            payload: serde_json::json!({
                "node_type": "scene",
                "name": scene.name,
                "scene_id": scene.scene_id,
                "entity_count": entities.len(),
            }),
            entity_ids: vec![],
            scene_id: scene.scene_id.clone(),
            importance: 0.7,
            tags: vec!["scene_node".to_string(), "semantic".to_string()],
        });

        // Entity type nodes with component relations
        let mut type_groups: HashMap<String, Vec<&EntityRecord>> = HashMap::new();
        for entity in entities {
            type_groups
                .entry(entity.entity_type.clone())
                .or_default()
                .push(entity);
        }

        for (entity_type, group) in &type_groups {
            entries.push(InjectedEntry {
                tier: "semantic".to_string(),
                summary: format!("EntityType: {} ({} instances)", entity_type, group.len()),
                payload: serde_json::json!({
                    "node_type": "entity_type",
                    "name": entity_type,
                    "instance_count": group.len(),
                    "scene_id": scene.scene_id,
                }),
                entity_ids: group.iter().map(|e| e.entity_id).collect(),
                scene_id: scene.scene_id.clone(),
                importance: 0.5,
                tags: vec!["entity_type".to_string(), "semantic".to_string()],
            });
        }

        // Component relations: which entities have which components
        for entity in entities {
            for component in &entity.components {
                entries.push(InjectedEntry {
                    tier: "semantic".to_string(),
                    summary: format!("Relation: {} HasA {}", entity.name, component.type_name),
                    payload: serde_json::json!({
                        "relation_type": "HasA",
                        "from_entity": entity.entity_id,
                        "from_name": entity.name,
                        "to_component": component.type_name,
                    }),
                    entity_ids: vec![entity.entity_id],
                    scene_id: scene.scene_id.clone(),
                    importance: 0.3,
                    tags: vec![
                        "component_relation".to_string(),
                        "HasA".to_string(),
                        "semantic".to_string(),
                    ],
                });
            }
        }

        entries
    }

    /// Prepare procedural-memory entries: entity creation/modification patterns.
    fn prepare_procedural_entries(
        &self,
        scene: &SceneRecord,
        entities: &[EntityRecord],
    ) -> Vec<InjectedEntry> {
        let mut entries = Vec::new();

        // Group entities by type to identify creation patterns
        let mut type_groups: HashMap<String, Vec<&EntityRecord>> = HashMap::new();
        for entity in entities {
            type_groups
                .entry(entity.entity_type.clone())
                .or_default()
                .push(entity);
        }

        for (entity_type, group) in &type_groups {
            // Build a workflow template for creating this entity type
            if let Some(sample) = group.first() {
                let component_names: Vec<String> = sample
                    .components
                    .iter()
                    .map(|c| c.type_name.clone())
                    .collect();

                entries.push(InjectedEntry {
                    tier: "procedural".to_string(),
                    summary: format!(
                        "Create {} entity: attach [{}]",
                        entity_type,
                        component_names.join(", ")
                    ),
                    payload: serde_json::json!({
                        "workflow": "create_entity",
                        "entity_type": entity_type,
                        "components": component_names,
                        "scene_id": scene.scene_id,
                        "pattern_confidence": 0.5,
                    }),
                    entity_ids: group.iter().map(|e| e.entity_id).collect(),
                    scene_id: scene.scene_id.clone(),
                    importance: 0.4,
                    tags: vec![
                        "workflow".to_string(),
                        "create_entity".to_string(),
                        entity_type.clone(),
                        "procedural".to_string(),
                    ],
                });
            }
        }

        // Modification pattern: entities with positions
        let positioned: Vec<&EntityRecord> =
            entities.iter().filter(|e| e.position.is_some()).collect();
        if !positioned.is_empty() {
            entries.push(InjectedEntry {
                tier: "procedural".to_string(),
                summary: format!(
                    "Positioned entities: {} have spatial positions",
                    positioned.len()
                ),
                payload: serde_json::json!({
                    "workflow": "position_entity",
                    "count": positioned.len(),
                    "scene_id": scene.scene_id,
                }),
                entity_ids: positioned.iter().map(|e| e.entity_id).collect(),
                scene_id: scene.scene_id.clone(),
                importance: 0.3,
                tags: vec![
                    "workflow".to_string(),
                    "position_entity".to_string(),
                    "procedural".to_string(),
                ],
            });
        }

        entries
    }

    // ========================================================================
    // Token budget control
    // ========================================================================

    /// Estimate the token count for a single injected entry.
    ///
    /// Uses a simple heuristic: ~1 token per 4 characters for CJK
    /// and ~1 token per 3 characters for ASCII (conservative).
    pub fn token_estimate(entry: &InjectedEntry) -> usize {
        let payload_str = entry.payload.to_string();
        let text = format!("{} {}", entry.summary, payload_str);
        let chars = text.chars().count();
        let cjk_count = text.chars().filter(|c| c > &'\u{2E80}').count();
        let ascii_count = chars - cjk_count;
        cjk_count / 4 + ascii_count / 3 + 1
    }

    /// Trim entries to fit within a token budget.
    ///
    /// Entries are sorted by importance (highest first) and kept until
    /// the budget is exhausted. Returns the trimmed list.
    pub fn trim_to_budget(entries: Vec<InjectedEntry>, budget: usize) -> Vec<InjectedEntry> {
        if entries.is_empty() {
            return entries;
        }
        let mut sorted: Vec<InjectedEntry> = entries;
        sorted.sort_by(|a, b| b.importance.partial_cmp(&a.importance).unwrap());
        let mut used = 0usize;
        let mut result = Vec::with_capacity(sorted.len());
        for entry in sorted {
            let tokens = Self::token_estimate(&entry);
            if used + tokens > budget {
                continue;
            }
            used += tokens;
            result.push(entry);
        }
        result
    }

    /// Inject a project profile summary into memory.
    ///
    /// Builds a compact project overview (name, scene count, top entity
    /// types, key patterns) and injects it as a semantic memory entry.
    /// Respects the configured token budget.
    pub fn inject_project_profile(&mut self, project_name: &str) -> Result<Vec<InjectedEntry>> {
        let scenes = {
            let db = self.db.lock().expect("mutex poisoned");
            db.get_all_scenes()
        };

        let mut total_entities = 0usize;
        let mut entity_types: HashMap<String, usize> = HashMap::new();
        let mut entity_names: Vec<String> = Vec::new();

        for scene in &scenes {
            let entities = {
                let db = self.db.lock().expect("mutex poisoned");
                db.get_scene_entities(&scene.scene_id)
            };
            total_entities += entities.len();
            for entity in &entities {
                *entity_types.entry(entity.entity_type.clone()).or_default() += 1;
                entity_names.push(entity.name.clone());
            }
        }

        let mut top_types: Vec<(&String, &usize)> = entity_types.iter().collect();
        top_types.sort_by(|a, b| b.1.cmp(a.1));
        let top_types_str: Vec<String> = top_types
            .iter()
            .take(5)
            .map(|(t, c)| format!("{} (x{})", t, c))
            .collect();

        let profile = serde_json::json!({
            "project": project_name,
            "scene_count": scenes.len(),
            "total_entities": total_entities,
            "top_entity_types": top_types_str,
            "entity_names": entity_names.iter().take(20).collect::<Vec<_>>(),
        });

        let mut entries = vec![InjectedEntry {
            tier: "semantic".to_string(),
            summary: format!(
                "Project '{}': {} scenes, {} entities, top types: [{}]",
                project_name,
                scenes.len(),
                total_entities,
                top_types_str.join(", ")
            ),
            payload: profile,
            entity_ids: vec![],
            scene_id: String::new(),
            importance: 0.9,
            tags: vec!["project_profile".to_string(), "semantic".to_string()],
        }];

        if let Some(budget) = self.config.max_tokens_per_injection {
            entries = Self::trim_to_budget(entries, budget);
        }

        self.stats.record_success(entries.len(), 0);
        Ok(entries)
    }
}

impl Drop for MulticaMemoryInjector {
    fn drop(&mut self) {
        if let (Some(ref event_bus), Some(subscriber_id)) = (&self.event_bus, self.subscriber_id) {
            if let Ok(mut bus) = event_bus.lock() {
                bus.unsubscribe(subscriber_id);
            }
        }
    }
}

// ============================================================================
// SceneMemoryEventHandler
// ============================================================================

/// Event handler that subscribes to the scene_event_bus and triggers
/// memory injection when scene entities change.
///
/// Implements `SceneEventSubscriber` so it can be registered on a
/// `SceneEventBus`. When events arrive, it delegates to the parent
/// `MulticaMemoryInjector` to prepare tier-specific memory entries.
pub struct SceneMemoryEventHandler {
    /// The injector that processes events. This is set via interior
    /// mutability because the `SceneEventSubscriber` trait takes `&self`.
    /// In practice, the handler is created first, registered on the bus,
    /// and then the injector reference is wired up.
    pub injector: Option<Arc<Mutex<MulticaMemoryInjector>>>,
}

impl SceneMemoryEventHandler {
    /// Create a new event handler with no injector yet.
    /// The injector must be set before events are processed.
    pub fn new() -> Self {
        Self { injector: None }
    }

    /// Create a new event handler wired to the given injector.
    pub fn with_injector(injector: Arc<Mutex<MulticaMemoryInjector>>) -> Self {
        Self {
            injector: Some(injector),
        }
    }
}

impl Default for SceneMemoryEventHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl SceneEventSubscriber for SceneMemoryEventHandler {
    fn on_event(&self, event: &SceneEvent) {
        if let Some(ref injector) = self.injector {
            let mut inj = injector.lock().expect("mutex poisoned");
            match inj.inject_from_event(event) {
                Ok(entries) => {
                    debug!(
                        "SceneMemoryEventHandler: injected {} entries for event {:?} on entity {}",
                        entries.len(),
                        event.event_type,
                        event.entity_id
                    );
                }
                Err(e) => {
                    warn!(
                        "SceneMemoryEventHandler: failed to inject from event: {}",
                        e
                    );
                }
            }
        } else {
            warn!(
                "SceneMemoryEventHandler: no injector wired, event {:?} on entity {} dropped",
                event.event_type, event.entity_id
            );
        }
    }
}

// ============================================================================
// Factory Functions
// ============================================================================

/// Create a thread-safe shared memory injector with the given configuration.
///
/// The injector wraps a `SceneContextMemory` and `MulticaDb` internally.
/// Callers should provide the shared instances they already hold.
pub fn create_shared_memory_injector(
    config: MemoryInjectorConfig,
) -> Arc<Mutex<MulticaMemoryInjector>> {
    let scene_memory = crate::memory_scene_context::create_shared_scene_context_memory();
    let db = crate::multica_db::create_shared_multica_db();
    Arc::new(Mutex::new(MulticaMemoryInjector::new(
        scene_memory,
        db,
        config,
    )))
}

/// Create a thread-safe shared memory injector with four-tier integration support.
///
/// Same as `create_shared_memory_injector` but also initializes the internal
/// state for four-tier memory injection (all layers enabled by default).
pub fn create_shared_memory_injector_with_four_tier(
    config: MemoryInjectorConfig,
) -> Arc<Mutex<MulticaMemoryInjector>> {
    let mut config = config;
    // Ensure all four tiers are enabled for four-tier mode
    config.inject_working = true;
    config.inject_episodic = true;
    config.inject_semantic = true;
    config.inject_procedural = true;
    config.build_relations = true;
    create_shared_memory_injector(config)
}

// ============================================================================
// Helpers
// ============================================================================

/// Convert a slice of `EntityRecord` into a `SceneSnapshot`.
fn entities_to_snapshot(entities: &[EntityRecord]) -> SceneSnapshot {
    let scene_entities: Vec<SceneEntity> = entities
        .iter()
        .map(|e| SceneEntity {
            id: e.entity_id,
            name: e.name.clone(),
            components: e.components.clone(),
            position: e.position,
        })
        .collect();

    SceneSnapshot {
        version: 1,
        timestamp: chrono::Utc::now().to_rfc3339(),
        entities: scene_entities,
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory_scene_context::create_shared_scene_context_memory;
    use crate::multica_db::create_shared_multica_db;

    fn create_test_injector() -> MulticaMemoryInjector {
        let scene_memory = create_shared_scene_context_memory();
        let db = create_shared_multica_db();
        MulticaMemoryInjector::new(scene_memory, db, MemoryInjectorConfig::default())
    }

    fn setup_test_scene(injector: &mut MulticaMemoryInjector) -> String {
        let mut db = injector.db.lock().expect("mutex poisoned");
        let scene_id = db
            .create_scene("TestScene".to_string(), Some("A test scene".to_string()))
            .unwrap();
        db.create_entity(
            scene_id.clone(),
            "Player".to_string(),
            "GameObject".to_string(),
            vec![ComponentData {
                type_name: "Transform".to_string(),
                properties: {
                    let mut m = HashMap::new();
                    m.insert("x".to_string(), serde_json::json!(0.0));
                    m.insert("y".to_string(), serde_json::json!(0.0));
                    m
                },
            }],
            Some([0.0, 0.0, 0.0]),
        )
        .unwrap();
        db.create_entity(
            scene_id.clone(),
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
        scene_id
    }

    #[test]
    fn test_memory_injector_config_default() {
        let config = MemoryInjectorConfig::default();
        assert!(config.auto_inject);
        assert_eq!(config.inject_interval_secs, 1);
        assert!(config.inject_working);
        assert!(config.inject_episodic);
        assert!(config.inject_semantic);
        assert!(config.inject_procedural);
    }

    #[test]
    fn test_memory_injector_config_manual_only() {
        let config = MemoryInjectorConfig::manual_only();
        assert!(!config.auto_inject);
    }

    #[test]
    fn test_inject_scene_produces_entries() {
        let mut injector = create_test_injector();
        let scene_id = setup_test_scene(&mut injector);

        let entries = injector.inject_scene(&scene_id).unwrap();
        // Should produce working + episodic + semantic + procedural entries
        assert!(!entries.is_empty(), "Should produce at least some entries");

        // Check tier distribution
        let working_count = entries.iter().filter(|e| e.tier == "working").count();
        let episodic_count = entries.iter().filter(|e| e.tier == "episodic").count();
        let semantic_count = entries.iter().filter(|e| e.tier == "semantic").count();
        let procedural_count = entries.iter().filter(|e| e.tier == "procedural").count();

        assert_eq!(
            working_count, 2,
            "Should have 2 working entries (one per entity)"
        );
        assert_eq!(
            episodic_count, 1,
            "Should have 1 episodic entry (scene load)"
        );
        assert!(
            semantic_count > 0,
            "Should have semantic entries for entity types and relations"
        );
        assert!(
            procedural_count > 0,
            "Should have procedural entries for creation patterns"
        );

        // Verify stats
        let stats = injector.get_stats();
        assert_eq!(stats.injections_count, 1);
        assert_eq!(stats.errors_count, 0);
        assert!(stats.last_injection_time.is_some());
    }

    #[test]
    fn test_inject_scene_not_found() {
        let mut injector = create_test_injector();
        let result = injector.inject_scene("nonexistent");
        assert!(result.is_err());

        let stats = injector.get_stats();
        assert_eq!(stats.errors_count, 1);
    }

    #[test]
    fn test_inject_changes() {
        let mut injector = create_test_injector();
        let scene_id = setup_test_scene(&mut injector);

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

        let entries = injector.inject_changes(&scene_id, &diff).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].tier, "episodic");
        assert!(entries[0].summary.contains("Entity created"));
        assert!(entries[0].summary.contains("NewObject"));

        let stats = injector.get_stats();
        assert_eq!(stats.changes_processed, 1);
    }

    #[test]
    fn test_inject_from_event() {
        let mut injector = create_test_injector();
        let scene_id = setup_test_scene(&mut injector);

        let event = SceneEvent {
            scene_id: scene_id.clone(),
            event_type: SceneEventType::EntityCreated,
            entity_id: 100,
            entity_before: None,
            entity_after: Some(SceneEntity {
                id: 100,
                name: "SpawnedEnemy".to_string(),
                components: vec![],
                position: Some([5.0, 0.0, 0.0]),
            }),
            component: None,
            timestamp: chrono::Utc::now().to_rfc3339(),
        };

        let entries = injector.inject_from_event(&event).unwrap();
        assert!(!entries.is_empty(), "Should produce entries from event");

        // Should have an episodic entry
        let episodic = entries.iter().find(|e| e.tier == "episodic");
        assert!(episodic.is_some());
        assert!(episodic.unwrap().summary.contains("SpawnedEnemy"));

        let stats = injector.get_stats();
        assert_eq!(stats.events_handled, 1);
    }

    #[test]
    fn test_injection_stats_tracking() {
        let mut injector = create_test_injector();

        let mut stats = injector.get_stats();
        assert_eq!(stats.injections_count, 0);
        assert_eq!(stats.errors_count, 0);
        assert_eq!(stats.last_injection_time, None);
        assert!(!stats.injection_in_progress);

        // Trigger a failed injection
        let _ = injector.inject_scene("missing_scene");

        stats = injector.get_stats();
        assert_eq!(stats.errors_count, 1);

        // Reset
        injector.reset_stats();
        stats = injector.get_stats();
        assert_eq!(stats.errors_count, 0);
    }

    #[test]
    fn test_scene_memory_event_handler() {
        let scene_memory = create_shared_scene_context_memory();
        let db = create_shared_multica_db();
        let injector = Arc::new(Mutex::new(MulticaMemoryInjector::new(
            scene_memory.clone(),
            db.clone(),
            MemoryInjectorConfig::default(),
        )));

        // Setup a scene first and capture the actual scene_id
        let scene_id = {
            let mut db = db.lock().unwrap();
            let sid = db.create_scene("HandlerScene".to_string(), None).unwrap();
            db.create_entity(
                sid.clone(),
                "TestEntity".to_string(),
                "GameObject".to_string(),
                vec![],
                None,
            )
            .unwrap();
            sid
        };

        let handler = SceneMemoryEventHandler::with_injector(injector.clone());

        let event = SceneEvent {
            scene_id: scene_id.clone(),
            event_type: SceneEventType::EntityCreated,
            entity_id: 42,
            entity_before: None,
            entity_after: Some(SceneEntity {
                id: 42,
                name: "DynamicEntity".to_string(),
                components: vec![],
                position: None,
            }),
            component: None,
            timestamp: chrono::Utc::now().to_rfc3339(),
        };

        // Handler should not panic
        handler.on_event(&event);

        // Verify event was processed
        let stats = injector.lock().unwrap().get_stats();
        assert_eq!(stats.events_handled, 1);
    }
}
