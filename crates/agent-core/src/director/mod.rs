//! DirectorRuntime - Central orchestrator for the Agent team system
//!
//! Ties together Planner, Permission, Executor, GoalChecker, and Reviewer
//! into a single command-driven runtime.

pub mod types;
pub mod events;
pub mod goals;
pub mod plans;
pub mod execution;
pub mod plan_manager;

// Re-export all public types so existing code doesn't break
pub use types::*;
pub use plan_manager::PlanManager;

use crate::plan::{EditPlan, EditPlanStep};
use crate::types::now_millis;
use crate::scene_bridge::SceneBridge;
use crate::event::EventBus;
use crate::skill::{SkillRegistry, SkillExecutor};
use crate::rollback::RollbackManager;
use crate::rollback::SnapshotEntity;
use crate::fallback::FallbackEngine;
use crate::metrics::AgentMetrics;
use crate::prompt::PromptSystem;
use crate::registry::AgentRegistry;
use crate::registry::AgentId;
use crate::specialized_agents::{CodeAgent, ReviewAgent, PlannerAgent};
use crate::scene_agent::SceneAgent;
use crate::agent::AgentInstanceId;
use crate::planner::{Planner, RuleBasedPlanner};

/// Default ReAct system prompt for editor operations (Sprint 1).
const REACT_SYSTEM_PROMPT: &str = r#"You are a helpful AI assistant that can interact with a game editor.
You have access to various tools to manipulate the scene, create entities, and modify components.

When responding, you MUST follow this format:

Thought: [Your reasoning about what needs to be done]
Action: [Tool name and parameters in JSON format]

OR if you have a final answer:

Thought: [Brief reasoning]
Final Answer: [Your response to the user]

Available tools:
- create_entity: Create a new entity with optional name, position, and component patches
- update_component: Update a component on an existing entity
- delete_entity: Delete an entity by ID
- query_entities: Query entities with optional filters

Always think step by step. If you're unsure about an entity ID, query first.
If an action fails, try to understand why and retry with corrected parameters.
"#;

impl DirectorRuntime {
    // ------------------------------------------------------------------
    // Constructor
    // ------------------------------------------------------------------

    /// Creates a new `DirectorRuntime` with no plans and no events.
    ///
    /// # Example
    ///
    /// ```rust
    /// use agent_core::director::DirectorRuntime;
    /// let runtime = DirectorRuntime::new();
    /// assert!(runtime.list_plans().is_empty());
    /// assert!(!runtime.has_pending_approvals());
    /// ```
    pub fn new() -> Self {
        // Try to auto-detect LLM configuration from environment
        let llm_config = crate::llm::config_from_env();
        
        // Initialize ReActAgent and LLM client (Sprint 1: LLM闭环执行)
        // We create the LLM client once and share it between react_agent and llm_client
        let (llm_client, react_agent) = if let Some(config) = llm_config {
            match crate::llm::create_llm_client(config) {
                Ok(client) => {
                    use crate::agent::BaseAgent;
                    use crate::strategy::{ReActConfig, create_react_agent};
                    use std::sync::Arc;
                    
                    eprintln!("[DirectorRuntime] LLM client auto-configured from environment");
                    
                    let base = BaseAgent::new(AgentInstanceId(0), "ReAct");
                    let tool_registry = Arc::new(std::sync::Mutex::new(crate::tool::ToolRegistry::new()));
                    
                    let _config = ReActConfig {
                        max_steps: 20,
                        temperature: 0.3,
                        include_observations: true,
                        system_prompt: REACT_SYSTEM_PROMPT.to_string(),
                    };
                    
                    // Sprint 1: Create layered context for L0-L3 prompt enrichment
                    let mut layered = crate::prompt::LayeredContext::default();
                    // L0: System context
                    layered.l0_system = crate::prompt::L0SystemContext::default_bevy();
                    // L1: Session context
                    layered.l1_session.project_name = "AgentEdit".to_string();
                    layered.l1_session.engine_version = "0.17".to_string();
                    // L2: Task context (will be updated per-request)
                    layered.l2_task.current_task = "Awaiting user request".to_string();
                    // Add few-shot examples
                    layered.add_few_shot(crate::prompt::FewShotExample::create_entity_example());
                    layered.add_few_shot(crate::prompt::FewShotExample::update_component_example());
                    layered.add_few_shot(crate::prompt::FewShotExample::query_entities_example());
                    
                    // Create Arc from Box - this consumes the Box
                    let client_arc: Arc<dyn crate::llm::LlmClient> = Arc::from(client);
                    let react = create_react_agent(base, client_arc.clone(), tool_registry)
                        .with_layered_context(layered);
                    
                    // Store the Arc as Box for backward compatibility
                    // Note: This works because Arc::from(Box) gives us Arc<Box<dyn LlmClient>>
                    // We need to convert it back. Since we can't clone the trait object,
                    // we'll store None for llm_client and rely on react_agent having the client.
                    // The react_agent already has Arc<dyn LlmClient>.
                    (None, Some(react))
                }
                Err(e) => {
                    eprintln!("[DirectorRuntime] LLM not available ({}), using fallback", e);
                    (None, None)
                }
            }
        } else {
            (None, None)
        };

        let planner: Box<dyn Planner> = Box::new(RuleBasedPlanner::new());
        let plan_manager = PlanManager::new(planner);

        Self {
            plan_manager,
            events: Vec::new(),
            trace_entries: Vec::new(),
            goal_checker_enabled: false,
            scene_bridge: None,
            event_bus: EventBus::new(),
            skill_registry: SkillRegistry::new(),
            skill_executor: SkillExecutor::new(),
            rollback_manager: RollbackManager::new(50),
            fallback_engine: FallbackEngine::new(),
            metrics: AgentMetrics::new(),
            prompt_system: PromptSystem::with_defaults(),
            agent_registry: Some(Self::init_internal_agents()),
            llm_client,
            comm_hub: crate::agent_comm::CommunicationHub::new(),
            edit_history: crate::edit_history::EditHistory::new(50),
            audit_log: crate::audit::AuditLog::new(10_000),
            react_agent,
            memory_system: crate::memory::MemorySystem::default(),
            memory_injector: crate::memory_injector::MemoryInjector::new(None),
        }
    }

