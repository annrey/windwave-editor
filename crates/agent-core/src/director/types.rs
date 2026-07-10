//! DirectorRuntime types, enums, and helper structs.

use super::plan_manager::PlanManager;
use crate::event::EventBus;
use crate::fallback::FallbackEngine;
use crate::hybrid_controller::HybridEditorController;
use crate::metrics::AgentMetrics;
use crate::prompt::PromptSystem;
use crate::registry::{AgentId, AgentRegistry};
use crate::rollback::RollbackManager;
use crate::scene_bridge::SceneBridge;
use crate::skill::{SkillActionHandler, SkillExecutor, SkillRegistry};
use crate::strategy::ReActAgent;

// ---------------------------------------------------------------------------
// SceneBridgeSkillHandler — translates SkillNode actions → SceneBridge calls
// ---------------------------------------------------------------------------

/// Wraps a `&mut dyn SceneBridge` to implement `SkillActionHandler`.
///
/// Each skill node's `tool_name` maps to a SceneBridge operation:
/// - "spawn_entity"     → `bridge.create_entity()`
/// - "set_transform"    → `bridge.update_component("Transform", ...)`
/// - "set_sprite"       → `bridge.update_component("Sprite", ...)`
/// - "query_scene"      → `bridge.query_entities()`
/// - "delete_entity"    → `bridge.delete_entity()`
pub(crate) struct SceneBridgeSkillHandler<'a> {
    pub(crate) bridge: &'a mut dyn SceneBridge,
}

