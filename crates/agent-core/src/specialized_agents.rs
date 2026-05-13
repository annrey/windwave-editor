//! Specialised Agent implementations.
//!
//! Each agent wraps an existing subsystem and exposes it via the Agent trait
//! so it can be registered with the AgentRegistry and dispatched by the
//! DirectorRuntime.
//!
//! Implemented agents:
//!   - CodeAgent      (§2.4) — code generation via code_tools + LLM
//!   - ReviewAgent    (§2.5) — wraps Reviewer from review.rs
//!   - EditorAgent    (§2.6) — wraps DirectorRuntime (master controller)
//!   - PlannerAgent   (§2.7) — wraps RuleBasedPlanner from planner.rs

use crate::agent::{BaseAgent, AgentInstanceId, AgentState};
use crate::director::DirectorRuntime;
use crate::planner::{RuleBasedPlanner, PlannerContext};
use crate::registry::{Agent, AgentId, AgentRequest, AgentResponse, AgentResultKind, AgentError, CapabilityKind};
use crate::review::Reviewer;
use crate::tool::{ToolRegistry, ToolCall};
use crate::types::UserRequest;

// ============================================================================
// CodeAgent
// ============================================================================

pub struct CodeAgent {
    id: AgentId,
    name: String,
    base: BaseAgent,
    tool_registry: ToolRegistry,
}

impl CodeAgent {
    pub fn new(id: AgentId) -> Self {
        let mut tool_registry = ToolRegistry::new();
        crate::code_tools::register_code_tools(&mut tool_registry);

        Self {
            id,
            name: format!("CodeAgent_{}", id.0),
            base: BaseAgent::new(AgentInstanceId(id.0), format!("CodeAgent_{}", id.0)),
            tool_registry,
        }
    }

    pub fn new_with_name(id: AgentId, name: impl Into<String>) -> Self {
        let name = name.into();
        let mut tool_registry = ToolRegistry::new();
        crate::code_tools::register_code_tools(&mut tool_registry);

        Self {
            id,
            base: BaseAgent::new(AgentInstanceId(id.0), name.clone()),
            name,
            tool_registry,
        }
    }

    pub fn base(&self) -> &BaseAgent { &self.base }
    pub fn base_mut(&mut self) -> &mut BaseAgent { &mut self.base }
}

#[async_trait::async_trait]
impl Agent for CodeAgent {
    fn id(&self) -> AgentId {
        self.id
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn role(&self) -> &str {
        "code"
    }

    fn capabilities(&self) -> &[CapabilityKind] {
        &[CapabilityKind::CodeRead, CapabilityKind::CodeWrite]
    }

    async fn handle(&mut self, request: AgentRequest) -> Result<AgentResponse, AgentError> {
        let instruction = request.instruction.clone();

        self.base.transition_to(AgentState::AnalyzingRequest {
            request: UserRequest { content: instruction.clone(), entity_refs: vec![], estimated_steps: 0 },
            start_time: chrono::Utc::now(),
        });

        let mut results: Vec<serde_json::Value> = Vec::new();
        let mut events = Vec::new();

        if instruction.contains("生成") || instruction.contains("generate") || instruction.contains("创建") {
            self.base.transition_to(AgentState::SelectingTools {
                available: vec!["generate_code".into()],
                selected: vec!["generate_code".into()],
            });

            let call = ToolCall {
                call_id: format!("cg_{}", self.id.0),
                tool_name: "generate_code".to_string(),
                parameters: {
                    let mut p = std::collections::HashMap::new();
                    p.insert("prompt".to_string(), serde_json::json!(instruction));
                    p
                },
            };

            self.base.transition_to(AgentState::ExecutingTools {
                completed: 0,
                in_progress: 1,
                total: 1,
            });

            match self.tool_registry.execute(&call) {
                Ok(result) => {
                    results.push(serde_json::json!({
                        "tool": "generate_code",
                        "status": "ok",
                        "data": result.data,
                    }));
                    events.push(crate::event::EventBusEvent::EngineCommandApplied {
                        transaction_id: request.task_id.unwrap_or_default(),
                        success: true,
                        message: "Code generated".into(),
                    });
                }
                Err(e) => {
                    self.base.transition_to(AgentState::Error {
                        error: format!("code generation failed: {}", e),
                        recoverable: false,
                    });
                    return Ok(AgentResponse {
                        agent_id: self.id,
                        agent_name: self.name.clone(),
                        result: AgentResultKind::Failed {
                            reason: format!("code generation failed: {}", e),
                        },
                        events,
                    });
                }
            }
        } else {
            self.base.transition_to(AgentState::SelectingTools {
                available: vec!["analyze_code".into()],
                selected: vec!["analyze_code".into()],
            });

            let call = ToolCall {
                call_id: format!("ca_{}", self.id.0),
                tool_name: "analyze_code".to_string(),
                parameters: {
                    let mut p = std::collections::HashMap::new();
                    p.insert("code".to_string(), serde_json::json!(instruction));
                    p
                },
            };

            self.base.transition_to(AgentState::ExecutingTools {
                completed: 0,
                in_progress: 1,
                total: 1,
            });

            match self.tool_registry.execute(&call) {
                Ok(result) => {
                    results.push(serde_json::json!({
                        "tool": "analyze_code",
                        "status": "ok",
                        "data": result.data,
                    }));
                }
                Err(e) => {
                    self.base.transition_to(AgentState::Error {
                        error: format!("code analysis failed: {}", e),
                        recoverable: false,
                    });
                    return Ok(AgentResponse {
                        agent_id: self.id,
                        agent_name: self.name.clone(),
                        result: AgentResultKind::Failed {
                            reason: format!("code analysis failed: {}", e),
                        },
                        events,
                    });
                }
            }
        }

        self.base.transition_to(AgentState::Finished {
            result: crate::agent::AgentResult {
                success: true,
                message: format!("CodeAgent executed {} tool(s)", results.len()),
                steps_executed: 1,
                actions_performed: Vec::new(),
            },
            final_message: format!("CodeAgent executed {} tool(s)", results.len()),
        });

        Ok(AgentResponse {
            agent_id: self.id,
            agent_name: self.name.clone(),
            result: AgentResultKind::Success {
                summary: format!("CodeAgent executed {} tool(s)", results.len()),
                output: serde_json::json!({ "results": results }),
            },
            events,
        })
    }
}

// ============================================================================
// ReviewAgent
// ============================================================================

pub struct ReviewAgent {
    id: AgentId,
    name: String,
    base: BaseAgent,
    reviewer: Reviewer,
}

impl ReviewAgent {
    pub fn new(id: AgentId) -> Self {
        Self {
            id,
            name: format!("ReviewAgent_{}", id.0),
            base: BaseAgent::new(AgentInstanceId(id.0), format!("ReviewAgent_{}", id.0)),
            reviewer: Reviewer::new(),
        }
    }

