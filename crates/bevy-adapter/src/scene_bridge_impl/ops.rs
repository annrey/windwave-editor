use super::bridge::BevySceneBridge;
use super::helpers::parse_color_array;
use crate::adapter::BevyAdapter;
use crate::open_world_components;
use crate::scene_index::{SceneEntityNode, SceneIndex};
use crate::EngineCommand;
use agent_core::goal_checker::SceneEntityInfo;
use agent_core::ports::scene::{ComponentPatch, EntityListItem};
use bevy::prelude::*;
use bevy::sprite::Sprite;
use std::collections::HashMap;

pub struct BevySceneOps;

impl BevySceneOps {
    /// Query entities from a built SceneIndex.
    pub fn query(
        index: &SceneIndex,
        filter: Option<&str>,
        component_type: Option<&str>,
    ) -> Vec<EntityListItem> {
        let mut results = Vec::new();
        for node in &index.root_entities {
            Self::collect_matching(node, filter, component_type, &mut results);
        }
        results
    }

    fn collect_matching(
        node: &SceneEntityNode,
        filter: Option<&str>,
        component_type: Option<&str>,
        results: &mut Vec<EntityListItem>,
    ) {
        let name_matches =
            filter.is_none_or(|f| f == "*" || node.name.to_lowercase().contains(&f.to_lowercase()));
        let component_matches =
            component_type.is_none_or(|ct| node.components.iter().any(|c| c.type_name == ct));

        if name_matches && component_matches {
            results.push(EntityListItem {
                id: node.id,
                name: node.name.clone(),
                components: node
                    .components
                    .iter()
                    .map(|c| c.type_name.clone())
                    .collect(),
            });
        }
        for child in &node.children {
            Self::collect_matching(child, filter, component_type, results);
        }
    }

    /// Build a goal-checking snapshot from a SceneIndex.
    pub fn snapshot_from_index(index: &SceneIndex) -> Vec<SceneEntityInfo> {
        let raw = index.to_entity_info_list();
        raw.into_iter()
            .map(|info| SceneEntityInfo {
                name: info.name,
                components: info.components,
                translation: info.translation,
                sprite_color: info.sprite_color,
            })
            .collect()
    }

    /// Apply an EngineCommand via BevyAdapter (requires `&mut World`).
    pub fn apply_command(
        adapter: &mut BevyAdapter,
        command: EngineCommand,
        world: &mut World,
    ) -> Result<u64, String> {
        let result = adapter
            .apply_engine_command(command, world)
            .map_err(|e| format!("{:?}", e))?;
        Ok(result.entity_id.unwrap_or(0))
    }

    /// Create an entity with components.
    pub fn create_entity(
        commands: &mut Commands,
        bridge: &mut BevySceneBridge,
        name: &str,
        position: Option<[f64; 2]>,
        components: &[ComponentPatch],
    ) -> Entity {
        let scene_id = bridge.generate_id();

        let entity = commands.spawn_empty().id();

        // Add Name component
        commands.entity(entity).insert(Name::new(name.to_string()));

        // Add Transform if position provided
        if let Some(pos) = position {
            commands
                .entity(entity)
                .insert(Transform::from_xyz(pos[0] as f32, pos[1] as f32, 0.0));
        }

        // Apply component patches
        for patch in components {
            Self::apply_component_patch(commands, entity, patch);
        }

        // Register in bridge
        bridge.register_entity(scene_id, entity);

        log::info!(
            "Created entity '{}' with ID {} -> {:?}",
            name,
            scene_id,
            entity
        );
        entity
    }

