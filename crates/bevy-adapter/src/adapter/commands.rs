//! EngineCommand DSL - High-level commands for Agent-to-Engine communication.
//!
//! Defines the `EngineCommand` enum, related data types, and the
//! `apply_engine_command` method that executes commands against the Bevy World.

use agent_core::{AdapterError, EntityId};
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use super::BevyAdapter;
use crate::open_world_components;

/// A high-level command that Agents issue to manipulate the game engine scene.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EngineCommand {
    /// Create a new entity with the given name and optional component patches.
    CreateEntity {
        name: String,
        components: Vec<ComponentPatch>,
    },
    /// Delete an existing entity by its Agent entity ID.
    DeleteEntity { entity_id: u64 },
    /// Set the Transform (translation, rotation, scale) of an entity.
    SetTransform {
        entity_id: u64,
        translation: Option<[f32; 3]>,
        rotation: Option<[f32; 3]>,
        scale: Option<[f32; 3]>,
    },
    /// Set the Sprite color (RGBA) of an entity.
    SetSpriteColor { entity_id: u64, rgba: [f32; 4] },
    /// Set the visibility of an entity.
    SetVisibility { entity_id: u64, visible: bool },

    // ------------------------------------------------------------------
    // Phase 7: Extended Commands (§7.1, §7.2, §7.3)
    // ------------------------------------------------------------------
    /// Add a component to an existing entity (7.1)
    AddComponent {
        entity_id: u64,
        component: ComponentPatch,
    },

    /// Remove a component from an entity (7.1)
    RemoveComponent {
        entity_id: u64,
        component_type: String,
    },

    /// Modify a specific component property (7.1)
    ModifyComponent {
        entity_id: u64,
        component_type: String,
        property: String,
        value: serde_json::Value,
    },

    /// Set parent-child relationship (7.2)
    SetParent {
        child_entity_id: u64,
        parent_entity_id: u64,
    },

    /// Remove entity from its parent (7.2)
    RemoveFromParent { entity_id: u64 },

    /// Reparent all children of one entity to another (7.2)
    ReparentChildren {
        source_parent_id: u64,
        target_parent_id: u64,
    },

    /// Load an asset and return a handle reference (7.3)
    LoadAsset { path: String, asset_type: AssetType },

    /// Set sprite texture from an asset handle (7.3)
    SetSpriteTexture {
        entity_id: u64,
        asset_handle: String,
    },

    /// Spawn a prefab/scene from an asset (7.3)
    SpawnPrefab {
        asset_handle: String,
        transform: Option<[f32; 3]>,
    },
}

/// Asset types for engine commands (7.3)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AssetType {
    Image,
    SpriteSheet,
    Scene,
    Mesh,
    Audio,
    Custom(String),
}

/// Reference to a loaded asset (7.3)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetReference {
    pub handle: String,
    pub asset_type: AssetType,
    pub path: String,
}

/// A patch describing a component to add when creating an entity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentPatch {
    pub type_name: String,
    pub value: serde_json::Value,
}

/// Result of executing an EngineCommand against the engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineCommandResult {
    pub success: bool,
    pub message: String,
    pub entity_id: Option<u64>,
}

