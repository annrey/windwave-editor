//! LLM Runtime Agent - Async LLM integration for runtime agents
//!
//! Provides a Bevy system that runs LLM inference for agents with LlmDriven behavior.
//! Bridges agent perception -> LLM prompt -> tool calls -> action execution.

use agent_core::llm::{LlmClient, LlmResponse, ToolDefinition};
use agent_core::runtime_agent::{RuntimeBehaviorSpec, RuntimeAgentAction, RuntimeTarget, RuntimeAgentStatus};
use agent_core::types::EntityId;
use bevy::prelude::*;
use std::sync::Arc;
use tokio::sync::mpsc;

// Use local RuntimeAgentComponent which is the Bevy Component wrapper
use crate::runtime_agent::RuntimeAgentComponent;

/// Resource holding the LLM client
#[derive(Resource)]
pub struct LlmRuntimeResource {
    /// The LLM client (wrapped in Arc for thread safety)
    pub client: Option<Arc<dyn LlmClient>>,
    /// Channel for sending LLM requests from Bevy systems
    pub request_tx: mpsc::UnboundedSender<LlmAgentRequest>,
    /// Channel for receiving LLM responses back to Bevy systems
    pub response_rx: mpsc::UnboundedReceiver<LlmAgentResponse>,
}

/// A request to the LLM runtime from an agent
pub struct LlmAgentRequest {
    pub agent_entity: Entity,
    pub agent_id: String,
    pub system_prompt: String,
    pub context: String,
    pub available_tools: Vec<ToolDefinition>,
}

/// A response from the LLM runtime back to an agent
pub struct LlmAgentResponse {
    pub agent_entity: Entity,
    pub agent_id: String,
    pub result: Result<LlmResponse, String>,
}

/// Component marking an agent as waiting for LLM response
#[derive(Component)]
pub struct PendingLlmRequest {
    pub request_sent_at: std::time::Instant,
}

/// Plugin for LLM runtime agent integration
pub struct LlmRuntimeAgentPlugin;

impl Plugin for LlmRuntimeAgentPlugin {
    fn build(&self, app: &mut App) {
        // Create channels for LLM communication
        let (request_tx, mut request_rx) = mpsc::unbounded_channel::<LlmAgentRequest>();
        let (response_tx, response_rx) = mpsc::unbounded_channel::<LlmAgentResponse>();
        
        // Spawn background task for LLM processing
        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                // Process incoming LLM requests
                while let Some(request) = request_rx.recv().await {
                    // For now, return a mock response
                    // In production, this would call the actual LLM client
                    let response = LlmAgentResponse {
                        agent_entity: request.agent_entity,
                        agent_id: request.agent_id,
                        result: Ok(LlmResponse {
                            content: "I'll move to the nearest target.".to_string(),
                            tool_calls: vec![],
                            usage: Default::default(),
                        }),
                    };
                    
                    if response_tx.send(response).is_err() {
                        log::error!("Failed to send LLM response back to Bevy");
                        break;
                    }
                }
            });
        });
        
        app.insert_resource(LlmRuntimeResource {
            client: None,
            request_tx,
            response_rx,
        })
        .add_systems(Update, (
            llm_agent_tick_system,
            process_llm_responses,
        ).chain());
    }
}

/// System: Check for agents needing LLM inference and send requests
fn llm_agent_tick_system(
    mut query: Query<(Entity, &mut RuntimeAgentComponent), Without<PendingLlmRequest>>,
    llm_runtime: Res<LlmRuntimeResource>,
) {
    for (entity, mut agent) in query.iter_mut() {
        // Only process agents with LlmDriven behavior that are active
        let should_request = match &agent.behavior {
            RuntimeBehaviorSpec::LlmDriven { .. } => {
                agent.is_active() &&
                matches!(agent.status, RuntimeAgentStatus::Idle | RuntimeAgentStatus::Thinking)
            }
            _ => false,
        };
        
        if !should_request {
            continue;
        }
        
        // Build context from agent's observation and blackboard
        let context = build_agent_context(&agent);
        
        // Get system prompt and available tools
        let (system_prompt, tool_allowlist) = match &agent.behavior {
            RuntimeBehaviorSpec::LlmDriven { system_prompt, tool_allowlist } => {
                (system_prompt.clone(), tool_allowlist.clone())
            }
            _ => continue,
        };
        
        // Build available tools based on allowlist
        let available_tools = build_available_tools(&tool_allowlist);
        
        // Send LLM request
        let request = LlmAgentRequest {
            agent_entity: entity,
            agent_id: agent.id.0.clone(),
            system_prompt,
            context,
            available_tools,
        };
        
        if llm_runtime.request_tx.send(request).is_ok() {
            agent.status = RuntimeAgentStatus::Thinking;
            log::debug!("Sent LLM request for agent {:?}", entity);
        } else {
            log::error!("Failed to send LLM request for agent {:?}", entity);
        }
    }
}

