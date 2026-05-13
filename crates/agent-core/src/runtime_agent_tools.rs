//! Runtime Agent Tools - Tools for controlling runtime AI entities
//!
//! Provides tools for the Agent to interact with runtime AI entities:
//! - Attach/detach runtime agents
//! - Set control modes
//! - Set goals and blackboard values
//! - Send events and execute actions

use crate::types::EntityId;
use crate::runtime_agent::*;
use crate::tool::{Tool, ToolCategory, ToolParameter, ToolResult, ToolError, ParameterType};
use serde_json::Value;
use std::collections::HashMap;

/// Tool to attach a runtime agent to an entity
pub struct AttachRuntimeAgentTool;

impl Tool for AttachRuntimeAgentTool {
    fn name(&self) -> &str {
        "attach_runtime_agent"
    }

    fn description(&self) -> &str {
        "Attach a runtime AI agent to a game entity"
    }

    fn parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter {
                name: "entity_id".to_string(),
                description: "Entity ID to attach agent to".to_string(),
                param_type: ParameterType::EntityId,
                required: true,
                default: None,
            },
            ToolParameter {
                name: "profile_id".to_string(),
                description: "Runtime agent profile ID".to_string(),
                param_type: ParameterType::String,
                required: true,
                default: Some(Value::String("default".to_string())),
            },
            ToolParameter {
                name: "control_mode".to_string(),
                description: "Control mode: Manual, Assisted, Autonomous".to_string(),
                param_type: ParameterType::String,
                required: false,
                default: Some(Value::String("Autonomous".to_string())),
            },
        ]
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Scene
    }

    fn execute(&self, params: HashMap<String, Value>) -> Result<ToolResult, ToolError> {
        let entity_id = params.get("entity_id")
            .and_then(|v| v.as_u64())
            .map(EntityId)
            .ok_or_else(|| ToolError::MissingParameter("entity_id".to_string()))?;

        let profile_id = params.get("profile_id")
            .and_then(|v| v.as_str())
            .unwrap_or("default");

        let control_mode = params.get("control_mode")
            .and_then(|v| v.as_str())
            .unwrap_or("Autonomous");

        let _mode = match control_mode {
            "Manual" => RuntimeAgentControlMode::Manual,
            "Assisted" => RuntimeAgentControlMode::Assisted,
            "Autonomous" => RuntimeAgentControlMode::Autonomous,
            _ => RuntimeAgentControlMode::Autonomous,
        };

        Ok(ToolResult {
            success: true,
            message: format!("Runtime agent attached to entity {:?} with profile '{}' and mode '{}'", entity_id, profile_id, control_mode),
            data: Some(serde_json::json!({
                "entity_id": entity_id.0,
                "profile_id": profile_id,
                "control_mode": control_mode,
                "command": "AttachRuntimeAgent",
            })),
            execution_time_ms: 0,
        })
    }
}

/// Tool to set runtime agent control mode
pub struct SetAgentControlModeTool;

impl Tool for SetAgentControlModeTool {
    fn name(&self) -> &str {
        "set_agent_control_mode"
    }

    fn description(&self) -> &str {
        "Set the control mode of a runtime AI agent"
    }

    fn parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter {
                name: "entity_id".to_string(),
                description: "Entity ID of the agent".to_string(),
                param_type: ParameterType::EntityId,
                required: true,
                default: None,
            },
            ToolParameter {
                name: "mode".to_string(),
                description: "Control mode: Disabled, Manual, Assisted, Autonomous, EditorControlled".to_string(),
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
            .map(EntityId)
            .ok_or_else(|| ToolError::MissingParameter("entity_id".to_string()))?;

        let mode_str = params.get("mode")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::MissingParameter("mode".to_string()))?;

        let _mode = match mode_str {
            "Disabled" => RuntimeAgentControlMode::Disabled,
            "Manual" => RuntimeAgentControlMode::Manual,
            "Assisted" => RuntimeAgentControlMode::Assisted,
            "Autonomous" => RuntimeAgentControlMode::Autonomous,
            "EditorControlled" => RuntimeAgentControlMode::EditorControlled { controller: crate::registry::AgentId(0) },
            _ => return Err(ToolError::InvalidParameter(format!("Unknown control mode: {}", mode_str))),
        };

        Ok(ToolResult {
            success: true,
            message: format!("Control mode set to '{}' for entity {:?}", mode_str, entity_id),
            data: Some(serde_json::json!({
                "entity_id": entity_id.0,
                "mode": mode_str,
                "command": "SetControlMode",
            })),
            execution_time_ms: 0,
        })
    }
}

/// Tool to set a runtime agent's goal
pub struct SetAgentGoalTool;