impl BevyAdapter {
    /// Apply an `EngineCommand` to the Bevy World.
    pub fn apply_engine_command(
        &mut self,
        command: EngineCommand,
        world: &mut World,
    ) -> Result<EngineCommandResult, AdapterError> {
        match command {
            EngineCommand::CreateEntity { name, components } => {
                self.cmd_create_entity(name, components, world)
            }
            EngineCommand::DeleteEntity { entity_id } => self.cmd_delete_entity(entity_id, world),
            EngineCommand::SetTransform {
                entity_id,
                translation,
                rotation,
                scale,
            } => self.cmd_set_transform(entity_id, translation, rotation, scale, world),
            EngineCommand::SetSpriteColor { entity_id, rgba } => {
                self.cmd_set_sprite_color(entity_id, rgba, world)
            }
            EngineCommand::SetVisibility { entity_id, visible } => {
                self.cmd_set_visibility(entity_id, visible, world)
            }
            EngineCommand::AddComponent {
                entity_id,
                component,
            } => self.cmd_add_component(entity_id, component, world),
            EngineCommand::RemoveComponent {
                entity_id,
                component_type,
            } => self.cmd_remove_component(entity_id, component_type, world),
            EngineCommand::ModifyComponent {
                entity_id,
                component_type,
                property,
                value,
            } => self.cmd_modify_component(entity_id, component_type, property, value, world),
            EngineCommand::SetParent {
                child_entity_id,
                parent_entity_id,
            } => self.cmd_set_parent(child_entity_id, parent_entity_id, world),
            EngineCommand::RemoveFromParent { entity_id } => {
                self.cmd_remove_from_parent(entity_id, world)
            }
            EngineCommand::ReparentChildren {
                source_parent_id,
                target_parent_id,
            } => self.cmd_reparent_children(source_parent_id, target_parent_id, world),
            EngineCommand::LoadAsset { path, asset_type } => {
                self.cmd_load_asset(path, asset_type, world)
            }
            EngineCommand::SetSpriteTexture {
                entity_id,
                asset_handle,
            } => self.cmd_set_sprite_texture(entity_id, asset_handle, world),
            EngineCommand::SpawnPrefab {
                asset_handle,
                transform,
            } => self.cmd_spawn_prefab(asset_handle, transform, world),
        }
    }

    fn cmd_create_entity(
        &mut self,
        name: String,
        components: Vec<ComponentPatch>,
        world: &mut World,
    ) -> Result<EngineCommandResult, AdapterError> {
        let mut entity_commands = world.spawn_empty();
        entity_commands.insert(Name::new(name.clone()));
        entity_commands.insert(Transform::default());
        entity_commands.insert(Visibility::default());

        let entity = entity_commands.id();
        let agent_id = self.register_entity(entity);

        for patch in &components {
            match patch.type_name.as_str() {
                "Sprite" => {
                    let mut sprite = Sprite::default();
                    if let Some(color_arr) = patch.value.get("color").and_then(|v| v.as_array()) {
                        if color_arr.len() == 4 {
                            let r = color_arr[0].as_f64().unwrap_or(1.0) as f32;
                            let g = color_arr[1].as_f64().unwrap_or(1.0) as f32;
                            let b = color_arr[2].as_f64().unwrap_or(1.0) as f32;
                            let a = color_arr[3].as_f64().unwrap_or(1.0) as f32;
                            sprite.color = Color::linear_rgba(r, g, b, a);
                        }
                    }
                    world.entity_mut(entity).insert(sprite);
                }
                _ => {
                    if !open_world_components::insert_engine_component_patch(
                        &mut world.entity_mut(entity),
                        patch,
                    ) {
                        log::warn!("Unknown component patch type: {}", patch.type_name);
                    }
                }
            }
        }

        Ok(EngineCommandResult {
            success: true,
            message: format!("Created entity: {}", name),
            entity_id: Some(agent_id.0),
        })
    }

    fn cmd_delete_entity(
        &mut self,
        entity_id: u64,
        world: &mut World,
    ) -> Result<EngineCommandResult, AdapterError> {
        let agent_eid = EntityId(entity_id);
        if let Some(bevy_entity) = self.get_bevy_entity(agent_eid) {
            world.despawn(bevy_entity);
            self.entity_map.remove(&agent_eid);
            self.reverse_map.remove(&bevy_entity);
            Ok(EngineCommandResult {
                success: true,
                message: format!("Deleted entity {}", entity_id),
                entity_id: None,
            })
        } else {
            Err(AdapterError::EntityNotFound(agent_eid))
        }
    }

    fn cmd_set_transform(
        &mut self,
        entity_id: u64,
        translation: Option<[f32; 3]>,
        rotation: Option<[f32; 3]>,
        scale: Option<[f32; 3]>,
        world: &mut World,
    ) -> Result<EngineCommandResult, AdapterError> {
        let agent_eid = EntityId(entity_id);
        match self.get_bevy_entity(agent_eid) {
            Some(bevy_entity) => {
                if let Some(trans) = translation {
                    if let Some(mut transform) = world.get_mut::<Transform>(bevy_entity) {
                        transform.translation = Vec3::new(trans[0], trans[1], trans[2]);
                    }
                }
                if let Some(rot) = rotation {
                    if let Some(mut transform) = world.get_mut::<Transform>(bevy_entity) {
                        transform.rotation =
                            Quat::from_euler(EulerRot::XYZ, rot[0], rot[1], rot[2]);
                    }
                }
                if let Some(scl) = scale {
                    if let Some(mut transform) = world.get_mut::<Transform>(bevy_entity) {
                        transform.scale = Vec3::new(scl[0], scl[1], scl[2]);
                    }
                }
                Ok(EngineCommandResult {
                    success: true,
                    message: format!("Updated transform for entity {}", entity_id),
                    entity_id: None,
                })
            }
            None => Err(AdapterError::EntityNotFound(agent_eid)),
        }
    }