/// System: Process incoming LLM responses
fn process_llm_responses(
    mut llm_runtime: ResMut<LlmRuntimeResource>,
    mut query: Query<(Entity, &mut RuntimeAgentComponent)>,
    _commands: Commands,
) {
    // Process all available responses (non-blocking)
    while let Ok(response) = llm_runtime.response_rx.try_recv() {
        if let Ok((entity, mut agent)) = query.get_mut(response.agent_entity) {
            match response.result {
                Ok(llm_response) => {
                    // Parse LLM response into actions
                    let actions = parse_llm_response_to_actions(&llm_response);
                    
                    if actions.is_empty() {
                        // No actions, just log the content
                        agent.blackboard.set(
                            "last_llm_thought",
                            serde_json::json!(llm_response.content),
                        );
                        agent.status = RuntimeAgentStatus::Idle;
                    } else {
                        // Queue actions for execution
                        let action_count = actions.len();
                        agent.pending_actions.extend(actions);
                        agent.status = RuntimeAgentStatus::Acting;
                        
                        agent.blackboard.set(
                            "last_llm_thought",
                            serde_json::json!(llm_response.content),
                        );
                        agent.blackboard.set(
                            "pending_action_count",
                            serde_json::json!(action_count),
                        );
                    }
                    
                    log::info!(
                        "Agent {:?} received LLM response with {} actions",
                        entity,
                        agent.pending_actions.len()
                    );
                }
                Err(e) => {
                    agent.status = RuntimeAgentStatus::Error {
                        message: format!("LLM error: {}", e),
                    };
                    log::error!("LLM error for agent {:?}: {}", entity, e);
                }
            }
        } else {
            log::warn!("Received LLM response for despawned agent {:?}", response.agent_entity);
        }
    }
}

/// Build context string from agent's current state
fn build_agent_context(agent: &RuntimeAgentComponent) -> String {
    let mut context_parts = Vec::new();
    
    // Add perception info
    if let Some(ref obs) = agent.last_observation {
        context_parts.push(format!(
            "You can see {} entities nearby.",
            obs.visible_entities.len()
        ));
        
        for (i, entity_id) in obs.visible_entities.iter().enumerate().take(5) {
            context_parts.push(format!("  - Entity {}: ID {:?}", i + 1, entity_id.0));
        }
        
        // Add recent events
        if !obs.events.is_empty() {
            context_parts.push("\nRecent events:".to_string());
            for event in obs.events.iter().rev().take(3) {
                context_parts.push(format!(
                    "  - {}: {:?}",
                    event.event_type,
                    event.payload
                ));
            }
        }
    } else {
        context_parts.push("No current perception data available.".to_string());
    }
    
    // Add blackboard info
    let blackboard_snapshot = agent.blackboard.snapshot();
    if !blackboard_snapshot.is_empty() {
        context_parts.push("\nYour memory:".to_string());
        for (key, value) in blackboard_snapshot.iter().take(5) {
            context_parts.push(format!("  - {}: {:?}", key, value));
        }
    }
    
    // Add goal if any
    if let Some(ref goal) = agent.active_goal {
        context_parts.push(format!("\nYour current goal: {}", goal.description));
    }
    
    context_parts.join("\n")
}

/// Build available tool definitions for the LLM
fn build_available_tools(tool_allowlist: &[String]) -> Vec<ToolDefinition> {
    let mut tools = Vec::new();
    
    for tool_name in tool_allowlist {
        match tool_name.as_str() {
            "move_to" => {
                tools.push(ToolDefinition {
                    name: "move_to".to_string(),
                    description: "Move to a target position or entity".to_string(),
                    parameters: serde_json::json!({
                        "type": "object",
                        "properties": {
                            "target": {
                                "type": "object",
                                "description": "Target to move to",
                                "properties": {
                                    "type": { "type": "string", "enum": ["position", "entity"] },
                                    "value": { "type": "array" }
                                }
                            },
                            "speed": { "type": "number", "description": "Movement speed" }
                        },
                        "required": ["target"]
                    }),
                });
            }
            "look_at" => {
                tools.push(ToolDefinition {
                    name: "look_at".to_string(),
                    description: "Look at a target position or entity".to_string(),
                    parameters: serde_json::json!({
                        "type": "object",
                        "properties": {
                            "target": {
                                "type": "object",
                                "description": "Target to look at"
                            }
                        },
                        "required": ["target"]
                    }),
                });
            }
            "emit_event" => {
                tools.push(ToolDefinition {
                    name: "emit_event".to_string(),
                    description: "Emit a game event".to_string(),
                    parameters: serde_json::json!({
                        "type": "object",
                        "properties": {
                            "event_type": { "type": "string" },
                            "payload": { "type": "object" }
                        },
                        "required": ["event_type"]
                    }),
                });
            }
            "set_blackboard" => {
                tools.push(ToolDefinition {
                    name: "set_blackboard".to_string(),
                    description: "Set a value in your memory".to_string(),
                    parameters: serde_json::json!({
                        "type": "object",
                        "properties": {
                            "key": { "type": "string" },
                            "value": { "type": "string" }
                        },
                        "required": ["key", "value"]
                    }),
                });
            }
            _ => {}
        }
    }
    
    tools
}