    // ------------------------------------------------------------------
    // Config / setup
    // ------------------------------------------------------------------

    /// Enable GoalChecker validation (for Phase 2 integration).
    pub fn enable_goal_checker(&mut self) {
        self.goal_checker_enabled = true;
    }

    /// Initialize the built-in skill library.
    ///
    /// Registers predefined skills (create_entity, modify_transform, query_scene, import_asset)
    /// into the internal `SkillRegistry`. Call once at startup.
    pub fn init_builtin_skills(&mut self) {
        self.skill_registry.register(crate::builtin_skills::create_entity_skill());
        self.skill_registry.register(crate::builtin_skills::modify_entity_transform_skill());
        self.skill_registry.register(crate::builtin_skills::query_scene_skill());
        self.skill_registry.register(crate::builtin_skills::import_asset_skill());
    }

    /// Look up a matching skill by step title text.
    pub fn lookup_skill_for_step(&self, step: &EditPlanStep) -> Option<crate::skill::SkillDefinition> {
        let skill_names = self.skill_registry.list_skill_names();
        let title_lower = step.title.to_lowercase();

        // Exact match first
        for &name in &skill_names {
            if title_lower.contains(&name.to_lowercase()) {
                return self.skill_registry.find_by_name(name).cloned();
            }
        }

        // Fuzzy: keyword-based
        if title_lower.contains("创建") || title_lower.contains("create") {
            return self.skill_registry.find_by_name("create_entity").cloned();
        }
        if title_lower.contains("移动") || title_lower.contains("move")
            || title_lower.contains("变换") || title_lower.contains("transform")
        {
            return self.skill_registry.find_by_name("modify_entity_transform").cloned();
        }
        if title_lower.contains("查询") || title_lower.contains("query")
            || title_lower.contains("列表") || title_lower.contains("list")
        {
            return self.skill_registry.find_by_name("query_scene").cloned();
        }
        if title_lower.contains("导入") || title_lower.contains("import")
            || title_lower.contains("资源") || title_lower.contains("asset")
        {
            return self.skill_registry.find_by_name("import_asset").cloned();
        }

        None
    }

    // ------------------------------------------------------------------
    // SceneBridge / AgentRegistry injection
    // ------------------------------------------------------------------

