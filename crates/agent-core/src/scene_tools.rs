//! Scene Tools - Entity and component manipulation tools
//!
//! Provides tools for the Agent to interact with the game engine scene:
//! - Query entities and components
//! - Create/modify/delete entities
//! - Update component properties
//!
//! All tools hold a `SharedSceneBridge` for real engine access.
//! When no bridge is connected, tools return an error instead of mock data.

use crate::scene_bridge::{SharedSceneBridge, ComponentPatch};
use crate::tool::{Tool, ToolCategory, ToolParameter, ToolResult, ToolError, ParameterType};
use serde_json::Value;
use std::collections::HashMap;

fn no_bridge_error() -> ToolError {
    ToolError::ExecutionFailed("No SceneBridge connected — scene tools require a live bridge".into())
}

/// Query entities in the scene
pub struct QueryEntitiesTool {
    bridge: SharedSceneBridge,
}

impl QueryEntitiesTool {
    pub fn new(bridge: SharedSceneBridge) -> Self {
        Self { bridge }
    }
}

impl Tool for QueryEntitiesTool {
    fn name(&self) -> &str {
        "query_entities"
    }

    fn description(&self) -> &str {
        "Query entities in the scene by name, type, or component"
    }

    fn parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter {
                name: "filter".to_string(),
                description: "Filter by name pattern (optional)".to_string(),
                param_type: ParameterType::String,
                required: false,
                default: Some(Value::String("*".to_string())),
            },
            ToolParameter {
                name: "with_component".to_string(),
                description: "Only return entities with this component type".to_string(),
                param_type: ParameterType::String,
                required: false,
                default: None,
            },
        ]
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Scene
    }

    fn execute(&self, params: HashMap<String, Value>) -> Result<ToolResult, ToolError> {
        let filter = params.get("filter")
            .and_then(|v| v.as_str())
            .unwrap_or("*");

        let with_component = params.get("with_component")
            .and_then(|v| v.as_str());

        let bridge = self.bridge.lock().map_err(|e| ToolError::ExecutionFailed(format!("Bridge lock failed: {}", e)))?;

        match bridge.as_ref() {
            Some(b) => {
                let filter_opt = if filter == "*" { None } else { Some(filter) };
                let results = b.query_entities(filter_opt, with_component);

                Ok(ToolResult {
                    success: true,
                    message: format!("Found {} entities matching '{}'", results.len(), filter),
                    data: Some(serde_json::to_value(results).unwrap_or(Value::Null)),
                    execution_time_ms: 0,
                })
            }
            None => Err(no_bridge_error()),
        }
    }
}

/// Get entity details
pub struct GetEntityTool {
    bridge: SharedSceneBridge,
}

impl GetEntityTool {
    pub fn new(bridge: SharedSceneBridge) -> Self {
        Self { bridge }
    }
}

impl Tool for GetEntityTool {
    fn name(&self) -> &str {
        "get_entity"
    }

    fn description(&self) -> &str {
        "Get detailed information about a specific entity"
    }

    fn parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter {
                name: "entity_id".to_string(),
                description: "Entity ID to query".to_string(),
                param_type: ParameterType::EntityId,
                required: true,
                default: None,
            },
        ]
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Scene
    }

    fn execute(&self, params: HashMap<String, Value>) -> Result<ToolResult, ToolError> {
        let entity_id = params.get("entity_id")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| ToolError::MissingParameter("entity_id".to_string()))?;

        let bridge = self.bridge.lock().map_err(|e| ToolError::ExecutionFailed(format!("Bridge lock failed: {}", e)))?;

        match bridge.as_ref() {
            Some(b) => {
                match b.get_entity(entity_id) {
                    Some(info) => Ok(ToolResult {
                        success: true,
                        message: format!("Entity {} details retrieved", entity_id),
                        data: Some(info),
                        execution_time_ms: 0,
                    }),
                    None => Err(ToolError::ExecutionFailed(format!("Entity {} not found", entity_id))),
                }
            }
            None => Err(no_bridge_error()),
        }
    }
}

/// Create a new entity
pub struct CreateEntityTool {
    bridge: SharedSceneBridge,
}

impl CreateEntityTool {
    pub fn new(bridge: SharedSceneBridge) -> Self {
        Self { bridge }
    }
}

impl Tool for CreateEntityTool {
    fn name(&self) -> &str {
        "create_entity"
    }

    fn description(&self) -> &str {
        "Create a new entity in the scene"
    }