impl Tool for SetAgentGoalTool {
    fn name(&self) -> &str {
        "set_agent_goal"
    }

    fn description(&self) -> &str {
        "Set a goal for a runtime AI agent"
    }

    fn parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter {
                name: "entity_id".to_string(),
                description: "Entity ID of the agent".to_string(),
                param_type: ParameterType::EntityId,
                required: true,
                default: None,
            },
            ToolParameter {
                name: "goal_description".to_string(),
                description: "Description of the goal".to_string(),
                param_type: ParameterType::String,
                required: true,
                default: None,
            },
            ToolParameter {
                name: "priority".to_string(),
                description: "Goal priority (0.0 - 1.0)".to_string(),
                param_type: ParameterType::Number,
                required: false,
                default: Some(Value::Number(serde_json::Number::from_f64(0.5).unwrap())),
            },
        ]
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Scene
    }

    fn execute(&self, params: HashMap<String, Value>) -> Result<ToolResult, ToolError> {
        let entity_id = params.get("entity_id")
            .and_then(|v| v.as_u64())
            .map(EntityId)
            .ok_or_else(|| ToolError::MissingParameter("entity_id".to_string()))?;

        let description = params.get("goal_description")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::MissingParameter("goal_description".to_string()))?;

        let priority = params.get("priority")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.5) as f32;

        Ok(ToolResult {
            success: true,
            message: format!("Goal set for entity {:?}: '{}' (priority: {})", entity_id, description, priority),
            data: Some(serde_json::json!({
                "entity_id": entity_id.0,
                "goal": description,
                "priority": priority,
                "command": "SetRuntimeGoal",
            })),
            execution_time_ms: 0,
        })
    }
}

/// Tool to set a blackboard value
pub struct SetAgentBlackboardTool;

impl Tool for SetAgentBlackboardTool {
    fn name(&self) -> &str {
        "set_agent_blackboard"
    }

    fn description(&self) -> &str {
        "Set a value in a runtime agent's blackboard"
    }

    fn parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter {
                name: "entity_id".to_string(),
                description: "Entity ID of the agent".to_string(),
                param_type: ParameterType::EntityId,
                required: true,
                default: None,
            },
            ToolParameter {
                name: "key".to_string(),
                description: "Blackboard key".to_string(),
                param_type: ParameterType::String,
                required: true,
                default: None,
            },
            ToolParameter {
                name: "value".to_string(),
                description: "Value to set (any JSON)".to_string(),
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
            .map(EntityId)
            .ok_or_else(|| ToolError::MissingParameter("entity_id".to_string()))?;

        let key = params.get("key")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::MissingParameter("key".to_string()))?;

        let value = params.get("value")
            .cloned()
            .ok_or_else(|| ToolError::MissingParameter("value".to_string()))?;

        Ok(ToolResult {
            success: true,
            message: format!("Blackboard value set for entity {:?}: '{}' = {:?}", entity_id, key, value),
            data: Some(serde_json::json!({
                "entity_id": entity_id.0,
                "key": key,
                "value": value,
                "command": "SetBlackboardValue",
            })),
            execution_time_ms: 0,
        })
    }
}

/// Tool to query runtime agent status
pub struct QueryRuntimeAgentsTool;

impl Tool for QueryRuntimeAgentsTool {
    fn name(&self) -> &str {
        "query_runtime_agents"
    }

    fn description(&self) -> &str {
        "Query all runtime AI agents in the scene"
    }

    fn parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter {
                name: "filter".to_string(),
                description: "Filter by profile ID (optional)".to_string(),
                param_type: ParameterType::String,
                required: false,
                default: Some(Value::String("*".to_string())),
            },
        ]
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Scene
    }

    fn execute(&self, params: HashMap<String, Value>) -> Result<ToolResult, ToolError> {
        let _filter = params.get("filter")
            .and_then(|v| v.as_str())
            .unwrap_or("*");

        // This is a placeholder - actual implementation would query the registry
        Ok(ToolResult {
            success: true,
            message: "Runtime agents query (placeholder - implement with actual registry access)".to_string(),
            data: Some(serde_json::json!({
                "agents": [],
                "count": 0,
            })),
            execution_time_ms: 0,
        })
    }
}

/// Register all runtime agent tools
pub fn register_runtime_agent_tools(registry: &mut crate::tool::ToolRegistry) {
    registry.register(AttachRuntimeAgentTool);
    registry.register(SetAgentControlModeTool);
    registry.register(SetAgentGoalTool);
    registry.register(SetAgentBlackboardTool);
    registry.register(QueryRuntimeAgentsTool);
}
