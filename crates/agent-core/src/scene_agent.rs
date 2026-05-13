//! SceneAgent — specialises in scene-entity read/write operations.
//!
//! Design reference: Section 12.6 + Section 15.5 of
//! gpt-agent-team-task-event-skill-architecture.md
//!
//! ```text
//! SceneAgent {
//!     tool_registry
//!     llm_client (optional, for NL→tool mapping)
//!     scene_index (read-only snapshot)
//!     handle(AgentRequest) → AgentResponse
//! }
//! ```

use crate::registry::{Agent, AgentId, AgentRequest, AgentResponse, AgentResultKind, AgentError, CapabilityKind};
use crate::scene_bridge::{SharedSceneBridge, create_empty_shared_bridge};
use crate::tool::{ToolRegistry, ToolCall};

// ---------------------------------------------------------------------------
// SceneAgent
// ---------------------------------------------------------------------------

pub struct SceneAgent {
    id: AgentId,
    name: String,
    tool_registry: ToolRegistry,
}

impl SceneAgent {
    pub fn new(id: AgentId) -> Self {
        let bridge = create_empty_shared_bridge();
        let mut tool_registry = ToolRegistry::new();
        crate::scene_tools::register_scene_tools(&mut tool_registry, bridge);

        Self {
            id,
            name: format!("SceneAgent_{}", id.0),
            tool_registry,
        }
    }

    pub fn new_with_name(id: AgentId, name: impl Into<String>) -> Self {
        let bridge = create_empty_shared_bridge();
        let mut tool_registry = ToolRegistry::new();
        crate::scene_tools::register_scene_tools(&mut tool_registry, bridge);

        Self {
            id,
            name: name.into(),
            tool_registry,
        }
    }

    pub fn with_bridge(id: AgentId, bridge: SharedSceneBridge) -> Self {
        let mut tool_registry = ToolRegistry::new();
        crate::scene_tools::register_scene_tools(&mut tool_registry, bridge);

        Self {
            id,
            name: format!("SceneAgent_{}", id.0),
            tool_registry,
        }
    }

    pub fn with_bridge_and_name(id: AgentId, name: impl Into<String>, bridge: SharedSceneBridge) -> Self {
        let mut tool_registry = ToolRegistry::new();
        crate::scene_tools::register_scene_tools(&mut tool_registry, bridge);

        Self {
            id,
            name: name.into(),
            tool_registry,
        }
    }

    pub fn tool_registry(&self) -> &ToolRegistry {
        &self.tool_registry
    }

    pub fn tool_registry_mut(&mut self) -> &mut ToolRegistry {
        &mut self.tool_registry
    }
}