    fn cmd_set_sprite_color(
        &mut self,
        entity_id: u64,
        rgba: [f32; 4],
        world: &mut World,
    ) -> Result<EngineCommandResult, AdapterError> {
        let agent_eid = EntityId(entity_id);
        match self.get_bevy_entity(agent_eid) {
            Some(bevy_entity) => {
                if let Some(mut sprite) = world.get_mut::<Sprite>(bevy_entity) {
                    sprite.color = Color::linear_rgba(rgba[0], rgba[1], rgba[2], rgba[3]);
                }
                Ok(EngineCommandResult {
                    success: true,
                    message: format!("Updated sprite color for entity {}", entity_id),
                    entity_id: None,
                })
            }
            None => Err(AdapterError::EntityNotFound(agent_eid)),
        }
    }

    fn cmd_set_visibility(
        &mut self,
        entity_id: u64,
        visible: bool,
        world: &mut World,
    ) -> Result<EngineCommandResult, AdapterError> {
        let agent_eid = EntityId(entity_id);
        match self.get_bevy_entity(agent_eid) {
            Some(bevy_entity) => {
                if let Some(mut vis) = world.get_mut::<Visibility>(bevy_entity) {
                    *vis = if visible {
                        Visibility::Visible
                    } else {
                        Visibility::Hidden
                    };
                }
                Ok(EngineCommandResult {
                    success: true,
                    message: format!("Set visibility of entity {} to {}", entity_id, visible),
                    entity_id: None,
                })
            }
            None => Err(AdapterError::EntityNotFound(agent_eid)),
        }
    }

    fn cmd_add_component(
        &mut self,
        entity_id: u64,
        component: ComponentPatch,
        world: &mut World,
    ) -> Result<EngineCommandResult, AdapterError> {
        let agent_eid = EntityId(entity_id);
        match self.get_bevy_entity(agent_eid) {
            Some(bevy_entity) => {
                let mut entity_mut = world.entity_mut(bevy_entity);
                match component.type_name.as_str() {
                    "Sprite" => {
                        entity_mut.insert(Sprite::default());
                    }
                    "Transform" => {
                        entity_mut.insert(Transform::default());
                    }
                    "Visibility" => {
                        entity_mut.insert(Visibility::default());
                    }
                    _ => {
                        if !open_world_components::insert_engine_component_patch(
                            &mut entity_mut,
                            &component,
                        ) {
                            log::warn!(
                                "Cannot add unknown component type: {}",
                                component.type_name
                            );
                        }
                    }
                }
                Ok(EngineCommandResult {
                    success: true,
                    message: format!("Added {} to entity {}", component.type_name, entity_id),
                    entity_id: None,
                })
            }
            None => Err(AdapterError::EntityNotFound(agent_eid)),
        }
    }

    fn cmd_remove_component(
        &mut self,
        entity_id: u64,
        component_type: String,
        world: &mut World,
    ) -> Result<EngineCommandResult, AdapterError> {
        let agent_eid = EntityId(entity_id);
        match self.get_bevy_entity(agent_eid) {
            Some(bevy_entity) => {
                let mut entity_mut = world.entity_mut(bevy_entity);
                match component_type.as_str() {
                    "Sprite" => {
                        entity_mut.remove::<Sprite>();
                    }
                    "Transform" => {
                        entity_mut.remove::<Transform>();
                    }
                    "Visibility" => {
                        entity_mut.remove::<Visibility>();
                    }
                    _ => {
                        if !open_world_components::remove_component(
                            &mut entity_mut,
                            &component_type,
                        ) {
                            log::warn!("Cannot remove unknown component type: {}", component_type);
                        }
                    }
                }
                Ok(EngineCommandResult {
                    success: true,
                    message: format!("Removed {} from entity {}", component_type, entity_id),
                    entity_id: None,
                })
            }
            None => Err(AdapterError::EntityNotFound(agent_eid)),
        }
    }

