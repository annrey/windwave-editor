//! Trace types — an append-only audit log that records every meaningful action
//! during task execution. Traces enable the UI to show progress, let agents
//! inspect history, and support post-mortem debugging.

use serde::{Deserialize, Serialize};

/// An ordered, append-only log of execution events for a single task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionTrace {
    /// Unique trace identifier (typically `"trace_{task_id}"`).
    pub id: String,

    /// The task this trace belongs to.
    pub task_id: u64,

    /// Ordered list of entries (earliest first).
    pub entries: Vec<TraceEntry>,
}

impl ExecutionTrace {
    /// Create a new, empty trace for the given task.
    pub fn new(task_id: u64) -> Self {
        Self {
            id: format!("trace_{}", task_id),
            task_id,
            entries: Vec::new(),
        }
    }

    /// Create a trace with a custom ID.
    pub fn with_id(id: impl Into<String>, task_id: u64) -> Self {
        Self {
            id: id.into(),
            task_id,
            entries: Vec::new(),
        }
    }

    /// Append a new entry to the trace. Returns the index of the new entry.
    pub fn push_entry(&mut self, entry: TraceEntry) -> usize {
        let idx = self.entries.len();
        self.entries.push(entry);
        idx
    }

    /// Return the `n` most recent entries (newest first).
    pub fn list_recent(&self, n: usize) -> &[TraceEntry] {
        let len = self.entries.len();
        let start = len.saturating_sub(n);
        &self.entries[start..]
    }

    /// Return all entries.
    pub fn all_entries(&self) -> &[TraceEntry] {
        &self.entries
    }

    /// Total number of entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns `true` when the trace contains no entries.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// A single record in an execution trace.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceEntry {
    /// Wall-clock timestamp in milliseconds since UNIX epoch.
    pub timestamp_ms: u64,

    /// Who performed this action.
    pub actor: TraceActor,

    /// What kind of action was performed.
    pub kind: TraceEntryKind,

    /// Short human-readable summary (1-2 lines).
    pub summary: String,

    /// If this entry was triggered by an event, its sequence number.
    pub related_event_id: Option<u64>,

    /// If this entry belongs to a specific transaction.
    pub related_transaction_id: Option<String>,

    /// If this entry corresponds to a plan step.
    pub related_step_id: Option<String>,
}

impl TraceEntry {
    /// Create a minimal trace entry.
    pub fn new(
        timestamp_ms: u64,
        actor: TraceActor,
        kind: TraceEntryKind,
        summary: impl Into<String>,
    ) -> Self {
        Self {
            timestamp_ms,
            actor,
            kind,
            summary: summary.into(),
            related_event_id: None,
            related_transaction_id: None,
            related_step_id: None,
        }
    }

    /// Link this entry to a specific event.
    pub fn with_event(mut self, event_id: u64) -> Self {
        self.related_event_id = Some(event_id);
        self
    }

    /// Link this entry to a specific transaction.
    pub fn with_transaction(mut self, transaction_id: impl Into<String>) -> Self {
        self.related_transaction_id = Some(transaction_id.into());
        self
    }

    /// Link this entry to a specific plan step.
    pub fn with_step(mut self, step_id: impl Into<String>) -> Self {
        self.related_step_id = Some(step_id.into());
        self
    }
}

/// Who performed the action recorded in a trace entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TraceActor {
    User,
    Director,
    PlannerAgent,
    ExecutorAgent,
    ReviewerAgent,
    SceneAgent,
    CodeAgent,
    AssetAgent,
    VisionAgent,
    RuleAgent,
    System,
}

/// What kind of action was recorded.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TraceEntryKind {
    /// The user submitted a new request.
    UserRequest,

    /// The Planner drafted a new edit plan.
    PlanCreated,

    /// The user or Director approved a plan.
    PlanApproved,

    /// The user or Director rejected a plan.
    PlanRejected,

    /// Execution of a plan step started.
    StepStarted,

    /// A plan step finished successfully.
    StepCompleted,

    /// A plan step failed.
    StepFailed,

    /// A new transaction was opened.
    TransactionStarted,

    /// A transaction was committed.
    TransactionCommitted,

    /// A transaction was rolled back.
    TransactionRolledBack,

    /// An agent invoked a tool.
    ToolCalled,

    /// An observation was collected from the engine / project.
    ObservationCollected,

    /// A goal was checked against current state.
    GoalChecked,

    /// The Reviewer made a decision.
    ReviewDecision,

    /// A purely informational system message.
    SystemMessage,

    /// An error occurred.
    Error,
}
