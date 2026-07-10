//! Scene Context — Unified interface for scene-aware operations.
//!
//! This module provides a unified scene context interface that integrates
//! with both multica-bridge and agent-core's scene_bridge system.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Entity representation in the scene context.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SceneEntity {
    pub id: u64,
    pub name: String,
    pub components: Vec<ComponentData>,
    pub position: Option<[f64; 3]>,
}

/// Component data for scene entities.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ComponentData {
    pub type_name: String,
    pub properties: HashMap<String, serde_json::Value>,
}

/// Scene snapshot for tracking state changes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneSnapshot {
    pub entities: Vec<SceneEntity>,
    pub timestamp: String,
    pub version: u64,
}

/// Difference between two scene snapshots.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneDiff {
    pub added: Vec<SceneEntity>,
    pub removed: Vec<SceneEntity>,
    pub modified: Vec<EntityChange>,
}

/// Change to a single entity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityChange {
    pub entity_id: u64,
    pub before: Option<SceneEntity>,
    pub after: Option<SceneEntity>,
    pub changes: Vec<String>,
}

/// Trait defining scene context operations.
pub trait SceneContext: Send + Sync {
    /// Query entities with optional filters.
    fn query_entities(
        &self,
        name_filter: Option<&str>,
        component_type: Option<&str>,
    ) -> Vec<SceneEntity>;

    /// Get a single entity by ID.
    fn get_entity(&self, id: u64) -> Option<SceneEntity>;

    /// Create a new entity in the scene.
    fn create_entity(
        &mut self,
        name: &str,
        position: Option<[f64; 3]>,
        components: &[ComponentData],
    ) -> Result<u64, String>;

    /// Update an entity's component.
    fn update_component(
        &mut self,
        entity_id: u64,
        component_type: &str,
        properties: HashMap<String, serde_json::Value>,
    ) -> Result<(), String>;

    /// Delete an entity from the scene.
    fn delete_entity(&mut self, entity_id: u64) -> Result<(), String>;

    /// Take a snapshot of the current scene state.
    fn snapshot(&self) -> SceneSnapshot;

    /// Compute the difference between two snapshots.
    fn diff(&self, before: &SceneSnapshot, after: &SceneSnapshot) -> SceneDiff;
}

/// In-memory implementation of SceneContext for testing and basic usage.
pub struct InMemorySceneContext {
    entities: HashMap<u64, SceneEntity>,
    next_id: u64,
    version: u64,
    #[allow(dead_code)] // Snapshots stored for potential future use (undo, history)
    snapshots: Vec<SceneSnapshot>,
}

impl InMemorySceneContext {
    pub fn new() -> Self {
        Self {
            entities: HashMap::new(),
            next_id: 1,
            version: 0,
            snapshots: Vec::new(),
        }
    }

    fn increment_version(&mut self) {
        self.version += 1;
    }
}

impl Default for InMemorySceneContext {
    fn default() -> Self {
        Self::new()
    }
}

impl SceneContext for InMemorySceneContext {
    fn query_entities(
        &self,
        name_filter: Option<&str>,
        component_type: Option<&str>,
    ) -> Vec<SceneEntity> {
        self.entities
            .values()
            .filter(|entity| {
                let name_match = name_filter
                    .map(|f| entity.name.contains(f) || f == "*")
                    .unwrap_or(true);

                let component_match = component_type
                    .map(|ct| entity.components.iter().any(|c| c.type_name == ct))
                    .unwrap_or(true);

                name_match && component_match
            })
            .cloned()
            .collect()
    }

    fn get_entity(&self, id: u64) -> Option<SceneEntity> {
        self.entities.get(&id).cloned()
    }

    fn create_entity(
        &mut self,
        name: &str,
        position: Option<[f64; 3]>,
        components: &[ComponentData],
    ) -> Result<u64, String> {
        let id = self.next_id;
        self.next_id += 1;

        let entity = SceneEntity {
            id,
            name: name.to_string(),
            components: components.to_vec(),
            position,
        };

        self.entities.insert(id, entity);
        self.increment_version();

        Ok(id)
    }

    fn update_component(
        &mut self,
        entity_id: u64,
        component_type: &str,
        properties: HashMap<String, serde_json::Value>,
    ) -> Result<(), String> {
        let entity = self
            .entities
            .get_mut(&entity_id)
            .ok_or_else(|| format!("Entity {} not found", entity_id))?;

        if let Some(component) = entity
            .components
            .iter_mut()
            .find(|c| c.type_name == component_type)
        {
            component.properties.extend(properties);
        } else {
            entity.components.push(ComponentData {
                type_name: component_type.to_string(),
                properties,
            });
        }

        self.increment_version();
        Ok(())
    }

