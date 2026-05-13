//! Scene Index - Hierarchical scene representation for Agent reasoning
//!
//! Provides a serializable snapshot of the Bevy ECS scene graph,
//! enabling Agents to query entities by name, component type, and hierarchy.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Maximum allowed depth for recursive tree traversal to prevent stack overflow.
const MAX_TREE_DEPTH: usize = 256;

/// A hierarchical tree of scene entities, starting from root-level entities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneIndex {
    pub root_entities: Vec<SceneEntityNode>,
    pub entities_by_name: HashMap<String, u64>,
    pub entities_by_component: HashMap<String, Vec<u64>>,
}

/// A single node in the scene entity tree
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneEntityNode {
    pub id: u64,
    pub name: String,
    pub components: Vec<ComponentSummary>,
    pub children: Vec<SceneEntityNode>,
}

/// Summary of a component attached to an entity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentSummary {
    pub type_name: String,
    pub properties: HashMap<String, serde_json::Value>,
}

/// Simplified entity info used for goal checking / validation
#[derive(Debug, Clone)]
pub struct SceneEntityInfo {
    pub id: u64,
    pub name: String,
    pub components: Vec<String>,
    pub translation: Option<[f32; 3]>,
    pub sprite_color: Option<[f32; 4]>,
}

impl SceneIndex {
    /// Create an empty SceneIndex
    pub fn new() -> Self {
        Self {
            root_entities: Vec::new(),
            entities_by_name: HashMap::new(),
            entities_by_component: HashMap::new(),
        }
    }

    /// Look up an entity by its name (recursive search through the tree)
    pub fn get_entity_by_name(&self, name: &str) -> Option<&SceneEntityNode> {
        self.entities_by_name
            .get(name)
            .and_then(|id| self.find_node_by_id(*id, &self.root_entities))
    }

    /// Find all entities that have a component with the given type name
    pub fn get_entities_with_component(&self, component: &str) -> Vec<&SceneEntityNode> {
        self.entities_by_component
            .get(component)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| self.find_node_by_id(*id, &self.root_entities))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Get all entity names in the scene
    pub fn entity_names(&self) -> Vec<String> {
        self.entities_by_name.keys().cloned().collect()
    }

    /// Convert SceneIndex to a flat list of SceneEntityInfo for goal checking
    pub fn to_entity_info_list(&self) -> Vec<SceneEntityInfo> {
        fn collect_info(nodes: &[SceneEntityNode], depth: usize) -> Vec<SceneEntityInfo> {
            if depth > MAX_TREE_DEPTH {
                return Vec::new();
            }
            let mut result = Vec::new();
            for node in nodes {
                let components: Vec<String> =
                    node.components.iter().map(|c| c.type_name.clone()).collect();

                let translation = node
                    .components
                    .iter()
                    .find(|c| c.type_name == "Transform")
                    .and_then(|c| c.properties.get("translation"))
                    .and_then(|v| v.as_array())
                    .and_then(|arr| {
                        if arr.len() >= 3 {
                            Some([
                                arr[0].as_f64().unwrap_or(0.0) as f32,
                                arr[1].as_f64().unwrap_or(0.0) as f32,
                                arr[2].as_f64().unwrap_or(0.0) as f32,
                            ])
                        } else {
                            None
                        }
                    });

                let sprite_color = node
                    .components
                    .iter()
                    .find(|c| c.type_name == "Sprite")
                    .and_then(|c| c.properties.get("color"))
                    .and_then(|v| v.as_array())
                    .and_then(|arr| {
                        if arr.len() >= 4 {
                            Some([
                                arr[0].as_f64().unwrap_or(0.0) as f32,
                                arr[1].as_f64().unwrap_or(0.0) as f32,
                                arr[2].as_f64().unwrap_or(0.0) as f32,
                                arr[3].as_f64().unwrap_or(0.0) as f32,
                            ])
                        } else {
                            None
                        }
                    });

                result.push(SceneEntityInfo {
                    id: node.id,
                    name: node.name.clone(),
                    components,
                    translation,
                    sprite_color,
                });

                // Recurse into children
                result.extend(collect_info(&node.children, depth + 1));
            }
            result
        }

        collect_info(&self.root_entities, 0)
    }

    /// Recursively find a node by its id in the entity tree
    fn find_node_by_id<'a>(
        &self,
        target_id: u64,
        nodes: &'a [SceneEntityNode],
    ) -> Option<&'a SceneEntityNode> {
        self.find_node_by_id_depth(target_id, nodes, 0)
    }

    /// Add or update an entity in the index (flat list, no hierarchy).
    pub fn add_entity(&mut self, name: String, id: u64, components: Vec<ComponentSummary>) {
        self.entities_by_name.insert(name.clone(), id);
        for c in &components {
            self.entities_by_component
                .entry(c.type_name.clone())
                .or_default()
                .push(id);
        }
        // Update or insert in root list
        if let Some(idx) = self.root_entities.iter().position(|n| n.id == id) {
            self.root_entities[idx] = SceneEntityNode { id, name, components, children: Vec::new() };
        } else {
            self.root_entities.push(SceneEntityNode { id, name, components, children: Vec::new() });
        }
    }

    /// Remove an entity from the index by its id (cleanup ghost entries).
    pub fn remove_entity(&mut self, id: u64) {
        // Find and remove the entity by id from root_entities and all children
        fn remove_by_id(nodes: &mut Vec<SceneEntityNode>, target_id: u64) -> bool {
            let mut found = false;
            nodes.retain(|node| {
                if node.id == target_id {
                    found = true;
                    false
                } else {
                    true
                }
            });
            // Process children separately to avoid borrow issues
            for node in nodes.iter_mut() {
                if remove_by_id(&mut node.children, target_id) {
                    found = true;
                }
            }
            found
        }

        // Remove from root_entities (including nested children)
        remove_by_id(&mut self.root_entities, id);

        // Remove from entities_by_name (find name by id)
        self.entities_by_name.retain(|_, nid| *nid != id);

        // Remove from entities_by_component (remove id from all component lists)
        for ids in self.entities_by_component.values_mut() {
            ids.retain(|nid| *nid != id);
        }
    }

    /// Depth-limited recursive search for a node by id.
    fn find_node_by_id_depth<'a>(
        &self,
        target_id: u64,
        nodes: &'a [SceneEntityNode],
        depth: usize,
    ) -> Option<&'a SceneEntityNode> {
        if depth > MAX_TREE_DEPTH {
            return None;
        }
        for node in nodes {
            if node.id == target_id {
                return Some(node);
            }
            if let Some(found) = self.find_node_by_id_depth(target_id, &node.children, depth + 1) {
                return Some(found);
            }
        }
        None
    }
}

impl Default for SceneIndex {
    fn default() -> Self {
        Self::new()
    }
}