/// Parse LLM response into runtime actions
fn parse_llm_response_to_actions(response: &LlmResponse) -> Vec<RuntimeAgentAction> {
    let mut actions = Vec::new();
    
    // Process tool calls
    for tool_call in &response.tool_calls {
        match tool_call.name.as_str() {
            "move_to" => {
                if let Some(target) = tool_call.arguments.get("target") {
                    // Parse target
                    let runtime_target = parse_target_from_json(target);
                    actions.push(RuntimeAgentAction::MoveTo { target: runtime_target });
                }
            }
            "look_at" => {
                if let Some(target) = tool_call.arguments.get("target") {
                    let runtime_target = parse_target_from_json(target);
                    actions.push(RuntimeAgentAction::LookAt { target: runtime_target });
                }
            }
            "emit_event" => {
                let event_type = tool_call.arguments
                    .get("event_type")
                    .and_then(|v| v.as_str())
                    .unwrap_or("generic")
                    .to_string();
                let payload = tool_call.arguments
                    .get("payload")
                    .cloned()
                    .unwrap_or(serde_json::Value::Null);
                actions.push(RuntimeAgentAction::EmitEvent { event_type, payload });
            }
            "set_blackboard" => {
                if let (Some(key), Some(value)) = (
                    tool_call.arguments.get("key").and_then(|v| v.as_str()),
                    tool_call.arguments.get("value")
                ) {
                    actions.push(RuntimeAgentAction::ModifyOwnComponent {
                        component_type: "Blackboard".to_string(),
                        property: key.to_string(),
                        value: value.clone(),
                    });
                }
            }
            _ => {
                log::warn!("Unknown tool call: {}", tool_call.name);
            }
        }
    }
    
    // If no tool calls but content suggests an action, try to parse from text
    if actions.is_empty() && !response.content.is_empty() {
        actions.extend(parse_actions_from_text(&response.content));
    }
    
    actions
}

/// Parse a target from JSON
fn parse_target_from_json(value: &serde_json::Value) -> RuntimeTarget {
    if let Some(obj) = value.as_object() {
        let target_type = obj.get("type").and_then(|v| v.as_str());
        let target_value = obj.get("value");
        
        match target_type {
            Some("position") => {
                if let Some(arr) = target_value.and_then(|v| v.as_array()) {
                    let x = arr.get(0).and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                    let y = arr.get(1).and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                    let z = arr.get(2).and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                    RuntimeTarget::Position([x, y, z])
                } else {
                    RuntimeTarget::SelfEntity
                }
            }
            Some("entity") => {
                if let Some(id) = target_value.and_then(|v| v.as_u64()) {
                    RuntimeTarget::Entity(EntityId(id))
                } else {
                    RuntimeTarget::SelfEntity
                }
            }
            _ => RuntimeTarget::SelfEntity,
        }
    } else {
        RuntimeTarget::SelfEntity
    }
}

/// Simple text parsing for actions (fallback)
fn parse_actions_from_text(text: &str) -> Vec<RuntimeAgentAction> {
    let mut actions = Vec::new();
    let text_lower = text.to_lowercase();
    
    // Simple keyword matching
    if text_lower.contains("move") || text_lower.contains("go to") || text_lower.contains("approach") {
        // Could parse position from text, for now just emit an event
        actions.push(RuntimeAgentAction::EmitEvent {
            event_type: "intention".to_string(),
            payload: serde_json::json!({ "action": "move", "context": text }),
        });
    }
    
    if text_lower.contains("look") || text_lower.contains("face") || text_lower.contains("turn") {
        actions.push(RuntimeAgentAction::EmitEvent {
            event_type: "intention".to_string(),
            payload: serde_json::json!({ "action": "look", "context": text }),
        });
    }
    
    actions
}

/// Helper to configure LLM client for runtime agents
pub fn configure_llm_runtime(
    _commands: &mut Commands,
    client: Arc<dyn LlmClient>,
) {
    // The resource is already inserted by the plugin, we need to update it
    // This is a placeholder - in production, use a proper initialization system
    log::info!("LLM runtime configured with client: {:?}", client.provider());
}
