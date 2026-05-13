//! BevyAdapter - Engine adapter for Bevy ECS
//!
//! Core adapter struct, entity mapping, Agent-tracked components,
//! and the EngineAdapter trait implementation.

pub mod commands;
pub mod scene_build;
pub mod rollback;

pub use commands::{EngineCommand, AssetType, AssetReference, ComponentPatch, EngineCommandResult};
pub use rollback::{RollbackOperation, EntitySnapshot};

use agent_core::{EntityId, EntityInfo, ComponentInfo, PropertyValue, AgentAction, ActionResult, AdapterError};
use bevy::prelude::*;
use crate::scene_index::SceneIndex;
use std::collections::HashMap;

pub struct BevyAdapterPlugin;

impl Plugin for BevyAdapterPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<BevyAdapter>()
           .add_message::<AgentActionEvent>()
           .add_systems(Update, (process_agent_actions, sync_entities_to_adapter));
    }
}

/// Trait for engine adapters that support scene queries and command execution.
///
/// This is the primary interface for Agents to interact with the engine
/// in a portable, engine-agnostic way.
pub trait EngineAdapter {
    /// Build a hierarchical scene index from the current engine state.
    fn build_scene_index(&self) -> Result<SceneIndex, String>;

    /// Apply an engine command and return the result.
    fn apply_command(&mut self, command: EngineCommand) -> Result<EngineCommandResult, String>;

    /// Roll back a previously applied operation.
    fn rollback(&mut self, rollback: RollbackOperation) -> Result<(), String>;
}

/// The Bevy-specific engine adapter.
///
/// Maintains bidirectional mapping between Agent `EntityId` and Bevy `Entity`,
/// supports command execution, scene index building, and rollback.
#[derive(Resource)]
pub struct BevyAdapter {
    pub(crate) entity_map: HashMap<EntityId, Entity>,
    pub(crate) reverse_map: HashMap<Entity, EntityId>,
    next_id: u64,
    /// Cached scene index for EngineAdapter trait methods
    pub(crate) scene_index_cache: Option<SceneIndex>,
    /// Loaded asset references for tracking
    pub(crate) asset_references: HashMap<String, AssetReference>,
}

impl Default for BevyAdapter {
    fn default() -> Self {
        Self {
            entity_map: HashMap::new(),
            reverse_map: HashMap::new(),
            next_id: 1,
            scene_index_cache: None,
            asset_references: HashMap::new(),
        }
    }
}

impl BevyAdapter {
    /// Register a Bevy entity with the adapter
    pub fn register_entity(&mut self, entity: Entity) -> EntityId {
        let id = EntityId(self.next_id);
        self.next_id += 1;
        self.entity_map.insert(id, entity);
        self.reverse_map.insert(entity, id);
        id
    }

    /// Get Bevy entity from Agent EntityId
    pub fn get_bevy_entity(&self, id: EntityId) -> Option<Entity> {
        self.entity_map.get(&id).copied()
    }

    /// Get Agent EntityId from Bevy entity
    pub fn get_agent_id(&self, entity: Entity) -> Option<EntityId> {
        self.reverse_map.get(&entity).copied()
    }

    /// Number of registered entities
    pub fn entity_count(&self) -> usize {
        self.entity_map.len()
    }

    /// Build a lookup table mapping agent entity IDs to Bevy Entities.
    pub fn entity_id_lookup(&self) -> HashMap<u64, Entity> {
        self.entity_map
            .iter()
            .map(|(&id, &entity)| (id.0, entity))
            .collect()
    }

    /// Unregister an entity from the adapter (e.g. when deleted).
    pub fn unregister_entity(&mut self, agent_id: EntityId) {
        if let Some(entity) = self.entity_map.remove(&agent_id) {
            self.reverse_map.remove(&entity);
        }
    }

