use crate::types::EntityId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PrefabId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PrefabInstanceId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LevelId(pub String);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LevelDocument {
    pub id: LevelId,
    pub name: String,
    pub root_entities: Vec<EntityId>,
    pub prefab_instances: Vec<PrefabInstanceInfo>,
    pub metadata: HashMap<String, serde_json::Value>,
}

impl LevelDocument {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: LevelId(id.into()),
            name: name.into(),
            root_entities: Vec::new(),
            prefab_instances: Vec::new(),
            metadata: HashMap::new(),
        }
    }

    pub fn add_root_entity(&mut self, entity: EntityId) {
        if !self.root_entities.contains(&entity) {
            self.root_entities.push(entity);
        }
    }

    pub fn add_prefab_instance(&mut self, instance: PrefabInstanceInfo) {
        self.prefab_instances.push(instance);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentPatch {
    pub type_name: String,
    pub properties: HashMap<String, serde_json::Value>,
}

impl ComponentPatch {
    pub fn new(type_name: impl Into<String>) -> Self {
        Self {
            type_name: type_name.into(),
            properties: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrefabDefinition {
    pub id: PrefabId,
    pub name: String,
    pub root: PrefabNode,
    pub metadata: HashMap<String, serde_json::Value>,
}

impl PrefabDefinition {
    pub fn new(id: impl Into<String>, name: impl Into<String>, root: PrefabNode) -> Self {
        Self {
            id: PrefabId(id.into()),
            name: name.into(),
            root,
            metadata: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrefabNode {
    pub name: String,
    pub components: Vec<ComponentPatch>,
    pub children: Vec<PrefabNode>,
}

impl PrefabNode {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            components: Vec::new(),
            children: Vec::new(),
        }
    }

    pub fn with_component(mut self, component: ComponentPatch) -> Self {
        self.components.push(component);
        self
    }

    pub fn with_child(mut self, child: PrefabNode) -> Self {
        self.children.push(child);
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrefabInstanceInfo {
    pub instance_id: PrefabInstanceId,
    pub prefab_id: PrefabId,
    pub root_entity: EntityId,
    pub overrides: Vec<ComponentOverride>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentOverride {
    pub entity_path: Vec<String>,
    pub component_type: String,
    pub property: String,
    pub value: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentSchema {
    pub name: String,
    pub properties: Vec<ComponentPropertySchema>,
    pub agent_editable: bool,
    pub runtime_visible: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentPropertySchema {
    pub name: String,
    pub value_type: ComponentValueType,
    pub writable: bool,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ComponentValueType {
    Bool,
    Int,
    Float,
    String,
    Vec2,
    Vec3,
    ColorRgba,
    EntityRef,
    AssetHandle,
    Json,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ComponentSchemaRegistry {
    schemas: HashMap<String, ComponentSchema>,
}

impl ComponentSchemaRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_bevy_defaults() -> Self {
        let mut registry = Self::new();
        registry.register(ComponentSchema {
            name: "Transform".to_string(),
            agent_editable: true,
            runtime_visible: true,
            properties: vec![
                ComponentPropertySchema {
                    name: "translation".to_string(),
                    value_type: ComponentValueType::Vec3,
                    writable: true,
                    description: "World/local translation [x, y, z]".to_string(),
                },
                ComponentPropertySchema {
                    name: "rotation".to_string(),
                    value_type: ComponentValueType::Vec3,
                    writable: true,
                    description: "Euler rotation [x, y, z]".to_string(),
                },
                ComponentPropertySchema {
                    name: "scale".to_string(),
                    value_type: ComponentValueType::Vec3,
                    writable: true,
                    description: "Scale [x, y, z]".to_string(),
                },
            ],
        });
        registry.register(ComponentSchema {
            name: "Sprite".to_string(),
            agent_editable: true,
            runtime_visible: true,
            properties: vec![
                ComponentPropertySchema {
                    name: "color".to_string(),
                    value_type: ComponentValueType::ColorRgba,
                    writable: true,
                    description: "Sprite tint [r, g, b, a]".to_string(),
                },
                ComponentPropertySchema {
                    name: "image".to_string(),
                    value_type: ComponentValueType::AssetHandle,
                    writable: true,
                    description: "Texture asset handle or path".to_string(),
                },
            ],
        });
        registry.register(ComponentSchema {
            name: "Visibility".to_string(),
            agent_editable: true,
            runtime_visible: true,
            properties: vec![ComponentPropertySchema {
                name: "visible".to_string(),
                value_type: ComponentValueType::Bool,
                writable: true,
                description: "Whether the entity is visible".to_string(),
            }],
        });
        registry.register(ComponentSchema {
            name: "RuntimeAgent".to_string(),
            agent_editable: true,
            runtime_visible: true,
            properties: vec![
                ComponentPropertySchema {
                    name: "profile_id".to_string(),
                    value_type: ComponentValueType::String,
                    writable: true,
                    description: "Runtime AI profile assigned to this entity".to_string(),
                },
                ComponentPropertySchema {
                    name: "control_mode".to_string(),
                    value_type: ComponentValueType::String,
                    writable: true,
                    description: "Manual, Assisted, Autonomous, or Disabled".to_string(),
                },
            ],
        });
        registry
    }

    pub fn register(&mut self, schema: ComponentSchema) {
        self.schemas.insert(schema.name.clone(), schema);
    }

    pub fn get(&self, name: &str) -> Option<&ComponentSchema> {
        self.schemas.get(name)
    }

    pub fn list(&self) -> Vec<&ComponentSchema> {
        self.schemas.values().collect()
    }

    pub fn validate_property(&self, component: &str, property: &str) -> bool {
        self.schemas
            .get(component)
            .map(|schema| schema.properties.iter().any(|p| p.name == property && p.writable))
            .unwrap_or(false)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BevyEditorCommand {
    CreateEntity {
        name: String,
        components: Vec<ComponentPatch>,
    },
    AddComponent {
        entity_id: EntityId,
        component: ComponentPatch,
    },
    RemoveComponent {
        entity_id: EntityId,
        component_type: String,
    },
    ModifyComponent {
        entity_id: EntityId,
        component_type: String,
        property: String,
        value: serde_json::Value,
    },
    SetParent {
        child: EntityId,
        parent: Option<EntityId>,
    },
    CreatePrefabFromEntity {
        entity_id: EntityId,
        prefab_id: PrefabId,
        prefab_name: String,
    },
    InstantiatePrefab {
        prefab_id: PrefabId,
        instance_id: PrefabInstanceId,
        transform: Option<[f32; 3]>,
    },
    ApplyInstanceOverride {
        instance_id: PrefabInstanceId,
        override_patch: ComponentOverride,
    },
    RevertInstanceOverride {
        instance_id: PrefabInstanceId,
        component_type: String,
        property: String,
    },
    SaveLevel {
        level_id: LevelId,
        path: String,
    },
    LoadLevel {
        path: String,
    },
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PrefabRegistry {
    prefabs: HashMap<PrefabId, PrefabDefinition>,
}

impl PrefabRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, prefab: PrefabDefinition) {
        self.prefabs.insert(prefab.id.clone(), prefab);
    }

    pub fn get(&self, id: &PrefabId) -> Option<&PrefabDefinition> {
        self.prefabs.get(id)
    }

    pub fn list(&self) -> Vec<&PrefabDefinition> {
        self.prefabs.values().collect()
    }

    pub fn remove(&mut self, id: &PrefabId) -> Option<PrefabDefinition> {
        self.prefabs.remove(id)
    }
}