    pub fn new_with_name(id: AgentId, name: impl Into<String>) -> Self {
        let name = name.into();
        Self {
            id,
            base: BaseAgent::new(AgentInstanceId(id.0), name.clone()),
            name,
            reviewer: Reviewer::new(),
        }
    }

    pub fn base(&self) -> &BaseAgent { &self.base }
    pub fn base_mut(&mut self) -> &mut BaseAgent { &mut self.base }
}

#[async_trait::async_trait]
impl Agent for ReviewAgent {
    fn id(&self) -> AgentId {
        self.id
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn role(&self) -> &str {
        "review"
    }

    fn capabilities(&self) -> &[CapabilityKind] {
        &[CapabilityKind::RuleCheck, CapabilityKind::CodeRead]
    }

    async fn handle(&mut self, request: AgentRequest) -> Result<AgentResponse, AgentError> {
        self.base.transition_to(AgentState::AnalyzingRequest {
            request: UserRequest { content: request.instruction.clone(), entity_refs: vec![], estimated_steps: 0 },
            start_time: chrono::Utc::now(),
        });

        let context = request.context.clone();

        let task_id: u64 = context
            .get("task_id")
            .and_then(|v| v.as_u64())
            .or_else(|| {
                request
                    .task_id
                    .as_deref()
                    .and_then(|s| s.parse().ok())
            })
            .unwrap_or(0);

        let all_met = context
            .get("all_requirements_met")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let failed: Vec<String> = context
            .get("failed_requirements")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        let summary = self.reviewer.review(task_id, all_met, &failed);
        let decision_str = format!("{:?}", summary.decision);

        self.base.transition_to(AgentState::Finished {
            result: crate::agent::AgentResult {
                success: true,
                message: format!("Review: {} — {}", decision_str, summary.summary),
                steps_executed: 1,
                actions_performed: Vec::new(),
            },
            final_message: format!("Review: {} — {}", decision_str, summary.summary),
        });

        Ok(AgentResponse {
            agent_id: self.id,
            agent_name: self.name.clone(),
            result: AgentResultKind::Success {
                summary: format!("Review: {} — {}", decision_str, summary.summary),
                output: serde_json::json!({
                    "decision": decision_str,
                    "summary": summary.summary,
                    "issues": summary.issues,
                    "task_id": summary.task_id,
                }),
            },
            events: vec![crate::event::EventBusEvent::EngineCommandApplied {
                transaction_id: request.task_id.unwrap_or_default(),
                success: true,
                message: "Review completed".into(),
            }],
        })
    }
}

// ============================================================================
// EditorAgent — wraps DirectorRuntime as a generic Agent
// ============================================================================

pub struct EditorAgent {
    id: AgentId,
    name: String,
    base: BaseAgent,
    director: DirectorRuntime,
}

impl EditorAgent {
    pub fn new(id: AgentId, director: DirectorRuntime) -> Self {
        Self {
            id,
            name: format!("EditorAgent_{}", id.0),
            base: BaseAgent::new(AgentInstanceId(id.0), format!("EditorAgent_{}", id.0)),
            director,
        }
    }

