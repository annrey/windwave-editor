//! Built-in skill definitions — predefined DAG workflows for common editor operations.
//!
//! Design reference: Section 13.4 of gpt-agent-team-task-event-skill-architecture.md
//!
//! These skills are statically defined and registered into SkillRegistry at startup.
//! Each skill represents a reusable workflow that the Director can invoke.

use crate::registry::CapabilityKind;
use crate::skill::{
    SkillDefinition, SkillId, SkillNode, SkillEdge, SkillInput, SkillInputType,
    RetryPolicy, SkillEdgeCondition, SkillRegistry,
};

// ---------------------------------------------------------------------------
// Skill definitions
// ---------------------------------------------------------------------------

/// create_entity: query_scene → create_entity → verify_entity_exists
pub fn create_entity_skill() -> SkillDefinition {
    SkillDefinition {
        id: SkillId(1),
        name: "create_entity".to_string(),
        description: "Create a new entity in the scene with optional components".to_string(),
        inputs: vec![
            SkillInput {
                name: "entity_type".to_string(),
                description: "Type/category of entity to create".to_string(),
                input_type: SkillInputType::String,
                required: true,
                default: None,
            },
            SkillInput {
                name: "position".to_string(),
                description: "Initial position [x, y]".to_string(),
                input_type: SkillInputType::Json,
                required: false,
                default: Some(serde_json::json!([0.0, 0.0])),
            },
        ],
        nodes: vec![
            SkillNode {
                id: "query_scene".to_string(),
                title: "Query current scene state".to_string(),
                required_capability: CapabilityKind::SceneRead,
                tool_name: Some("query_entities".to_string()),
                input_mapping: serde_json::json!({ "entity_type": "${entity_type}" }),
                retry: RetryPolicy::default(),
                rollback: None,
            },
            SkillNode {
                id: "create_entity".to_string(),
                title: "Create the entity".to_string(),
                required_capability: CapabilityKind::SceneWrite,
                tool_name: Some("create_entity".to_string()),
                input_mapping: serde_json::json!({
                    "entity_type": "${entity_type}",
                    "entity_name": "${entity_type}_${counter}"
                }),
                retry: RetryPolicy { max_retries: 3, ..RetryPolicy::default() },
                rollback: Some("delete_entity".to_string()),
            },
            SkillNode {
                id: "verify_entity_exists".to_string(),
                title: "Verify entity was created".to_string(),
                required_capability: CapabilityKind::RuleCheck,
                tool_name: None,
                input_mapping: serde_json::json!({}),
                retry: RetryPolicy { max_retries: 1, ..RetryPolicy::default() },
                rollback: None,
            },
        ],
        edges: vec![
            SkillEdge {
                from: "query_scene".to_string(),
                to: "create_entity".to_string(),
                condition: SkillEdgeCondition::Always,
            },
            SkillEdge {
                from: "create_entity".to_string(),
                to: "verify_entity_exists".to_string(),
                condition: SkillEdgeCondition::OnSuccess,
            },
        ],
    }
}

/// modify_entity_transform: resolve_entity → update_transform → verify_transform
pub fn modify_entity_transform_skill() -> SkillDefinition {
    SkillDefinition {
        id: SkillId(2),
        name: "modify_entity_transform".to_string(),
        description: "Change an entity's position, rotation, or scale".to_string(),
        inputs: vec![
            SkillInput {
                name: "entity_name".to_string(),
                description: "Name of entity to modify".to_string(),
                input_type: SkillInputType::String,
                required: true,
                default: None,
            },
            SkillInput {
                name: "new_position".to_string(),
                description: "New position [x, y]".to_string(),
                input_type: SkillInputType::Json,
                required: true,
                default: None,
            },
        ],
        nodes: vec![
            SkillNode {
                id: "resolve_entity".to_string(),
                title: "Resolve entity by name".to_string(),
                required_capability: CapabilityKind::SceneRead,
                tool_name: Some("query_entities".to_string()),
                input_mapping: serde_json::json!({ "filter": "${entity_name}" }),
                retry: RetryPolicy::default(),
                rollback: None,
            },
            SkillNode {
                id: "update_transform".to_string(),
                title: "Update transform component".to_string(),
                required_capability: CapabilityKind::SceneWrite,
                tool_name: Some("update_component".to_string()),
                input_mapping: serde_json::json!({
                    "component": "Transform",
                    "properties": { "position": "${new_position}" }
                }),
                retry: RetryPolicy { max_retries: 2, ..RetryPolicy::default() },
                rollback: Some("update_component".to_string()),
            },
            SkillNode {
                id: "verify_transform".to_string(),
                title: "Verify transform was applied".to_string(),
                required_capability: CapabilityKind::RuleCheck,
                tool_name: None,
                input_mapping: serde_json::json!({
                    "expected_position": "${new_position}"
                }),
                retry: RetryPolicy { max_retries: 1, ..RetryPolicy::default() },
                rollback: None,
            },
        ],
        edges: vec![
            SkillEdge {
                from: "resolve_entity".to_string(),
                to: "update_transform".to_string(),
                condition: SkillEdgeCondition::OnSuccess,
            },
            SkillEdge {
                from: "update_transform".to_string(),
                to: "verify_transform".to_string(),
                condition: SkillEdgeCondition::Always,
            },
        ],
    }
}

