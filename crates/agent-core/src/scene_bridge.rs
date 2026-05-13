//! SceneBridge — engine-agnostic interface for scene read/write operations.
//!
//! Abstract bridge between agent-core logic and the real game engine (Bevy).
//! agent-core remains bevy-free; the real engine access happens through
//! this trait's implementation in bevy-adapter.
//!
//! For testing, a MockSceneBridge is provided that stores entities in memory.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};

use crate::goal_checker::SceneEntityInfo;

// ---------------------------------------------------------------------------
// SharedSceneBridge — Arc<Mutex<>> wrapper for cross-tool sharing
// ---------------------------------------------------------------------------

/// Thread-safe shared reference to an optional SceneBridge.
///
/// Used by scene tools to access the bridge without requiring `&mut self`
/// on the `Tool` trait. The `Mutex` provides interior mutability so that
/// write operations (create_entity, update_component, delete_entity) can
/// be performed through `&self`.
pub type SharedSceneBridge = Arc<Mutex<Option<Box<dyn SceneBridge>>>>;

/// Create a `SharedSceneBridge` wrapping a real bridge implementation.
pub fn create_shared_bridge(bridge: Box<dyn SceneBridge>) -> SharedSceneBridge {
    Arc::new(Mutex::new(Some(bridge)))
}

/// Create a `SharedSceneBridge` with no bridge (disconnected mode).
pub fn create_empty_shared_bridge() -> SharedSceneBridge {
    Arc::new(Mutex::new(None))
}

// ---------------------------------------------------------------------------
// Entity list item (for query results)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityListItem {
    pub id: u64,
    pub name: String,
    pub components: Vec<String>,
}

// ---------------------------------------------------------------------------
// Component patch (for entity creation)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentPatch {
    pub type_name: String,
    pub properties: HashMap<String, serde_json::Value>,
}

// ---------------------------------------------------------------------------
// SceneBridge trait
// ---------------------------------------------------------------------------

/// Engine-agnostic trait for scene query and mutation.
///
/// Implementations:
/// - `MockSceneBridge` (agent-core, for unit tests)
/// - `BevySceneBridge` (bevy-adapter, real Bevy ECS)
pub trait SceneBridge: Send + Sync {
    /// Query entities matching optional filter and component type.
    fn query_entities(
        &self,
        filter: Option<&str>,
        component_type: Option<&str>,
    ) -> Vec<EntityListItem>;

    /// Get detailed info for a single entity.
    fn get_entity(&self, id: u64) -> Option<serde_json::Value>;

    /// Create a new entity. Returns the new entity ID.
    fn create_entity(
        &mut self,
        name: &str,
        position: Option<[f64; 2]>,
        components: &[ComponentPatch],
    ) -> Result<u64, String>;

    /// Update a property on an entity's component.
    fn update_component(
        &mut self,
        entity_id: u64,
        _component: &str,
        properties: HashMap<String, serde_json::Value>,
    ) -> Result<(), String>;

    /// Delete an entity by ID.
    fn delete_entity(&mut self, entity_id: u64) -> Result<(), String>;

    /// Take a snapshot of the current scene for goal checking.
    fn get_scene_snapshot(&self) -> Vec<SceneEntityInfo>;

    /// Drain accumulated engine commands for external application.
    ///
    /// Returns serialized commands (JSON). The default returns an empty vec.
    /// Bridges that queue write commands (e.g. SceneIndexSceneBridge) override
    /// this to export pending commands for real ECS application.
    fn drain_commands(&mut self) -> Vec<serde_json::Value> {
        vec![]
    }
}

// ---------------------------------------------------------------------------
// MockSceneBridge for unit tests
// ---------------------------------------------------------------------------

#[derive(Default)]
pub struct MockSceneBridge {
    entities: HashMap<u64, MockEntity>,
    next_id: u64,
    snapshot: Vec<SceneEntityInfo>,
}

#[derive(Debug, Clone)]
struct MockEntity {
    name: String,
    components: Vec<ComponentPatch>,
    position: [f64; 2],
}

impl MockSceneBridge {
    pub fn new() -> Self {
        Self {
            entities: HashMap::new(),
            next_id: 1,
            snapshot: Vec::new(),
        }
    }

    pub fn with_entity(
        mut self,
        name: &str,
        sprite_color: Option<[f32; 4]>,
        position: Option<[f32; 3]>,
    ) -> Self {
        let name_owned = name.to_string();
        let pos_f64 = position.map(|p| [p[0] as f64, p[1] as f64]);

        let components: Vec<ComponentPatch> = if let Some(color) = sprite_color {
            let mut props = HashMap::new();
            props.insert("color".to_string(), serde_json::json!(color));
            vec![ComponentPatch {
                type_name: "Sprite".to_string(),
                properties: props,
            }]
        } else {
            vec![]
        };

        self.create_entity(&name_owned, pos_f64, &components).ok();

        self
    }
}

impl SceneBridge for MockSceneBridge {
    fn query_entities(
        &self,
        filter: Option<&str>,
        _component_type: Option<&str>,
    ) -> Vec<EntityListItem> {
        self.entities
            .iter()
            .filter(|(_, e)| {
                filter.is_none_or(|f| e.name.contains(f) || f == "*")
            })
            .map(|(id, e)| EntityListItem {
                id: *id,
                name: e.name.clone(),
                components: e.components.iter().map(|c| c.type_name.clone()).collect(),
            })
            .collect()
    }

