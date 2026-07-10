use super::bridge::BevySceneBridge;
use super::ops::BevySceneOps;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Serializable scene data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializableScene {
    pub version: String,
    pub entities: Vec<SerializableEntity>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializableEntity {
    pub id: u64,
    pub name: String,
    pub components: HashMap<String, serde_json::Value>,
    pub children: Vec<u64>,
}

impl BevySceneOps {
    /// Save current scene to file.
    pub fn save_scene(
        world: &mut World,
        bridge: &BevySceneBridge,
        path: &str,
    ) -> Result<(), String> {
        let mut entities = Vec::new();

        for (scene_id, bevy_entity) in &bridge.entity_map {
            if let Some(data) = Self::serialize_entity(world, *bevy_entity, *scene_id, bridge) {
                entities.push(data);
            }
        }

        let scene = SerializableScene {
            version: "1.0".to_string(),
            entities,
        };

        let json = serde_json::to_string_pretty(&scene)
            .map_err(|e| format!("Serialization error: {}", e))?;

        std::fs::write(path, json).map_err(|e| format!("File write error: {}", e))?;

        log::info!("Scene saved to: {}", path);
        Ok(())
    }

    fn serialize_entity(
        world: &World,
        bevy_entity: Entity,
        scene_id: u64,
        bridge: &BevySceneBridge,
    ) -> Option<SerializableEntity> {
        let name = world
            .get::<Name>(bevy_entity)
            .map(|n| n.as_str().to_string())
            .unwrap_or_else(|| format!("Entity_{}", scene_id));

        let mut components = HashMap::new();

        // Serialize Transform
        if let Some(transform) = world.get::<Transform>(bevy_entity) {
            components.insert("Transform".to_string(), serde_json::json!({
                "translation": [transform.translation.x, transform.translation.y, transform.translation.z],
                "rotation": [transform.rotation.x, transform.rotation.y, transform.rotation.z, transform.rotation.w],
                "scale": [transform.scale.x, transform.scale.y, transform.scale.z],
            }));
        }

        // Serialize Visibility
        if let Some(visibility) = world.get::<Visibility>(bevy_entity) {
            components.insert(
                "Visibility".to_string(),
                serde_json::json!({
                    "visible": *visibility == Visibility::Visible,
                }),
            );
        }

        // Collect child entity IDs from Bevy's Children component
        // Note: This captures direct children only; the full hierarchy is preserved
        // through each entity's children list in the serialized format.
        let children = world
            .get::<Children>(bevy_entity)
            .map(|children_component| {
                children_component
                    .iter()
                    .filter_map(|child_entity| bridge.get_scene_id(child_entity))
                    .collect()
            })
            .unwrap_or_default();

        Some(SerializableEntity {
            id: scene_id,
            name,
            components,
            children,
        })
    }

    /// Load scene from file.
    pub fn load_scene(
        commands: &mut Commands,
        bridge: &mut BevySceneBridge,
        path: &str,
    ) -> Result<(), String> {
        let json = std::fs::read_to_string(path).map_err(|e| format!("File read error: {}", e))?;

        let scene: SerializableScene =
            serde_json::from_str(&json).map_err(|e| format!("Deserialization error: {}", e))?;

        // Clear existing mappings
        bridge.clear_mappings();

        // First pass: spawn all entities and register them in the bridge
        for entity_data in &scene.entities {
            Self::deserialize_entity(commands, bridge, entity_data.clone());
        }

        // Second pass: rebuild parent-child hierarchy using Bevy's built_in system
        // This ensures proper Transform propagation and scene graph integrity.
        for entity_data in &scene.entities {
            if !entity_data.children.is_empty() {
                if let Some(parent_entity) = bridge.get_bevy_entity(entity_data.id) {
                    for child_id in &entity_data.children {
                        if let Some(child_entity) = bridge.get_bevy_entity(*child_id) {
                            commands
                                .entity(child_entity)
                                .set_parent_in_place(parent_entity);
                        }
                    }
                }
            }
        }

        log::info!("Scene loaded from: {}", path);
        Ok(())
    }

    fn deserialize_entity(
        commands: &mut Commands,
        bridge: &mut BevySceneBridge,
        data: SerializableEntity,
    ) -> Entity {
        let entity = commands.spawn_empty().id();

        // Set name
        commands.entity(entity).insert(Name::new(data.name));

        // Deserialize Transform
        if let Some(transform_data) = data.components.get("Transform") {
            if let Some(translation) = transform_data.get("translation") {
                if let Some(arr) = translation.as_array() {
                    let x = arr.first().and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                    let y = arr.get(1).and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                    let z = arr.get(2).and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                    commands.entity(entity).insert(Transform::from_xyz(x, y, z));
                }
            }
        }

        // Deserialize Visibility
        if let Some(vis_data) = data.components.get("Visibility") {
            if let Some(visible) = vis_data.get("visible").and_then(|v| v.as_bool()) {
                commands.entity(entity).insert(if visible {
                    Visibility::Visible
                } else {
                    Visibility::Hidden
                });
            }
        }

        // Register with bridge
        bridge.register_entity(data.id, entity);

        entity
    }
}