    fn parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter {
                name: "name".to_string(),
                description: "Name for the new entity".to_string(),
                param_type: ParameterType::String,
                required: true,
                default: None,
            },
            ToolParameter {
                name: "position".to_string(),
                description: "Initial position as [x, y]".to_string(),
                param_type: ParameterType::Vec2,
                required: false,
                default: Some(Value::Array(vec![
                    Value::Number(0.into()),
                    Value::Number(0.into())
                ])),
            },
            ToolParameter {
                name: "components".to_string(),
                description: "Component types to add (e.g., [Sprite, RigidBody])".to_string(),
                param_type: ParameterType::Array(Box::new(ParameterType::String)),
                required: false,
                default: Some(Value::Array(vec![])),
            },
        ]
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Scene
    }

    fn execute(&self, params: HashMap<String, Value>) -> Result<ToolResult, ToolError> {
        let name = params.get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::MissingParameter("name".to_string()))?;

        let position = params.get("position")
            .and_then(|v| v.as_array())
            .map(|arr| {
                let x = arr.first().and_then(|v| v.as_f64()).unwrap_or(0.0);
                let y = arr.get(1).and_then(|v| v.as_f64()).unwrap_or(0.0);
                [x, y]
            });

        let component_names: Vec<String> = params.get("components")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
            .unwrap_or_default();

        let patches: Vec<ComponentPatch> = component_names.iter().map(|cn| {
            ComponentPatch {
                type_name: cn.clone(),
                properties: HashMap::new(),
            }
        }).collect();

        let mut bridge = self.bridge.lock().map_err(|e| ToolError::ExecutionFailed(format!("Bridge lock failed: {}", e)))?;

        match bridge.as_mut() {
            Some(b) => {
                match b.create_entity(name, position, &patches) {
                    Ok(new_id) => Ok(ToolResult {
                        success: true,
                        message: format!("Created entity '{}' with ID {}", name, new_id),
                        data: Some(serde_json::json!({
                            "id": new_id,
                            "name": name,
                            "position": position.map(|[x, y]| serde_json::json!({"x": x, "y": y})),
                            "components": component_names
                        })),
                        execution_time_ms: 0,
                    }),
                    Err(e) => Err(ToolError::ExecutionFailed(format!("Failed to create entity: {}", e))),
                }
            }
            None => Err(no_bridge_error()),
        }
    }
}

/// Update component property
pub struct UpdateComponentTool {
    bridge: SharedSceneBridge,
}

impl UpdateComponentTool {
    pub fn new(bridge: SharedSceneBridge) -> Self {
        Self { bridge }
    }
}

impl Tool for UpdateComponentTool {
    fn name(&self) -> &str {
        "update_component"
    }

    fn description(&self) -> &str {
        "Update a property on an entity's component"
    }

    fn parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter {
                name: "entity_id".to_string(),
                description: "Entity ID".to_string(),
                param_type: ParameterType::EntityId,
                required: true,
                default: None,
            },
            ToolParameter {
                name: "component".to_string(),
                description: "Component type name".to_string(),
                param_type: ParameterType::String,
                required: true,
                default: None,
            },
            ToolParameter {
                name: "property".to_string(),
                description: "Property name to update".to_string(),
                param_type: ParameterType::String,
                required: true,
                default: None,
            },
            ToolParameter {
                name: "value".to_string(),
                description: "New value".to_string(),
                param_type: ParameterType::String,
                required: true,
                default: None,
            },
        ]
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Scene
    }

    fn execute(&self, params: HashMap<String, Value>) -> Result<ToolResult, ToolError> {
        let entity_id = params.get("entity_id")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| ToolError::MissingParameter("entity_id".to_string()))?;

        let component = params.get("component")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::MissingParameter("component".to_string()))?;

        let property = params.get("property")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::MissingParameter("property".to_string()))?;

        let value = params.get("value")
            .ok_or_else(|| ToolError::MissingParameter("value".to_string()))?;

        let mut properties = HashMap::new();
        properties.insert(property.to_string(), value.clone());

        let mut bridge = self.bridge.lock().map_err(|e| ToolError::ExecutionFailed(format!("Bridge lock failed: {}", e)))?;

        match bridge.as_mut() {
            Some(b) => {
                match b.update_component(entity_id, component, properties) {
                    Ok(()) => Ok(ToolResult {
                        success: true,
                        message: format!("Updated {}.{} on entity {}", component, property, entity_id),
                        data: Some(serde_json::json!({
                            "entity_id": entity_id,
                            "component": component,
                            "property": property,
                            "value": value
                        })),
                        execution_time_ms: 0,
                    }),
                    Err(e) => Err(ToolError::ExecutionFailed(format!("Failed to update component: {}", e))),
                }
            }
            None => Err(no_bridge_error()),
        }
    }
}

/// Delete an entity
pub struct DeleteEntityTool {
    bridge: SharedSceneBridge,
}

impl DeleteEntityTool {
    pub fn new(bridge: SharedSceneBridge) -> Self {
        Self { bridge }
    }
}

impl Tool for DeleteEntityTool {
    fn name(&self) -> &str {
        "delete_entity"
    }

    fn description(&self) -> &str {
        "Delete an entity from the scene"
    }