    /// Inject a SceneBridge for real engine operations.
    /// Without this, scene operations will be simulated (MVP mode).
    pub fn set_scene_bridge(&mut self, bridge: Box<dyn SceneBridge>) {
        self.scene_bridge = Some(bridge);
    }

    /// Check if a SceneBridge is available for real engine execution.
    pub fn has_scene_bridge(&self) -> bool {
        self.scene_bridge.is_some()
    }

    /// Inject an AgentRegistry for Team mode dispatch.
    /// Overwrites any existing registry (including the internal one).
    pub fn set_agent_registry(&mut self, registry: AgentRegistry) {
        self.agent_registry = Some(registry);
    }

    /// Initialize the internal agent team (Scene, Code, Review, Planner).
    ///
    /// Called automatically during `DirectorRuntime::new()`.
    /// These agents handle capability-specific work in Team mode.
    pub fn init_internal_agents() -> AgentRegistry {
        let mut registry = AgentRegistry::new();

        let scene_agent = SceneAgent::new(AgentId(1));
        let code_agent = CodeAgent::new(AgentId(2));
        let review_agent = ReviewAgent::new(AgentId(3));
        let planner_agent = PlannerAgent::new(AgentId(4));

        registry.register(Box::new(scene_agent));
        registry.register(Box::new(code_agent));
        registry.register(Box::new(review_agent));
        registry.register(Box::new(planner_agent));

        registry
    }

    /// Check if an AgentRegistry is available for Team mode.
    pub fn has_agent_registry(&self) -> bool {
        self.agent_registry.is_some()
    }

    /// Drain accumulated engine commands from the current SceneBridge.
    ///
    /// Returns JSON-serialized commands. Call after execution to collect
    /// pending write operations for application to the real ECS World.
    pub fn drain_bridge_commands(&mut self) -> Vec<serde_json::Value> {
        match self.scene_bridge.as_mut() {
            Some(bridge) => bridge.drain_commands(),
            None => vec![],
        }
    }

    // ------------------------------------------------------------------
    // Undo / Redo API (Phase 3: Real Undo/Redo Operations)
    // ------------------------------------------------------------------

    /// Perform undo operation.
    ///
    /// Returns true if undo was successful, false if nothing to undo.
    pub fn undo(&mut self) -> bool {
        if !self.rollback_manager.can_undo() {
            return false;
        }

        if let Some(op) = self.rollback_manager.undo() {
            self.trace_entries.push(DirectorTraceEntry {
                timestamp_ms: now_millis(),
                actor: "RollbackManager".into(),
                summary: format!("Undo operation {} ({:?})", op.id.0, op.operation_type),
            });

            self.events.push(EditorEvent::TransactionRolledBack {
                transaction_id: format!("undo_{}", op.id.0),
            });

            return true;
        }

        false
    }

    /// Perform redo operation.
    ///
    /// Returns true if redo was successful, false if nothing to redo.
    pub fn redo(&mut self) -> bool {
        if !self.rollback_manager.can_redo() {
            return false;
        }

        if let Some(op) = self.rollback_manager.redo() {
            self.trace_entries.push(DirectorTraceEntry {
                timestamp_ms: now_millis(),
                actor: "RollbackManager".into(),
                summary: format!("Redo operation {} ({:?})", op.id.0, op.operation_type),
            });

            self.events.push(EditorEvent::TransactionCommitted {
                transaction_id: format!("redo_{}", op.id.0),
            });

            return true;
        }

        false
    }

    /// Check if undo is available.
    pub fn can_undo(&self) -> bool {
        self.rollback_manager.can_undo()
    }

    /// Check if redo is available.
    pub fn can_redo(&self) -> bool {
        self.rollback_manager.can_redo()
    }

    /// Access the RollbackManager (undo/redo stacks).
    pub fn rollback(&self) -> &RollbackManager {
        &self.rollback_manager
    }

    /// Mutable access to RollbackManager.
    pub fn rollback_mut(&mut self) -> &mut RollbackManager {
        &mut self.rollback_manager
    }

    // ------------------------------------------------------------------
    // LLM API (Phase 1: Real LLM Integration)
    // ------------------------------------------------------------------

    /// Check if LLM client is available.
    pub fn has_llm(&self) -> bool {
        self.llm_client.as_ref().map(|c| c.is_ready()).unwrap_or(false)
    }

    /// Check if ReActAgent is available for LLM-driven execution (Sprint 1).
    pub fn has_react_agent(&self) -> bool {
        self.react_agent.is_some()
    }

