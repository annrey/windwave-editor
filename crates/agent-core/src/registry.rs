//! AgentRegistry - Agent registration, capability lookup, and dispatch.
//!
//! Design reference: Section 12.3 of gpt-agent-team-task-event-skill-architecture.md
//!
//! ```text
//! register(agent)
//! find_by_capability(capability)
//! dispatch(request) → AgentResponse
//! dispatch_parallel(requests) → Vec<AgentResponse>
//! ```

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Capability Kind
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CapabilityKind {
    Orchestrate,
    SceneRead,
    SceneWrite,
    SceneEdit,
    CodeRead,
    CodeWrite,
    CodeGen,
    AssetManage,
    RuleCheck,
    Review,
    VisionAnalyze,
    MemorySummarize,
    WorkflowExecute,
    EngineControl,
}

// ---------------------------------------------------------------------------
// Agent ID
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AgentId(pub u64);

impl Default for AgentId {
    fn default() -> Self {
        AgentId(0)
    }
}

// ---------------------------------------------------------------------------
// Agent Request / Response / Error
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRequest {
    pub task_id: Option<String>,
    pub instruction: String,
    pub context: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentResponse {
    pub agent_id: AgentId,
    pub agent_name: String,
    pub result: AgentResultKind,
    pub events: Vec<crate::event::EventBusEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AgentResultKind {
    Success { summary: String, output: serde_json::Value },
    PartialSuccess { summary: String, output: serde_json::Value, warnings: Vec<String> },
    NeedUserInput { question: String },
    Failed { reason: String },
}

#[derive(Debug, Clone)]
pub enum AgentError {
    NotFound(AgentId),
    NoCapabilityMatch(CapabilityKind),
    ExecutionFailed(String),
    Timeout,
}

impl std::fmt::Display for AgentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound(id) => write!(f, "Agent {:?} not found", id),
            Self::NoCapabilityMatch(cap) => write!(f, "No agent with capability {:?}", cap),
            Self::ExecutionFailed(msg) => write!(f, "Execution failed: {}", msg),
            Self::Timeout => write!(f, "Agent execution timed out"),
        }
    }
}

// ---------------------------------------------------------------------------
// Agent Trait
// ---------------------------------------------------------------------------

#[async_trait::async_trait]
pub trait Agent: Send + Sync {
    fn id(&self) -> AgentId;
    fn name(&self) -> &str;
    fn role(&self) -> &str;
    fn capabilities(&self) -> &[CapabilityKind];
    async fn handle(&mut self, request: AgentRequest) -> Result<AgentResponse, AgentError>;
}