    fn cmd_modify_component(
        &mut self,
        entity_id: u64,
        component_type: String,
        property: String,
        value: serde_json::Value,
        world: &mut World,
    ) -> Result<EngineCommandResult, AdapterError> {
        let agent_eid = EntityId(entity_id);
        match self.get_bevy_entity(agent_eid) {
            Some(bevy_entity) => {
                let mut entity_mut = world.entity_mut(bevy_entity);
                match component_type.as_str() {
                    "Sprite" => {
                        if let Some(mut sprite) = entity_mut.get_mut::<Sprite>() {
                            match property.as_str() {
                                "color" => {
                                    if let Some(arr) = value.as_array() {
                                        if arr.len() == 4 {
                                            let r = arr[0].as_f64().unwrap_or(1.0) as f32;
                                            let g = arr[1].as_f64().unwrap_or(1.0) as f32;
                                            let b = arr[2].as_f64().unwrap_or(1.0) as f32;
                                            let a = arr[3].as_f64().unwrap_or(1.0) as f32;
                                            sprite.color = Color::linear_rgba(r, g, b, a);
                                        }
                                    }
                                }
                                "flip_x" => {
                                    if let Some(flip) = value.as_bool() {
                                        sprite.flip_x = flip;
                                    }
                                }
                                "flip_y" => {
                                    if let Some(flip) = value.as_bool() {
                                        sprite.flip_y = flip;
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                    _ => {
                        log::warn!("Cannot modify unknown component type: {}", component_type);
                    }
                }
                Ok(EngineCommandResult {
                    success: true,
                    message: format!(
                        "Modified {}.{} on entity {}",
                        component_type, property, entity_id
                    ),
                    entity_id: None,
                })
            }
            None => Err(AdapterError::EntityNotFound(agent_eid)),
        }
    }

    fn cmd_set_parent(
        &mut self,
        child_entity_id: u64,
        parent_entity_id: u64,
        world: &mut World,
    ) -> Result<EngineCommandResult, AdapterError> {
        let child_eid = EntityId(child_entity_id);
        let parent_eid = EntityId(parent_entity_id);
        match (
            self.get_bevy_entity(child_eid),
            self.get_bevy_entity(parent_eid),
        ) {
            (Some(child), Some(parent)) => {
                world.entity_mut(child).set_parent_in_place(parent);
                Ok(EngineCommandResult {
                    success: true,
                    message: format!(
                        "Set entity {} as child of {}",
                        child_entity_id, parent_entity_id
                    ),
                    entity_id: None,
                })
            }
            (None, _) => Err(AdapterError::EntityNotFound(child_eid)),
            (_, None) => Err(AdapterError::EntityNotFound(parent_eid)),
        }
    }

    fn cmd_remove_from_parent(
        &mut self,
        entity_id: u64,
        world: &mut World,
    ) -> Result<EngineCommandResult, AdapterError> {
        let agent_eid = EntityId(entity_id);
        match self.get_bevy_entity(agent_eid) {
            Some(bevy_entity) => {
                world.entity_mut(bevy_entity).remove_parent_in_place();
                Ok(EngineCommandResult {
                    success: true,
                    message: format!("Removed entity {} from its parent", entity_id),
                    entity_id: None,
                })
            }
            None => Err(AdapterError::EntityNotFound(agent_eid)),
        }
    }

    fn cmd_reparent_children(
        &mut self,
        source_parent_id: u64,
        target_parent_id: u64,
        world: &mut World,
    ) -> Result<EngineCommandResult, AdapterError> {
        let source_eid = EntityId(source_parent_id);
        let target_eid = EntityId(target_parent_id);
        match (
            self.get_bevy_entity(source_eid),
            self.get_bevy_entity(target_eid),
        ) {
            (Some(source), Some(target)) => {
                let children_to_reparent: Vec<Entity> = world
                    .get::<Children>(source)
                    .map(|c| c.iter().collect())
                    .unwrap_or_default();
                for child in children_to_reparent {
                    world.entity_mut(child).set_parent_in_place(target);
                }
                Ok(EngineCommandResult {
                    success: true,
                    message: format!(
                        "Reparented children from {} to {}",
                        source_parent_id, target_parent_id
                    ),
                    entity_id: None,
                })
            }
            (None, _) => Err(AdapterError::EntityNotFound(source_eid)),
            (_, None) => Err(AdapterError::EntityNotFound(target_eid)),
        }
    }

    fn cmd_load_asset(
        &mut self,
        path: String,
        asset_type: AssetType,
        world: &mut World,
    ) -> Result<EngineCommandResult, AdapterError> {
        use bevy::asset::AssetServer;
        let Some(asset_server) = world.get_resource::<AssetServer>() else {
            return Err(AdapterError::EngineNotConnected);
        };

        let handle_id = match asset_type {
            AssetType::Image | AssetType::SpriteSheet => {
                let handle: Handle<Image> = asset_server.load(&path);
                handle.id().to_string()
            }
            AssetType::Scene => {
                log::info!("Loading scene asset: {}", path);
                format!("scene://{}", path)
            }
            _ => {
                log::warn!("Asset type {:?} not fully supported yet", asset_type);
                format!("unsupported://{}", path)
            }
        };

        let asset_ref = AssetReference {
            handle: handle_id.clone(),
            asset_type: asset_type.clone(),
            path: path.clone(),
        };
        self.asset_references.insert(handle_id.clone(), asset_ref);
        log::info!("Loaded asset {} as {}", path, handle_id);

        Ok(EngineCommandResult {
            success: true,
            message: format!("Asset {} loaded with handle {}", path, handle_id),
            entity_id: None,
        })
    }

    fn cmd_set_sprite_texture(
        &mut self,
        entity_id: u64,
        asset_handle: String,
        world: &mut World,
    ) -> Result<EngineCommandResult, AdapterError> {
        use bevy::asset::AssetServer;
        let agent_eid = EntityId(entity_id);
        let Some(bevy_entity) = self.get_bevy_entity(agent_eid) else {
            return Err(AdapterError::EntityNotFound(agent_eid));
        };
        let Some(asset_server) = world.get_resource::<AssetServer>() else {
            return Err(AdapterError::EngineNotConnected);
        };

        let texture_handle: Handle<Image> = asset_server.load(&asset_handle);
        match world.get_mut::<Sprite>(bevy_entity) {
            Some(mut sprite) => {
                sprite.image = texture_handle;
                Ok(EngineCommandResult {
                    success: true,
                    message: format!("Set texture for entity {} to {}", entity_id, asset_handle),
                    entity_id: Some(entity_id),
                })
            }
            None => {
                let mut entity_mut = world.entity_mut(bevy_entity);
                entity_mut.insert(Sprite {
                    image: texture_handle,
                    ..Default::default()
                });
                Ok(EngineCommandResult {
                    success: true,
                    message: format!(
                        "Added Sprite with texture {} to entity {}",
                        asset_handle, entity_id
                    ),
                    entity_id: Some(entity_id),
                })
            }
        }
    }

    fn cmd_spawn_prefab(
        &mut self,
        asset_handle: String,
        transform: Option<[f32; 3]>,
        world: &mut World,
    ) -> Result<EngineCommandResult, AdapterError> {
        use bevy::asset::AssetServer;
        let Some(asset_server) = world.get_resource::<AssetServer>() else {
            return Err(AdapterError::EngineNotConnected);
        };

        let _scene_handle: Handle<bevy::asset::LoadedFolder> =
            asset_server.load_folder(&asset_handle);
        let transform = transform
            .map(|t| Transform {
                translation: Vec3::new(t[0], t[1], t[2]),
                ..Default::default()
            })
            .unwrap_or_default();

        let bevy_entity = world
            .spawn((
                transform,
                GlobalTransform::default(),
                Visibility::default(),
                Name::new(format!("Prefab({})", asset_handle)),
            ))
            .id();

        let agent_id = self.register_entity(bevy_entity);
        log::info!(
            "Spawned prefab {} at {:?} as entity {:?}",
            asset_handle,
            transform,
            agent_id
        );

        Ok(EngineCommandResult {
            success: true,
            message: format!("Spawned prefab {} as entity {}", asset_handle, agent_id.0),
            entity_id: Some(agent_id.0),
        })
    }
}
