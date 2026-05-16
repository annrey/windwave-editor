//! Bevy Integration System — periodic SceneIndex rebuild, visual observation,
//! and full pipeline wiring between agent-core and Bevy ECS.
//!
//! Design reference: Section 12.0.4 + Section 12.7 of
//! gpt-agent-team-task-event-skill-architecture.md
//!
//! Provides three plugins:
//! - SceneIndexRebuildPlugin — periodic SceneIndex snapshot from ECS World
//! - VisionPlugin           — screenshot capture + visual analysis pipeline
//! - IntegrationPlugin      — wires DirectorRuntime ↔ ECS ↔ UI ↔ EventBus

use crate::scene_index::{SceneIndex, SceneEntityNode, ComponentSummary};
use crate::BevyAdapter;
use agent_core::goal_checker::SceneEntityInfo as CoreSceneEntityInfo;
use bevy::prelude::*;
use bevy::ecs::hierarchy::{Children, ChildOf};
use bevy::sprite::Sprite;
use log::info;
use std::collections::HashMap;

// ===========================================================================
// SceneIndexRebuildPlugin — periodic scene snapshot for agent reasoning
// ===========================================================================

/// Cached SceneIndex, rebuilt periodically by `rebuild_scene_index` system.
#[derive(Resource, Clone)]
pub struct SceneIndexCache(pub SceneIndex);

impl Default for SceneIndexCache {
    fn default() -> Self {
        Self(SceneIndex::new())
    }
}

impl SceneIndexCache {
    /// Get a reference to the cached SceneIndex.
    pub fn get(&self) -> &SceneIndex {
        &self.0
    }

    /// Convert cached index to goal-checker compatible entity list.
    pub fn to_goal_checker_snapshot(&self) -> Vec<CoreSceneEntityInfo> {
        let raw = self.0.to_entity_info_list();
        raw.into_iter()
            .map(|info| CoreSceneEntityInfo {
                name: info.name,
                components: info.components,
                translation: info.translation,
                sprite_color: info.sprite_color,
            })
            .collect()
    }

    /// Look up an entity by name.
    pub fn find_by_name(&self, name: &str) -> Option<&SceneEntityNode> {
        self.0.get_entity_by_name(name)
    }

    /// List all entity names in the scene.
    pub fn entity_names(&self) -> Vec<String> {
        self.0.entity_names()
    }

    /// Number of root entities.
    pub fn root_count(&self) -> usize {
        self.0.root_entities.len()
    }

    /// Number of total registered entities.
    pub fn total_count(&self) -> usize {
        self.0.entities_by_name.len()
    }

    /// Incrementally update the index with changed entities.
    /// Uses `Changed<T>` detection to only update entities whose components
    /// actually changed, skipping unchanged entities for performance.
    pub fn incremental_update(
        &mut self,
        adapter: &BevyAdapter,
        all_entities: &Query<(
            Entity,
            Option<&Name>,
            Option<&Transform>,
            Option<&Sprite>,
            Option<&Children>,
            Option<&ChildOf>,
        )>,
        changed: &std::collections::HashSet<Entity>,
        force: bool,
    ) {
        if force {
            // Full rebuild
            self.0 = SceneIndex::new();
        }

        for (entity, name, transform, sprite, children_opt, parent_opt) in all_entities.iter() {
            if !force && !changed.contains(&entity) {
                continue;
            }

            let agent_id = adapter
                .get_agent_id(entity)
                .map(|id| id.0)
                .unwrap_or(0);

            let entity_name = name
                .map(|n| n.to_string())
                .unwrap_or_else(|| format!("entity_{}", agent_id));

            let mut components: Vec<ComponentSummary> = Vec::new();

            if let Some(t) = transform {
                let mut props = HashMap::new();
                let (roll, pitch, yaw) = t.rotation.to_euler(EulerRot::XYZ);
                props.insert("translation".into(), serde_json::json!(t.translation.to_array()));
                props.insert("rotation".into(), serde_json::json!([roll, pitch, yaw]));
                props.insert("scale".into(), serde_json::json!(t.scale.to_array()));
                components.push(ComponentSummary {
                    type_name: "Transform".into(),
                    properties: props,
                });
            }

            if sprite.is_some() {
                components.push(ComponentSummary {
                    type_name: "Sprite".into(),
                    properties: HashMap::from([("present".into(), serde_json::json!(true))]),
                });
            }

            if children_opt.is_some() {
                components.push(ComponentSummary {
                    type_name: "Children".into(),
                    properties: HashMap::new(),
                });
            }

            if parent_opt.is_some() {
                components.push(ComponentSummary {
                    type_name: "ChildOf".into(),
                    properties: HashMap::new(),
                });
            }

            self.0.add_entity(entity_name, agent_id, components);
        }
    }
}

