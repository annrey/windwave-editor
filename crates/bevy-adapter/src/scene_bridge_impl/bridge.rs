use super::commands::SceneCommand;
use super::ops::BevySceneOps;
use crate::scene_index::{SceneEntityNode, SceneIndex};
use agent_core::goal_checker::SceneEntityInfo;
use agent_core::scene_bridge::{ComponentPatch, EntityListItem, SceneBridge};
use bevy::prelude::*;
use std::collections::HashMap;

/// Resource for scene operations with command queue
#[derive(Resource, Default, Debug)]
pub struct BevySceneBridge {
    /// Entity ID to Bevy Entity mapping
    pub(crate) entity_map: HashMap<u64, Entity>,
    /// Reverse mapping
    pub(crate) reverse_map: HashMap<Entity, u64>,
    /// Next entity ID to assign
    next_entity_id: u64,
    /// Cached scene index for quick queries
    scene_index_cache: Option<SceneIndex>,
    /// Pending scene commands queued by SceneBridge trait methods
    pending_commands: Vec<SceneCommand>,
    /// Pending component updates to be processed by exclusive system
    pub(crate) pending_updates: Vec<(u64, String, HashMap<String, serde_json::Value>)>,
}

impl BevySceneBridge {
    pub fn new() -> Self {
        Self {
            entity_map: HashMap::new(),
            reverse_map: HashMap::new(),
            next_entity_id: 1,
            scene_index_cache: None,
            pending_commands: Vec::new(),
            pending_updates: Vec::new(),
        }
    }

    /// Register a Bevy Entity with a scene entity ID
    pub fn register_entity(&mut self, scene_id: u64, bevy_entity: Entity) {
        self.entity_map.insert(scene_id, bevy_entity);
        self.reverse_map.insert(bevy_entity, scene_id);
    }

    /// Generate a new scene entity ID
    pub fn generate_id(&mut self) -> u64 {
        let id = self.next_entity_id;
        self.next_entity_id += 1;
        id
    }

    /// Get Bevy Entity from scene ID
    pub fn get_bevy_entity(&self, scene_id: u64) -> Option<Entity> {
        self.entity_map.get(&scene_id).copied()
    }

    /// Get scene ID from Bevy Entity
    pub fn get_scene_id(&self, bevy_entity: Entity) -> Option<u64> {
        self.reverse_map.get(&bevy_entity).copied()
    }

    /// Remove entity from mappings
    pub fn unregister_entity(&mut self, scene_id: u64) {
        if let Some(bevy_entity) = self.entity_map.remove(&scene_id) {
            self.reverse_map.remove(&bevy_entity);
        }
    }

    /// Clear all mappings (e.g., on scene load)
    pub fn clear_mappings(&mut self) {
        self.entity_map.clear();
        self.reverse_map.clear();
        self.next_entity_id = 1;
    }

    /// Drain and return all pending scene commands queued by SceneBridge trait methods.
    pub fn take_pending_commands(&mut self) -> Vec<SceneCommand> {
        std::mem::take(&mut self.pending_commands)
    }

    /// Update the cached scene index from BevyAdapter or a pre-built index.
    pub fn sync_scene_index(&mut self, index: SceneIndex) {
        self.scene_index_cache = Some(index);
    }

    /// Flatten a scene entity node tree into EntityListItem list.
    fn flatten_nodes(node: &SceneEntityNode) -> Vec<EntityListItem> {
        let mut items = vec![EntityListItem {
            id: node.id,
            name: node.name.clone(),
            components: node
                .components
                .iter()
                .map(|c| c.type_name.clone())
                .collect(),
        }];
        for child in &node.children {
            items.extend(Self::flatten_nodes(child));
        }
        items
    }
}

impl SceneBridge for BevySceneBridge {
    fn query_entities(
        &self,
        filter: Option<&str>,
        component_type: Option<&str>,
    ) -> Vec<EntityListItem> {
        if let Some(ref index) = self.scene_index_cache {
            let mut results: Vec<EntityListItem> = index
                .root_entities
                .iter()
                .flat_map(Self::flatten_nodes)
                .collect();

            if let Some(name_filter) = filter {
                if name_filter != "*" {
                    results.retain(|e| e.name.contains(name_filter));
                }
            }
            if let Some(comp_filter) = component_type {
                results.retain(|e| e.components.iter().any(|c| c.contains(comp_filter)));
            }

            results
        } else {
            Vec::new()
        }
    }

    fn get_entity(&self, id: u64) -> Option<serde_json::Value> {
        self.scene_index_cache.as_ref().and_then(|index| {
            let node = index.find_node_by_id(id, &index.root_entities)?;
            let comps: serde_json::Map<String, serde_json::Value> = node
                .components
                .iter()
                .map(|c| (c.type_name.clone(), serde_json::json!(c.properties)))
                .collect();
            Some(serde_json::json!({
                "id": node.id,
                "name": node.name,
                "components": comps,
            }))
        })
    }

    fn create_entity(
        &mut self,
        name: &str,
        position: Option<[f64; 2]>,
        components: &[ComponentPatch],
    ) -> Result<u64, String> {
        let id = self.generate_id();

        self.pending_commands.push(SceneCommand::CreateEntity {
            name: name.to_string(),
            position,
            components: components.to_vec(),
        });

        log::debug!("Queued entity creation: {} (id: {})", name, id);
        Ok(id)
    }

    fn update_component(
        &mut self,
        entity_id: u64,
        component: &str,
        properties: HashMap<String, serde_json::Value>,
    ) -> Result<(), String> {
        self.pending_commands.push(SceneCommand::UpdateComponent {
            entity_id,
            component: component.to_string(),
            properties,
        });
        log::debug!(
            "Queued component update: entity={}, component={}",
            entity_id,
            component
        );
        Ok(())
    }

    fn delete_entity(&mut self, entity_id: u64) -> Result<(), String> {
        self.unregister_entity(entity_id);
        log::debug!("Queued entity deletion: {}", entity_id);
        Ok(())
    }

    fn get_scene_snapshot(&self) -> Vec<SceneEntityInfo> {
        self.scene_index_cache
            .as_ref()
            .map(BevySceneOps::snapshot_from_index)
            .unwrap_or_default()
    }
}