impl SkillActionHandler for SceneBridgeSkillHandler<'_> {
    fn handle(
        &mut self,
        action: &str,
        params: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<serde_json::Value, String> {
        match action {
            "spawn_entity" => {
                let name = params
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("entity");
                let position = params
                    .get("position")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        [
                            arr[0].as_f64().unwrap_or(0.0),
                            arr[1].as_f64().unwrap_or(0.0),
                        ]
                    });

                let mut patches = Vec::new();
                if let Some(sprite_color) = params.get("sprite_color") {
                    let mut props = std::collections::HashMap::new();
                    props.insert("color".into(), sprite_color.clone());
                    patches.push(crate::scene_bridge::ComponentPatch {
                        type_name: "Sprite".into(),
                        properties: props,
                    });
                }

                let id = self
                    .bridge
                    .create_entity(name, position, &patches)
                    .map_err(|e| format!("spawn_entity failed: {}", e))?;
                Ok(serde_json::json!({"entity_id": id, "name": name}))
            }
            "set_transform" => {
                let entity_id = params
                    .get("entity_id")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                let mut props = std::collections::HashMap::new();
                if let Some(pos) = params.get("position") {
                    props.insert("position".into(), pos.clone());
                }
                self.bridge
                    .update_component(entity_id, "Transform", props)
                    .map_err(|e| format!("set_transform failed: {}", e))?;
                Ok(serde_json::Value::Null)
            }
            "set_sprite" => {
                let entity_id = params
                    .get("entity_id")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                let mut props = std::collections::HashMap::new();
                if let Some(color) = params.get("color") {
                    props.insert("color".into(), color.clone());
                }
                self.bridge
                    .update_component(entity_id, "Sprite", props)
                    .map_err(|e| format!("set_sprite failed: {}", e))?;
                Ok(serde_json::Value::Null)
            }
            "query_scene" => {
                let filter = params.get("filter").and_then(|v| v.as_str());
                let comp_type = params.get("component_type").and_then(|v| v.as_str());
                let entities = self.bridge.query_entities(filter, comp_type);
                Ok(serde_json::to_value(entities).unwrap_or(serde_json::Value::Null))
            }
            "delete_entity" => {
                let entity_id = params
                    .get("entity_id")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                self.bridge
                    .delete_entity(entity_id)
                    .map_err(|e| format!("delete_entity failed: {}", e))?;
                Ok(serde_json::Value::Null)
            }
            "noop" => Ok(serde_json::Value::Null),
            _ => {
                // Unknown actions are logged and succeed (extensible)
                Ok(serde_json::json!({"unhandled_action": action}))
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Commands (from UI / user to runtime)
// ---------------------------------------------------------------------------

/// Commands sent to the `DirectorRuntime` to drive state transitions.
#[derive(Debug, Clone)]
pub enum EditorCommand {
    /// Create a new edit plan from a user text request.
    CreateEditPlan {
        /// The raw text of the user's request.
        request_text: String,
        /// The task identifier to associate with this plan.
        task_id: u64,
    },
    /// Approve a plan that is waiting for user confirmation.
    ApprovePlan {
        /// ID of the plan to approve.
        plan_id: String,
    },
    /// Reject a plan, optionally providing a reason.
    RejectPlan {
        /// ID of the plan to reject.
        plan_id: String,
        /// Optional reason for rejection (shown to the user / logged).
        reason: Option<String>,
    },
    /// Execute an already-approved plan.
    ExecutePlan {
        /// ID of the plan to execute.
        plan_id: String,
    },
    /// Roll back a previously committed transaction.
    RollbackTransaction {
        /// ID of the transaction to roll back.
        transaction_id: String,
    },
    /// Run the goal checker against a task to see if all objectives are met.
    CheckGoal {
        /// ID of the task to check.
        task_id: u64,
    },
}

// ---------------------------------------------------------------------------
// Events (from runtime to UI / subscribers)
// ---------------------------------------------------------------------------

/// Events emitted by the `DirectorRuntime` to notify the UI and other
/// subscribers of state changes.
#[derive(Debug, Clone)]
pub enum EditorEvent {
    /// A new edit plan has been created.
    EditPlanCreated {
        /// ID of the newly created plan.
        plan_id: String,
        /// Title of the plan.
        title: String,
        /// Risk level as a display string.
        risk: String,
        /// Execution mode as a display string.
        mode: String,
        /// Number of steps in the plan.
        steps_count: usize,
    },
    /// The runtime is requesting user permission for a plan.
    PermissionRequested {
        /// ID of the plan needing approval.
        plan_id: String,
        /// Risk level as a display string.
        risk: String,
        /// Human-readable reason why approval is needed.
        reason: String,
    },
    /// A permission request has been resolved (approved or denied).
    PermissionResolved {
        /// ID of the plan.
        plan_id: String,
        /// Whether the plan was approved.
        approved: bool,
        /// Reason if denied, or `None` if approved.
        reason: Option<String>,
    },
    /// Execution of a plan has started.
    PlanExecutionStarted {
        /// ID of the plan being executed.
        plan_id: String,
    },
    /// A single step within a plan has started.
    StepStarted {
        /// ID of the parent plan.
        plan_id: String,
        /// ID of the step.
        step_id: String,
        /// Title of the step.
        title: String,
    },
    /// A single step completed successfully.
    StepCompleted {
        /// ID of the parent plan.
        plan_id: String,
        /// ID of the step.
        step_id: String,
        /// Title of the step.
        title: String,
        /// Human-readable result description.
        result: String,
    },
    /// A single step failed.
    StepFailed {
        /// ID of the parent plan.
        plan_id: String,
        /// ID of the step.
        step_id: String,
        /// Title of the step.
        title: String,
        /// Error message describing what went wrong.
        error: String,
    },
    /// A transaction was started (for rollback support).
    TransactionStarted {
        /// Unique transaction identifier.
        transaction_id: String,
        /// ID of the step associated with this transaction.
        step_id: String,
    },
    /// A transaction was committed successfully.
    TransactionCommitted {
        /// Transaction identifier.
        transaction_id: String,
    },
    /// A transaction was rolled back.
    TransactionRolledBack {
        /// Transaction identifier.
        transaction_id: String,
    },
    /// Goal checker has finished evaluating a task.
    GoalChecked {
        /// ID of the task.
        task_id: u64,
        /// Whether all goals were matched.
        all_matched: bool,
        /// Human-readable summary of the goal check.
        summary: String,
    },
    /// Reviewer has completed evaluating a task.
    ReviewCompleted {
        /// ID of the task.
        task_id: u64,
        /// Review decision (e.g., "approved", "needs_revision").
        decision: String,
        /// Human-readable summary.
        summary: String,
    },
    /// Entire plan execution finished.
    ExecutionCompleted {
        /// ID of the plan.
        plan_id: String,
        /// Whether execution succeeded overall.
        success: bool,
    },
    /// Editor mode changed (LLM ↔ rule-based).
    ModeChanged {
        /// Current execution mode: "llm" or "rule".
        mode: String,
    },
    /// Generic error event.
    Error {
        /// Error message.
        message: String,
    },
    /// Direct execution started (SmartRouter chose Direct mode).
    DirectExecutionStarted {
        /// Original user request text.
        request: String,
        /// Execution mode display string.
        mode: String,
        /// Complexity score from SmartRouter (0-10).
        complexity_score: u8,
    },
    /// Direct execution completed.
    DirectExecutionCompleted {
        /// Original user request text.
        request: String,
        /// Whether all operations succeeded.
        success: bool,
    },
}

// ---------------------------------------------------------------------------
// Review summary (internal to reviewer)
// ---------------------------------------------------------------------------

/// Summary produced by the reviewer after evaluating a task.
#[derive(Debug, Clone)]
pub struct ReviewSummary {
    /// ID of the task that was reviewed.
    pub task_id: u64,
    /// Review decision ("approved", "needs_revision", "rejected").
    pub decision: String,
    /// Human-readable summary of the review.
    pub summary: String,
    /// List of issues identified during review, if any.
    pub issues: Vec<String>,
}

// ---------------------------------------------------------------------------
// Trace entry (for debugging / audit trail)
// ---------------------------------------------------------------------------

/// A single entry in the execution trace log.
#[derive(Debug, Clone)]
pub struct DirectorTraceEntry {
    /// Millisecond-precision timestamp of the event.
    pub timestamp_ms: u64,
    /// Name of the actor that produced this trace entry (e.g., "Executor", "Planner").
    pub actor: String,
    /// Human-readable summary of what happened.
    pub summary: String,
}

// ===========================================================================
// DirectorExecutionResult, ExecuteContext — added for SmartRouter integration
// ===========================================================================

/// Result of a CEO-run flow: routing decision + emitted events + trace.
#[derive(Debug, Clone)]
pub struct DirectorExecutionResult {
    /// The routing decision made by SmartRouter.
    pub decision: crate::router::RoutingDecision,
    /// All events emitted during this run.
    pub events: Vec<EditorEvent>,
    /// Trace log entries collected during execution.
    pub trace: Vec<DirectorTraceEntry>,
    /// Total elapsed wall-time in microseconds.
    pub elapsed_us: u64,
}

/// Context for direct (plan-less) execution mode.
#[derive(Debug, Clone)]
pub struct ExecuteContext {
    /// Matched entity names referenced in user request.
    pub entity_names: Vec<String>,
    /// Colors extracted from the request text.
    pub colors: Vec<String>,
    /// Positional hints (right, left, above, below).
    pub positions: Vec<String>,
    /// Action keywords (create, move, delete, query).
    pub action: String,
}

// ===========================================================================
// DirectorRuntime
// ===========================================================================

/// LLM connection status for diagnostics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LlmStatus {
    /// LLM is ready to use.
    Ready { provider: String },
    /// LLM client exists but not ready (e.g., invalid API key).
    NotReady,
    /// No LLM client configured.
    NotConfigured,
}

/// Central orchestrator that ties Planner, PermissionEngine, TransactionStore,
/// GoalChecker, and Reviewer together into a single command-driven runtime.
///
/// The flow now starts with **SmartRouter** (§3.4), which analyzes the request
/// and chooses between:
///
/// * **Direct mode** — simple single-step commands executed immediately.
/// * **Plan mode** — full plan → permission → execute pipeline.
/// * **ReAct mode** — LLM-driven think-act-observe loop (Sprint 1).
///
/// DirectorRuntime is the central orchestrator for all agent subsystems.
///
/// - `plans`: all plans currently managed by the runtime (active and completed).
/// - `pending_approvals`: list of plan IDs waiting for user confirmation.
///
/// ## Subsystem Groups
///
/// | Group | Fields | Responsibility |
/// |-------|--------|----------------|
/// | Planning | plan_manager, dynamic_planner, skill_registry, skill_executor | Plan lifecycle + skill DAG + dynamic revision |
/// | Observability | events, trace_entries, event_bus, audit_log, metrics | Event log, tracing, pub/sub, audit |
/// | Intelligence | llm_client, react_agent, hybrid_controller, prompt_system, fallback_engine | LLM, ReAct, hybrid execution, prompt engineering, fallback |
/// | Memory | memory_registry, active_agent, memory_injector, event_bridge | 4-tier memory, auto-capture, event archiving |
/// | Safety | rollback_manager, edit_history, reflection_engine, shadow_git | Transaction rollback, undo/redo, error recovery, file snapshots |
/// | Multi-Agent | agent_registry, comm_hub | Team dispatch, inter-agent messaging |
/// | Scene & Visual | scene_bridge, vgrc_controller | Scene entity ops, visual reasoning loop |
///
/// See `execution.rs` for test coverage per subsystem.
pub struct DirectorRuntime {
    // ── Planning & Skills ──
    pub(crate) plan_manager: PlanManager,
    pub(crate) dynamic_planner: crate::dynamic_planner::DynamicPlanner,
    pub(crate) skill_registry: SkillRegistry,
    pub(crate) skill_executor: SkillExecutor,

    // ── Observability & Auditing ──
    pub(crate) events: Vec<EditorEvent>,
    pub(crate) trace_entries: Vec<DirectorTraceEntry>,
    pub(crate) event_bus: EventBus,
    pub(crate) audit_log: crate::audit::AuditLog,
    pub(crate) metrics: AgentMetrics,

    // ── AI Intelligence ──
    pub(crate) llm_client: Option<Box<dyn crate::llm::LlmClient>>,
    pub(crate) react_agent: Option<ReActAgent>,
    pub(crate) hybrid_controller: Option<HybridEditorController>,
    pub(crate) prompt_system: PromptSystem,
    pub(crate) fallback_engine: FallbackEngine,

    // ── Memory (4-tier) ──
    pub(crate) memory_registry: crate::memory::MemorySystemRegistry,
    pub(crate) active_agent: crate::memory::AgentMemoryId,
    pub(crate) memory_injector: crate::memory_injector::MemoryInjector,
    pub(crate) event_bridge: crate::memory_injector::EventMemoryBridge,

    // ── Safety & Recovery ──
    pub(crate) rollback_manager: RollbackManager,
    pub(crate) edit_history: crate::edit_history::EditHistory,
    pub(crate) reflection_engine: crate::reflection_engine::ReflectionEngine,
    pub(crate) shadow_git: crate::shadow_git::ShadowGitService,

    // ── Multi-Agent ──
    pub(crate) agent_registry: Option<AgentRegistry>,
    pub(crate) agent_pending_approvals: std::collections::HashMap<String, AgentId>,
    pub(crate) comm_hub: crate::agent_comm::CommunicationHub,

    // ── Scene & Visual ──
    pub(crate) scene_bridge: Option<Box<dyn SceneBridge>>,
    pub(crate) vgrc_controller: Option<crate::visual_system::VgcrController>,
    pub(crate) capture_pipeline: crate::capture_pipeline::MemoryCapturePipeline,

    // ── Visual Snapshot (for UI consumption) ──
    pub(crate) last_vgrc_result: Option<crate::visual_system::VgcrCycleResult>,
    pub(crate) last_vgrc_cycle: Option<crate::visual_system::VgrcCycleResult>,
    pub(crate) last_visual_observation: Option<crate::visual_system::VisualObservation>,

    // ── Configuration ──
    pub(crate) goal_checker_enabled: bool,

    // ── Rule System ──
    pub(crate) rule_system: crate::rule_system::RuleSystem,

    // ── Collaboration Extensions (from Ruflo & Multica) ──
    pub(crate) collaboration: Option<crate::agent_collaboration::AgentCollaborationSystem>,

    // ── Reasoning & Learning ──
    pub(crate) reasoning_bank: crate::reasoning_bank::ReasoningBankManager,

    // ── Squad ──
    pub(crate) squad: Option<crate::squad::Squad>,

    // ── Skill Compound ──
    pub(crate) compound: crate::skills_compound::SkillCompound,

    // ── Hybrid Controller State ──
    pub(crate) previous_llm_mode: String,

    // ── Persistence ──
    pub(crate) memory_dir: Option<String>,
}
