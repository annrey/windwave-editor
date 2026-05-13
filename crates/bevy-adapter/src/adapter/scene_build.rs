//! Scene index building for the BevyAdapter.
//!
//! Walks the Bevy ECS World to produce a hierarchical `SceneIndex` that
//! Agents can use for spatial reasoning about the scene graph.

use bevy::prelude::*;
use bevy::ecs::hierarchy::{Children, ChildOf};
use bevy::sprite::Sprite;
use crate::scene_index::{SceneIndex, SceneEntityNode, ComponentSummary};
use std::collections::HashMap;

use super::BevyAdapter;

impl BevyAdapter {
    /// Build a hierarchical SceneIndex from the Bevy World.
    ///
    /// Uses a two-pass approach: first collects all entities into a flat
    /// HashMap, then wires up parent-child relationships using Bevy's
    /// `Children` and `Parent` components.
    pub fn build_scene_index(&mut self, world: &mut World) -> SceneIndex {
        let mut index = SceneIndex::new();

        let mut query = world.query::<(
            Entity,
            Option<&Name>,
            Option<&Transform>,
            Option<&Sprite>,
            Option<&Children>,
            Option<&ChildOf>,
            Option<&Visibility>,
            Option<&ViewVisibility>,
            Option<&InheritedVisibility>,
        )>();

        // Pass 1: Build nodes and collect hierarchy metadata
        let mut nodes_map: HashMap<Entity, SceneEntityNode> = HashMap::new();
        let mut roots: Vec<Entity> = Vec::new();
        let mut child_map: HashMap<Entity, Vec<Entity>> = HashMap::new();

        for (entity, name, transform, sprite, children_opt, parent_opt, visibility, view_visibility, _inherited_visibility) in query.iter(world) {
            let agent_id = self
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
                // Phase 6: Extended Sprite properties
                if let Some(custom_size) = s.custom_size {
                    props.insert(
                        "custom_size".to_string(),
                        serde_json::json!([custom_size.x, custom_size.y]),
                    );
                }
                props.insert(
                    "flip_x".to_string(),
                    serde_json::json!(s.flip_x),
                );
                props.insert(
                    "flip_y".to_string(),
                    serde_json::json!(s.flip_y),
                );
                components.push(ComponentSummary {
                    type_name: "Sprite".to_string(),
                    properties: props,
                });
            }

            // Phase 6: Visibility component
            if let Some(v) = visibility {
                let mut props = HashMap::new();
                props.insert(
                    "visible".to_string(),
                    serde_json::json!(matches!(v, Visibility::Visible)),
                );
                components.push(ComponentSummary {
                    type_name: "Visibility".to_string(),
                    properties: props,
                });
            }

            // Phase 6: View visibility
            if let Some(_v) = view_visibility {
                components.push(ComponentSummary {
                    type_name: "ViewVisibility".to_string(),
                    properties: HashMap::new(),
                });
            }

            let node = SceneEntityNode {
                id: agent_id,
                name: entity_name.clone(),
                components: components.clone(),
                children: Vec::new(),
            };

            // Populate flat indexes
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

        // Pass 2: Build hierarchy by wiring children into parent nodes
        fn build_tree(
            entity: Entity,
            nodes_map: &mut HashMap<Entity, SceneEntityNode>,
            child_map: &HashMap<Entity, Vec<Entity>>,
        ) -> SceneEntityNode {
            let mut node = nodes_map.remove(&entity).expect("node must exist in map");
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

        self.scene_index_cache = Some(index.clone());
        index
    }
}
