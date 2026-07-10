//! Game Skill Bridge
//!
//! Bridge between game skills and Multica skill system.
//! This module provides integration between WindWave's game skills
//! and the Multica agent platform's skill system.

use crate::error::{BridgeError, Result};
use crate::scene_context::{ComponentData, SharedSceneContext};
use log::info;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Type alias for skill handler function
pub type SkillHandler = Box<dyn Fn(&serde_json::Value) -> Result<serde_json::Value> + Send + Sync>;

/// Parameters for creating an entity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateEntityParams {
    pub name: String,
    pub position: Option<[f64; 3]>,
    pub components: Vec<ComponentData>,
}

/// Parameters for updating a component
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateComponentParams {
    pub entity_id: u64,
    pub component_type: String,
    pub properties: HashMap<String, serde_json::Value>,
}

/// Parameters for deleting an entity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteEntityParams {
    pub entity_id: u64,
}

/// Parameters for querying entities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryEntitiesParams {
    pub name_filter: Option<String>,
    pub component_type: Option<String>,
}

/// Result of a skill execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillExecutionResult {
    pub success: bool,
    pub message: String,
    pub data: Option<serde_json::Value>,
}

/// Game skill bridge
pub struct GameSkillBridge {
    scene_context: SharedSceneContext,
    registered_skills: HashMap<String, SkillHandler>,
}

impl GameSkillBridge {
    /// Create a new game skill bridge
    pub fn new(scene_context: SharedSceneContext) -> Self {
        let mut bridge = Self {
            scene_context,
            registered_skills: HashMap::new(),
        };

        // Register default game skills
        bridge.register_default_skills();

        bridge
    }

    /// Register default game skills
    fn register_default_skills(&mut self) {
        // Register create_entity skill
        self.register_skill(
            "create_entity",
            "Create a new entity in the scene",
            vec!["scene".to_string(), "entity".to_string()],
            {
                let ctx = self.scene_context.clone();
                move |params: &serde_json::Value| {
                    let params: CreateEntityParams = serde_json::from_value(params.clone())?;
                    let mut ctx = ctx.lock().expect("mutex poisoned");

                    let entity_id = ctx
                        .create_entity(&params.name, params.position, &params.components)
                        .map_err(BridgeError::Other)?;

                    Ok(serde_json::json!({
                        "entity_id": entity_id,
                        "name": params.name
                    }))
                }
            },
        );

        // Register update_component skill
        self.register_skill(
            "update_component",
            "Update an entity's component",
            vec![
                "scene".to_string(),
                "entity".to_string(),
                "component".to_string(),
            ],
            {
                let ctx = self.scene_context.clone();
                move |params: &serde_json::Value| {
                    let params: UpdateComponentParams = serde_json::from_value(params.clone())?;
                    let mut ctx = ctx.lock().expect("mutex poisoned");

                    ctx.update_component(
                        params.entity_id,
                        &params.component_type,
                        params.properties,
                    )
                    .map_err(BridgeError::Other)?;

                    Ok(serde_json::json!({
                        "entity_id": params.entity_id,
                        "component_type": params.component_type,
                        "success": true
                    }))
                }
            },
        );

        // Register delete_entity skill
        self.register_skill(
            "delete_entity",
            "Delete an entity from the scene",
            vec!["scene".to_string(), "entity".to_string()],
            {
                let ctx = self.scene_context.clone();
                move |params: &serde_json::Value| {
                    let params: DeleteEntityParams = serde_json::from_value(params.clone())?;
                    let mut ctx = ctx.lock().expect("mutex poisoned");

                    ctx.delete_entity(params.entity_id)
                        .map_err(BridgeError::Other)?;

                    Ok(serde_json::json!({
                        "entity_id": params.entity_id,
                        "success": true
                    }))
                }
            },
        );

        // Register query_entities skill
        self.register_skill(
            "query_entities",
            "Query entities in the scene",
            vec![
                "scene".to_string(),
                "entity".to_string(),
                "query".to_string(),
            ],
            {
                let ctx = self.scene_context.clone();
                move |params: &serde_json::Value| {
                    let params: QueryEntitiesParams = serde_json::from_value(params.clone())?;
                    let ctx = ctx.lock().expect("mutex poisoned");

                    let entities = ctx.query_entities(
                        params.name_filter.as_deref(),
                        params.component_type.as_deref(),
                    );

                    Ok(serde_json::json!({
                        "entities": entities,
                        "count": entities.len()
                    }))
                }
            },
        );

        info!("Registered default game skills");
    }

    /// Register a custom game skill
    pub fn register_skill<F>(
        &mut self,
        name: &str,
        _description: &str,
        _tags: Vec<String>,
        handler: F,
    ) where
        F: Fn(&serde_json::Value) -> Result<serde_json::Value> + Send + Sync + 'static,
    {
        info!("Registering game skill: {}", name);
        self.registered_skills
            .insert(name.to_string(), Box::new(handler));
    }

    /// Execute a game skill
    pub fn execute_skill(
        &self,
        skill_name: &str,
        params: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        info!("Executing game skill: {}", skill_name);

        let handler = self
            .registered_skills
            .get(skill_name)
            .ok_or_else(|| BridgeError::Other(format!("Game skill not found: {}", skill_name)))?;

        handler(params)
    }

    /// List all available game skills
    pub fn list_skills(&self) -> Vec<String> {
        self.registered_skills.keys().cloned().collect()
    }

    /// Get a reference to the scene context
    pub fn scene_context(&self) -> &SharedSceneContext {
        &self.scene_context
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene_context::create_shared_scene_context;

    #[test]
    fn test_create_entity_skill() {
        let ctx = create_shared_scene_context();
        let bridge = GameSkillBridge::new(ctx);

        let params = CreateEntityParams {
            name: "TestEntity".to_string(),
            position: None,
            components: vec![],
        };

        let result = bridge
            .execute_skill("create_entity", &serde_json::to_value(params).unwrap())
            .unwrap();

        assert!(result.get("entity_id").is_some());
    }

    #[test]
    fn test_query_entities_skill() {
        let ctx = create_shared_scene_context();
        let bridge = GameSkillBridge::new(ctx);

        // First create an entity
        let create_params = CreateEntityParams {
            name: "TestEntity".to_string(),
            position: None,
            components: vec![],
        };

        bridge
            .execute_skill(
                "create_entity",
                &serde_json::to_value(create_params).unwrap(),
            )
            .unwrap();

        // Then query it
        let query_params = QueryEntitiesParams {
            name_filter: None,
            component_type: None,
        };

        let result = bridge
            .execute_skill(
                "query_entities",
                &serde_json::to_value(query_params).unwrap(),
            )
            .unwrap();

        let count = result.get("count").and_then(|v| v.as_u64()).unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_list_skills() {
        let ctx = create_shared_scene_context();
        let bridge = GameSkillBridge::new(ctx);

        let skills = bridge.list_skills();
        assert!(skills.contains(&"create_entity".to_string()));
        assert!(skills.contains(&"update_component".to_string()));
        assert!(skills.contains(&"delete_entity".to_string()));
        assert!(skills.contains(&"query_entities".to_string()));
    }
}