    fn parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter {
                name: "entity_id".to_string(),
                description: "Entity ID to delete".to_string(),
                param_type: ParameterType::EntityId,
                required: true,
                default: None,
            },
            ToolParameter {
                name: "confirm".to_string(),
                description: "Confirm deletion (safety check)".to_string(),
                param_type: ParameterType::Boolean,
                required: true,
                default: Some(Value::Bool(false)),
            },
        ]
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Scene
    }

    fn execute(&self, params: HashMap<String, Value>) -> Result<ToolResult, ToolError> {
        let entity_id = params.get("entity_id")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| ToolError::MissingParameter("entity_id".to_string()))?;

        let confirm = params.get("confirm")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        if !confirm {
            return Err(ToolError::ExecutionFailed(
                "Deletion not confirmed. Set confirm=true to delete.".to_string()
            ));
        }

        let mut bridge = self.bridge.lock().map_err(|e| ToolError::ExecutionFailed(format!("Bridge lock failed: {}", e)))?;

        match bridge.as_mut() {
            Some(b) => {
                match b.delete_entity(entity_id) {
                    Ok(()) => Ok(ToolResult {
                        success: true,
                        message: format!("Deleted entity {}", entity_id),
                        data: None,
                        execution_time_ms: 0,
                    }),
                    Err(e) => Err(ToolError::ExecutionFailed(format!("Failed to delete entity: {}", e))),
                }
            }
            None => Err(no_bridge_error()),
        }
    }
}

/// Register all scene tools with a shared SceneBridge
pub fn register_scene_tools(registry: &mut crate::tool::ToolRegistry, bridge: SharedSceneBridge) {
    registry.register(QueryEntitiesTool::new(bridge.clone()));
    registry.register(GetEntityTool::new(bridge.clone()));
    registry.register(CreateEntityTool::new(bridge.clone()));
    registry.register(UpdateComponentTool::new(bridge.clone()));
    registry.register(DeleteEntityTool::new(bridge));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene_bridge::{MockSceneBridge, create_shared_bridge, create_empty_shared_bridge, SceneBridge};

    #[test]
    fn test_query_entities_tool_with_bridge() {
        let mut mock = MockSceneBridge::new();
        mock.create_entity("Player", None, &[]).ok();
        mock.create_entity("Enemy", None, &[]).ok();

        let bridge = create_shared_bridge(Box::new(mock));
        let tool = QueryEntitiesTool::new(bridge);
        let mut params = HashMap::new();
        params.insert("filter".to_string(), Value::String("*".to_string()));

        let result = tool.execute(params).unwrap();
        assert!(result.success);
    }

    #[test]
    fn test_query_entities_tool_no_bridge() {
        let bridge = create_empty_shared_bridge();
        let tool = QueryEntitiesTool::new(bridge);
        let params = HashMap::new();

        let result = tool.execute(params);
        assert!(result.is_err());
    }

    #[test]
    fn test_create_entity_tool_with_bridge() {
        let mock = MockSceneBridge::new();
        let bridge = create_shared_bridge(Box::new(mock));
        let tool = CreateEntityTool::new(bridge);
        let mut params = HashMap::new();
        params.insert("name".to_string(), Value::String("TestEntity".to_string()));

        let result = tool.execute(params).unwrap();
        assert!(result.success);
        assert!(result.message.contains("TestEntity"));
    }

    #[test]
    fn test_create_entity_tool_no_bridge() {
        let bridge = create_empty_shared_bridge();
        let tool = CreateEntityTool::new(bridge);
        let mut params = HashMap::new();
        params.insert("name".to_string(), Value::String("TestEntity".to_string()));

        let result = tool.execute(params);
        assert!(result.is_err());
    }

    #[test]
    fn test_delete_entity_tool_requires_confirm() {
        let mock = MockSceneBridge::new();
        let bridge = create_shared_bridge(Box::new(mock));
        let tool = DeleteEntityTool::new(bridge);
        let mut params = HashMap::new();
        params.insert("entity_id".to_string(), Value::Number(1.into()));
        params.insert("confirm".to_string(), Value::Bool(false));

        let result = tool.execute(params);
        assert!(result.is_err());
    }

    #[test]
    fn test_get_entity_tool_not_found() {
        let mock = MockSceneBridge::new();
        let bridge = create_shared_bridge(Box::new(mock));
        let tool = GetEntityTool::new(bridge);
        let mut params = HashMap::new();
        params.insert("entity_id".to_string(), Value::Number(999.into()));

        let result = tool.execute(params);
        assert!(result.is_err());
    }

    #[test]
    fn test_register_scene_tools() {
        let mock = MockSceneBridge::new();
        let bridge = create_shared_bridge(Box::new(mock));
        let mut registry = crate::tool::ToolRegistry::new();
        register_scene_tools(&mut registry, bridge);

        assert!(registry.has("query_entities"));
        assert!(registry.has("get_entity"));
        assert!(registry.has("create_entity"));
        assert!(registry.has("update_component"));
        assert!(registry.has("delete_entity"));
    }
}
