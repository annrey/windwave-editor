//! Perception System for Runtime Agents
//!
//! Provides runtime agents with the ability to perceive their environment:
//! - Visual perception: see nearby entities
//! - Spatial perception: detect distances and positions
//! - Event perception: receive game events
//! - Memory: store and recall past observations

use agent_core::runtime_agent::{RuntimeObservation, RuntimeAgentEvent};
use agent_core::types::EntityId;
use bevy::prelude::*;
use crate::runtime_agent::RuntimeAgentComponent;
use std::collections::HashMap;

/// Component for entities that can be perceived by agents
#[derive(Component, Debug, Clone)]
pub struct Perceivable {
    pub visibility_range: f32,
    pub tags: Vec<String>,
    pub faction: Option<String>,
    pub importance: f32, // 0.0 - 1.0, affects detection priority
}

impl Perceivable {
    pub fn new(visibility_range: f32) -> Self {
        Self {
            visibility_range,
            tags: Vec::new(),
            faction: None,
            importance: 0.5,
        }
    }

    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    pub fn with_faction(mut self, faction: impl Into<String>) -> Self {
        self.faction = Some(faction.into());
        self
    }
}

/// Component defining an agent's perception capabilities
#[derive(Component, Debug, Clone)]
pub struct PerceptionCapability {
    pub vision_range: f32,
    pub vision_angle: f32, // in degrees, 360.0 = omnidirectional
    pub hearing_range: f32,
    pub can_detect_hidden: bool,
}

impl Default for PerceptionCapability {
    fn default() -> Self {
        Self {
            vision_range: 100.0,
            vision_angle: 360.0,
            hearing_range: 50.0,
            can_detect_hidden: false,
        }
    }
}

impl PerceptionCapability {
    pub fn with_vision_range(mut self, range: f32) -> Self {
        self.vision_range = range;
        self
    }

    pub fn with_vision_angle(mut self, angle: f32) -> Self {
        self.vision_angle = angle;
        self
    }
}

/// Resource managing perception system configuration
#[derive(Resource, Debug)]
pub struct PerceptionConfig {
    pub max_perceived_entities: usize,
    pub perception_tick_rate: f32, // seconds between perception updates
    pub use_line_of_sight: bool,
    pub show_debug_gizmos: bool,
}

impl Default for PerceptionConfig {
    fn default() -> Self {
        Self {
            max_perceived_entities: 20,
            perception_tick_rate: 0.1, // 10Hz
            use_line_of_sight: false, // simplified for now
            show_debug_gizmos: false,
        }
    }
}

/// Plugin for the perception system
pub struct PerceptionPlugin;

impl Plugin for PerceptionPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PerceptionConfig>()
            .add_systems(Update, perception_system.run_if(should_update_perception));
    }
}

/// Condition to run perception system based on tick rate
fn should_update_perception(
    config: Res<PerceptionConfig>,
    time: Res<Time>,
) -> bool {
    // Simple throttling - in production, use a timer resource
    time.elapsed_secs() % config.perception_tick_rate < time.delta_secs()
}

