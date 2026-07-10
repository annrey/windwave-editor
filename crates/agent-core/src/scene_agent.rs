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

use crate::llm::{LlmClient, LlmMessage, LlmRequest, Role, ToolDefinition};
use crate::registry::{
    Agent, AgentError, AgentId, AgentRequest, AgentResponse, AgentResultKind, CapabilityKind,
};
use crate::scene_bridge::{create_empty_shared_bridge, SharedSceneBridge};
use crate::tool::{ToolCall, ToolRegistry};

/// System prompt for the SceneAgent when using LLM-driven tool selection.
/// Instructs the LLM to return only tool calls, no explanatory text.
const SCENE_AGENT_SYSTEM_PROMPT: &str = r#"You are a scene manipulation agent for a game editor.
Your only job is to translate the user's natural-language instruction into tool calls.

Rules:
1. Return ONLY tool calls — no explanatory text, no markdown, no prose.
2. Use the available tools to create, update, query, or delete entities.
3. For entity creation, infer entity_type from the instruction (enemy, player, object, npc, etc.).
4. For color descriptions, use RGBA float values: red=[1,0,0,1], blue=[0,0,1,1], green=[0,1,0,1], etc.
5. For spatial descriptions, use 3D coordinates: right=positive X, left=negative X, above=positive Y, below=negative Y.
6. If the instruction is unclear, default to query_entities instead of guessing.
7. Chain multiple tools when needed (e.g., create entity THEN set its properties)."#;

// ---------------------------------------------------------------------------
// SceneAgent
// ---------------------------------------------------------------------------

pub struct SceneAgent {
    id: AgentId,
    name: String,
    tool_registry: ToolRegistry,
    llm_client: Option<Box<dyn LlmClient>>,
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
            llm_client: None,
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
            llm_client: None,
        }
    }

    pub fn with_bridge(id: AgentId, bridge: SharedSceneBridge) -> Self {
        let mut tool_registry = ToolRegistry::new();
        crate::scene_tools::register_scene_tools(&mut tool_registry, bridge);

        Self {
            id,
            name: format!("SceneAgent_{}", id.0),
            tool_registry,
            llm_client: None,
        }
    }

    pub fn with_bridge_and_name(
        id: AgentId,
        name: impl Into<String>,
        bridge: SharedSceneBridge,
    ) -> Self {
        let mut tool_registry = ToolRegistry::new();
        crate::scene_tools::register_scene_tools(&mut tool_registry, bridge);

        Self {
            id,
            name: name.into(),
            tool_registry,
            llm_client: None,
        }
    }

    /// Attach an LLM client for NL→Tool mapping.
    /// When set, `handle()` will use function-calling to select tools.
    /// When not set, falls back to keyword-based parsing.
    pub fn with_llm(mut self, client: Box<dyn LlmClient>) -> Self {
        self.llm_client = Some(client);
        self
    }

    pub fn tool_registry(&self) -> &ToolRegistry {
        &self.tool_registry
    }

    pub fn tool_registry_mut(&mut self) -> &mut ToolRegistry {
        &mut self.tool_registry
    }

    /// Use LLM function-calling to map a natural-language instruction to tool steps.
    /// Returns the parsed steps, or falls back to keyword parsing on any error.
    async fn llm_select_tools(&self, instruction: &str) -> Vec<InstructionStep> {
        let llm = match &self.llm_client {
            Some(c) => c,
            None => return vec![], // caller falls back to keyword
        };

        let tool_descriptions: Vec<ToolDefinition> = self
            .tool_registry
            .all_mcp_descriptions()
            .into_iter()
            .map(|v| ToolDefinition {
                name: v
                    .get("name")
                    .and_then(|n| n.as_str())
                    .unwrap_or("")
                    .to_string(),
                description: v
                    .get("description")
                    .and_then(|d| d.as_str())
                    .unwrap_or("")
                    .to_string(),
                parameters: v
                    .get("inputSchema")
                    .cloned()
                    .unwrap_or(serde_json::json!({})),
            })
            .filter(|td| !td.name.is_empty())
            .collect();

        let request = LlmRequest {
            model: crate::llm::models::OPENAI_FAST.to_string(),
            messages: vec![
                LlmMessage {
                    role: Role::System,
                    content: SCENE_AGENT_SYSTEM_PROMPT.to_string(),
                },
                LlmMessage {
                    role: Role::User,
                    content: instruction.to_string(),
                },
            ],
            tools: Some(tool_descriptions),
            max_tokens: Some(1024),
            temperature: Some(0.1),
        };

        match llm.chat(request).await {
            Ok(response) => {
                if response.tool_calls.is_empty() {
                    // LLM returned no tools — fall back to keyword
                    log::warn!("LLM returned no tool calls for: {instruction}");
                    return vec![];
                }
                response
                    .tool_calls
                    .into_iter()
                    .map(|tc| InstructionStep {
                        tool_name: tc.name,
                        params: tc.arguments,
                    })
                    .collect()
            }
            Err(e) => {
                log::warn!("LLM tool selection failed ({e}), falling back to keyword parser");
                vec![]
            }
        }
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

        // Prefer LLM function-calling, fall back to keyword parser
        let parsed: Vec<InstructionStep> = {
            let llm_steps = self.llm_select_tools(&instruction).await;
            if llm_steps.is_empty() {
                parse_instruction_fallback(&instruction)
            } else {
                llm_steps
            }
        };

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
// Keyword-based fallback instruction parser (used when LLM is unavailable)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct InstructionStep {
    tool_name: String,
    params: std::collections::HashMap<String, serde_json::Value>,
}