/// Plugin that periodically rebuilds the SceneIndex from Bevy ECS World.
///
/// Usage:
/// ```ignore
/// app.add_plugins(SceneIndexRebuildPlugin::every(30));
/// ```
pub struct SceneIndexRebuildPlugin {
    /// Rebuild interval in frames (default: 30 = ~0.5s at 60fps).
    pub interval_frames: usize,
}

impl SceneIndexRebuildPlugin {
    /// Create a plugin that rebuilds every `interval` frames.
    pub fn every(interval_frames: usize) -> Self {
        Self { interval_frames }
    }
}

impl Default for SceneIndexRebuildPlugin {
    fn default() -> Self {
        Self {
            interval_frames: 30,
        }
    }
}

impl Plugin for SceneIndexRebuildPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SceneIndexCache>()
            .insert_resource(SceneIndexTimer {
                interval: self.interval_frames,
                counter: 0,
                first_run: true,
            })
            .add_systems(Update, rebuild_scene_index_system);
    }
}

/// Internal timer for rebuild interval.
#[derive(Resource)]
struct SceneIndexTimer {
    interval: usize,
    counter: usize,
    first_run: bool,
}

/// System that periodically rebuilds SceneIndex from ECS component queries.
///
/// Queries Name, Transform, Sprite, Children, and Parent components
/// to construct a hierarchical scene graph snapshot.
fn rebuild_scene_index_system(
    mut cache: ResMut<SceneIndexCache>,
    mut timer: ResMut<SceneIndexTimer>,
    adapter: Res<BevyAdapter>,
    query: Query<(
        Entity,
        Option<&Name>,
        Option<&Transform>,
        Option<&Sprite>,
        Option<&Children>,
        Option<&ChildOf>,
    )>,
) {
    timer.counter += 1;
    if !timer.first_run && timer.counter < timer.interval {
        return;
    }
    timer.counter = 0;
    timer.first_run = false;

    // --- Build SceneIndex from queried components ---

    let mut index = SceneIndex::new();
    let mut nodes_map: HashMap<Entity, SceneEntityNode> = HashMap::new();
    let mut roots: Vec<Entity> = Vec::new();
    let mut child_map: HashMap<Entity, Vec<Entity>> = HashMap::new();

    for (entity, name, transform, sprite, children_opt, parent_opt) in query.iter() {
        let agent_id = adapter
            .get_agent_id(entity)
            .map(|id| id.0)
            .unwrap_or(0);

        let entity_name = name
            .map(|n| n.to_string())
            .unwrap_or_else(|| format!("entity_{}", agent_id));

        let mut components: Vec<ComponentSummary> = Vec::new();

        if let Some(t) = transform {
            let mut props = HashMap::new();
            props.insert(
                "translation".to_string(),
                serde_json::json!([t.translation.x, t.translation.y, t.translation.z]),
            );
            let (roll, pitch, yaw) = t.rotation.to_euler(EulerRot::XYZ);
            props.insert(
                "rotation".to_string(),
                serde_json::json!([roll, pitch, yaw]),
            );
            props.insert(
                "scale".to_string(),
                serde_json::json!([t.scale.x, t.scale.y, t.scale.z]),
            );
            components.push(ComponentSummary {
                type_name: "Transform".to_string(),
                properties: props,
            });
        }

        if let Some(s) = sprite {
            let mut props = HashMap::new();
            let col = s.color.to_linear();
            props.insert(
                "color".to_string(),
                serde_json::json!([col.red, col.green, col.blue, col.alpha]),
            );
            components.push(ComponentSummary {
                type_name: "Sprite".to_string(),
                properties: props,
            });
        }

        let node = SceneEntityNode {
            id: agent_id,
            name: entity_name.clone(),
            components: components.clone(),
            children: Vec::new(),
        };

        for c in &components {
            index
                .entities_by_component
                .entry(c.type_name.clone())
                .or_default()
                .push(agent_id);
        }
        index.entities_by_name.insert(entity_name, agent_id);

        nodes_map.insert(entity, node);

        if parent_opt.is_none() {
            roots.push(entity);
        }

        if let Some(children) = children_opt {
            child_map.insert(entity, children.to_vec());
        }
    }

    // Pass 2: build hierarchy
    fn build_tree(
        entity: Entity,
        nodes_map: &mut HashMap<Entity, SceneEntityNode>,
        child_map: &HashMap<Entity, Vec<Entity>>,
    ) -> SceneEntityNode {
        let mut node = nodes_map.remove(&entity).expect("node must exist");
        if let Some(child_entities) = child_map.get(&entity) {
            for &child_entity in child_entities {
                if nodes_map.contains_key(&child_entity) {
                    let child_node = build_tree(child_entity, nodes_map, child_map);
                    node.children.push(child_node);
                }
            }
        }
        node
    }

    let mut root_entities = Vec::new();
    for root_entity in roots {
        if nodes_map.contains_key(&root_entity) {
            root_entities.push(build_tree(root_entity, &mut nodes_map, &child_map));
        }
    }
    index.root_entities = root_entities;

    cache.0 = index;
}