    fn delete_entity(&mut self, entity_id: u64) -> Result<(), String> {
        if self.entities.remove(&entity_id).is_none() {
            return Err(format!("Entity {} not found", entity_id));
        }

        self.increment_version();
        Ok(())
    }

    fn snapshot(&self) -> SceneSnapshot {
        SceneSnapshot {
            entities: self.entities.values().cloned().collect(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            version: self.version,
        }
    }

    fn diff(&self, before: &SceneSnapshot, after: &SceneSnapshot) -> SceneDiff {
        let before_map: HashMap<u64, &SceneEntity> =
            before.entities.iter().map(|e| (e.id, e)).collect();

        let after_map: HashMap<u64, &SceneEntity> =
            after.entities.iter().map(|e| (e.id, e)).collect();

        let mut added = Vec::new();
        let mut removed = Vec::new();
        let mut modified = Vec::new();

        // Find added entities
        for (id, entity) in &after_map {
            if !before_map.contains_key(id) {
                added.push((*entity).clone());
            }
        }

        // Find removed entities
        for (id, entity) in &before_map {
            if !after_map.contains_key(id) {
                removed.push((*entity).clone());
            }
        }

        // Find modified entities
        for (id, after_entity) in &after_map {
            if let Some(before_entity) = before_map.get(id) {
                let mut changes = Vec::new();

                if before_entity.name != after_entity.name {
                    changes.push("name".to_string());
                }

                if before_entity.position != after_entity.position {
                    changes.push("position".to_string());
                }

                if before_entity.components != after_entity.components {
                    changes.push("components".to_string());
                }

                if !changes.is_empty() {
                    modified.push(EntityChange {
                        entity_id: *id,
                        before: Some((*before_entity).clone()),
                        after: Some((*after_entity).clone()),
                        changes,
                    });
                }
            }
        }

        SceneDiff {
            added,
            removed,
            modified,
        }
    }
}

/// Thread-safe shared scene context.
pub type SharedSceneContext = Arc<Mutex<Box<dyn SceneContext>>>;

/// Create a shared scene context with in-memory implementation.
pub fn create_shared_scene_context() -> SharedSceneContext {
    Arc::new(Mutex::new(Box::new(InMemorySceneContext::new())))
}

/// Create a shared scene context with a custom implementation.
pub fn create_shared_scene_context_with(
    implementation: Box<dyn SceneContext>,
) -> SharedSceneContext {
    Arc::new(Mutex::new(implementation))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_and_query_entity() {
        let mut ctx = InMemorySceneContext::new();
        let id = ctx.create_entity("TestEntity", None, &[]).unwrap();
        assert_eq!(id, 1);

        let entities = ctx.query_entities(None, None);
        assert_eq!(entities.len(), 1);
        assert_eq!(entities[0].name, "TestEntity");
    }

    #[test]
    fn test_update_component() {
        let mut ctx = InMemorySceneContext::new();
        let id = ctx.create_entity("TestEntity", None, &[]).unwrap();

        let mut props = HashMap::new();
        props.insert("color".to_string(), serde_json::json!([1.0, 0.0, 0.0, 1.0]));

        ctx.update_component(id, "Sprite", props).unwrap();

        let entity = ctx.get_entity(id).unwrap();
        assert_eq!(entity.components.len(), 1);
        assert_eq!(entity.components[0].type_name, "Sprite");
    }

    #[test]
    fn test_delete_entity() {
        let mut ctx = InMemorySceneContext::new();
        let id = ctx.create_entity("TestEntity", None, &[]).unwrap();

        ctx.delete_entity(id).unwrap();

        let entities = ctx.query_entities(None, None);
        assert!(entities.is_empty());
    }

    #[test]
    fn test_snapshot_and_diff() {
        let mut ctx = InMemorySceneContext::new();

        let before = ctx.snapshot();

        ctx.create_entity("TestEntity", None, &[]).unwrap();

        let after = ctx.snapshot();

        let diff = ctx.diff(&before, &after);
        assert_eq!(diff.added.len(), 1);
        assert_eq!(diff.removed.len(), 0);
        assert_eq!(diff.modified.len(), 0);
    }
}