fn parse_instruction_fallback(text: &str) -> Vec<InstructionStep> {
    let lower = text.to_lowercase();
    let mut steps = Vec::new();

    if crate::keyword_matcher::KeywordMatcher::is_create_operation(text) {
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
        params.insert(
            "entity_name".to_string(),
            serde_json::json!(format!("{}_{}", entity_type, steps.len())),
        );

        steps.push(InstructionStep {
            tool_name: "create_entity".to_string(),
            params,
        });

        if lower.contains("红色")
            || lower.contains("蓝色")
            || lower.contains("绿色")
            || lower.contains("red")
            || lower.contains("blue")
            || lower.contains("green")
        {
            let mut color_params = std::collections::HashMap::new();
            color_params.insert(
                "entity_id".to_string(),
                serde_json::json!(format!("{}_{}", entity_type, 0)),
            );
            color_params.insert("component".to_string(), serde_json::json!("sprite"));
            color_params.insert(
                "properties".to_string(),
                serde_json::json!({
                    "color": color
                }),
            );

            steps.push(InstructionStep {
                tool_name: "update_component".to_string(),
                params: color_params,
            });
        }

        if lower.contains("右侧")
            || lower.contains("左侧")
            || lower.contains("上方")
            || lower.contains("right")
            || lower.contains("left")
            || lower.contains("above")
        {
            let mut pos_params = std::collections::HashMap::new();
            pos_params.insert(
                "entity_id".to_string(),
                serde_json::json!(format!("{}_{}", entity_type, 0)),
            );
            pos_params.insert("component".to_string(), serde_json::json!("transform"));
            pos_params.insert(
                "properties".to_string(),
                serde_json::json!({
                    "position": position
                }),
            );

            steps.push(InstructionStep {
                tool_name: "update_component".to_string(),
                params: pos_params,
            });
        }
    }

    if lower.contains("查询")
        || lower.contains("query")
        || lower.contains("列表")
        || lower.contains("list")
    {
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
        let steps = parse_instruction_fallback("创建一个红色敌人放在右侧");
        assert!(!steps.is_empty());

        let create = steps.iter().find(|s| s.tool_name == "create_entity");
        assert!(create.is_some());
    }

    #[test]
    fn test_parse_query() {
        let steps = parse_instruction_fallback("查询所有实体");
        assert!(!steps.is_empty());
        assert_eq!(steps[0].tool_name, "query_entities");
    }

    #[test]
    fn test_parse_unknown_falls_back_to_query() {
        let steps = parse_instruction_fallback("hello");
        assert!(!steps.is_empty());
        assert_eq!(steps[0].tool_name, "query_entities");
    }

    #[tokio::test]
    async fn test_handle_request() {
        let bridge = crate::scene_bridge::create_shared_bridge(Box::new(
            crate::scene_bridge::MockSceneBridge::new(),
        ));
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

    #[cfg(feature = "llm-openai")]
    #[test]
    fn test_with_llm_builder() {
        use crate::llm::{LlmConfig, LlmProvider};
        let config = LlmConfig {
            provider: LlmProvider::OpenAI,
            api_key: "sk-test".into(),
            ..Default::default()
        };
        let client = crate::llm::OpenAiClient::new(config).expect("OpenAiClient::new");
        let agent = SceneAgent::new(AgentId(99)).with_llm(Box::new(client));
        assert!(agent.llm_client.is_some());
    }

    #[tokio::test]
    async fn test_handle_without_llm_falls_back_to_keyword() {
        let bridge = crate::scene_bridge::create_shared_bridge(Box::new(
            crate::scene_bridge::MockSceneBridge::new(),
        ));
        // No LLM client attached — should fall back to keyword parser
        let mut agent = SceneAgent::with_bridge(AgentId(1), bridge);

        let request = AgentRequest {
            task_id: Some("task_1".into()),
            instruction: "创建一个红色敌人".into(),
            context: serde_json::json!({}),
        };

        let response = agent.handle(request).await.unwrap();
        assert!(matches!(response.result, AgentResultKind::Success { .. }));
    }
}