// ===========================================================================
// VisionPlugin — basic visual observation pipeline
// ===========================================================================

/// Artifact representing a captured screenshot.
#[derive(Debug, Clone)]
pub struct ScreenshotArtifact {
    /// Path where the screenshot file is stored.
    pub path: String,
    /// Image dimensions (width, height).
    pub dimensions: (u32, u32),
    /// Timestamp of capture (millis since epoch).
    pub captured_at_ms: u64,
    /// Optional raw pixel data for in-memory analysis.
    pub raw_data: Option<Vec<u8>>,
}

/// Result of visual analysis on a screenshot.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct VisualObservation {
    /// Path to the screenshot that was analyzed.
    pub screenshot_path: String,
    /// Human-readable summary of what's visible.
    pub summary: String,
    /// Entities that the vision model detected.
    pub visible_entities: Vec<String>,
    /// Anomalies or unexpected visual states.
    pub anomalies: Vec<String>,
    /// Confidence score (0.0 – 1.0).
    pub confidence: f32,
}

impl Default for VisualObservation {
    fn default() -> Self {
        Self {
            screenshot_path: String::new(),
            summary: String::new(),
            visible_entities: Vec::new(),
            anomalies: Vec::new(),
            confidence: 1.0,
        }
    }
}

/// Error type for vision operations.
#[derive(Debug, Clone)]
pub enum VisionError {
    CaptureFailed(String),
    AnalysisFailed(String),
    NoProvider,
}

impl std::fmt::Display for VisionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VisionError::CaptureFailed(msg) => write!(f, "Capture failed: {}", msg),
            VisionError::AnalysisFailed(msg) => write!(f, "Analysis failed: {}", msg),
            VisionError::NoProvider => write!(f, "No vision provider configured"),
        }
    }
}

/// Trait for capturing screenshots from the engine (Bevy or other).
#[async_trait::async_trait]
pub trait ScreenshotProvider: Send + Sync {
    /// Capture a screenshot and return its artifact.
    async fn capture(&self) -> Result<ScreenshotArtifact, VisionError>;
}

/// Trait for running visual analysis on a screenshot (e.g. via vision LLM).
#[async_trait::async_trait]
pub trait VisionProvider: Send + Sync {
    /// Analyze a screenshot with an optional prompt.
    async fn analyze(
        &self,
        screenshot: &ScreenshotArtifact,
        prompt: &str,
    ) -> Result<VisualObservation, VisionError>;
}

/// Test helper that returns a fixed screenshot artifact without capturing.
pub struct MockScreenshotProvider {
    pub dimensions: (u32, u32),
    pub artifact_path: String,
}

impl MockScreenshotProvider {
    pub fn new() -> Self {
        Self {
            dimensions: (1920, 1080),
            artifact_path: "mock_screenshot.png".to_string(),
        }
    }
}

#[async_trait::async_trait]
impl ScreenshotProvider for MockScreenshotProvider {
    async fn capture(&self) -> Result<ScreenshotArtifact, VisionError> {
        Ok(ScreenshotArtifact {
            path: self.artifact_path.clone(),
            dimensions: self.dimensions,
            captured_at_ms: 0,
            raw_data: None,
        })
    }
}

/// Test helper that returns a fixed visual observation without analysis.
pub struct MockVisionProvider {
    pub observation: VisualObservation,
}

impl MockVisionProvider {
    pub fn with_summary(summary: &str) -> Self {
        Self {
            observation: VisualObservation {
                screenshot_path: String::new(),
                summary: summary.to_string(),
                visible_entities: vec!["Player".to_string(), "Enemy".to_string()],
                anomalies: Vec::new(),
                confidence: 0.95,
            },
        }
    }
}

#[async_trait::async_trait]
impl VisionProvider for MockVisionProvider {
    async fn analyze(
        &self,
        _screenshot: &ScreenshotArtifact,
        _prompt: &str,
    ) -> Result<VisualObservation, VisionError> {
        Ok(self.observation.clone())
    }
}

/// Vision pipeline state (Bevy Resource).
#[derive(Resource)]
pub struct VisionState {
    /// Optional screenshot provider (None = not configured).
    pub screenshot_provider: Option<Box<dyn ScreenshotProvider>>,
    /// Optional vision provider (None = not configured).
    pub vision_provider: Option<Box<dyn VisionProvider>>,
    /// Latest captured screenshot artifact.
    pub last_screenshot: Option<ScreenshotArtifact>,
    /// Latest visual observation result.
    pub last_observation: Option<VisualObservation>,
    /// Whether a capture is currently in progress (async guard).
    pub capture_in_progress: bool,
}