    fn get_entity(&self, id: u64) -> Option<serde_json::Value> {
        self.entities.get(&id).map(|e| {
            // Extract sprite_color from components
            let sprite_color = e.components.iter()
                .find(|c| c.type_name == "Sprite")
                .and_then(|c| c.properties.get("color"))
                .and_then(|v| v.as_array())
                .map(|arr| {
                    serde_json::json!([arr[0].as_f64().unwrap_or(1.0),
                                       arr[1].as_f64().unwrap_or(1.0),
                                       arr[2].as_f64().unwrap_or(1.0),
                                       arr[3].as_f64().unwrap_or(1.0)])
                });

            let visible = e.components.iter()
                .find(|c| c.type_name == "Visibility")
                .and_then(|c| c.properties.get("visible"))
                .and_then(|v| v.as_bool());

            let mut json = serde_json::json!({
                "id": id,
                "name": e.name,
                "position": [e.position[0], e.position[1]],
                "components": e.components.iter().map(|c| {
                    serde_json::json!({
                        "type": c.type_name,
                        "properties": c.properties
                    })
                }).collect::<Vec<_>>()
            });

            if let Some(sc) = sprite_color {
                json["sprite_color"] = sc;
            }
            if let Some(v) = visible {
                json["visible"] = serde_json::json!(v);
            }

            json
        })
    }

    fn create_entity(
        &mut self,
        name: &str,
        position: Option<[f64; 2]>,
        components: &[ComponentPatch],
    ) -> Result<u64, String> {
        let id = self.next_id;
        self.next_id += 1;

        self.entities.insert(
            id,
            MockEntity {
                name: name.to_string(),
                components: components.to_vec(),
                position: position.unwrap_or([0.0, 0.0]),
            },
        );

        let mut info = SceneEntityInfo {
            name: name.to_string(),
            components: vec!["Transform".to_string()],
            translation: position.map(|p| [p[0] as f32, p[1] as f32, 0.0]),
            sprite_color: None,
        };

        for c in components {
            info.components.push(c.type_name.clone());
            if c.type_name == "Sprite" {
                if let Some(color) = c.properties.get("color").and_then(|v| v.as_array()) {
                    info.sprite_color = Some([
                        color.first().and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
                        color.get(1).and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
                        color.get(2).and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
                        color.get(3).and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
                    ]);
                }
            }
        }

        self.snapshot.push(info);
        Ok(id)
    }

    fn update_component(
        &mut self,
        entity_id: u64,
        component: &str,
        properties: HashMap<String, serde_json::Value>,
    ) -> Result<(), String> {
        if let Some(entity) = self.entities.get_mut(&entity_id) {
            // Find or create the component patch
            if let Some(c) = entity.components.iter_mut().find(|c| c.type_name == component) {
                c.properties.extend(properties);
            } else {
                entity.components.push(ComponentPatch {
                    type_name: component.to_string(),
                    properties,
                });
            }
            Ok(())
        } else {
            Err(format!("Entity {} not found", entity_id))
        }
    }

    fn delete_entity(&mut self, entity_id: u64) -> Result<(), String> {
        if self.entities.remove(&entity_id).is_none() {
            return Err(format!("Entity {} not found", entity_id));
        }
        self.snapshot.retain(|e| {
            self.entities.iter().any(|(_id, ent)| ent.name == e.name)
        });
        Ok(())
    }

    fn get_scene_snapshot(&self) -> Vec<SceneEntityInfo> {
        self.snapshot.clone()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_create_and_query() {
        let mut bridge = MockSceneBridge::new();
        let id = bridge.create_entity("TestEntity", Some([100.0, 200.0]), &[]).unwrap();
        assert_eq!(id, 1);

        let results = bridge.query_entities(None, None);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "TestEntity");
    }

    #[test]
    fn test_mock_create_with_sprite_color() {
        let mut bridge = MockSceneBridge::new();
        let patch = ComponentPatch {
            type_name: "Sprite".to_string(),
            properties: {
                let mut m = HashMap::new();
                m.insert("color".to_string(), serde_json::json!([1.0, 0.0, 0.0, 1.0]));
                m
            },
        };
        bridge.create_entity("Enemy", None, &[patch]).ok();

        let snapshot = bridge.get_scene_snapshot();
        assert_eq!(snapshot.len(), 1);
        assert_eq!(snapshot[0].sprite_color, Some([1.0, 0.0, 0.0, 1.0]));
    }

    #[test]
    fn test_mock_snapshot_for_goal_checker() {
        let bridge = MockSceneBridge::new()
            .with_entity("Player", Some([0.0, 0.0, 1.0, 1.0]), Some([0.0, 0.0, 0.0]))
            .with_entity("Enemy", Some([1.0, 0.0, 0.0, 1.0]), Some([5.0, 0.0, 0.0]));

        let snapshot = bridge.get_scene_snapshot();
        assert_eq!(snapshot.len(), 2);

        let checker = crate::goal_checker::GoalChecker::new();
        let reqs = vec![
            crate::goal::GoalRequirementKind::EntityExists {
                name: "Enemy".to_string(),
            },
            crate::goal::GoalRequirementKind::SpriteColorIs {
                entity_name: "Enemy".to_string(),
                rgba: [1.0, 0.0, 0.0, 1.0],
            },
        ];
        let result = checker.check(&reqs, &snapshot);
        assert!(result.all_matched);
    }

    #[test]
    fn test_mock_query_with_filter() {
        let mut bridge = MockSceneBridge::new();
        bridge.create_entity("Player", None, &[]).ok();
        bridge.create_entity("Enemy_1", None, &[]).ok();
        bridge.create_entity("Enemy_2", None, &[]).ok();

        let results = bridge.query_entities(Some("Enemy"), None);
        assert_eq!(results.len(), 2);
    }
}