    /// Query entity information from the ECS world
    pub fn query_entity_info(
        &self,
        id: EntityId,
        world: &World,
    ) -> Result<EntityInfo, AdapterError> {
        let bevy_entity = self.get_bevy_entity(id)
            .ok_or(AdapterError::EntityNotFound(id))?;

        let entity_ref = world.get_entity(bevy_entity)
            .map_err(|_| AdapterError::EntityNotFound(id))?;

        let mut info = EntityInfo {
            id,
            name: "Unknown".to_string(),
            entity_type: "Entity".to_string(),
            components: Vec::new(),
            children: Vec::new(),
        };

        if let Some(name) = entity_ref.get::<Name>() {
            info.name = name.to_string();
        }

        if let Some(transform) = entity_ref.get::<Transform>() {
            let mut props = HashMap::new();
            props.insert("position".to_string(), PropertyValue::Vec3 {
                x: transform.translation.x,
                y: transform.translation.y,
                z: transform.translation.z,
            });
            let (roll, pitch, yaw) = transform.rotation.to_euler(EulerRot::XYZ);
            props.insert("rotation".to_string(), PropertyValue::Vec3 {
                x: roll,
                y: pitch,
                z: yaw,
            });
            props.insert("scale".to_string(), PropertyValue::Vec3 {
                x: transform.scale.x,
                y: transform.scale.y,
                z: transform.scale.z,
            });

            info.components.push(ComponentInfo {
                name: "Transform".to_string(),
                properties: props,
            });
        }

        Ok(info)
    }

    /// Apply an action to the Bevy world
    pub fn apply_action(
        &mut self,
        action: AgentAction,
        world: &mut World,
    ) -> Result<ActionResult, AdapterError> {
        match action {
            AgentAction::UpdateComponent { entity_id, component_name, property, value } => {
                self.update_component(entity_id, &component_name, &property, value, world)
            }
            AgentAction::CreateComponent { entity_id, component_type, properties } => {
                self.create_component(entity_id, &component_type, properties, world)
            }
            AgentAction::DeleteComponent { entity_id, component_name } => {
                self.delete_component(entity_id, &component_name, world)
            }
            _ => Err(AdapterError::ActionNotSupported(
                format!("{:?}", action)
            )),
        }
    }

    fn update_component(
        &self,
        entity_id: EntityId,
        component_name: &str,
        property: &str,
        value: PropertyValue,
        world: &mut World,
    ) -> Result<ActionResult, AdapterError> {
        let bevy_entity = self.get_bevy_entity(entity_id)
            .ok_or(AdapterError::EntityNotFound(entity_id))?;

        match component_name {
            "Transform" => {
                let mut entity_mut = world.get_entity_mut(bevy_entity)
                    .map_err(|_| AdapterError::EntityNotFound(entity_id))?;

                if let Some(mut transform) = entity_mut.get_mut::<Transform>() {
                    match property {
                        "position" => {
                            if let PropertyValue::Vec3 { x, y, z } = value {
                                transform.translation = Vec3::new(x, y, z);
                            }
                        }
                        "scale" => {
                            if let PropertyValue::Vec3 { x, y, z } = value {
                                transform.scale = Vec3::new(x, y, z);
                            }
                        }
                        _ => {}
                    }
                }
            }
            _ => return Err(AdapterError::ComponentNotFound(component_name.to_string())),
        }

        Ok(ActionResult {
            success: true,
            message: format!("Updated {}.{} on entity {:?}", component_name, property, entity_id),
            data: None,
        })
    }