    /// Get LLM status information.
    pub fn llm_status(&self) -> LlmStatus {
        match &self.llm_client {
            Some(client) => {
                if client.is_ready() {
                    LlmStatus::Ready {
                        provider: format!("{:?}", client.provider()),
                    }
                } else {
                    LlmStatus::NotReady
                }
            }
            None => LlmStatus::NotConfigured,
        }
    }

    /// Set or replace the LLM client.
    pub fn set_llm_client(&mut self, client: Box<dyn crate::llm::LlmClient>) {
        self.llm_client = Some(client);
    }

    /// Remove the LLM client (force fallback mode).
    pub fn disable_llm(&mut self) {
        self.llm_client = None;
    }

    /// Set or replace the planner implementation.
    pub fn set_planner(&mut self, planner: Box<dyn Planner>) {
        self.plan_manager.set_planner(planner);
    }

    /// Access the FallbackEngine for LLM-unavailable scenarios.
    pub fn fallback(&self) -> &FallbackEngine {
        &self.fallback_engine
    }

    /// Access the AgentMetrics for performance dashboards.
    pub fn metrics(&self) -> &AgentMetrics {
        &self.metrics
    }

    /// Access the PromptSystem for prompt engineering.
    pub fn prompt(&self) -> &PromptSystem {
        &self.prompt_system
    }

    /// Access the CommunicationHub for inter-agent messaging.
    pub fn comm_hub(&self) -> &crate::agent_comm::CommunicationHub {
        &self.comm_hub
    }

    /// Access the EditHistory for fine-grained undo/redo.
    pub fn edit_history(&self) -> &crate::edit_history::EditHistory {
        &self.edit_history
    }

    /// Access the EditHistory mutably for fine-grained undo/redo operations.
    pub fn edit_history_mut(&mut self) -> &mut crate::edit_history::EditHistory {
        &mut self.edit_history
    }

    /// Push an executed EditOp onto the undo history.
    pub fn push_edit_op(&mut self, op: Box<dyn crate::edit_ops::EditOp>) {
        self.edit_history.push(op);
    }

    /// Undo the most recent edit operation via the EditHistory.
    pub fn undo_edit(&mut self) -> Result<bool, crate::edit_ops::EditOpError> {
        if let Some(ref mut bridge) = self.scene_bridge {
            self.edit_history.undo(bridge.as_mut())
        } else {
            Err(crate::edit_ops::EditOpError::Bridge("No SceneBridge connected".into()))
        }
    }

    /// Redo the most recently undone operation via the EditHistory.
    pub fn redo_edit(&mut self) -> Result<bool, crate::edit_ops::EditOpError> {
        if let Some(ref mut bridge) = self.scene_bridge {
            self.edit_history.redo(bridge.as_mut())
        } else {
            Err(crate::edit_ops::EditOpError::Bridge("No SceneBridge connected".into()))
        }
    }

    /// Record an operation to the tamper-evident audit log.
    pub fn record_audit(
        &mut self,
        agent_id: u64,
        action: &str,
        target: &str,
        result: &str,
        risk_level: &str,
        user_approved: bool,
    ) {
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        self.audit_log.record(ts, agent_id, action, target, result, risk_level, user_approved);
    }

    /// Verify the integrity of the audit log chain.
    pub fn verify_audit_log(&self) -> bool {
        self.audit_log.verify()
    }

    /// Access the audit log for read-only inspection.
    pub fn audit_log(&self) -> &crate::audit::AuditLog {
        &self.audit_log
    }

    // ------------------------------------------------------------------
    // Plan query API
    // ------------------------------------------------------------------

    /// Look up a plan by its ID.
    ///
    /// Returns `None` if no plan with the given ID exists.
    ///
    /// # Arguments
    ///
    /// * `plan_id` - The plan ID to look up.
    pub fn get_plan(&self, plan_id: &str) -> Option<&EditPlan> {
        self.plan_manager.get(plan_id)
    }

    /// List all plans currently managed by the runtime.
    ///
    /// Returns an empty `Vec` if there are no plans.
    pub fn list_plans(&self) -> Vec<&EditPlan> {
        self.plan_manager.list()
    }

    /// Check whether there are any plans waiting for user approval.
    pub fn has_pending_approvals(&self) -> bool {
        self.plan_manager.has_pending_approvals()
    }