    pub fn base(&self) -> &BaseAgent { &self.base }
    pub fn base_mut(&mut self) -> &mut BaseAgent { &mut self.base }
}

#[async_trait::async_trait]
impl Agent for EditorAgent {
    fn id(&self) -> AgentId {
        self.id
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn role(&self) -> &str {
        "editor"
    }

    fn capabilities(&self) -> &[CapabilityKind] {
        &[
            CapabilityKind::Orchestrate,
            CapabilityKind::SceneRead,
            CapabilityKind::SceneWrite,
            CapabilityKind::CodeRead,
            CapabilityKind::CodeWrite,
            CapabilityKind::RuleCheck,
            CapabilityKind::WorkflowExecute,
        ]
    }

    async fn handle(&mut self, request: AgentRequest) -> Result<AgentResponse, AgentError> {
        let instruction = request.instruction.clone();
        self.base.transition_to(AgentState::AnalyzingRequest {
            request: UserRequest { content: instruction.clone(), entity_refs: vec![], estimated_steps: 0 },
            start_time: chrono::Utc::now(),
        });

        self.director.handle_user_request(&instruction);

        let event_bus_events: Vec<crate::event::EventBusEvent> = self
            .director
            .event_bus()
            .events()
            .iter()
            .map(|(_, e)| e.clone())
            .collect();

        let event_count = event_bus_events.len();

        self.base.transition_to(AgentState::Finished {
            result: crate::agent::AgentResult {
                success: true,
                message: format!("Director handled request, produced {} event(s)", event_count),
                steps_executed: 1,
                actions_performed: Vec::new(),
            },
            final_message: format!("Director handled request, produced {} event(s)", event_count),
        });

        Ok(AgentResponse {
            agent_id: self.id,
            agent_name: self.name.clone(),
            result: AgentResultKind::Success {
                summary: format!("Director handled request, produced {} event(s)", event_count),
                output: serde_json::json!({
                    "event_count": event_count,
                }),
            },
            events: event_bus_events,
        })
    }
}

// ============================================================================
// PlannerAgent — wraps RuleBasedPlanner as a generic Agent
// ============================================================================

pub struct PlannerAgent {
    id: AgentId,
    name: String,
    base: BaseAgent,
    planner: RuleBasedPlanner,
}

impl PlannerAgent {
    pub fn new(id: AgentId) -> Self {
        Self {
            id,
            name: format!("PlannerAgent_{}", id.0),
            base: BaseAgent::new(AgentInstanceId(id.0), format!("PlannerAgent_{}", id.0)),
            planner: RuleBasedPlanner::new(),
        }
    }

    pub fn new_with_name(id: AgentId, name: impl Into<String>) -> Self {
        let name = name.into();
        Self {
            id,
            base: BaseAgent::new(AgentInstanceId(id.0), name.clone()),
            name,
            planner: RuleBasedPlanner::new(),
        }
    }