/// query_scene: simple scene inspection (single-node skill)
pub fn query_scene_skill() -> SkillDefinition {
    SkillDefinition {
        id: SkillId(3),
        name: "query_scene".to_string(),
        description: "List all entities in the current scene".to_string(),
        inputs: vec![
            SkillInput {
                name: "filter".to_string(),
                description: "Optional name filter".to_string(),
                input_type: SkillInputType::String,
                required: false,
                default: None,
            },
        ],
        nodes: vec![
            SkillNode {
                id: "list_entities".to_string(),
                title: "List all scene entities".to_string(),
                required_capability: CapabilityKind::SceneRead,
                tool_name: Some("query_entities".to_string()),
                input_mapping: serde_json::json!({ "entity_type": "${filter}" }),
                retry: RetryPolicy::default(),
                rollback: None,
            },
        ],
        edges: vec![],
    }
}

/// import_asset: locate_file → validate_format → register_asset
pub fn import_asset_skill() -> SkillDefinition {
    SkillDefinition {
        id: SkillId(4),
        name: "import_asset".to_string(),
        description: "Import a texture, model, or audio file into the project".to_string(),
        inputs: vec![
            SkillInput {
                name: "file_path".to_string(),
                description: "Path to the asset file".to_string(),
                input_type: SkillInputType::String,
                required: true,
                default: None,
            },
        ],
        nodes: vec![
            SkillNode {
                id: "locate_file".to_string(),
                title: "Locate and validate file path".to_string(),
                required_capability: CapabilityKind::CodeRead,
                tool_name: Some("find_file".to_string()),
                input_mapping: serde_json::json!({ "path": "${file_path}" }),
                retry: RetryPolicy::default(),
                rollback: None,
            },
            SkillNode {
                id: "validate_format".to_string(),
                title: "Validate file format".to_string(),
                required_capability: CapabilityKind::AssetManage,
                tool_name: None,
                input_mapping: serde_json::json!({ "path": "${file_path}" }),
                retry: RetryPolicy { max_retries: 1, ..RetryPolicy::default() },
                rollback: None,
            },
            SkillNode {
                id: "register_asset".to_string(),
                title: "Register asset in project index".to_string(),
                required_capability: CapabilityKind::AssetManage,
                tool_name: None,
                input_mapping: serde_json::json!({ "path": "${file_path}" }),
                retry: RetryPolicy::default(),
                rollback: Some("unregister_asset".to_string()),
            },
        ],
        edges: vec![
            SkillEdge {
                from: "locate_file".to_string(),
                to: "validate_format".to_string(),
                condition: SkillEdgeCondition::OnSuccess,
            },
            SkillEdge {
                from: "validate_format".to_string(),
                to: "register_asset".to_string(),
                condition: SkillEdgeCondition::OnSuccess,
            },
        ],
    }
}

// ---------------------------------------------------------------------------
// Registration
// ---------------------------------------------------------------------------

/// Register all built-in skills into a SkillRegistry.
pub fn register_builtin_skills(registry: &mut SkillRegistry) {
    registry.register(create_entity_skill());
    registry.register(modify_entity_transform_skill());
    registry.register(query_scene_skill());
    registry.register(import_asset_skill());
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skill::SkillExecutor;

    #[test]
    fn test_create_entity_skill_is_valid() {
        let skill = create_entity_skill();
        let executor = SkillExecutor::new();
        assert!(executor.validate(&skill).is_ok());
    }

    #[test]
    fn test_modify_transform_skill_is_valid() {
        let skill = modify_entity_transform_skill();
        let executor = SkillExecutor::new();
        assert!(executor.validate(&skill).is_ok());
    }

    #[test]
    fn test_query_scene_skill_is_valid() {
        let skill = query_scene_skill();
        let executor = SkillExecutor::new();
        assert!(executor.validate(&skill).is_ok());
    }

    #[test]
    fn test_import_asset_skill_is_valid() {
        let skill = import_asset_skill();
        let executor = SkillExecutor::new();
        assert!(executor.validate(&skill).is_ok());
    }

    #[test]
    fn test_register_all_builtin() {
        let mut registry = SkillRegistry::new();
        register_builtin_skills(&mut registry);
        assert_eq!(registry.len(), 4);
    }

    #[test]
    fn test_create_entity_topological_order() {
        let skill = create_entity_skill();
        let executor = SkillExecutor::new();
        let order = executor.build_execution_order(&skill).unwrap();
        assert!(order.iter().position(|n| n == "query_scene").unwrap()
            < order.iter().position(|n| n == "create_entity").unwrap());
        assert!(order.iter().position(|n| n == "create_entity").unwrap()
            < order.iter().position(|n| n == "verify_entity_exists").unwrap());
    }

    #[test]
    fn test_builtin_dry_run() {
        let mut registry = SkillRegistry::new();
        register_builtin_skills(&mut registry);

        let names = registry.list_skill_names();
        assert!(names.contains(&"create_entity"));
        assert!(names.contains(&"modify_entity_transform"));
        assert!(names.contains(&"query_scene"));
        assert!(names.contains(&"import_asset"));
    }
}