    /// Return the total step count of the most recently created plan.
    ///
    /// Returns 0 if no plans have been created yet.
    pub fn current_plan_step_count(&self) -> usize {
        self.plan_manager.current_step_count()
    }

    /// Get the list of plan IDs currently waiting for approval.
    pub fn pending_approval_ids(&self) -> Vec<String> {
        self.plan_manager.pending_approval_ids()
    }

    /// Rollback a committed transaction by its ID.
    ///
    /// Restores the scene state from the snapshot stored in the
    /// `RollbackManager`'s undo log. When a `SceneBridge` is connected,
    /// the method deletes entities not present in the snapshot and
    /// recreates / updates entities to match the snapshot state.
    ///
    /// # Arguments
    ///
    /// * `transaction_id` - The ID of the transaction to roll back.
    pub fn rollback_transaction(&mut self, transaction_id: &str) -> Vec<EditorEvent> {
        let mut snapshot_entities: Vec<SnapshotEntity> = Vec::new();

        if self.rollback_manager.can_undo() {
            if let Some(op) = self.rollback_manager.undo() {
                snapshot_entities = op.snapshot.entities.clone();

                self.trace_entries.push(DirectorTraceEntry {
                    timestamp_ms: now_millis(),
                    actor: "RollbackManager".into(),
                    summary: format!(
                        "Undo operation {} ({:?}), restoring {} snapshot entities",
                        op.id.0,
                        op.operation_type,
                        snapshot_entities.len()
                    ),
                });
            }
        }

        if !snapshot_entities.is_empty() {
            if let Some(ref mut bridge) = self.scene_bridge {
                let current = bridge.query_entities(None, None);
                let current_names: std::collections::HashSet<String> =
                    current.iter().map(|e| e.name.clone()).collect();
                let snapshot_names: std::collections::HashSet<String> =
                    snapshot_entities.iter().map(|e| e.name.clone()).collect();

                for entity in &current {
                    if !snapshot_names.contains(&entity.name) {
                        let _ = bridge.delete_entity(entity.id);
                    }
                }

                for snap_entity in &snapshot_entities {
                    if current_names.contains(&snap_entity.name) {
                        if let Some(existing) = bridge.query_entities(Some(&snap_entity.name), None).first() {
                            let _ = bridge.update_component(
                                existing.id,
                                "Transform",
                                std::collections::HashMap::from([(
                                    "serialized_state".to_string(),
                                    snap_entity.serialized_state.clone(),
                                )]),
                            );
                        }
                    } else {
                        let patches: Vec<crate::scene_bridge::ComponentPatch> = snap_entity
                            .component_names
                            .iter()
                            .map(|cn| crate::scene_bridge::ComponentPatch {
                                type_name: cn.clone(),
                                properties: std::collections::HashMap::new(),
                            })
                            .collect();
                        let _ = bridge.create_entity(&snap_entity.name, None, &patches);
                    }
                }

                self.trace_entries.push(DirectorTraceEntry {
                    timestamp_ms: now_millis(),
                    actor: "SceneBridge".into(),
                    summary: format!(
                        "Restored scene to snapshot ({} entities)",
                        snapshot_entities.len()
                    ),
                });
            }
        }

        let events = vec![EditorEvent::TransactionRolledBack {
            transaction_id: transaction_id.to_string(),
        }];
        self.events.extend(events.clone());

        self.trace_entries.push(DirectorTraceEntry {
            timestamp_ms: now_millis(),
            actor: "TransactionStore".into(),
            summary: format!("Rolled back transaction '{}'", transaction_id),
        });

        events
    }

    /// Return a mutable reference to the SkillRegistry.
    pub fn skill_registry_mut(&mut self) -> &mut SkillRegistry {
        &mut self.skill_registry
    }
}

// ------------------------------------------------------------------
// Default impl
// ------------------------------------------------------------------