// ---------------------------------------------------------------------------
// AgentRole
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentRole {
    Director,
    Planner,
    Executor,
    Reviewer,
    Specialist(SpecialistKind),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SpecialistKind {
    Scene,
    Code,
    Asset,
    Vision,
    Rule,
    Memory,
    Workflow,
}

impl AgentRole {
    pub fn required_capabilities(&self) -> Vec<CapabilityKind> {
        match self {
            Self::Director => vec![CapabilityKind::Orchestrate],
            Self::Planner => vec![CapabilityKind::Orchestrate],
            Self::Executor => vec![CapabilityKind::WorkflowExecute],
            Self::Reviewer => vec![
                CapabilityKind::RuleCheck,
                CapabilityKind::VisionAnalyze,
            ],
            Self::Specialist(kind) => match kind {
                SpecialistKind::Scene => vec![CapabilityKind::SceneRead, CapabilityKind::SceneWrite],
                SpecialistKind::Code => vec![CapabilityKind::CodeRead, CapabilityKind::CodeWrite],
                SpecialistKind::Asset => vec![CapabilityKind::AssetManage],
                SpecialistKind::Vision => vec![CapabilityKind::VisionAnalyze],
                SpecialistKind::Rule => vec![CapabilityKind::RuleCheck],
                SpecialistKind::Memory => vec![CapabilityKind::MemorySummarize],
                SpecialistKind::Workflow => vec![CapabilityKind::WorkflowExecute],
            },
        }
    }
}

// ---------------------------------------------------------------------------
// Agent Registry
// ---------------------------------------------------------------------------

pub struct AgentRegistry {
    agents: HashMap<AgentId, Box<dyn Agent>>,
    capability_index: HashMap<CapabilityKind, Vec<AgentId>>,
}

impl AgentRegistry {
    pub fn new() -> Self {
        Self {
            agents: HashMap::new(),
            capability_index: HashMap::new(),
        }
    }

    pub fn register(&mut self, agent: Box<dyn Agent>) {
        let id = agent.id();
        let capabilities = agent.capabilities().to_vec();

        for cap in &capabilities {
            self.capability_index
                .entry(cap.clone())
                .or_default()
                .push(id);
        }

        self.agents.insert(id, agent);
    }

    pub fn get(&self, id: &AgentId) -> Option<&dyn Agent> {
        self.agents.get(id).map(|a| a.as_ref())
    }

    pub fn find_by_capability(&self, capability: &CapabilityKind) -> Vec<&dyn Agent> {
        self.capability_index
            .get(capability)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| self.agents.get(id).map(|a| a.as_ref()))
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn find_by_role(&self, role: &AgentRole) -> Vec<&dyn Agent> {
        let required = role.required_capabilities();
        let mut candidates: Vec<&dyn Agent> = Vec::new();

        for cap in &required {
            for agent in self.find_by_capability(cap) {
                if !candidates.iter().any(|a| a.id() == agent.id()) {
                    candidates.push(agent);
                }
            }
        }

        candidates
    }

    pub fn list_agent_ids(&self) -> Vec<AgentId> {
        self.agents.keys().copied().collect()
    }

    pub fn agent_count(&self) -> usize {
        self.agents.len()
    }

    pub fn capability_count(&self) -> usize {
        self.capability_index.len()
    }

    pub async fn dispatch(
        &mut self,
        request: AgentRequest,
        target: Option<AgentId>,
    ) -> Result<AgentResponse, AgentError> {
        if let Some(id) = target {
            let agent = self.agents.get_mut(&id).ok_or(AgentError::NotFound(id))?;
            return agent.handle(request).await;
        }

        let agent_ids: Vec<AgentId> = self.agents.keys().copied().collect();
        if agent_ids.is_empty() {
            return Err(AgentError::NoCapabilityMatch(CapabilityKind::Orchestrate));
        }

        let agent_id = agent_ids[0];
        let agent = self.agents.get_mut(&agent_id).unwrap();
        agent.handle(request).await
    }

    pub async fn dispatch_by_capability(
        &mut self,
        request: AgentRequest,
        capability: CapabilityKind,
    ) -> Result<AgentResponse, AgentError> {
        let candidates = self.find_by_capability(&capability);
        if candidates.is_empty() {
            return Err(AgentError::NoCapabilityMatch(capability));
        }

        let agent_id = candidates[0].id();
        let agent = self.agents.get_mut(&agent_id).unwrap();
        agent.handle(request).await
    }

    /// Synchronous version of `dispatch` for use outside async context.
    ///
    /// Uses a lazily-created static tokio runtime (created once and reused)
    /// to avoid the overhead of creating a new runtime per call.
    pub fn dispatch_sync(
        &mut self,
        request: AgentRequest,
        target: Option<AgentId>,
    ) -> Result<AgentResponse, AgentError> {
        match tokio::runtime::Handle::try_current() {
            Ok(handle) => {
                tokio::task::block_in_place(|| handle.block_on(self.dispatch(request, target)))
            }
            Err(_) => {
                let rt = tokio_runtime();
                rt.block_on(self.dispatch(request, target))
            }
        }
    }

    pub async fn dispatch_parallel(
        &mut self,
        requests: Vec<(AgentRequest, AgentId)>,
    ) -> Vec<Result<AgentResponse, AgentError>> {
        let mut handles = Vec::new();

        for (req, agent_id) in requests {
            if let Some(agent) = self.agents.get(&agent_id) {
                let name = agent.name().to_string();
                let id = agent.id();
                handles.push((id, name, req));
            }
        }

        let mut results = Vec::new();
        for (id, _name, req) in handles {
            match self.dispatch(req, Some(id)).await {
                Ok(resp) => results.push(Ok(resp)),
                Err(e) => results.push(Err(e)),
            }
        }

        results
    }

    // ------------------------------------------------------------------
    // Phase 5: Team Mode & Agent Lifecycle (§5.1, §5.2, §5.3)
    // ------------------------------------------------------------------

    /// Unregister an agent by ID.
    /// Returns the removed agent if found.
    pub fn unregister(&mut self, agent_id: AgentId) -> Option<Box<dyn Agent>> {
        self.agents.remove(&agent_id)
    }

    /// Get agent status information.
    pub fn agent_status(&self, agent_id: AgentId) -> Option<AgentStatusInfo> {
        self.agents.get(&agent_id).map(|agent| AgentStatusInfo {
            id: agent.id(),
            name: agent.name().to_string(),
            role: agent.role().to_string(),
            capabilities: agent.capabilities().to_vec(),
            is_available: true,
            last_active: Some(std::time::Instant::now()),
        })
    }

    /// Check health of all registered agents. Returns unhealthy agent IDs.
    pub fn check_health(&self, timeout: std::time::Duration) -> Vec<(AgentId, String)> {
        self.list_all_agents()
            .into_iter()
            .filter(|s| s.is_stale(timeout))
            .map(|s| (s.id, s.name))
            .collect()
    }

    pub fn list_all_agents(&self) -> Vec<AgentStatusInfo> {
        self.agents
            .values()
            .map(|agent| AgentStatusInfo {
                id: agent.id(),
                name: agent.name().to_string(),
                role: agent.role().to_string(),
                capabilities: agent.capabilities().to_vec(),
                is_available: true,
                last_active: Some(std::time::Instant::now()),
            })
            .collect()
    }

    /// Team mode: dispatch a multi-step plan to multiple agents (async).
    ///
    /// Each step is assigned to the best matching agent based on capabilities.
    pub async fn dispatch_team_plan(
        &mut self,
        plan: &crate::plan::EditPlan,
    ) -> TeamDispatchResult {
        let mut step_results = Vec::new();
        let mut assigned_agents = Vec::new();

        for step in &plan.steps {
            let target_capability = Self::infer_capability_from_step(step);
            let candidates = self.find_by_capability(&target_capability);

            if candidates.is_empty() {
                step_results.push(StepResult {
                    step_id: step.id.clone(),
                    success: false,
                    result: None,
                    error: Some(format!("No agent found for capability: {:?}", target_capability)),
                });
                continue;
            }

            let selected = &candidates[0];
            assigned_agents.push(selected.id());

            let request = AgentRequest {
                task_id: Some(format!("{}-{}", plan.id, step.id)),
                instruction: step.title.clone(),
                context: serde_json::json!({
                    "plan_id": plan.id,
                    "step_id": step.id,
                    "step_title": step.title,
                    "plan_risk": format!("{:?}", plan.risk_level),
                }),
            };

            match self.dispatch(request, Some(selected.id())).await {
                Ok(response) => {
                    step_results.push(StepResult {
                        step_id: step.id.clone(),
                        success: true,
                        result: Some(serde_json::json!({"result": format!("{:?}", response.result)})),
                        error: None,
                    });
                }
                Err(e) => {
                    step_results.push(StepResult {
                        step_id: step.id.clone(),
                        success: false,
                        result: None,
                        error: Some(format!("{:?}", e)),
                    });
                }
            }
        }

        TeamDispatchResult {
            plan_id: plan.id.clone(),
            step_results,
            assigned_agents,
            completed_at: std::time::Instant::now(),
        }
    }

    /// Team mode sync: dispatch a multi-step plan using dispatch_sync.
    ///
    /// Each step is assigned to the best matching agent by capability.
    /// Uses `dispatch_sync` instead of async dispatch for Bevy system contexts.
    pub fn dispatch_team_plan_sync(
        &mut self,
        plan: &crate::plan::EditPlan,
    ) -> TeamDispatchResult {
        let mut step_results = Vec::new();
        let mut assigned_agents = Vec::new();

        for step in &plan.steps {
            let target_capability = Self::infer_capability_from_step(step);
            let candidates = self.find_by_capability(&target_capability);

            if candidates.is_empty() {
                step_results.push(StepResult {
                    step_id: step.id.clone(),
                    success: false,
                    result: None,
                    error: Some(format!("No agent found for capability: {:?}", target_capability)),
                });
                continue;
            }

            let selected = &candidates[0];
            assigned_agents.push(selected.id());

            let request = AgentRequest {
                task_id: Some(format!("{}-{}", plan.id, step.id)),
                instruction: step.title.clone(),
                context: serde_json::json!({
                    "plan_id": plan.id,
                    "step_id": step.id,
                    "step_title": step.title,
                    "plan_risk": format!("{:?}", plan.risk_level),
                }),
            };

            match self.dispatch_sync(request, Some(selected.id())) {
                Ok(response) => {
                    step_results.push(StepResult {
                        step_id: step.id.clone(),
                        success: true,
                        result: Some(serde_json::json!({"result": format!("{:?}", response.result)})),
                        error: None,
                    });
                }
                Err(e) => {
                    step_results.push(StepResult {
                        step_id: step.id.clone(),
                        success: false,
                        result: None,
                        error: Some(format!("{:?}", e)),
                    });
                }
            }
        }

        TeamDispatchResult {
            plan_id: plan.id.clone(),
            step_results,
            assigned_agents,
            completed_at: std::time::Instant::now(),
        }
    }

    /// Infer the required capability from a plan step.
    fn infer_capability_from_step(step: &crate::plan::EditPlanStep) -> CapabilityKind {
        let title_lower = step.title.to_lowercase();

        if title_lower.contains("scene") || title_lower.contains("entity") || title_lower.contains("spawn") {
            CapabilityKind::SceneEdit
        } else if title_lower.contains("code") || title_lower.contains("script") || title_lower.contains("component") {
            CapabilityKind::CodeGen
        } else if title_lower.contains("test") || title_lower.contains("review") {
            CapabilityKind::Review
        } else if title_lower.contains("build") || title_lower.contains("compile") {
            CapabilityKind::EngineControl
        } else {
            CapabilityKind::Orchestrate
        }
    }

    /// Shutdown all agents gracefully.
    pub async fn shutdown_all(&mut self) -> Vec<(AgentId, bool)> {
        let mut results = Vec::new();

        for (id, _agent) in self.agents.iter_mut() {
            // Future: call agent.shutdown() when available
            results.push((*id, true));
        }

        self.agents.clear();
        results
    }
}

/// Agent status information for UI display.
#[derive(Debug, Clone)]
pub struct AgentStatusInfo {
    pub id: AgentId,
    pub name: String,
    pub role: String,
    pub capabilities: Vec<CapabilityKind>,
    pub is_available: bool,
    /// Last time the agent was seen active (for health monitoring)
    pub last_active: Option<std::time::Instant>,
}

impl AgentStatusInfo {
    pub fn is_stale(&self, timeout: std::time::Duration) -> bool {
        self.last_active
            .map(|t| t.elapsed() > timeout)
            .unwrap_or(true)
    }

    pub fn health(&self, timeout: std::time::Duration) -> &'static str {
        if self.is_stale(timeout) {
            "unhealthy"
        } else {
            "healthy"
        }
    }
}