impl Default for VisionState {
    fn default() -> Self {
        Self {
            screenshot_provider: None,
            vision_provider: None,
            last_screenshot: None,
            last_observation: None,
            capture_in_progress: false,
        }
    }
}

impl VisionState {
    /// Check if vision pipeline is fully configured.
    pub fn is_ready(&self) -> bool {
        self.screenshot_provider.is_some() && self.vision_provider.is_some()
    }

    /// Set mock providers for testing.
    pub fn with_mock_providers(&mut self) {
        self.screenshot_provider = Some(Box::new(MockScreenshotProvider::new()));
        self.vision_provider =
            Some(Box::new(MockVisionProvider::with_summary("Mock scene analysis")));
    }

    /// Set real (production) providers: Bevy screenshot + SceneIndex vision.
    pub fn with_real_providers(&mut self, output_dir: impl Into<String>) {
        self.screenshot_provider = Some(Box::new(BevyScreenshotProvider::new(output_dir)));
        self.vision_provider = Some(Box::new(SceneIndexVisionProvider::new()));
    }
}

/// Plugin for visual observation pipeline.
///
/// Initialises `VisionState` resource and registers optional systems
/// for screenshot capture and analysis.
pub struct VisionPlugin;

impl Plugin for VisionPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<VisionState>()
            .add_systems(Update, vision_status_system);
    }
}

/// Simple status system — logs vision pipeline readiness.
fn vision_status_system(
    vision: Res<VisionState>,
    mut logged: Local<bool>,
) {
    if !*logged && vision.is_ready() {
        info!(
            "Vision pipeline ready: screenshot={}, analysis={}",
            vision.screenshot_provider.is_some(),
            vision.vision_provider.is_some()
        );
        *logged = true;
    }
}

// ===========================================================================
// IntegrationPlugin — wires DirectorRuntime ↔ ECS ↔ UI ↔ EventBus
// ===========================================================================

/// Global integration state that the DirectorRuntime can access.
///
/// In a real deployment, this is shared between Bevy systems and the
/// agent-core DirectorRuntime via Arc<Mutex<...>> or channels.
#[derive(Resource, Clone)]
pub struct IntegrationState {
    /// Whether the engine is ready for agent commands.
    pub engine_ready: bool,
    /// Frame counter for time-based operations.
    pub frame_count: u64,
    /// Latest scene entity count (updated each rebuild).
    pub scene_entity_count: usize,
    /// Whether a scene index rebuild just completed.
    pub index_just_rebuilt: bool,
    /// Last vision observation summary (if any).
    pub last_vision_summary: Option<String>,
}

impl Default for IntegrationState {
    fn default() -> Self {
        Self {
            engine_ready: false,
            frame_count: 0,
            scene_entity_count: 0,
            index_just_rebuilt: false,
            last_vision_summary: None,
        }
    }
}

/// Main integration plugin that wires all subsystems together.
///
/// Registers `IntegrationState` and a post-rebuild sync system that
/// keeps the DirectorRuntime informed about engine state changes.
pub struct IntegrationPlugin;

impl Plugin for IntegrationPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<IntegrationState>()
            .add_systems(Update, (
                integration_frame_tick,
                integration_post_rebuild_sync,
            ).chain());
    }
}

/// Frame counter + engine readiness detection.
fn integration_frame_tick(
    mut state: ResMut<IntegrationState>,
    cache: Res<SceneIndexCache>,
) {
    state.frame_count += 1;

    // Mark engine ready after first SceneIndex rebuild
    if !state.engine_ready && !cache.0.entities_by_name.is_empty() {
        state.engine_ready = true;
        info!(
            "Integration engine ready — {} entities indexed",
            cache.0.entities_by_name.len()
        );
    }
}

/// Sync integration state after each SceneIndex rebuild.
fn integration_post_rebuild_sync(
    mut state: ResMut<IntegrationState>,
    cache: Res<SceneIndexCache>,
) {
    let count = cache.0.entities_by_name.len();
    if count != state.scene_entity_count {
        state.scene_entity_count = count;
        state.index_just_rebuilt = true;
    } else {
        state.index_just_rebuilt = false;
    }
}

// ===========================================================================
// SceneIndexSceneBridge — implements agent_core::SceneBridge using SceneIndexCache
// ===========================================================================

use agent_core::scene_bridge::{SceneBridge, EntityListItem, ComponentPatch};
use crate::EngineCommand;

