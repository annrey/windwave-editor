use crate::permission::{OperationRisk, PermissionDecision, PermissionEngine, PermissionRequirement};
use crate::registry::{AgentError, AgentId, AgentRegistry, AgentRequest, AgentResponse, CapabilityKind};
use crate::tool::{ToolCall, ToolError, ToolRegistry, ToolResult};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AgentSessionId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AgentRunId(pub u64);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AgentPlatformRole {
    User,
    Assistant,
    Tool,
    System,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentPlatformMessage {
    pub role: AgentPlatformRole,
    pub content: String,
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AgentPlatformEvent {
    SessionCreated { session_id: AgentSessionId },
    RunStarted { run_id: AgentRunId, session_id: AgentSessionId, request: String },
    AgentDispatched { run_id: AgentRunId, agent_id: AgentId, capability: Option<CapabilityKind> },
    ToolPlanned { run_id: AgentRunId, call_id: String, tool_name: String, risk: OperationRisk },
    PermissionRequired { run_id: AgentRunId, call_id: String, risk: OperationRisk, reason: String },
    PermissionResolved { run_id: AgentRunId, call_id: String, decision: PermissionDecision },
    ToolStarted { run_id: AgentRunId, call_id: String, tool_name: String },
    ToolFinished { run_id: AgentRunId, call_id: String, tool_name: String, success: bool, message: String },
    RunFinished { run_id: AgentRunId, success: bool, summary: String },
    RunFailed { run_id: AgentRunId, error: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AgentPlatformStatus {
    Idle,
    Running,
    WaitingForPermission,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingToolApproval {
    pub run_id: AgentRunId,
    pub call: ToolCall,
    pub risk: OperationRisk,
    pub reason: String,
}

#[derive(Debug, Clone)]
pub struct AgentPlatformConfig {
    pub max_iterations: usize,
    pub max_recent_events: usize,
    pub run_timeout: Duration,
    pub auto_dispatch_agents: bool,
}

impl Default for AgentPlatformConfig {
    fn default() -> Self {
        Self {
            max_iterations: 16,
            max_recent_events: 256,
            run_timeout: Duration::from_secs(300),
            auto_dispatch_agents: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSession {
    pub id: AgentSessionId,
    pub title: String,
    pub messages: Vec<AgentPlatformMessage>,
    pub metadata: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRunState {
    pub id: AgentRunId,
    pub session_id: AgentSessionId,
    pub status: AgentPlatformStatus,
    pub iterations: usize,
    pub pending_tools: Vec<ToolCall>,
    pub completed_tools: Vec<ToolResult>,
    pub assigned_agent: Option<AgentId>,
    pub summary: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentPlatformRunResult {
    pub run_id: AgentRunId,
    pub session_id: AgentSessionId,
    pub success: bool,
    pub summary: String,
    pub tool_results: Vec<ToolResult>,
    pub agent_response: Option<AgentResponse>,
    pub events: Vec<AgentPlatformEvent>,
}

#[derive(Debug, thiserror::Error)]
pub enum AgentPlatformError {
    #[error("session not found: {0:?}")]
    SessionNotFound(AgentSessionId),
    #[error("run not found: {0:?}")]
    RunNotFound(AgentRunId),
    #[error("permission required for tool call: {0}")]
    PermissionRequired(String),
    #[error("tool error: {0}")]
    Tool(String),
    #[error("agent error: {0}")]
    Agent(String),
    #[error("timeout")]
    Timeout,
}

impl From<ToolError> for AgentPlatformError {
    fn from(value: ToolError) -> Self {
        Self::Tool(value.to_string())
    }
}

impl From<AgentError> for AgentPlatformError {
    fn from(value: AgentError) -> Self {
        Self::Agent(value.to_string())
    }
}

pub struct AgentPlatform {
    pub config: AgentPlatformConfig,
    pub tools: ToolRegistry,
    pub agents: AgentRegistry,
    pub permissions: PermissionEngine,
    sessions: HashMap<AgentSessionId, AgentSession>,
    runs: HashMap<AgentRunId, AgentRunState>,
    pending_approvals: HashMap<String, PendingToolApproval>,
    events: VecDeque<AgentPlatformEvent>,
    next_session_id: u64,
    next_run_id: u64,
}

impl AgentPlatform {
    pub fn new(tools: ToolRegistry, agents: AgentRegistry, permissions: PermissionEngine) -> Self {
        Self {
            config: AgentPlatformConfig::default(),
            tools,
            agents,
            permissions,
            sessions: HashMap::new(),
            runs: HashMap::new(),
            pending_approvals: HashMap::new(),
            events: VecDeque::new(),
            next_session_id: 1,
            next_run_id: 1,
        }
    }

    pub fn with_config(mut self, config: AgentPlatformConfig) -> Self {
        self.config = config;
        self
    }

    pub fn create_session(&mut self, title: impl Into<String>) -> AgentSessionId {
        let id = AgentSessionId(self.next_session_id);
        self.next_session_id += 1;
        self.sessions.insert(
            id,
            AgentSession {
                id,
                title: title.into(),
                messages: Vec::new(),
                metadata: HashMap::new(),
            },
        );
        self.emit(AgentPlatformEvent::SessionCreated { session_id: id });
        id
    }

    pub fn session(&self, id: AgentSessionId) -> Option<&AgentSession> {
        self.sessions.get(&id)
    }

    pub fn run_state(&self, id: AgentRunId) -> Option<&AgentRunState> {
        self.runs.get(&id)
    }

    pub fn recent_events(&self, limit: usize) -> Vec<AgentPlatformEvent> {
        self.events.iter().rev().take(limit).cloned().collect()
    }

    pub async fn run_user_request(
        &mut self,
        session_id: AgentSessionId,
        request: impl Into<String>,
    ) -> Result<AgentPlatformRunResult, AgentPlatformError> {
        let request = request.into();
        let run_id = self.start_run(session_id, request.clone())?;
        let started = Instant::now();
        let mut agent_response = None;

        if self.config.auto_dispatch_agents && self.agents.agent_count() > 0 {
            let capability = self.infer_capability(&request);
            let agent_request = AgentRequest {
                task_id: Some(format!("run-{}", run_id.0)),
                instruction: request.clone(),
                context: serde_json::json!({
                    "session_id": session_id.0,
                    "run_id": run_id.0,
                    "available_tools": self.tools.list_tools(),
                }),
            };
            let response = self.agents.dispatch_by_capability(agent_request, capability.clone()).await?;
            self.emit(AgentPlatformEvent::AgentDispatched {
                run_id,
                agent_id: response.agent_id,
                capability: Some(capability),
            });
            if let Some(run) = self.runs.get_mut(&run_id) {
                run.assigned_agent = Some(response.agent_id);
            }
            agent_response = Some(response);
        }

        let planned_tools = self.plan_tool_calls(run_id, &request);
        let mut tool_results = Vec::new();

        for call in planned_tools {
            if started.elapsed() > self.config.run_timeout {
                self.fail_run(run_id, "timeout".to_string());
                return Err(AgentPlatformError::Timeout);
            }

            let risk = self.classify_tool_risk(&call);
            self.emit(AgentPlatformEvent::ToolPlanned {
                run_id,
                call_id: call.call_id.clone(),
                tool_name: call.tool_name.clone(),
                risk,
            });

            match self.permissions.decide_for_plan(risk) {
                PermissionRequirement::AutoApproved => {
                    let result = self.execute_tool(run_id, &call)?;
                    tool_results.push(result);
                }
                PermissionRequirement::NeedUserConfirmation { risk, reason } => {
                    self.pending_approvals.insert(
                        call.call_id.clone(),
                        PendingToolApproval {
                            run_id,
                            call: call.clone(),
                            risk,
                            reason: reason.clone(),
                        },
                    );
                    if let Some(run) = self.runs.get_mut(&run_id) {
                        run.status = AgentPlatformStatus::WaitingForPermission;
                        run.pending_tools.push(call.clone());
                    }
                    self.emit(AgentPlatformEvent::PermissionRequired {
                        run_id,
                        call_id: call.call_id.clone(),
                        risk,
                        reason,
                    });
                    return Err(AgentPlatformError::PermissionRequired(call.call_id));
                }
                PermissionRequirement::Forbidden { reason } => {
                    self.fail_run(run_id, reason.clone());
                    return Err(AgentPlatformError::Tool(reason));
                }
            }
        }

        let summary = self.build_summary(&request, &tool_results, agent_response.as_ref());
        self.finish_run(run_id, true, summary.clone(), tool_results.clone());
        self.add_message(session_id, AgentPlatformRole::Assistant, summary.clone(), serde_json::json!({}))?;

        Ok(AgentPlatformRunResult {
            run_id,
            session_id,
            success: true,
            summary,
            tool_results,
            agent_response,
            events: self.recent_events(self.config.max_recent_events),
        })
    }

    pub fn approve_tool_call(
        &mut self,
        call_id: &str,
    ) -> Result<ToolResult, AgentPlatformError> {
        let pending = self
            .pending_approvals
            .remove(call_id)
            .ok_or_else(|| AgentPlatformError::Tool(format!("pending tool call not found: {}", call_id)))?;
        self.emit(AgentPlatformEvent::PermissionResolved {
            run_id: pending.run_id,
            call_id: call_id.to_string(),
            decision: PermissionDecision::UserApproved,
        });
        let result = self.execute_tool(pending.run_id, &pending.call)?;
        if let Some(run) = self.runs.get_mut(&pending.run_id) {
            run.status = AgentPlatformStatus::Running;
            run.pending_tools.retain(|call| call.call_id != call_id);
        }
        Ok(result)
    }

    pub fn deny_tool_call(
        &mut self,
        call_id: &str,
        reason: impl Into<String>,
    ) -> Result<(), AgentPlatformError> {
        let reason = reason.into();
        let pending = self
            .pending_approvals
            .remove(call_id)
            .ok_or_else(|| AgentPlatformError::Tool(format!("pending tool call not found: {}", call_id)))?;
        self.emit(AgentPlatformEvent::PermissionResolved {
            run_id: pending.run_id,
            call_id: call_id.to_string(),
            decision: PermissionDecision::Denied { reason: reason.clone() },
        });
        self.fail_run(pending.run_id, reason);
        Ok(())
    }

    fn start_run(&mut self, session_id: AgentSessionId, request: String) -> Result<AgentRunId, AgentPlatformError> {
        if !self.sessions.contains_key(&session_id) {
            return Err(AgentPlatformError::SessionNotFound(session_id));
        }
        self.add_message(
            session_id,
            AgentPlatformRole::User,
            request.clone(),
            serde_json::json!({}),
        )?;
        let run_id = AgentRunId(self.next_run_id);
        self.next_run_id += 1;
        self.runs.insert(
            run_id,
            AgentRunState {
                id: run_id,
                session_id,
                status: AgentPlatformStatus::Running,
                iterations: 0,
                pending_tools: Vec::new(),
                completed_tools: Vec::new(),
                assigned_agent: None,
                summary: None,
            },
        );
        self.emit(AgentPlatformEvent::RunStarted { run_id, session_id, request });
        Ok(run_id)
    }

    fn add_message(
        &mut self,
        session_id: AgentSessionId,
        role: AgentPlatformRole,
        content: String,
        metadata: serde_json::Value,
    ) -> Result<(), AgentPlatformError> {
        let session = self
            .sessions
            .get_mut(&session_id)
            .ok_or(AgentPlatformError::SessionNotFound(session_id))?;
        session.messages.push(AgentPlatformMessage { role, content, metadata });
        Ok(())
    }

    fn execute_tool(&mut self, run_id: AgentRunId, call: &ToolCall) -> Result<ToolResult, AgentPlatformError> {
        self.emit(AgentPlatformEvent::ToolStarted {
            run_id,
            call_id: call.call_id.clone(),
            tool_name: call.tool_name.clone(),
        });
        let result = self.tools.execute(call)?;
        self.emit(AgentPlatformEvent::ToolFinished {
            run_id,
            call_id: call.call_id.clone(),
            tool_name: call.tool_name.clone(),
            success: result.success,
            message: result.message.clone(),
        });
        if let Some(run) = self.runs.get_mut(&run_id) {
            run.completed_tools.push(result.clone());
            run.iterations += 1;
        }
        Ok(result)
    }

    fn finish_run(&mut self, run_id: AgentRunId, success: bool, summary: String, tool_results: Vec<ToolResult>) {
        if let Some(run) = self.runs.get_mut(&run_id) {
            run.status = if success { AgentPlatformStatus::Completed } else { AgentPlatformStatus::Failed };
            run.completed_tools = tool_results;
            run.summary = Some(summary.clone());
        }
        self.emit(AgentPlatformEvent::RunFinished { run_id, success, summary });
    }

    fn fail_run(&mut self, run_id: AgentRunId, error: String) {
        if let Some(run) = self.runs.get_mut(&run_id) {
            run.status = AgentPlatformStatus::Failed;
            run.summary = Some(error.clone());
        }
        self.emit(AgentPlatformEvent::RunFailed { run_id, error });
    }

    fn emit(&mut self, event: AgentPlatformEvent) {
        self.events.push_back(event);
        while self.events.len() > self.config.max_recent_events {
            self.events.pop_front();
        }
    }

    fn infer_capability(&self, request: &str) -> CapabilityKind {
        let lower = request.to_lowercase();
        if lower.contains("scene") || lower.contains("entity") || lower.contains("场景") || lower.contains("实体") {
            CapabilityKind::SceneWrite
        } else if lower.contains("code") || lower.contains("代码") || lower.contains("script") {
            CapabilityKind::CodeWrite
        } else if lower.contains("asset") || lower.contains("资源") || lower.contains("texture") {
            CapabilityKind::AssetManage
        } else if lower.contains("review") || lower.contains("检查") || lower.contains("审查") {
            CapabilityKind::Review
        } else {
            CapabilityKind::Orchestrate
        }
    }

    fn plan_tool_calls(&self, run_id: AgentRunId, request: &str) -> Vec<ToolCall> {
        let lower = request.to_lowercase();
        let mut calls = Vec::new();
        if self.tools.has("query_scene") && (lower.contains("query") || lower.contains("list") || lower.contains("查询") || lower.contains("列出")) {
            calls.push(ToolCall {
                tool_name: "query_scene".to_string(),
                parameters: HashMap::new(),
                call_id: format!("run-{}-tool-{}", run_id.0, calls.len() + 1),
            });
        }
        if self.tools.has("echo") && calls.is_empty() {
            let mut parameters = HashMap::new();
            parameters.insert("message".to_string(), serde_json::Value::String(request.to_string()));
            calls.push(ToolCall {
                tool_name: "echo".to_string(),
                parameters,
                call_id: format!("run-{}-tool-{}", run_id.0, calls.len() + 1),
            });
        }
        calls
    }

    fn classify_tool_risk(&self, call: &ToolCall) -> OperationRisk {
        let name = call.tool_name.to_lowercase();
        if name.contains("delete") || name.contains("remove") || name.contains("rollback") {
            OperationRisk::HighRisk
        } else if name.contains("create") || name.contains("set") || name.contains("apply") || name.contains("write") || name.contains("spawn") {
            OperationRisk::MediumRisk
        } else {
            OperationRisk::Safe
        }
    }

    fn build_summary(
        &self,
        request: &str,
        tool_results: &[ToolResult],
        agent_response: Option<&AgentResponse>,
    ) -> String {
        let tool_summary = if tool_results.is_empty() {
            "no tools executed".to_string()
        } else {
            tool_results.iter().map(|r| r.summary()).collect::<Vec<_>>().join("; ")
        };
        let agent_summary = agent_response
            .map(|response| format!("agent={} result={:?}", response.agent_name, response.result))
            .unwrap_or_else(|| "no agent dispatched".to_string());
        format!("request='{}'; {}; {}", request, agent_summary, tool_summary)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::permission::PermissionEngine;
    use crate::registry::AgentRegistry;
    use crate::tool::{EchoTool, ToolRegistry};

    #[tokio::test]
    async fn platform_runs_echo_tool() {
        let mut tools = ToolRegistry::new();
        tools.register(EchoTool);
        let agents = AgentRegistry::new();
        let permissions = PermissionEngine::new();
        let mut platform = AgentPlatform::new(tools, agents, permissions);
        let session_id = platform.create_session("test");
        let result = platform.run_user_request(session_id, "hello").await.unwrap();
        assert!(result.success);
        assert_eq!(result.tool_results.len(), 1);
    }
}