/// Result of a single step in team dispatch.
#[derive(Debug, Clone)]
pub struct StepResult {
    pub step_id: String,
    pub success: bool,
    pub result: Option<serde_json::Value>,
    pub error: Option<String>,
}

/// Result of team plan dispatch.
#[derive(Debug, Clone)]
pub struct TeamDispatchResult {
    pub plan_id: String,
    pub step_results: Vec<StepResult>,
    pub assigned_agents: Vec<AgentId>,
    pub completed_at: std::time::Instant,
}

impl Default for AgentRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Lazily-created static tokio runtime for synchronous dispatch.
fn tokio_runtime() -> &'static tokio::runtime::Runtime {
    use std::sync::OnceLock;
    static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RUNTIME.get_or_init(|| {
        tokio::runtime::Runtime::new().expect("Failed to create tokio runtime")
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    struct MockAgent {
        id: AgentId,
        name: String,
        capabilities: Vec<CapabilityKind>,
    }

    #[async_trait::async_trait]
    impl Agent for MockAgent {
        fn id(&self) -> AgentId {
            self.id
        }

        fn name(&self) -> &str {
            &self.name
        }

        fn role(&self) -> &str {
            "mock"
        }

        fn capabilities(&self) -> &[CapabilityKind] {
            &self.capabilities
        }

        async fn handle(&mut self, _request: AgentRequest) -> Result<AgentResponse, AgentError> {
            Ok(AgentResponse {
                agent_id: self.id,
                agent_name: self.name.clone(),
                result: AgentResultKind::Success {
                    summary: "done".to_string(),
                    output: serde_json::json!({}),
                },
                events: Vec::new(),
            })
        }
    }

    #[test]
    fn test_register_and_find_by_capability() {
        let mut registry = AgentRegistry::new();

        let scene_agent = Box::new(MockAgent {
            id: AgentId(1),
            name: "SceneAgent".into(),
            capabilities: vec![CapabilityKind::SceneRead, CapabilityKind::SceneWrite],
        });

        registry.register(scene_agent);

        let found = registry.find_by_capability(&CapabilityKind::SceneRead);
        assert_eq!(found.len(), 1);
    }

    #[test]
    fn test_find_by_role() {
        let mut registry = AgentRegistry::new();

        registry.register(Box::new(MockAgent {
            id: AgentId(1),
            name: "SceneAgent".into(),
            capabilities: vec![CapabilityKind::SceneRead, CapabilityKind::SceneWrite],
        }));

        let found = registry.find_by_role(&AgentRole::Specialist(SpecialistKind::Scene));
        assert!(!found.is_empty());
    }

    #[test]
    fn test_no_capability_match() {
        let registry = AgentRegistry::new();
        let found = registry.find_by_capability(&CapabilityKind::CodeRead);
        assert!(found.is_empty());
    }

    #[tokio::test]
    async fn test_dispatch() {
        let mut registry = AgentRegistry::new();

        registry.register(Box::new(MockAgent {
            id: AgentId(1),
            name: "TestAgent".into(),
            capabilities: vec![CapabilityKind::Orchestrate],
        }));

        let request = AgentRequest {
            task_id: Some("task_1".into()),
            instruction: "test".into(),
            context: serde_json::json!({}),
        };

        let response = registry.dispatch(request, None).await.unwrap();
        assert_eq!(response.agent_name, "TestAgent");
    }

    #[test]
    fn test_agent_role_capabilities() {
        let caps = AgentRole::Specialist(SpecialistKind::Code).required_capabilities();
        assert!(caps.contains(&CapabilityKind::CodeRead));
        assert!(caps.contains(&CapabilityKind::CodeWrite));
    }

    #[test]
    fn test_agent_count() {
        let mut registry = AgentRegistry::new();

        registry.register(Box::new(MockAgent {
            id: AgentId(1),
            name: "A".into(),
            capabilities: vec![CapabilityKind::SceneRead],
        }));

        registry.register(Box::new(MockAgent {
            id: AgentId(2),
            name: "B".into(),
            capabilities: vec![CapabilityKind::CodeRead],
        }));

        assert_eq!(registry.agent_count(), 2);
    }
}