/// Main perception system
fn perception_system(
    config: Res<PerceptionConfig>,
    mut agent_query: Query<(Entity, &Transform, &PerceptionCapability, &mut RuntimeAgentComponent), With<RuntimeAgentComponent>>,
    perceivable_query: Query<(Entity, &Transform, &Perceivable, Option<&Name>), Without<RuntimeAgentComponent>>,
) {
    // Collect all perceivable entities first (to avoid nested queries)
    let all_perceivables: Vec<(Entity, Vec3, &Perceivable, Option<&Name>)> = perceivable_query
        .iter()
        .map(|(entity, transform, perceivable, name)| {
            (entity, transform.translation, perceivable, name)
        })
        .collect();

    // Update each agent's perception
    for (agent_entity, agent_transform, capability, mut agent_component) in agent_query.iter_mut() {
        let agent_pos = agent_transform.translation;
        let mut perceived_entities = Vec::new();
        let mut events = Vec::new();

        // Visual perception
        for (target_entity, target_pos, perceivable, name) in &all_perceivables {
            // Skip self
            if *target_entity == agent_entity {
                continue;
            }

            let distance = agent_pos.distance(*target_pos);

            // Check if within vision range
            if distance > capability.vision_range {
                continue;
            }

            // Check if within vision angle (simplified - assumes agent faces +X)
            if capability.vision_angle < 360.0 {
                let direction = (*target_pos - agent_pos).normalize();
                let forward = Vec3::X; // Agent facing direction
                let angle = direction.dot(forward).acos().to_degrees();
                if angle > capability.vision_angle / 2.0 {
                    continue;
                }
            }

            // Check target visibility range
            if distance > perceivable.visibility_range {
                continue;
            }

            // Calculate perceived entity info
            let entity_id = EntityId(target_entity.index() as u64);
            perceived_entities.push(entity_id);

            // Generate perception event
            let entity_name = name.map(|n| n.to_string()).unwrap_or_else(|| format!("Entity_{:?}", target_entity));
            events.push(RuntimeAgentEvent {
                event_type: "entity_detected".to_string(),
                source_entity: Some(entity_id),
                payload: serde_json::json!({
                    "entity_name": entity_name,
                    "distance": distance,
                    "position": [target_pos.x, target_pos.y, target_pos.z],
                    "tags": perceivable.tags,
                    "faction": perceivable.faction,
                    "importance": perceivable.importance,
                }),
            });

            if perceived_entities.len() >= config.max_perceived_entities {
                break;
            }
        }

        // Store facts in blackboard before creating observation
        let visible_count = perceived_entities.len();
        if let Some(nearest) = perceived_entities.first() {
            let nearest_id = nearest.0;
            agent_component.blackboard.set(
                "nearest_entity",
                serde_json::json!(nearest_id),
            );
        }
        agent_component.blackboard.set(
            "visible_entity_count",
            serde_json::json!(visible_count),
        );

        // Update agent's observation
        let observation = RuntimeObservation {
            entity_id: EntityId(agent_entity.index() as u64),
            visible_entities: perceived_entities,
            nearby_prefab_instances: Vec::new(), // would come from prefab system
            events,
            facts: HashMap::new(),
        };

        agent_component.last_observation = Some(observation);
    }
}

/// Helper to get perceived entity information
/// Note: This requires a valid Entity reference. In practice, use the entity from your query.
pub fn get_perceived_entity_info(
    world: &World,
    entity: Entity,
) -> Option<PerceivedEntityInfo> {
    if let Some(transform) = world.get::<Transform>(entity) {
        let name = world.get::<Name>(entity)
            .map(|n| n.to_string())
            .unwrap_or_default();
        
        let perceivable = world.get::<Perceivable>(entity);
        
        Some(PerceivedEntityInfo {
            entity_id: EntityId(entity.index() as u64),
            name,
            position: transform.translation,
            tags: perceivable.map(|p| p.tags.clone()).unwrap_or_default(),
            faction: perceivable.and_then(|p| p.faction.clone()),
        })
    } else {
        None
    }
}

/// Information about a perceived entity
#[derive(Debug, Clone)]
pub struct PerceivedEntityInfo {
    pub entity_id: EntityId,
    pub name: String,
    pub position: Vec3,
    pub tags: Vec<String>,
    pub faction: Option<String>,
}

/// System to emit perception events for the agent platform
pub fn emit_perception_events(
    query: Query<(Entity, &RuntimeAgentComponent), Changed<RuntimeAgentComponent>>,
) {
    for (_entity, agent) in query.iter() {
        if let Some(ref observation) = agent.last_observation {
            for event in &observation.events {
                // In a real implementation, this would send events to the agent platform
                log::debug!(
                    "Agent {:?} perceived event: {} from {:?}",
                    agent.id,
                    event.event_type,
                    event.source_entity
                );
            }
        }
    }
}

/// Helper function to spawn a perceivable entity
pub fn spawn_perceivable_entity(
    commands: &mut Commands,
    name: &str,
    position: Vec3,
    perceivable: Perceivable,
) -> Entity {
    commands.spawn((
        Name::new(name.to_string()),
        Transform::from_translation(position),
        GlobalTransform::default(),
        Visibility::default(),
        Sprite::default(),
        perceivable,
    )).id()
}

/// Query what an agent can currently perceive
pub fn query_agent_perception(
    world: &World,
    agent_entity: Entity,
) -> Option<RuntimeObservation> {
    world.get::<RuntimeAgentComponent>(agent_entity)
        .and_then(|agent| agent.last_observation.clone())
}