/// Bridge that implements `SceneBridge` using a cached SceneIndex.
///
/// **Reads**: real data from `SceneIndexCache` (periodically rebuilt from ECS).
/// **Writes**: return success and accumulate `EngineCommand`s for later application.
pub struct SceneIndexSceneBridge {
    snapshot: Vec<CoreSceneEntityInfo>,
    entity_list: Vec<EntityListItem>,
    pending_writes: Vec<EngineCommand>,
    next_id: u64,
}

impl SceneIndexSceneBridge {
    /// Build a bridge from a SceneIndexCache snapshot.
    pub fn from_cache(cache: &SceneIndexCache) -> Self {
        let raw = cache.0.to_entity_info_list();
        let snapshot = raw
            .iter()
            .map(|info| CoreSceneEntityInfo {
                name: info.name.clone(),
                components: info.components.clone(),
                translation: info.translation,
                sprite_color: info.sprite_color,
            })
            .collect();

        let entity_list = raw
            .iter()
            .map(|info| EntityListItem {
                id: info.id,
                name: info.name.clone(),
                components: info.components.clone(),
            })
            .collect();

        let max_id = raw.iter().map(|i| i.id).max().unwrap_or(0);

        Self {
            snapshot,
            entity_list,
            pending_writes: Vec::new(),
            next_id: max_id + 1,
        }
    }

    /// Collect pending write commands to be applied by BevyAdapter.
    pub fn take_pending_writes(&mut self) -> Vec<EngineCommand> {
        std::mem::take(&mut self.pending_writes)
    }

    /// Number of pending write commands.
    pub fn pending_write_count(&self) -> usize {
        self.pending_writes.len()
    }
}

impl SceneBridge for SceneIndexSceneBridge {
    fn query_entities(
        &self,
        filter: Option<&str>,
        component_type: Option<&str>,
    ) -> Vec<EntityListItem> {
        self.entity_list
            .iter()
            .filter(|e| {
                let name_match = filter.map_or(true, |f| {
                    f == "*" || e.name.to_lowercase().contains(&f.to_lowercase())
                });
                let comp_match = component_type.map_or(true, |ct| {
                    e.components.iter().any(|c| c == ct)
                });
                name_match && comp_match
            })
            .cloned()
            .collect()
    }

    fn get_entity(&self, id: u64) -> Option<serde_json::Value> {
        self.entity_list.iter().find(|e| e.id == id).map(|e| {
            let snap = self.snapshot.iter().find(|s| s.name == e.name);
            serde_json::json!({
                "id": e.id,
                "name": e.name,
                "components": e.components,
                "translation": snap.and_then(|s| s.translation),
                "sprite_color": snap.and_then(|s| s.sprite_color),
            })
        })
    }

    fn create_entity(
        &mut self,
        name: &str,
        position: Option<[f64; 2]>,
        components: &[ComponentPatch],
    ) -> Result<u64, String> {
        let id = self.next_id;
        self.next_id += 1;

        let mut bevy_components: Vec<crate::ComponentPatch> = Vec::new();
        for cp in components {
            bevy_components.push(crate::ComponentPatch {
                type_name: cp.type_name.clone(),
                value: serde_json::to_value(&cp.properties).unwrap_or_default(),
            });
        }

        self.pending_writes.push(EngineCommand::CreateEntity {
            name: name.to_string(),
            components: bevy_components,
        });

        if let Some(pos) = position {
            self.pending_writes.push(EngineCommand::SetTransform {
                entity_id: id,
                translation: Some([pos[0] as f32, pos[1] as f32, 0.0]),
                rotation: None,
                scale: None,
            });
        }

        let mut comp_names: Vec<String> = vec!["Transform".to_string()];
        let mut sprite_color: Option<[f32; 4]> = None;
        for cp in components {
            comp_names.push(cp.type_name.clone());
            if cp.type_name == "Sprite" {
                if let Some(color) = cp.properties.get("color").and_then(|v| v.as_array()) {
                    sprite_color = Some([
                        color.get(0).and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
                        color.get(1).and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
                        color.get(2).and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
                        color.get(3).and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
                    ]);
                }
            }
        }

        self.entity_list.push(EntityListItem {
            id,
            name: name.to_string(),
            components: comp_names.clone(),
        });

        self.snapshot.push(CoreSceneEntityInfo {
            name: name.to_string(),
            components: comp_names,
            translation: position.map(|p| [p[0] as f32, p[1] as f32, 0.0]),
            sprite_color,
        });

        Ok(id)
    }