    fn create_component(
        &self,
        entity_id: EntityId,
        component_type: &str,
        _properties: HashMap<String, PropertyValue>,
        world: &mut World,
    ) -> Result<ActionResult, AdapterError> {
        let bevy_entity = self.get_bevy_entity(entity_id)
            .ok_or(AdapterError::EntityNotFound(entity_id))?;

        let mut entity_mut = world.get_entity_mut(bevy_entity)
            .map_err(|_| AdapterError::EntityNotFound(entity_id))?;

        match component_type {
            "Transform" => {
                if entity_mut.get::<Transform>().is_none() {
                    entity_mut.insert(Transform::default());
                }
            }
            "Sprite" => {
                entity_mut.insert(Sprite::default());
                entity_mut.insert(Visibility::default());
            }
            _ => return Err(AdapterError::ActionNotSupported(
                format!("Create component: {}", component_type)
            )),
        }

        Ok(ActionResult {
            success: true,
            message: format!("Created {} on entity {:?}", component_type, entity_id),
            data: None,
        })
    }

    fn delete_component(
        &self,
        entity_id: EntityId,
        component_name: &str,
        world: &mut World,
    ) -> Result<ActionResult, AdapterError> {
        let bevy_entity = self.get_bevy_entity(entity_id)
            .ok_or(AdapterError::EntityNotFound(entity_id))?;

        let mut entity_mut = world.get_entity_mut(bevy_entity)
            .map_err(|_| AdapterError::EntityNotFound(entity_id))?;

        match component_name {
            "Transform" => {
                entity_mut.remove::<Transform>();
            }
            "Sprite" => {
                entity_mut.remove::<Sprite>();
            }
            _ => return Err(AdapterError::ComponentNotFound(component_name.to_string())),
        }

        Ok(ActionResult {
            success: true,
            message: format!("Deleted {} from entity {:?}", component_name, entity_id),
            data: None,
        })
    }
}

// ---------------------------------------------------------------------------
// EngineAdapter implementation for BevyAdapter
// ---------------------------------------------------------------------------

impl EngineAdapter for BevyAdapter {
    fn build_scene_index(&self) -> Result<SceneIndex, String> {
        self.scene_index_cache
            .clone()
            .ok_or_else(|| "Scene index not built yet. Call BevyAdapter::build_scene_index(world) first.".to_string())
    }

    fn apply_command(&mut self, _command: EngineCommand) -> Result<EngineCommandResult, String> {
        Err(
            "apply_command requires World access. Use BevyAdapter::apply_engine_command(command, world) instead."
                .to_string(),
        )
    }

    fn rollback(&mut self, _rollback: RollbackOperation) -> Result<(), String> {
        Err(
            "rollback requires World access. Use BevyAdapter::rollback_operation(rollback, world) instead."
                .to_string(),
        )
    }
}

/// Event for Agent actions targeting Bevy (Bevy 0.17: renamed Event→Message)
#[derive(Message)]
pub struct AgentActionEvent {
    pub action: AgentAction,
}

/// System to process Agent actions.
///
/// Note: Bevy 0.17 no longer allows `ResMut<World>` as a system parameter.
/// Real-world integration should pass the action through a channel or
/// use the apply_action method directly from the game loop.
#[allow(unused_variables)]
fn process_agent_actions(
    _adapter: ResMut<BevyAdapter>,
    _events: MessageReader<AgentActionEvent>,
) {
    // Bevy 0.17: MessageReader replaces EventReader
    // World access via system params requires architectural adaptation.
    // For MVP: actions flow through DirectorRuntime → BevyAdapter directly.
}

/// Marker component for Agent-tracked entities
#[derive(Component)]
pub struct AgentTracked;

/// Component to store Agent entity ID on Bevy entities
#[derive(Component)]
pub struct AgentEntityId(pub EntityId);

/// System to sync new entities to adapter
pub fn sync_entities_to_adapter(
    mut commands: Commands,
    mut adapter: ResMut<BevyAdapter>,
    query: Query<Entity, (Without<AgentEntityId>, With<AgentTracked>)>,
) {
    for entity in &query {
        let id = adapter.register_entity(entity);
        commands.entity(entity).insert(AgentEntityId(id));
        log::info!("Registered entity {:?} with Agent ID {:?}", entity, id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_entity_registration() {
        let _adapter = BevyAdapter::default();
        // Integration tests require real Bevy entities
    }
}
