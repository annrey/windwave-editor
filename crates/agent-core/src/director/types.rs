//! DirectorRuntime types, enums, and helper structs.

use crate::event::EventBus;
use crate::skill::{SkillRegistry, SkillExecutor, SkillActionHandler};
use crate::rollback::RollbackManager;
use crate::fallback::FallbackEngine;
use crate::metrics::AgentMetrics;
use crate::prompt::PromptSystem;
use crate::registry::AgentRegistry;
use crate::scene_bridge::SceneBridge;
use crate::strategy::ReActAgent;
use super::plan_manager::PlanManager;

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
                let position = params.get("position").and_then(|v| v.as_array()).map(|arr| {
                    [arr[0].as_f64().unwrap_or(0.0), arr[1].as_f64().unwrap_or(0.0)]
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
                let entity_id = params.get("entity_id").and_then(|v| v.as_u64()).unwrap_or(0);
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
                let entity_id = params.get("entity_id").and_then(|v| v.as_u64()).unwrap_or(0);
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
                let filter = params
                    .get("filter")
                    .and_then(|v| v.as_str());
                let comp_type = params
                    .get("component_type")
                    .and_then(|v| v.as_str());
                let entities = self.bridge.query_entities(filter, comp_type);
                Ok(serde_json::to_value(entities).unwrap_or(serde_json::Value::Null))
            }
            "delete_entity" => {
                let entity_id = params.get("entity_id").and_then(|v| v.as_u64()).unwrap_or(0);
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
/// # State
///
/// - `plans`: all plans currently managed by the runtime (active and completed).
/// - `pending_approvals`: list of plan IDs waiting for user confirmation.
/// - `events`: all events emitted (for subscribers to consume).
/// - `trace_entries`: detailed execution trace for debugging.
pub struct DirectorRuntime {
    /// Plan lifecycle management (CRUD, approval, permission).
    pub(crate) plan_manager: PlanManager,
    /// Log of all emitted events.
    pub(crate) events: Vec<EditorEvent>,
    /// Detailed execution trace for debugging / audit purposes.
    pub(crate) trace_entries: Vec<DirectorTraceEntry>,
    /// Whether GoalChecker validation is enabled (MVP: false; Phase 2: true).
    pub(crate) goal_checker_enabled: bool,
    pub(crate) scene_bridge: Option<Box<dyn SceneBridge>>,
    /// EventBus for publishing key events to UI / engine subscribers.
    pub(crate) event_bus: EventBus,
    /// Skill system for structured DAG-based execution.
    pub(crate) skill_registry: SkillRegistry,
    pub(crate) skill_executor: SkillExecutor,
    /// Operation log + undo/redo stacks.
    pub(crate) rollback_manager: RollbackManager,
    /// LLM unavailable → local rule engine fallback.
    pub(crate) fallback_engine: FallbackEngine,
    /// Aggregated performance metrics.
    pub(crate) metrics: AgentMetrics,
    /// Layered prompt engineering system.
    pub(crate) prompt_system: PromptSystem,
    /// Optional agent registry for Team mode dispatch.
    pub(crate) agent_registry: Option<AgentRegistry>,
    /// Optional LLM client for AI-powered planning and tool selection.
    /// When None, falls back to rule-based systems.
    pub(crate) llm_client: Option<Box<dyn crate::llm::LlmClient>>,
    /// Inter-agent communication hub (pub/sub messaging + shared context).
    pub(crate) comm_hub: crate::agent_comm::CommunicationHub,
    /// Fine-grained EditOp-based undo/redo history (SuperSplat-inspired).
    pub(crate) edit_history: crate::edit_history::EditHistory,
    /// Append-only cryptographically-linked audit log.
    pub(crate) audit_log: crate::audit::AuditLog,
    /// ReAct Agent for LLM-driven think-act-observe execution loop (Sprint 1).
    /// When Some, uses ReAct for Direct/Plan execution instead of keyword matching.
    pub(crate) react_agent: Option<ReActAgent>,
    /// Memory system for four-tier memory (Working/Episodic/Semantic/Procedural).
    pub(crate) memory_system: crate::memory::MemorySystem,
    /// Sprint 2: MemoryInjector for automatic context capture and LLM injection.
    pub(crate) memory_injector: crate::memory_injector::MemoryInjector,
}