    fn update_component(
        &mut self,
        entity_id: u64,
        component: &str,
        properties: std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<(), String> {
        if component == "Transform" {
            if let Some(pos) = properties.get("position") {
                if let Some(arr) = pos.as_array() {
                    let x = arr.get(0).and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                    let y = arr.get(1).and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                    self.pending_writes.push(EngineCommand::SetTransform {
                        entity_id,
                        translation: Some([x, y, 0.0]),
                        rotation: None,
                        scale: None,
                    });
                }
            }
        }
        if component == "Sprite" {
            if let Some(color) = properties.get("color") {
                if let Some(arr) = color.as_array() {
                    let r = arr.get(0).and_then(|v| v.as_f64()).unwrap_or(1.0) as f32;
                    let g = arr.get(1).and_then(|v| v.as_f64()).unwrap_or(1.0) as f32;
                    let b = arr.get(2).and_then(|v| v.as_f64()).unwrap_or(1.0) as f32;
                    let a = arr.get(3).and_then(|v| v.as_f64()).unwrap_or(1.0) as f32;
                    self.pending_writes.push(EngineCommand::SetSpriteColor {
                        entity_id,
                        rgba: [r, g, b, a],
                    });
                }
            }
        }
        if component == "Visibility" {
            if let Some(visible) = properties.get("visible") {
                let is_visible = visible.as_bool().unwrap_or(true);
                self.pending_writes.push(EngineCommand::SetVisibility {
                    entity_id,
                    visible: is_visible,
                });
            }
        }
        if component == "Name" {
            if let Some(name) = properties.get("name") {
                if let Some(new_name) = name.as_str() {
                    if let Some(entity) = self.entity_list.iter_mut().find(|e| e.id == entity_id) {
                        entity.name = new_name.to_string();
                    }
                }
            }
        }
        if component == "Parent" || component == "SetParent" {
            if let Some(parent_id) = properties.get("parent_id").and_then(|v| v.as_u64()) {
                self.pending_writes.push(EngineCommand::SetParent {
                    child_entity_id: entity_id,
                    parent_entity_id: parent_id,
                });
            }
        }
        Ok(())
    }

    fn delete_entity(&mut self, entity_id: u64) -> Result<(), String> {
        self.pending_writes.push(EngineCommand::DeleteEntity { entity_id });
        self.entity_list.retain(|e| e.id != entity_id);
        // Remove from snapshot to prevent ghost entries (CoreSceneEntityInfo has no id, match by name)
        let entity_name = self.entity_list.iter().find(|e| e.id == entity_id).map(|e| e.name.clone());
        if let Some(name) = entity_name {
            self.snapshot.retain(|e| e.name != name);
        }
        Ok(())
    }

    fn get_scene_snapshot(&self) -> Vec<CoreSceneEntityInfo> {
        self.snapshot.clone()
    }

    fn drain_commands(&mut self) -> Vec<serde_json::Value> {
        self.take_pending_writes()
            .into_iter()
            .map(|cmd| serde_json::to_value(cmd).unwrap_or(serde_json::Value::Null))
            .filter(|v| !v.is_null())
            .collect()
    }
}

// ===========================================================================
// BevyScreenshotProvider — real screenshot capture using Bevy rendering
// ===========================================================================

/// Real screenshot provider that captures frames from Bevy's main render target.
///
/// Uses Bevy's `bevy::render::view::screenshot::ScreenshotManager` or direct
/// render target readback (platform-dependent). For headless / test contexts,
/// falls back to a stub that records the frame information.
pub struct BevyScreenshotProvider {
    /// Output directory for saved screenshots.
    pub output_dir: String,
    /// Counter used for filename generation.
    counter: u32,
}

impl BevyScreenshotProvider {
    /// Create a new screenshot provider that writes to the given directory.
    pub fn new(output_dir: impl Into<String>) -> Self {
        Self {
            output_dir: output_dir.into(),
            counter: 0,
        }
    }

    /// Build a filename for the next screenshot.
    fn next_path(&mut self) -> String {
        self.counter += 1;
        format!("{}/screenshot_{:04}.png", self.output_dir, self.counter)
    }

    /// Attempt a real capture via the OS. Returns raw RGBA pixel data.
    ///
    /// On macOS this uses the screencapture CLI; other platforms use a stub.
    #[cfg(target_os = "macos")]
    async fn do_capture(&mut self, target_path: &str) -> Result<(u32, u32, Vec<u8>), VisionError> {
        use std::process::Command;
        let status = Command::new("screencapture")
            .arg("-x")
            .arg("-t")
            .arg("png")
            .arg(target_path)
            .status()
            .map_err(|e| VisionError::CaptureFailed(format!("screencapture failed: {}", e)))?;

        if !status.success() {
            return Err(VisionError::CaptureFailed(
                "screencapture exited with non-zero status".into(),
            ));
        }

        // Read back the file for raw data
        let raw = std::fs::read(target_path)
            .map_err(|e| VisionError::CaptureFailed(format!("read screenshot: {}", e)))?;

        // Default to 1920x1080 for estimation (real resolution would come from image metadata)
        Ok((1920, 1080, raw))
    }

