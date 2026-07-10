use super::bridge::BevySceneBridge;
use super::commands::{SceneCommand, SceneCommandQueue, SceneCommandResult};
use super::ops::BevySceneOps;
use crate::adapter::BevyAdapter;
use agent_core::types::EntityId;
use bevy::prelude::*;
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Deferred Component Update System (Exclusive)
// Processes pending_updates from BevySceneBridge using &mut World access.
// ---------------------------------------------------------------------------

/// Exclusive system that processes deferred UpdateComponent commands.
/// These commands require `&mut World` access which conflicts with `Commands`
/// in regular systems.
pub fn process_deferred_updates(world: &mut World) {
    let updates: Vec<(u64, String, HashMap<String, serde_json::Value>)> = {
        let mut bridge = world.resource_mut::<BevySceneBridge>();
        std::mem::take(&mut bridge.pending_updates)
    };

    if updates.is_empty() {
        return;
    }

    // Resolve entity IDs from the bridge before releasing the immutable reference
    let resolved: Vec<(Entity, String, HashMap<String, serde_json::Value>)> = {
        let bridge = world.resource::<BevySceneBridge>();
        updates
            .iter()
            .filter_map(|(entity_id, component, properties)| {
                bridge
                    .get_bevy_entity(*entity_id)
                    .map(|e| (e, component.clone(), properties.clone()))
            })
            .collect()
    };
    // Bridge borrow released here

    for (bevy_entity, component, properties) in &resolved {
        let result = match component.as_str() {
            "Transform" => {
                if let Some(mut transform) = world.get_mut::<Transform>(*bevy_entity) {
                    if let Some(pos) = properties.get("translation") {
                        if let Some(arr) = pos.as_array() {
                            transform.translation.x =
                                arr.first().and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                            transform.translation.y =
                                arr.get(1).and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                            transform.translation.z =
                                arr.get(2).and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                        }
                    }
                    Ok(())
                } else {
                    Err("Transform component not found".to_string())
                }
            }
            "Name" => {
                if let Some(name_val) = properties.get("name") {
                    if let Some(name_str) = name_val.as_str() {
                        if let Some(mut name) = world.get_mut::<Name>(*bevy_entity) {
                            *name = Name::new(name_str.to_string());
                            Ok(())
                        } else {
                            Err("Name component not found".to_string())
                        }
                    } else {
                        Err("Name property is not a string".to_string())
                    }
                } else {
                    Err("Name property not found".to_string())
                }
            }
            _ => Err(format!("Component '{}' update not implemented", component)),
        };

        match result {
            Ok(_) => log::debug!(
                "Deferred update: '{}' on entity {:?}",
                component,
                bevy_entity
            ),
            Err(e) => log::warn!("Deferred update failed: {}", e),
        }
    }

    log::info!("Processed {} deferred component update(s)", updates.len());
}

// ---------------------------------------------------------------------------
// Command Processing System
// ---------------------------------------------------------------------------

/// System that processes queued scene commands
pub fn process_scene_commands(
    mut commands: Commands,
    mut command_queue: ResMut<SceneCommandQueue>,
    mut bridge: ResMut<BevySceneBridge>,
) {
    let mut cmds = command_queue.take_commands();
    cmds.extend(bridge.take_pending_commands());

    // Separate update commands for later processing by exclusive system
    let mut deferred_updates: Vec<(u64, String, HashMap<String, serde_json::Value>)> = Vec::new();

    for cmd in cmds {
        let result = match cmd {
            SceneCommand::CreateEntity {
                name,
                position,
                components,
            } => {
                let entity = BevySceneOps::create_entity(
                    &mut commands,
                    &mut bridge,
                    &name,
                    position,
                    &components,
                );
                let scene_id = bridge.get_scene_id(entity).unwrap_or(0);
                SceneCommandResult::Success {
                    entity_id: Some(scene_id),
                }
            }
            SceneCommand::UpdateComponent {
                entity_id,
                component,
                properties,
            } => {
                deferred_updates.push((entity_id, component, properties));
                SceneCommandResult::Success {
                    entity_id: Some(entity_id),
                }
            }
            SceneCommand::DeleteEntity { entity_id } => {
                match BevySceneOps::delete_entity(&mut commands, &mut bridge, entity_id) {
                    Ok(_) => SceneCommandResult::Success { entity_id: None },
                    Err(e) => SceneCommandResult::Error(e),
                }
            }
            SceneCommand::InstantiatePrefab {
                prefab_path,
                position,
                rotation,
                scale,
            } => {
                match BevySceneOps::instantiate_prefab(
                    &mut commands,
                    &mut bridge,
                    &prefab_path,
                    position,
                    rotation,
                    scale,
                ) {
                    Ok(id) => SceneCommandResult::Success {
                        entity_id: Some(id),
                    },
                    Err(e) => SceneCommandResult::Error(e),
                }
            }
            _ => SceneCommandResult::Error("Command not implemented in system".to_string()),
        };

        log::debug!("Command result: {:?}", result);
    }

    // Queue deferred updates for exclusive system processing
    if !deferred_updates.is_empty() {
        bridge.pending_updates = deferred_updates;
    }
}

// ---------------------------------------------------------------------------
// Entity Map Synchronization
// ---------------------------------------------------------------------------

/// Keeps `BevyAdapter.entity_map` and `BevySceneBridge.entity_map` in sync.
///
/// Two entity maps exist in bevy-adapter:
/// 1. `BevyAdapter.entity_map` (keyed by `EntityId`) — used for EngineCommand processing
/// 2. `BevySceneBridge.entity_map` (keyed by `u64`) — used for scene save/load/query
///
/// Both map to the same Bevy `Entity` handles. This system ensures every entity
/// registered in one map is also present in the other, preventing the "ghost entity"
/// problem where an entity created via EngineCommand is invisible to scene queries
/// or vice versa.
fn sync_entity_maps(mut adapter: ResMut<BevyAdapter>, mut scene_bridge: ResMut<BevySceneBridge>) {
    // Forward: register Adapter entities in SceneBridge
    for (&agent_id, &bevy_entity) in &adapter.entity_map {
        if !scene_bridge.reverse_map.contains_key(&bevy_entity) {
            scene_bridge.entity_map.insert(agent_id.0, bevy_entity);
            scene_bridge.reverse_map.insert(bevy_entity, agent_id.0);
        }
    }

    // Reverse: register SceneBridge entities in Adapter
    for (&scene_id, &bevy_entity) in &scene_bridge.entity_map {
        let agent_id = EntityId(scene_id);
        if !adapter.reverse_map.contains_key(&bevy_entity) {
            adapter.entity_map.insert(agent_id, bevy_entity);
            adapter.reverse_map.insert(bevy_entity, agent_id);
        }
    }
}

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

pub struct BevySceneBridgePlugin;

impl Plugin for BevySceneBridgePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<BevySceneBridge>()
            .init_resource::<SceneCommandQueue>()
            .add_systems(Update, process_scene_commands)
            .add_systems(PreUpdate, sync_entity_maps)
            .add_systems(PreUpdate, sync_scene_index_to_bridge);
    }
}

/// Copy the SceneIndex from BevyAdapter to BevySceneBridge so that
/// SceneBridge trait read methods (query_entities, get_entity, get_scene_snapshot)
/// return real scene data instead of empty results.
fn sync_scene_index_to_bridge(adapter: Res<BevyAdapter>, mut bridge: ResMut<BevySceneBridge>) {
    if let Some(ref index) = adapter.scene_index_cache {
        bridge.sync_scene_index(index.clone());
    }
}