impl Default for DirectorRuntime {
    fn default() -> Self {
        Self::new()
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan::{EditPlanStatus, ExecutionMode};

    #[test]
    fn test_new_runtime_is_empty() {
        let rt = DirectorRuntime::new();
        assert!(rt.list_plans().is_empty());
        assert!(!rt.has_pending_approvals());
        assert!(rt.pending_approval_ids().is_empty());
        assert!(rt.trace().is_empty());
    }

    #[test]
    fn test_handle_simple_request_auto_approves() {
        let mut rt = DirectorRuntime::new();
        // "创建一个红色敌人" is now Direct mode via SmartRouter
        let events = rt.handle_user_request("创建一个红色敌人");
        // Should auto-execute directly (no plan/permission overhead)
        let has_direct_completed = events
            .iter()
            .any(|e| matches!(e, EditorEvent::DirectExecutionCompleted { success: true, .. }));
        assert!(has_direct_completed);
        assert!(!rt.has_pending_approvals());
    }

    #[test]
    fn test_handle_medium_risk_requires_approval() {
        let mut rt = DirectorRuntime::new();
        // "批量创建多个红色敌人" → SmartRouter routes to Plan (has batch kw)
        let events = rt.handle_user_request("批量创建多个红色敌人");
        // Should need confirmation (MediumRisk due to "批量")
        let has_permission_requested = events
            .iter()
            .any(|e| matches!(e, EditorEvent::PermissionRequested { .. }));
        assert!(has_permission_requested);
        assert!(rt.has_pending_approvals());
    }

    #[test]
    fn test_handle_high_risk_requires_approval() {
        let mut rt = DirectorRuntime::new();
        // "删除所有红色敌人" → routes to Plan (HighRisk due to "删除")
        let events = rt.handle_user_request("删除所有红色敌人");
        // Should need confirmation (HighRisk)
        assert!(rt.has_pending_approvals());
        let has_permission = events
            .iter()
            .any(|e| matches!(e, EditorEvent::PermissionRequested { .. }));
        assert!(has_permission);
    }

    #[test]
    fn test_approve_and_execute() {
        let mut rt = DirectorRuntime::new();
        rt.handle_user_request("批量创建红色敌人");
        assert!(rt.has_pending_approvals());

        let pending = rt.pending_approval_ids();
        let plan_id = &pending[0];

        let events = rt.approve_plan(plan_id);
        assert!(!rt.has_pending_approvals());
        let has_completed = events
            .iter()
            .any(|e| matches!(e, EditorEvent::ExecutionCompleted { success: true, .. }));
        assert!(has_completed);
    }

    #[test]
    fn test_reject_plan() {
        let mut rt = DirectorRuntime::new();
        rt.handle_user_request("批量创建红色敌人");
        let pending = rt.pending_approval_ids();
        let plan_id = &pending[0];

        let events = rt.reject_plan(plan_id, Some("Not needed"));
        assert!(!rt.has_pending_approvals());
        let has_resolved = events
            .iter()
            .any(|e| matches!(e, EditorEvent::PermissionResolved { approved: false, .. }));
        assert!(has_resolved);

        let plan = rt.get_plan(plan_id).unwrap();
        assert_eq!(plan.status, EditPlanStatus::Rejected);
    }

    #[test]
    fn test_get_plan_and_list() {
        let mut rt = DirectorRuntime::new();
        // "批量创建红色敌人" routes to Plan mode (has batch), creating a plan
        rt.handle_user_request("批量创建红色敌人");

        let plans = rt.list_plans();
        assert_eq!(plans.len(), 1);
        assert_eq!(plans[0].title, "批量创建红色敌人");
    }

    #[test]
    fn test_get_plan_not_found() {
        let rt = DirectorRuntime::new();
        assert!(rt.get_plan("nonexistent").is_none());
    }

    #[test]
    fn test_approve_nonexistent_plan() {
        let mut rt = DirectorRuntime::new();
        let events = rt.approve_plan("nonexistent");
        assert!(events
            .iter()
            .any(|e| matches!(e, EditorEvent::Error { .. })));
    }

    #[test]
    fn test_execute_non_approved_plan() {
        let mut rt = DirectorRuntime::new();
        // This request is medium risk, so it won't be auto-executed
        rt.handle_user_request("批量修改颜色");
        let plan_id = {
            let plans = rt.list_plans();
            plans[0].id.clone()
        };

        // Try to execute without approval
        let events = rt.execute_plan(&plan_id);
        assert!(events
            .iter()
            .any(|e| matches!(e, EditorEvent::Error { .. })));
    }

    #[test]
    fn test_check_goal() {
        let mut rt = DirectorRuntime::new();
        // "批量创建红色敌人" routes to Plan, creates a plan
        rt.handle_user_request("批量创建红色敌人");

        let events = rt.check_goal(0);
        // For Plan mode, the task_id is 0 and a plan should exist
        let has_goal = events
            .iter()
            .any(|e| matches!(e, EditorEvent::GoalChecked { .. }));
        assert!(has_goal);
    }

    #[test]
    fn test_check_goal_no_task() {
        let mut rt = DirectorRuntime::new();
        let events = rt.check_goal(999);
        let has_goal = events
            .iter()
            .any(|e| matches!(e, EditorEvent::GoalChecked { all_matched: false, .. }));
        assert!(has_goal);
    }

    #[test]
    fn test_review_task() {
        let mut rt = DirectorRuntime::new();
        // "批量创建红色敌人" routes to Plan, creates a plan
        let events = rt.handle_user_request("批量创建红色敌人");

        // Check if it went into Plan mode (needs approval) or Direct mode
        let needs_approval = events
            .iter()
            .any(|e| matches!(e, EditorEvent::PermissionRequested { .. }));

        if needs_approval {
            // Plan mode: auto-approved in tests, so execute it first
            let pending = rt.pending_approval_ids();
            if !pending.is_empty() {
                let plan_id = pending[0].clone();
                rt.approve_plan(&plan_id);
            }
        }

        let review = rt.review_task(0);
        // For Plan mode, the task_id should be 0 and a plan is found
        assert_eq!(review.task_id, 0);
        assert!(
            review.decision == "approved" || review.decision == "needs_revision",
            "Expected 'approved' or 'needs_revision', got '{}'",
            review.decision
        );
    }

    #[test]
    fn test_review_nonexistent_task() {
        let mut rt = DirectorRuntime::new();
        let review = rt.review_task(999);
        assert_eq!(review.decision, "no_data");
        assert!(!review.issues.is_empty());
    }

    #[test]
    fn test_recent_events_clamps() {
        let mut rt = DirectorRuntime::new();
        rt.handle_user_request("创建红色敌人");

        let events = rt.recent_events(1);
        assert_eq!(events.len(), 1);

        // Asking for more than we have returns what we have
        let events = rt.recent_events(1000);
        assert!(events.len() < 1000);
    }

    #[test]
    fn test_rollback_transaction() {
        let mut rt = DirectorRuntime::new();
        let events = rt.rollback_transaction("txn_test_1");
        assert!(events
            .iter()
            .any(|e| matches!(e, EditorEvent::TransactionRolledBack { .. })));
    }

    #[test]
    fn test_trace_populated() {
        let mut rt = DirectorRuntime::new();
        rt.handle_user_request("批量创建红色敌人");

        let trace = rt.trace();
        assert!(!trace.is_empty());

        // Should contain traces from SmartRouter and Planner
        let actors: Vec<&str> = trace.iter().map(|t| t.actor.as_str()).collect();
        assert!(actors.contains(&"SmartRouter"));
        assert!(actors.contains(&"Planner"));
    }

    #[test]
    fn test_empty_request_fallback_step() {
        let mut rt = DirectorRuntime::new();
        // "生成一个敌人AI的代码脚本" has "代码" → routes to Plan
        let _events = rt.handle_user_request("生成一个敌人AI的代码脚本");
        // Should create a plan via Plan mode
        let plans = rt.list_plans();
        assert!(plans.len() >= 1);
    }

    #[test]
    fn test_code_request_mode_direct_or_plan() {
        let mut rt = DirectorRuntime::new();
        rt.handle_user_request("生成一个敌人AI的代码脚本");

        let plans = rt.list_plans();
        // Depending on keyword matching, mode can be Direct (Simple+LowRisk)
        // or Plan (Medium+LowRisk). Both are valid.
        assert!(matches!(
            plans[0].mode,
            ExecutionMode::Direct | ExecutionMode::Plan
        ));
    }

    #[test]
    fn test_default_impl() {
        let rt = DirectorRuntime::default();
        assert!(rt.list_plans().is_empty());
    }

    // Sprint 1: 验收测试 - ReActAgent 集成和分层上下文
    #[test]
    fn test_react_agent_layered_context() {
        let mut rt = DirectorRuntime::new();

        // ReActAgent initialization depends on LLM environment configuration
        // If LLM is not configured, has_react_agent() returns false
        if rt.has_react_agent() {
            // Verify L0-L3 context is set
            if let Some(ref react) = rt.react_agent {
                // L0: System context should have agent name
                // L1: Session context should have project name
                // L2: Task context should be set
                // L3: Entity context should be empty initially
                // Few-shot examples should be present
                assert!(!react.config.system_prompt.is_empty(), "System prompt should not be empty");
            }
        }

        // Test layered context structure independently
        let layered = crate::prompt::LayeredContext {
            l0_system: crate::prompt::L0SystemContext::default_bevy(),
            ..Default::default()
        };
        assert!(!layered.l0_system.agent_name.is_empty(), "L0 agent name should be set");
        assert!(!layered.l0_system.engine_name.is_empty(), "L0 engine name should be set");
    }

    #[test]
    fn test_few_shot_example_selection() {
        use crate::prompt::{LayeredContext, FewShotExample};

        let mut layered = LayeredContext::default();
        layered.add_few_shot(FewShotExample::create_entity_example());
        layered.add_few_shot(FewShotExample::update_component_example());
        layered.add_few_shot(FewShotExample::query_entities_example());

        // Test selecting examples for "create" request
        let selected = layered.select_few_shot_examples("创建一个红色敌人", 2);
        assert!(!selected.is_empty(), "Should select at least one example");
        // The create_entity_example should be most relevant
        assert!(selected[0].action.contains("create"), "Should select create example first");

        // Test selecting examples for "update" request
        let selected = layered.select_few_shot_examples("把 Player 改成蓝色", 2);
        assert!(!selected.is_empty(), "Should select at least one example");
        assert!(selected[0].action.contains("update"), "Should select update example first");
    }

    #[test]
    fn test_dynamic_revision_skips_duplicate_steps() {
        use crate::plan::{EditPlan, EditPlanStep, TargetModule, ExecutionMode, EditPlanStatus};
        use crate::permission::OperationRisk;

        let mut rt = DirectorRuntime::new();

        // Create a plan with duplicate creation steps
        let mut plan = EditPlan::new("test_plan", 1, "Test Plan", "Test dynamic revision", ExecutionMode::Plan);
        plan.status = EditPlanStatus::Draft;
        plan.steps = vec![
            EditPlanStep {
                id: "step_1".into(),
                title: "Create Enemy".into(),
                target_module: TargetModule::Scene,
                action_description: "Create Enemy".into(),
                risk: OperationRisk::LowRisk,
                validation_requirements: vec![],
            },
            EditPlanStep {
                id: "step_2".into(),
                title: "Create Player".into(),
                target_module: TargetModule::Scene,
                action_description: "Create Player".into(),
                risk: OperationRisk::LowRisk,
                validation_requirements: vec![],
            },
        ];

        rt.plan_manager.insert("test_plan".into(), plan);

        // Apply revision to skip duplicate creation steps
        rt.apply_plan_revision("test_plan", "Skip duplicate creation steps");

        // Verify steps are marked as skipped
        let updated_plan = rt.plan_manager.get("test_plan").unwrap();
        assert!(updated_plan.steps[0].title.starts_with("[SKIPPED]"), "Step 1 should be skipped");
        assert!(updated_plan.steps[1].title.starts_with("[SKIPPED]"), "Step 2 should be skipped");
    }

    #[test]
    fn test_reflection_alternative_step_generation() {
        let mut rt = DirectorRuntime::new();

        // Test "not found" error generates alternative
        let alt = rt.generate_alternative_step("Delete Enemy", "Entity not found");
        assert!(alt.is_some(), "Should generate alternative for not found error");
        let alt_text = alt.unwrap();
        assert!(alt_text.contains("Create entity"), "Alternative should suggest creating entity first");

        // Test "already exists" error generates alternative
        let alt = rt.generate_alternative_step("Create Player", "Entity already exists");
        assert!(alt.is_some(), "Should generate alternative for already exists error");
        let alt_text = alt.unwrap();
        assert!(alt_text.contains("Modify") || alt_text.contains("修改"), "Alternative should suggest modification");

        // Test permission error generates alternative
        let alt = rt.generate_alternative_step("Delete Boss", "Permission denied");
        assert!(alt.is_some(), "Should generate alternative for permission error");
        let alt_text = alt.unwrap();
        assert!(alt_text.contains("LOW_RISK"), "Alternative should use low risk approach");
    }
}