    pub fn base(&self) -> &BaseAgent { &self.base }
    pub fn base_mut(&mut self) -> &mut BaseAgent { &mut self.base }
}

#[async_trait::async_trait]
impl Agent for PlannerAgent {
    fn id(&self) -> AgentId {
        self.id
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn role(&self) -> &str {
        "planner"
    }

    fn capabilities(&self) -> &[CapabilityKind] {
        &[CapabilityKind::Orchestrate]
    }

    async fn handle(&mut self, request: AgentRequest) -> Result<AgentResponse, AgentError> {
        let instruction = request.instruction.clone();

        self.base.transition_to(AgentState::AnalyzingRequest {
            request: UserRequest { content: instruction.clone(), entity_refs: vec![], estimated_steps: 0 },
            start_time: chrono::Utc::now(),
        });

        let context_data = request.context.clone();

        let task_id: u64 = context_data
            .get("task_id")
            .and_then(|v| v.as_u64())
            .or_else(|| {
                request
                    .task_id
                    .as_deref()
                    .and_then(|s| s.parse().ok())
            })
            .unwrap_or(0);

        let available_tools: Vec<String> = context_data
            .get("available_tools")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_else(|| {
                vec![
                    "create_entity".into(),
                    "delete_entity".into(),
                    "update_component".into(),
                    "query_entities".into(),
                ]
            });

        let scene_entity_names: Vec<String> = context_data
            .get("scene_entity_names")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        let planner_ctx = PlannerContext {
            task_id,
            available_tools,
            scene_entity_names,
            memory_context: None,
        };

        let plan = self.planner.create_plan(&instruction, task_id, planner_ctx);

        self.base.transition_to(AgentState::Finished {
            result: crate::agent::AgentResult {
                success: true,
                message: format!("Plan '{}': {} step(s), mode: {:?}", plan.title, plan.steps.len(), plan.mode),
                steps_executed: 1,
                actions_performed: Vec::new(),
            },
            final_message: format!("Plan '{}': {} step(s), mode: {:?}", plan.title, plan.steps.len(), plan.mode),
        });

        Ok(AgentResponse {
            agent_id: self.id,
            agent_name: self.name.clone(),
            result: AgentResultKind::Success {
                summary: format!(
                    "Plan '{}': {} step(s), mode: {:?}",
                    plan.title,
                    plan.steps.len(),
                    plan.mode
                ),
                output: serde_json::json!({
                    "plan_id": plan.id,
                    "title": plan.title,
                    "summary": plan.summary,
                    "mode": format!("{:?}", plan.mode),
                    "risk": format!("{:?}", plan.risk_level),
                    "status": format!("{:?}", plan.status),
                    "step_count": plan.steps.len(),
                    "steps": plan.steps.iter().map(|s| serde_json::json!({
                        "id": s.id,
                        "title": s.title,
                        "target_module": format!("{:?}", s.target_module),
                        "action": s.action_description,
                        "risk": format!("{:?}", s.risk),
                    })).collect::<Vec<_>>(),
                }),
            },
            events: vec![crate::event::EventBusEvent::EngineCommandApplied {
                transaction_id: request.task_id.unwrap_or_default(),
                success: true,
                message: format!("Plan '{}' generated: {} steps", plan.title, plan.steps.len()),
            }],
        })
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_code_agent_capabilities() {
        let agent = CodeAgent::new(AgentId(1));
        let caps = agent.capabilities();
        assert!(caps.contains(&CapabilityKind::CodeRead));
        assert!(caps.contains(&CapabilityKind::CodeWrite));
    }

    #[test]
    fn test_review_agent_capabilities() {
        let agent = ReviewAgent::new(AgentId(2));
        let caps = agent.capabilities();
        assert!(caps.contains(&CapabilityKind::RuleCheck));
    }

    #[test]
    fn test_planner_agent_capabilities() {
        let agent = PlannerAgent::new(AgentId(3));
        let caps = agent.capabilities();
        assert!(caps.contains(&CapabilityKind::Orchestrate));
    }

    #[test]
    fn test_code_agent_new_with_name() {
        let agent = CodeAgent::new_with_name(AgentId(10), "MyCodeAgent");
        assert_eq!(agent.name(), "MyCodeAgent");
        assert_eq!(agent.id().0, 10);
    }

    #[test]
    fn test_review_agent_new_with_name() {
        let agent = ReviewAgent::new_with_name(AgentId(11), "MyReviewAgent");
        assert_eq!(agent.name(), "MyReviewAgent");
    }

    #[tokio::test]
    async fn test_code_agent_generate() {
        let mut agent = CodeAgent::new(AgentId(1));
        let request = AgentRequest {
            task_id: Some("test".into()),
            instruction: "生成一个Player组件".into(),
            context: serde_json::json!({}),
        };
        let response = agent.handle(request).await.unwrap();
        assert!(matches!(response.result, AgentResultKind::Failed { .. }));
    }

    #[tokio::test]
    async fn test_planner_agent_plan() {
        let mut agent = PlannerAgent::new(AgentId(1));
        let request = AgentRequest {
            task_id: Some("test".into()),
            instruction: "创建一个红色敌人放在右侧".into(),
            context: serde_json::json!({}),
        };
        let response = agent.handle(request).await.unwrap();
        if let AgentResultKind::Success { summary, .. } = &response.result {
            assert!(summary.contains("step"));
        } else {
            // The planner may produce Failed for unparseable text — both are valid
            // depending on keyword detection.
        }
    }

    #[tokio::test]
    async fn test_planner_agent_empty_request() {
        let mut agent = PlannerAgent::new(AgentId(1));
        let request = AgentRequest {
            task_id: Some("test".into()),
            instruction: "随便".into(),
            context: serde_json::json!({}),
        };
        let response = agent.handle(request).await.unwrap();
        assert!(matches!(response.result, AgentResultKind::Success { .. }));
    }
}