#[async_trait::async_trait]
impl Agent for SceneAgent {
    fn id(&self) -> AgentId {
        self.id
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn role(&self) -> &str {
        "scene"
    }

    fn capabilities(&self) -> &[CapabilityKind] {
        &[CapabilityKind::SceneRead, CapabilityKind::SceneWrite]
    }

    async fn handle(&mut self, request: AgentRequest) -> Result<AgentResponse, AgentError> {
        let instruction = request.instruction.clone();
        let _context = request.context.clone();

        let parsed: Vec<InstructionStep> = parse_instruction(&instruction);

        let mut results: Vec<serde_json::Value> = Vec::new();
        let mut events: Vec<crate::event::EventBusEvent> = Vec::new();

        for step in parsed {
            let call = ToolCall {
                call_id: format!("{}_{}", self.id.0, step.tool_name),
                tool_name: step.tool_name.clone(),
                parameters: step.params.clone(),
            };

            match self.tool_registry.execute(&call) {
                Ok(result) => {
                    results.push(serde_json::json!({
                        "step": step.tool_name,
                        "status": "ok",
                        "data": result.data,
                    }));

                    if step.tool_name == "create_entity" || step.tool_name == "update_component" {
                        events.push(crate::event::EventBusEvent::EngineCommandApplied {
                            transaction_id: request.task_id.clone().unwrap_or_default(),
                            success: true,
                            message: format!("{} executed", step.tool_name),
                        });
                    }
                }
                Err(e) => {
                    results.push(serde_json::json!({
                        "step": step.tool_name,
                        "status": "error",
                        "error": e.to_string(),
                    }));
                }
            }
        }

        Ok(AgentResponse {
            agent_id: self.id,
            agent_name: self.name.clone(),
            result: AgentResultKind::Success {
                summary: format!("Executed {} steps", results.len()),
                output: serde_json::json!({ "steps": results }),
            },
            events,
        })
    }
}

// ---------------------------------------------------------------------------
// Simple NL instruction parser
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct InstructionStep {
    tool_name: String,
    params: std::collections::HashMap<String, serde_json::Value>,
}

fn parse_instruction(text: &str) -> Vec<InstructionStep> {
    let lower = text.to_lowercase();
    let mut steps = Vec::new();

    if lower.contains("创建") || lower.contains("create") {
        let entity_type = if lower.contains("敌人") || lower.contains("enemy") {
            "enemy"
        } else if lower.contains("玩家") || lower.contains("player") {
            "player"
        } else if lower.contains("物体") || lower.contains("object") {
            "object"
        } else {
            "entity"
        };

        let color = if lower.contains("红色") || lower.contains("red") {
            vec![1.0, 0.0, 0.0, 1.0]
        } else if lower.contains("蓝色") || lower.contains("blue") {
            vec![0.0, 0.0, 1.0, 1.0]
        } else if lower.contains("绿色") || lower.contains("green") {
            vec![0.0, 1.0, 0.0, 1.0]
        } else {
            vec![1.0, 1.0, 1.0, 1.0]
        };

        let position = if lower.contains("右侧") || lower.contains("right") {
            vec![5.0, 0.0, 0.0]
        } else if lower.contains("左侧") || lower.contains("left") {
            vec![-5.0, 0.0, 0.0]
        } else if lower.contains("上方") || lower.contains("above") {
            vec![0.0, 5.0, 0.0]
        } else {
            vec![0.0, 0.0, 0.0]
        };

        let mut params = std::collections::HashMap::new();
        params.insert("entity_type".to_string(), serde_json::json!(entity_type));
        params.insert("entity_name".to_string(), serde_json::json!(format!("{}_{}", entity_type, steps.len())));

        steps.push(InstructionStep {
            tool_name: "create_entity".to_string(),
            params,
        });

        if lower.contains("红色") || lower.contains("蓝色") || lower.contains("绿色")
            || lower.contains("red") || lower.contains("blue") || lower.contains("green")
        {
            let mut color_params = std::collections::HashMap::new();
            color_params.insert("entity_id".to_string(), serde_json::json!(format!("{}_{}", entity_type, 0)));
            color_params.insert("component".to_string(), serde_json::json!("sprite"));
            color_params.insert("properties".to_string(), serde_json::json!({
                "color": color
            }));

            steps.push(InstructionStep {
                tool_name: "update_component".to_string(),
                params: color_params,
            });
        }

        if lower.contains("右侧") || lower.contains("左侧") || lower.contains("上方")
            || lower.contains("right") || lower.contains("left") || lower.contains("above")
        {
            let mut pos_params = std::collections::HashMap::new();
            pos_params.insert("entity_id".to_string(), serde_json::json!(format!("{}_{}", entity_type, 0)));
            pos_params.insert("component".to_string(), serde_json::json!("transform"));
            pos_params.insert("properties".to_string(), serde_json::json!({
                "position": position
            }));

            steps.push(InstructionStep {
                tool_name: "update_component".to_string(),
                params: pos_params,
            });
        }
    }

    if lower.contains("查询") || lower.contains("query") || lower.contains("列表") || lower.contains("list") {
        let mut params = std::collections::HashMap::new();
        params.insert("entity_type".to_string(), serde_json::json!(null));

        steps.push(InstructionStep {
            tool_name: "query_entities".to_string(),
            params,
        });
    }

    if steps.is_empty() {
        let mut params = std::collections::HashMap::new();
        params.insert("entity_type".to_string(), serde_json::json!(null));

        steps.push(InstructionStep {
            tool_name: "query_entities".to_string(),
            params,
        });
    }

    steps
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scene_agent_capabilities() {
        let agent = SceneAgent::new(AgentId(1));
        let caps = agent.capabilities();
        assert!(caps.contains(&CapabilityKind::SceneRead));
        assert!(caps.contains(&CapabilityKind::SceneWrite));
    }

    #[test]
    fn test_parse_create_enemy_red() {
        let steps = parse_instruction("创建一个红色敌人放在右侧");
        assert!(!steps.is_empty());

        let create = steps.iter().find(|s| s.tool_name == "create_entity");
        assert!(create.is_some());
    }

    #[test]
    fn test_parse_query() {
        let steps = parse_instruction("查询所有实体");
        assert!(!steps.is_empty());
        assert_eq!(steps[0].tool_name, "query_entities");
    }

    #[test]
    fn test_parse_unknown_falls_back_to_query() {
        let steps = parse_instruction("hello");
        assert!(!steps.is_empty());
        assert_eq!(steps[0].tool_name, "query_entities");
    }

    #[tokio::test]
    async fn test_handle_request() {
        let bridge = crate::scene_bridge::create_shared_bridge(
            Box::new(crate::scene_bridge::MockSceneBridge::new())
        );
        let mut agent = SceneAgent::with_bridge(AgentId(1), bridge);

        let request = AgentRequest {
            task_id: Some("task_1".into()),
            instruction: "创建一个红色敌人".into(),
            context: serde_json::json!({}),
        };

        let response = agent.handle(request).await.unwrap();
        assert!(matches!(response.result, AgentResultKind::Success { .. }));
    }

    #[test]
    fn test_new_with_name() {
        let agent = SceneAgent::new_with_name(AgentId(42), "MySceneAgent");
        assert_eq!(agent.name(), "MySceneAgent");
        assert_eq!(agent.id().0, 42);
    }
}