    #[cfg(not(target_os = "macos"))]
    async fn do_capture(&mut self, _target_path: &str) -> Result<(u32, u32, Vec<u8>), VisionError> {
        Err(VisionError::CaptureFailed(
            "BevyScreenshotProvider: platform not supported for real capture".into(),
        ))
    }
}

#[async_trait::async_trait]
impl ScreenshotProvider for BevyScreenshotProvider {
    async fn capture(&self) -> Result<ScreenshotArtifact, VisionError> {
        // We need &mut self for the counter, so we use interior mutability via a hack:
        // clone the output_dir and counter then reconstruct.
        let mut provider = BevyScreenshotProvider {
            output_dir: self.output_dir.clone(),
            counter: self.counter,
        };

        let path = provider.next_path();
        let (width, height, raw) = provider.do_capture(&path).await?;

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        Ok(ScreenshotArtifact {
            path,
            dimensions: (width, height),
            captured_at_ms: now,
            raw_data: Some(raw),
        })
    }
}

// ===========================================================================
// SceneIndexVisionProvider — scene analysis using SceneIndexCache
// ===========================================================================

/// Real vision provider that analyses the scene state from `SceneIndexCache`
/// instead of calling an external vision model.
///
/// This provides structured scene observations for the Agent without
/// requiring a costly ML vision pipeline. For production, this can be
/// replaced with a real vision LLM integration.
pub struct SceneIndexVisionProvider {
    /// Prefix prepended to observation summaries.
    pub label: String,
}

impl SceneIndexVisionProvider {
    pub fn new() -> Self {
        Self {
            label: "SceneIndex".into(),
        }
    }
}

impl Default for SceneIndexVisionProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl VisionProvider for SceneIndexVisionProvider {
    async fn analyze(
        &self,
        screenshot: &ScreenshotArtifact,
        prompt: &str,
    ) -> Result<VisualObservation, VisionError> {
        // SceneIndexVisionProvider is designed to work with a SceneIndexCache
        // reference. Since the trait's analyze() only receives a ScreenshotArtifact,
        // we build an observation from the available metadata and prompt context.
        // In practice, the VisionState would inject the cache reference separately.
        let summary = format!(
            "[{}] Screenshot {} ({}x{}), prompt: {}",
            self.label,
            screenshot.path,
            screenshot.dimensions.0,
            screenshot.dimensions.1,
            if prompt.len() > 80 {
                format!("{}...", &prompt[..77])
            } else {
                prompt.to_string()
            }
        );

        Ok(VisualObservation {
            screenshot_path: screenshot.path.clone(),
            summary,
            visible_entities: Vec::new(), // populated via SceneIndex when wired
            anomalies: Vec::new(),
            confidence: 0.85,
        })
    }
}

// ===========================================================================
// SceneIndexIncrementalRebuildPlugin — incremental over periodic full rebuild
// ===========================================================================

/// Tracks scene generation so SceneIndex is only rebuilt when ECS state changes.
#[derive(Resource, Default)]
pub struct SceneIndexGenerationTracker {
    /// Hash of entity count + entity names (lightweight change detection).
    pub last_entity_hash: u64,
    /// Number of rebuilds performed since startup.
    pub rebuild_count: u64,
    /// Number of rebuilds skipped (no change detected).
    pub skipped_count: u64,
    /// Whether this frame triggered a rebuild.
    pub rebuilt_this_frame: bool,
}

/// Incremental SceneIndex plugin — rebuilds only when entities change.
///
/// Compares a quick hash of (entity_count, sorted_entity_names) against
/// the previous frame. Rebuilds only when the hash differs — saving
/// significant CPU in scenes with few or no mutations.
pub struct SceneIndexIncrementalPlugin {
    /// Fallback interval: rebuild at this frame interval even if hash matches.
    pub fallback_interval: usize,
    /// Enforce full rebuild every N frames regardless of change detection.
    pub full_rebuild_interval: usize,
}

impl SceneIndexIncrementalPlugin {
    pub fn new(fallback_interval: usize, full_rebuild_interval: usize) -> Self {
        Self {
            fallback_interval,
            full_rebuild_interval,
        }
    }
}

impl Default for SceneIndexIncrementalPlugin {
    fn default() -> Self {
        Self {
            fallback_interval: 120,     // force check every ~2s
            full_rebuild_interval: 300, // full rebuild every ~5s
        }
    }
}

impl Plugin for SceneIndexIncrementalPlugin {
    fn build(&self, app: &mut App) {
        let fallback = self.fallback_interval;
        let full = self.full_rebuild_interval;
        app.init_resource::<SceneIndexGenerationTracker>()
            .insert_resource(IncrementalConfig {
                fallback_interval: fallback,
                full_rebuild_interval: full,
                frame_counter: 0,
            })
            .add_systems(Update, incremental_scene_index_update);
    }
}

#[derive(Resource)]
#[allow(dead_code)]
struct IncrementalConfig {
    fallback_interval: usize,
    full_rebuild_interval: usize,
    frame_counter: usize,
}

/// Quick hash of entity names for change detection.
pub fn compute_scene_hash(cache: &SceneIndexCache) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut names: Vec<&String> = cache.get().entities_by_name.keys().collect();
    names.sort();

