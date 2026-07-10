//! DirectorRuntime - Central orchestrator for the Agent team system
//!
//! Ties together Planner, Permission, Executor, GoalChecker, and Reviewer
//! into a single command-driven runtime.

pub mod events;
pub mod execution;
pub mod goals;
pub mod plan_manager;
pub mod types;

// Decomposed sub-modules (from execution.rs)
pub mod react_runner;
pub mod react_tools;

// Impl sub-modules (extracted from DirectorRuntime impl)
pub mod agent_dispatch;
mod context_ops;
mod open_world_ops;
pub mod plan_executor;
mod plan_ops;
pub mod plan_revision;
mod snapshot_ops;
mod vgrc_ops;

// Re-export all public types so existing code doesn't break
pub use plan_manager::PlanManager;
pub use types::*;

use crate::event::EventBus;
use crate::fallback::FallbackEngine;
use crate::hr_agent::HrAgent;
use crate::hybrid_controller::HybridEditorController;
use crate::memory::AgentMemoryId;
use crate::metrics::AgentMetrics;
use crate::plan::{EditPlan, EditPlanStep};
use crate::planner::{Planner, RuleBasedPlanner};
use crate::prompt::PromptSystem;
use crate::registry::AgentId;
use crate::registry::AgentRegistry;
use crate::rollback::RollbackManager;
use crate::scene_agent::SceneAgent;
use crate::scene_bridge::SceneBridge;
use crate::skill::{SkillExecutor, SkillRegistry};
use crate::specialized_agents::{CodeAgent, PlannerAgent, ReviewAgent};
use crate::team_structure::TeamRoster;
use crate::types::now_millis;
use std::sync::Arc;

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
                    use crate::strategy::{create_react_agent, ReActConfig};
                    use std::sync::Arc;

                    eprintln!("[DirectorRuntime] LLM client auto-configured from environment");

                    let mut registry = crate::tool::ToolRegistry::new();
                    // Register bridge-independent tools so ReAct can actually use them.
                    crate::code_tools::register_code_tools(&mut registry);
                    crate::file_tools::register_file_tools(&mut registry);
                    // Scene/engine tools require a SceneBridge which is set per-request.
                    // They are registered later via set_scene_bridge → register_react_scene_tools.
                    let tool_registry = Arc::new(std::sync::Mutex::new(registry));

                    let _config = ReActConfig {
                        max_steps: 20,
                        temperature: 0.3,
                        include_observations: true,
                        system_prompt: REACT_SYSTEM_PROMPT.to_string(),
                    };

                    // Sprint 1: Create layered context for L0-L3 prompt enrichment
                    let mut layered = crate::prompt::LayeredContext {
                        l0_system: crate::prompt::L0SystemContext::default_bevy(),
                        l1_session: crate::prompt::L1SessionContext {
                            project_name: "AgentEdit".to_string(),
                            engine_version: "0.17".to_string(),
                            ..Default::default()
                        },
                        ..Default::default()
                    };
                    // L2: Task context (will be updated per-request)
                    layered.l2_task.current_task = "Awaiting user request".to_string();
                    // Add few-shot examples
                    layered.add_few_shot(crate::prompt::FewShotExample::create_entity_example());
                    layered.add_few_shot(crate::prompt::FewShotExample::update_component_example());
                    layered.add_few_shot(crate::prompt::FewShotExample::query_entities_example());

                    // Create Arc from Box - this consumes the Box
                    let client_arc: Arc<dyn crate::llm::LlmClient> = Arc::from(client);
                    let react = create_react_agent(client_arc.clone(), tool_registry)
                        .with_layered_context(layered);

                    // Store the Arc as Box for backward compatibility
                    // Note: This works because Arc::from(Box) gives us Arc<Box<dyn LlmClient>>
                    // We need to convert it back. Since we can't clone the trait object,
                    // we'll store None for llm_client and rely on react_agent having the client.
                    // The react_agent already has Arc<dyn LlmClient>.
                    (None, Some(react))
                }
                Err(e) => {
                    eprintln!(
                        "[DirectorRuntime] LLM not available ({}), using fallback",
                        e
                    );
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
            memory_registry: crate::memory::MemorySystemRegistry::new(),
            active_agent: AgentMemoryId(0),
            memory_injector: crate::memory_injector::MemoryInjector::new(None),
            event_bridge: crate::memory_injector::EventMemoryBridge::new(),
            dynamic_planner: crate::dynamic_planner::DynamicPlanner::new(),
            reflection_engine: crate::reflection_engine::ReflectionEngine::new(),
            vgrc_controller: None,
            last_vgrc_result: None,
            last_vgrc_cycle: None,
            last_visual_observation: None,
            hybrid_controller: None,
            rule_system: crate::rule_system::RuleSystem::new(
                std::env::current_dir().unwrap_or_else(|e| {
                    eprintln!(
                        "[DirectorRuntime] Cannot access CWD ({}), falling back to '.'",
                        e
                    );
                    std::path::PathBuf::from(".")
                }),
            ),
            collaboration: None,
            reasoning_bank: crate::reasoning_bank::ReasoningBankManager::new(),
            squad: None,
            compound: crate::skills_compound::SkillCompound::new(),
            capture_pipeline: crate::capture_pipeline::MemoryCapturePipeline::new(
                std::time::Duration::from_secs(60),
            ),
            previous_llm_mode: "rule".to_string(),
            shadow_git: crate::shadow_git::ShadowGitService::new(
                std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")),
            ),
            memory_dir: None,
            agent_pending_approvals: std::collections::HashMap::new(),
        }
    }

    // ------------------------------------------------------------------
    // Memory access (ADR-001 Phase 2: per-agent isolation)
    // ------------------------------------------------------------------

    /// Get the memory system for the currently active agent.
    pub(crate) fn memory(&mut self) -> &mut crate::memory::MemorySystem {
        &mut self.memory_registry.get_mut(self.active_agent).system
    }

    /// Set the currently active agent for memory operations.
    pub fn set_active_agent(&mut self, agent_id: impl Into<AgentMemoryId>) {
        self.active_agent = agent_id.into();
    }

    /// Get the agent ID currently active for memory operations.
    pub fn active_agent(&self) -> AgentMemoryId {
        self.active_agent
    }

    /// Switch memory context to another agent, creating memory if needed.
    pub fn ensure_agent_memory(&mut self, agent_id: impl Into<AgentMemoryId>) {
        let id = agent_id.into();
        self.memory_registry.get_mut(id);
        self.active_agent = id;
    }

    /// Transfer memory from one agent to another (for task handoffs).
    pub fn transfer_agent_memory(
        &mut self,
        from: impl Into<AgentMemoryId>,
        to: impl Into<AgentMemoryId>,
    ) -> bool {
        self.memory_registry.transfer(from, to)
    }

    // ------------------------------------------------------------------
    // Memory Persistence
    // ------------------------------------------------------------------

    /// Set the directory for memory persistence (auto-save/auto-load).
    pub fn set_memory_dir(&mut self, dir: String) {
        self.memory_dir = Some(dir);
    }

    /// Load all agent memory from the configured directory.
    /// Call during initialization to restore state from a previous session.
    pub fn auto_load_memory(&mut self) {
        if let Some(ref dir) = self.memory_dir {
            let path = std::path::Path::new(dir);
            match self.memory_registry.load_all_from_dir(path) {
                Ok(count) => {
                    if count > 0 {
                        log::info!(
                            "[Memory] Loaded {} agent memory file(s) from {:?}",
                            count,
                            dir
                        );
                    }
                }
                Err(e) => {
                    log::warn!("[Memory] Failed to load memory from {:?}: {}", dir, e);
                }
            }
        }
    }

    /// Save all agent memory to the configured directory.
    /// Called automatically after each user request.
    pub fn auto_save_memory(&self) {
        if let Some(ref dir) = self.memory_dir {
            let path = std::path::Path::new(dir);
            match self.memory_registry.save_all_to_dir(path) {
                Ok(count) => {
                    log::debug!("[Memory] Saved {} agent memory file(s) to {:?}", count, dir);
                }
                Err(e) => {
                    log::warn!("[Memory] Failed to save memory to {:?}: {}", dir, e);
                }
            }
        }
    }

    // ------------------------------------------------------------------
    // Config / setup
    // ------------------------------------------------------------------

    /// Enable GoalChecker validation (for Phase 2 integration).
    pub fn enable_goal_checker(&mut self) {
        self.goal_checker_enabled = true;
    }

    /// Check an action against the RuleSystem (allow/deny/confirmation).
    pub fn check_rule(&self, action: &str) -> crate::rule_system::RuleResult {
        self.rule_system.check_action(action)
    }

    /// Check whether an action requires user confirmation via RuleSystem.
    pub fn needs_rule_confirmation(&self, action: &str) -> bool {
        self.rule_system.needs_confirmation(action)
    }

    /// Initialize HybridEditorController with LLM client.
    ///
    /// This enables automatic fallback to RuleBasedPlanner when LLM is unavailable.
    pub fn init_hybrid_controller(&mut self, llm_client: Arc<dyn crate::llm::LlmClient>) {
        self.hybrid_controller = Some(HybridEditorController::with_llm_client(Some(llm_client)));
    }

    /// Get reference to HybridEditorController if initialized.
    pub fn hybrid_controller(&self) -> Option<&HybridEditorController> {
        self.hybrid_controller.as_ref()
    }

    /// Check whether LLM mode is currently available.
    ///
    /// Returns "llm" if a react_agent is available AND the hybrid_controller
    /// reports LLM is available, "rule" otherwise.
    pub fn check_llm_mode(&self) -> String {
        let react_available = self.react_agent.is_some();

        if let Some(hc) = self.hybrid_controller.as_ref() {
            if matches!(hc.current_mode(), crate::hybrid_controller::EditorMode::Llm)
                && react_available
            {
                return "llm".to_string();
            }
            return "rule".to_string();
        }

        if react_available {
            "llm".to_string()
        } else {
            "rule".to_string()
        }
    }

    /// Initialize the Agent Collaboration System (Squad, ReasoningBank, SkillsCompound).
    pub fn init_collaboration(&mut self) {
        self.collaboration = Some(crate::agent_collaboration::AgentCollaborationSystem::new());
    }

    /// Get reference to Collaboration System if initialized.
    pub fn collaboration(&self) -> Option<&crate::agent_collaboration::AgentCollaborationSystem> {
        self.collaboration.as_ref()
    }

    /// Get mutable reference to Collaboration System if initialized.
    pub fn collaboration_mut(
        &mut self,
    ) -> Option<&mut crate::agent_collaboration::AgentCollaborationSystem> {
        self.collaboration.as_mut()
    }

    /// Initialize the built-in skill library.
    ///
    /// Registers predefined skills (create_entity, modify_transform, query_scene, import_asset)
    /// into the internal `SkillRegistry`. Call once at startup.
    pub fn init_builtin_skills(&mut self) {
        self.skill_registry
            .register(crate::builtin_skills::create_entity_skill());
        self.skill_registry
            .register(crate::builtin_skills::modify_entity_transform_skill());
        self.skill_registry
            .register(crate::builtin_skills::query_scene_skill());
        self.skill_registry
            .register(crate::builtin_skills::import_asset_skill());
    }

    /// Look up a matching skill by step title text.
    pub fn lookup_skill_for_step(
        &self,
        step: &EditPlanStep,
    ) -> Option<crate::skill::SkillDefinition> {
        let skill_names = self.skill_registry.list_skill_names();
        let title_lower = step.title.to_lowercase();

        // Exact match first
        for &name in &skill_names {
            if title_lower.contains(&name.to_lowercase()) {
                return self.skill_registry.find_by_name(name).cloned();
            }
        }

        // Fuzzy: keyword-based (delegated to KeywordMatcher)
        if let Some(skill_name) =
            crate::keyword_matcher::KeywordMatcher::resolve_skill_name(&step.title)
        {
            return self.skill_registry.find_by_name(skill_name).cloned();
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
        let hr_agent = HrAgent::new(AgentId(5), TeamRoster::new());
        let squad_agent = crate::squad_agent::SquadAgent::new(
            AgentId(6),
            "SquadManager".into(),
            crate::squad::SquadId(0),
            crate::squad::SquadRegistry::new(),
        );

        registry.register(Box::new(scene_agent));
        registry.register(Box::new(code_agent));
        registry.register(Box::new(review_agent));
        registry.register(Box::new(planner_agent));
        registry.register(Box::new(hr_agent));
        registry.register(Box::new(squad_agent));

        registry
    }

    /// Check if an AgentRegistry is available for Team mode.
    pub fn has_agent_registry(&self) -> bool {
        self.agent_registry.is_some()
    }

    /// Drain accumulated events for UI consumption.
    ///
    /// Returns all events since the last drain. Used by the visual
    /// understanding bridge and other UI subsystems.
    pub fn drain_events(&mut self) -> Vec<crate::director::types::EditorEvent> {
        std::mem::take(&mut self.events)
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

    /// Mutable access to ShadowGitService for file-level snapshots.
    pub fn shadow_git_mut(&mut self) -> &mut crate::shadow_git::ShadowGitService {
        &mut self.shadow_git
    }

    // ------------------------------------------------------------------
    // LLM API (Phase 1: Real LLM Integration)
    // ------------------------------------------------------------------

    /// Check if LLM client is available.
    ///
    /// Returns true if an explicit llm_client is configured and ready, OR
    /// if the ReAct agent (which internally holds an Arc<dyn LlmClient>)
    /// is present. This ensures the main path enters the real LLM loop
    /// when the runtime was auto-configured from environment (where
    /// llm_client is intentionally left None and react_agent carries
    /// the client instead — see constructor lines 133–138).
    pub fn has_llm(&self) -> bool {
        self.llm_client
            .as_ref()
            .map(|c| c.is_ready())
            .unwrap_or(false)
            || self.react_agent.is_some()
    }

    /// Check if ReActAgent is available for LLM-driven execution (Sprint 1).
    pub fn has_react_agent(&self) -> bool {
        self.react_agent.is_some()
    }

    /// Get LLM status information.
    ///
    /// When the runtime was auto-configured from environment, llm_client may be
    /// None while react_agent holds the real client. We reflect that here so
    /// the UI / status API doesn't report NotConfigured when LLM is actually
    /// available through the ReAct agent.
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
            None => {
                if self.react_agent.is_some() {
                    LlmStatus::Ready {
                        provider: "ReActAgent (auto-configured)".to_string(),
                    }
                } else {
                    LlmStatus::NotConfigured
                }
            }
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

    /// Mutable access to ReasoningBankManager for recording traces.
    pub fn reasoning_bank_mut(&mut self) -> &mut crate::reasoning_bank::ReasoningBankManager {
        &mut self.reasoning_bank
    }

    /// Access the Squad if initialized.
    pub fn squad(&self) -> Option<&crate::squad::Squad> {
        self.squad.as_ref()
    }

    /// Mutable access to the Squad.
    pub fn squad_mut(&mut self) -> Option<&mut crate::squad::Squad> {
        self.squad.as_mut()
    }

    /// Initialize the squad collaboration system.
    pub fn init_squad(&mut self) {
        self.squad = Some(crate::squad::Squad::new(
            crate::squad::SquadId(0),
            "default".to_string(),
            crate::registry::AgentId(0),
            crate::squad::RoutingPolicy::RoundRobin,
        ));
    }

    /// Mutable access to SkillCompound for recording skill successes.
    pub fn compound_mut(&mut self) -> &mut crate::skills_compound::SkillCompound {
        &mut self.compound
    }

    /// Access the capture pipeline for hook-driven memory capture.
    pub fn capture_pipeline(&self) -> &crate::capture_pipeline::MemoryCapturePipeline {
        &self.capture_pipeline
    }

    /// Trigger a memory capture hook event.
    pub fn trigger_capture_hook(
        &mut self,
        hook: &crate::capture_pipeline::MemoryHook,
    ) -> Vec<crate::capture_pipeline::CapturedMemory> {
        self.capture_pipeline.process(hook)
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
        self.plan_manager.has_pending_approvals() || !self.agent_pending_approvals.is_empty()
    }

    /// Return the total step count of the most recently created plan.
    ///
    /// Returns 0 if no plans have been created yet.
    pub fn current_plan_step_count(&self) -> usize {
        self.plan_manager.current_step_count()
    }

    /// Get the list of plan IDs currently waiting for approval.
    pub fn pending_approval_ids(&self) -> Vec<String> {
        let mut ids = self.plan_manager.pending_approval_ids();
        ids.extend(self.agent_pending_approvals.keys().cloned());
        ids
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

#[cfg(test)]
mod tests;