    fn apply_component_patch(commands: &mut Commands, entity: Entity, patch: &ComponentPatch) {
        match patch.type_name.as_str() {
            "Transform" => {
                let mut transform = Transform::default();
                if let Some(pos) = patch.properties.get("translation") {
                    if let Some(arr) = pos.as_array() {
                        transform.translation.x =
                            arr.first().and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                        transform.translation.y =
                            arr.get(1).and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                        transform.translation.z =
                            arr.get(2).and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                    }
                }
                if let Some(rot) = patch.properties.get("rotation") {
                    if let Some(arr) = rot.as_array() {
                        transform.rotation = Quat::from_xyzw(
                            arr.first().and_then(|v| v.as_f64()).unwrap_or(0.0) as f32,
                            arr.get(1).and_then(|v| v.as_f64()).unwrap_or(0.0) as f32,
                            arr.get(2).and_then(|v| v.as_f64()).unwrap_or(0.0) as f32,
                            arr.get(3).and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
                        );
                    }
                }
                if let Some(scale) = patch.properties.get("scale") {
                    if let Some(arr) = scale.as_array() {
                        transform.scale.x =
                            arr.first().and_then(|v| v.as_f64()).unwrap_or(1.0) as f32;
                        transform.scale.y =
                            arr.get(1).and_then(|v| v.as_f64()).unwrap_or(1.0) as f32;
                        transform.scale.z =
                            arr.get(2).and_then(|v| v.as_f64()).unwrap_or(1.0) as f32;
                    }
                }
                commands.entity(entity).insert(transform);
            }
            "Sprite" => {
                let mut sprite = Sprite::default();
                if let Some(color) = patch.properties.get("color") {
                    if let Some(arr) = color.as_array() {
                        sprite.color = Color::srgba(
                            arr.first().and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
                            arr.get(1).and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
                            arr.get(2).and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
                            arr.get(3).and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
                        );
                    }
                }
                if let Some(size) = patch.properties.get("custom_size") {
                    if let Some(arr) = size.as_array() {
                        sprite.custom_size = Some(Vec2::new(
                            arr.first().and_then(|v| v.as_f64()).unwrap_or(100.0) as f32,
                            arr.get(1).and_then(|v| v.as_f64()).unwrap_or(100.0) as f32,
                        ));
                    }
                }
                commands.entity(entity).insert(sprite);
            }
            "Visibility" => {
                let visible = patch
                    .properties
                    .get("visible")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true);
                commands.entity(entity).insert(if visible {
                    Visibility::Visible
                } else {
                    Visibility::Hidden
                });
            }
            "Name" => {
                if let Some(name_val) = patch.properties.get("name") {
                    if let Some(name_str) = name_val.as_str() {
                        commands
                            .entity(entity)
                            .insert(Name::new(name_str.to_string()));
                    }
                }
            }
            "Color" => {
                if let Some(color) = patch.properties.get("color") {
                    if let Some(arr) = color.as_array() {
                        commands.entity(entity).insert(Sprite {
                            color: Color::srgba(
                                arr.first().and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
                                arr.get(1).and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
                                arr.get(2).and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
                                arr.get(3).and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
                            ),
                            ..default()
                        });
                    }
                }
            }
            _ => {
                if !open_world_components::insert_scene_component_patch(
                    &mut commands.entity(entity),
                    patch,
                ) {
                    log::warn!("Unknown component type: {}", patch.type_name);
                }
            }
        }
    }

    /// Update entity component properties.
    pub fn update_component(
        world: &mut World,
        bridge: &BevySceneBridge,
        entity_id: u64,
        component: &str,
        properties: &HashMap<String, serde_json::Value>,
    ) -> Result<(), String> {
        let bevy_entity = bridge
            .get_bevy_entity(entity_id)
            .ok_or_else(|| format!("Entity {} not found", entity_id))?;

        match component {
            "Transform" => {
                if let Some(mut transform) = world.get_mut::<Transform>(bevy_entity) {
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
                }
            }
            "Name" => {
                if let Some(name_val) = properties.get("name") {
                    if let Some(name_str) = name_val.as_str() {
                        if let Some(mut name) = world.get_mut::<Name>(bevy_entity) {
                            *name = Name::new(name_str.to_string());
                        }
                    }
                }
            }
            _ => {
                return Err(format!("Component '{}' update not implemented", component));
            }
        }

        log::debug!("Updated component '{}' on entity {}", component, entity_id);
        Ok(())
    }

    /// Delete an entity.
    pub fn delete_entity(
        commands: &mut Commands,
        bridge: &mut BevySceneBridge,
        entity_id: u64,
    ) -> Result<(), String> {
        let bevy_entity = bridge
            .get_bevy_entity(entity_id)
            .ok_or_else(|| format!("Entity {} not found", entity_id))?;

        commands.entity(bevy_entity).despawn();
        bridge.unregister_entity(entity_id);

        log::info!("Deleted entity {}", entity_id);
        Ok(())
    }

    /// Instantiate a prefab.
    pub fn instantiate_prefab(
        commands: &mut Commands,
        bridge: &mut BevySceneBridge,
        prefab_path: &str,
        position: Option<[f64; 3]>,
        rotation: Option<[f64; 4]>,
        scale: Option<[f64; 3]>,
    ) -> Result<u64, String> {
        // Generate new entity ID
        let scene_id = bridge.generate_id();

        // Spawn entity
        let entity = commands.spawn_empty().id();

        // Set up transform
        let mut transform = Transform::IDENTITY;
        if let Some(pos) = position {
            transform.translation = Vec3::new(pos[0] as f32, pos[1] as f32, pos[2] as f32);
        }
        if let Some(rot) = rotation {
            transform.rotation =
                Quat::from_array([rot[0] as f32, rot[1] as f32, rot[2] as f32, rot[3] as f32]);
        }
        if let Some(scl) = scale {
            transform.scale = Vec3::new(scl[0] as f32, scl[1] as f32, scl[2] as f32);
        }

        commands.entity(entity).insert((
            Name::new(format!("Prefab: {}", prefab_path)),
            transform,
            Visibility::Visible,
        ));

        if let Err(e) = Self::load_prefab_components(commands, entity, prefab_path) {
            log::warn!("Failed to load prefab components: {}", e);
        }

        // Register
        bridge.register_entity(scene_id, entity);

        log::info!(
            "Instantiated prefab '{}' -> ID {}: {:?}",
            prefab_path,
            scene_id,
            entity
        );
        Ok(scene_id)
    }

    /// Load prefab components from a file or use defaults
    fn load_prefab_components(
        commands: &mut Commands,
        entity: Entity,
        prefab_path: &str,
    ) -> Result<(), String> {
        let prefab_path_lower = prefab_path.to_lowercase();

        if prefab_path_lower.ends_with(".json") || prefab_path_lower.contains("prefab") {
            if let Ok(json_content) = std::fs::read_to_string(prefab_path) {
                if let Ok(prefab_data) = serde_json::from_str::<serde_json::Value>(&json_content) {
                    Self::apply_prefab_data(commands, entity, prefab_data)?;
                    log::info!("Loaded prefab components from '{}'", prefab_path);
                    return Ok(());
                }
            }
        }

        Self::apply_default_prefab_components(commands, entity, prefab_path);
        Ok(())
    }

    fn apply_prefab_data(
        commands: &mut Commands,
        entity: Entity,
        data: serde_json::Value,
    ) -> Result<(), String> {
        if let Some(obj) = data.as_object() {
            for (component_type, props) in obj {
                if let Some(props_map) = props.as_object() {
                    Self::apply_component_from_props(commands, entity, component_type, props_map);
                }
            }
        }
        Ok(())
    }

    fn apply_component_from_props(
        commands: &mut Commands,
        entity: Entity,
        component_type: &str,
        props: &serde_json::Map<String, serde_json::Value>,
    ) {
        match component_type.to_lowercase().as_str() {
            "sprite" | "spriterenderer" => {
                if let Some(color) = props.get("color") {
                    let rgba: [f32; 4] = parse_color_array(color);
                    commands.entity(entity).insert(bevy::sprite::Sprite {
                        color: Color::linear_rgba(rgba[0], rgba[1], rgba[2], rgba[3]),
                        ..Default::default()
                    });
                }
            }
            "camera" => {
                commands.entity(entity).insert(Camera2d);
            }
            _ => {
                log::debug!("Unknown prefab component type: {}", component_type);
            }
        }
    }

    fn apply_default_prefab_components(commands: &mut Commands, entity: Entity, prefab_path: &str) {
        commands.entity(entity).insert(Sprite::default());
        log::debug!("Applied default components for prefab '{}'", prefab_path);
    }

    /// Get entity data as JSON.
    pub fn get_entity_data(
        world: &World,
        bridge: &BevySceneBridge,
        entity_id: u64,
    ) -> Option<serde_json::Value> {
        let bevy_entity = bridge.get_bevy_entity(entity_id)?;

        let mut data = serde_json::Map::new();

        // Get Name
        if let Some(name) = world.get::<Name>(bevy_entity) {
            data.insert("name".to_string(), serde_json::json!(name.as_str()));
        }

        // Get Transform
        if let Some(transform) = world.get::<Transform>(bevy_entity) {
            data.insert(
                "transform".to_string(),
                serde_json::json!({
                    "translation": [
                        transform.translation.x,
                        transform.translation.y,
                        transform.translation.z
                    ],
                    "rotation": [
                        transform.rotation.x,
                        transform.rotation.y,
                        transform.rotation.z,
                        transform.rotation.w
                    ],
                    "scale": [
                        transform.scale.x,
                        transform.scale.y,
                        transform.scale.z
                    ]
                }),
            );
        }

        Some(serde_json::Value::Object(data))
    }
}