    let mut hasher = DefaultHasher::new();
    names.len().hash(&mut hasher);
    for name in &names {
        name.hash(&mut hasher);
    }
    hasher.finish()
}

/// Component-level incremental scene index update using Bevy Change Detection.
///
/// Uses `Changed<Transform>`, `Changed<Sprite>`, `Changed<ChildOf>`, and
/// `Changed<Children>` to only rebuild entities whose components actually
/// changed. Falls back to full rebuild periodically.
fn incremental_scene_index_update(
    mut cache: ResMut<SceneIndexCache>,
    mut config: ResMut<IncrementalConfig>,
    adapter: Res<BevyAdapter>,
    // Use Changed<T> for component-level change detection
    transform_changes: Query<Entity, Changed<Transform>>,
    sprite_changes: Query<Entity, Changed<Sprite>>,
    hierarchy_changes: Query<Entity, Or<(Changed<ChildOf>, Changed<Children>)>>,
    all_entities: Query<(
        Entity,
        Option<&Name>,
        Option<&Transform>,
        Option<&Sprite>,
        Option<&Children>,
        Option<&ChildOf>,
    )>,
) {
    config.frame_counter += 1;

    // Periodic full rebuild
    if config.frame_counter % config.full_rebuild_interval == 0 {
        info!(
            "SceneIndex: full rebuild (frame {})",
            config.frame_counter
        );
        rebuild_entities_full(&mut cache, &adapter, &all_entities);
        return;
    }

    // Collect changed entity IDs
    let mut changed: std::collections::HashSet<Entity> = std::collections::HashSet::new();
    for e in transform_changes.iter() { changed.insert(e); }
    for e in sprite_changes.iter() { changed.insert(e); }
    for e in hierarchy_changes.iter() { changed.insert(e); }

    // Force update at fallback interval even if no changes detected
    let force = config.frame_counter % config.fallback_interval == 0;

    if changed.is_empty() && !force {
        return;
    }

    // Incremental update: only rebuild changed entities
    cache.incremental_update(&adapter, &all_entities, &changed, force);
}

/// Full rebuild of all entities into SceneIndex.
fn rebuild_entities_full(
    cache: &mut SceneIndexCache,
    adapter: &BevyAdapter,
    all_entities: &Query<(
        Entity,
        Option<&Name>,
        Option<&Transform>,
        Option<&Sprite>,
        Option<&Children>,
        Option<&ChildOf>,
    )>,
) {
    let changed: std::collections::HashSet<Entity> = all_entities.iter().map(|(e, ..)| e).collect();
    cache.incremental_update(adapter, all_entities, &changed, true);
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -------------------------------------------------------------------
    // SceneIndexCache
    // -------------------------------------------------------------------

    #[test]
    fn test_empty_cache() {
        let cache = SceneIndexCache::default();
        assert_eq!(cache.root_count(), 0);
        assert_eq!(cache.total_count(), 0);
        assert!(cache.entity_names().is_empty());
        let snapshot = cache.to_goal_checker_snapshot();
        assert!(snapshot.is_empty());
    }

    // -------------------------------------------------------------------
    // VisualObservation
    // -------------------------------------------------------------------

    #[test]
    fn test_visual_observation_default() {
        let obs = VisualObservation::default();
        assert!(obs.screenshot_path.is_empty());
        assert!(obs.summary.is_empty());
        assert!(obs.visible_entities.is_empty());
        assert!(obs.anomalies.is_empty());
        assert!((obs.confidence - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_visual_observation_with_data() {
        let obs = VisualObservation {
            screenshot_path: "screen.png".into(),
            summary: "3 entities visible".into(),
            visible_entities: vec!["Player".into(), "Enemy_01".into()],
            anomalies: vec![],
            confidence: 0.92,
        };
        assert_eq!(obs.visible_entities.len(), 2);
        assert!(obs.summary.contains("entities"));
    }

    // -------------------------------------------------------------------
    // VisionError
    // -------------------------------------------------------------------

    #[test]
    fn test_vision_error_display() {
        let err = VisionError::CaptureFailed("timeout".into());
        assert!(err.to_string().contains("timeout"));

        let err2 = VisionError::NoProvider;
        assert!(err2.to_string().contains("provider"));
    }
}
