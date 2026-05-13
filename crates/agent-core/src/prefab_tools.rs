//! Prefab Tools - Phase 4.2
//!
//! Tools for creating and instantiating prefabs.

use crate::bevy_editor_model::{
    ComponentPatch, PrefabDefinition, PrefabId, PrefabNode, PrefabRegistry,
};
use crate::scene_bridge::{ComponentProperty, EntityListItem};
use crate::tool::{Tool, ToolContext, ToolError, ToolResult};
use std::collections::HashMap;

/// Tool to create a prefab from an existing entity
pub struct CreatePrefabTool;

impl Tool for CreatePrefabTool {
    fn name(&self) -> &str {
        "create_prefab"
    }

    fn description(&self) -> &str {
        "Create a prefab from a selected entity"
    }

    fn execute(&self, params: &HashMap<String, serde_json::Value>, ctx: &ToolContext) -> ToolResult {
        // Get entity ID
        let entity_id = params
            .get("entity_id")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| ToolError::InvalidParameters("Missing entity_id".to_string()))?;

        // Get prefab name
        let name = params
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidParameters("Missing name".to_string()))?;

        // Query entity info
        let entity_info = ctx
            .scene_bridge
            .query_entities(Some(&format!("id={}", entity_id)))
            .into_iter()
            .next()
            .ok_or_else(|| ToolError::ExecutionFailed(format!("Entity {} not found", entity_id)))?;

        // Create prefab node from entity
        let root_node = create_prefab_node_from_entity(&entity_info, ctx)?;

        // Generate prefab ID
        let prefab_id = format!("prefab_{}_{}", name.to_lowercase().replace(" ", "_"), entity_id);

        // Create prefab definition
        let prefab = PrefabDefinition::new(&prefab_id, name, root_node);

        // Store prefab in registry
        if let Some(registry) = ctx.prefab_registry {
            registry.register(prefab.clone());
        }

        Ok(serde_json::json!({
            "prefab_id": prefab_id,
            "name": name,
            "component_count": count_components(&prefab.root),
        }))
    }
}

/// Tool to instantiate a prefab into the scene
pub struct InstantiatePrefabTool;

impl Tool for InstantiatePrefabTool {
    fn name(&self) -> &str {
        "instantiate_prefab"
    }

    fn description(&self) -> &str {
        "Instantiate a prefab into the scene"
    }

    fn execute(&self, params: &HashMap<String, serde_json::Value>, ctx: &ToolContext) -> ToolResult {
        // Get prefab ID
        let prefab_id = params
            .get("prefab_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidParameters("Missing prefab_id".to_string()))?;

        // Get optional position
        let position = params.get("position").and_then(|v| {
            if let Some(arr) = v.as_array() {
                if arr.len() >= 3 {
                    return Some((
                        arr[0].as_f64()?,
                        arr[1].as_f64()?,
                        arr[2].as_f64()?,
                    ));
                }
            }
            None
        });

        // Look up prefab
        let prefab = ctx
            .prefab_registry
            .and_then(|r| r.get(&PrefabId(prefab_id.to_string())))
            .cloned()
            .ok_or_else(|| ToolError::ExecutionFailed(format!("Prefab {} not found", prefab_id)))?;

        // Generate instance ID
        let instance_id = format!("{}_instance_{}", prefab_id, ctx.generate_id());

        // Create entity from prefab
        let entity_id = instantiate_prefab_node(&prefab.root, &instance_id, position, ctx)?;

        Ok(serde_json::json!({
            "entity_id": entity_id,
            "prefab_id": prefab_id,
            "instance_id": instance_id,
            "name": prefab.root.name,
        }))
    }
}

/// Tool to list all available prefabs
pub struct ListPrefabsTool;

impl Tool for ListPrefabsTool {
    fn name(&self) -> &str {
        "list_prefabs"
    }

    fn description(&self) -> &str {
        "List all registered prefabs"
    }

    fn execute(&self, _params: &HashMap<String, serde_json::Value>, ctx: &ToolContext) -> ToolResult {
        let prefabs = ctx
            .prefab_registry
            .map(|r| {
                r.list()
                    .into_iter()
                    .map(|p| {
                        serde_json::json!({
                            "id": p.id.0,
                            "name": p.name,
                            "component_count": count_components(&p.root),
                        })
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        Ok(serde_json::json!({ "prefabs": prefabs, "count": prefabs.len() }))
    }
}

/// Helper: Create prefab node from entity info
fn create_prefab_node_from_entity(
    entity: &EntityListItem,
    ctx: &ToolContext,
) -> Result<PrefabNode, ToolError> {
    let mut node = PrefabNode::new(&entity.name);

    // Get entity details to extract components
    if let Some(details) = ctx.scene_bridge.get_entity_info(entity.id) {
        for (component_type, properties) in &details.components {
            let mut patch = ComponentPatch {
                component_type: component_type.clone(),
                properties: HashMap::new(),
            };

            for (prop_name, value) in properties {
                patch.properties.insert(
                    prop_name.clone(),
                    ComponentProperty::Value(value.clone()),
                );
            }

            node.components.push(patch);
        }
    }

    // Recursively process children (if scene bridge supports hierarchy)
    // For now, we assume flat structure

    Ok(node)
}

/// Helper: Instantiate prefab node as entity
fn instantiate_prefab_node(
    node: &PrefabNode,
    instance_id: &str,
    position: Option<(f64, f64, f64)>,
    ctx: &ToolContext,
) -> Result<u64, ToolError> {
    // Build component patches
    let mut components = node.components.clone();

    // Add/override Transform if position provided
    if let Some((x, y, z)) = position {
        let transform_patch = ComponentPatch {
            component_type: "Transform".to_string(),
            properties: {
                let mut props = HashMap::new();
                props.insert(
                    "translation".to_string(),
                    ComponentProperty::Array(vec![
                        ComponentProperty::Value(serde_json::json!(x)),
                        ComponentProperty::Value(serde_json::json!(y)),
                        ComponentProperty::Value(serde_json::json!(z)),
                    ]),
                );
                props
            },
        };

        // Replace existing Transform or add new
        if let Some(idx) = components.iter().position(|c| c.component_type == "Transform") {
            components[idx] = transform_patch;
        } else {
            components.push(transform_patch);
        }
    }

    // Create entity via scene bridge
    let entity_id = ctx
        .scene_bridge
        .create_entity(&format!("{}_{}", node.name, instance_id), &components)
        .map_err(|e| ToolError::ExecutionFailed(format!("Failed to create entity: {}", e)))?;

    // Recursively create children
    for child in &node.children {
        let _child_id = instantiate_prefab_node(
            child,
            &format!("{}_child", instance_id),
            None,
            ctx,
        )?;
        // TODO: Set parent relationship
    }

    Ok(entity_id)
}

/// Helper: Count total components in prefab (including children)
fn count_components(node: &PrefabNode) -> usize {
    let mut count = node.components.len();
    for child in &node.children {
        count += count_components(child);
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_count_components() {
        let mut node = PrefabNode::new("Root");
        node.components.push(ComponentPatch {
            component_type: "Transform".to_string(),
            properties: HashMap::new(),
        });

        let child = PrefabNode::new("Child");
        node.children.push(child);

        assert_eq!(count_components(&node), 1);
    }
}
