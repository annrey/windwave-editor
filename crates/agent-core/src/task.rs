//! Task types — the unit of work that drives the agent team. Every user request
//! is decomposed into one or more tasks, each tracked through a lifecycle that
//! spans planning, execution, and review.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Unique identifier for a task. Wraps a `u64` so it can double as a database
/// primary key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub struct TaskId(pub u64);

impl fmt::Display for TaskId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "task_{}", self.0)
    }
}

/// Lifecycle states a task moves through from creation to terminal outcome.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskStatus {
    /// Initial state before planning has started.
    Draft,

    /// The Planner is working on an edit plan for this task.
    Planning,

    /// The plan is being executed by executor agents.
    Running,

    /// Execution is paused waiting for user input / approval.
    WaitingForUser,

    /// The task cannot proceed (e.g. dependency not met).
    Blocked,

    /// All steps finished and the reviewer accepted the outcome.
    Done,

    /// An unrecoverable error occurred.
    Failed,

    /// The user explicitly cancelled the task.
    Cancelled,
}

/// A single task in the editing workflow.
///
/// Tasks form a tree via `parent` / `children`, enabling hierarchical
/// decomposition of complex editing requests.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    /// Unique identifier.
    pub id: TaskId,

    /// Short human-readable title (e.g. "Add player character").
    pub title: String,

    /// Longer description of what this task aims to accomplish.
    pub description: String,

    /// Current lifecycle status.
    pub status: TaskStatus,

    /// If this task has an associated goal state, its ID is stored here.
    pub goal_state_id: Option<String>,

    /// Agent IDs (u64) assigned to work on this task.
    pub assigned_agents: Vec<u64>,

    /// Parent task, if this is a sub-task.
    pub parent: Option<TaskId>,

    /// Child sub-tasks.
    pub children: Vec<TaskId>,

    /// Sequence numbers of events associated with this task (for traceability).
    pub events: Vec<u64>,
}

impl Task {
    /// Create a new task in `Draft` status.
    pub fn new(id: TaskId, title: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            id,
            title: title.into(),
            description: description.into(),
            status: TaskStatus::Draft,
            goal_state_id: None,
            assigned_agents: Vec::new(),
            parent: None,
            children: Vec::new(),
            events: Vec::new(),
        }
    }

    /// Returns `true` when the task has reached a terminal state.
    pub fn is_finished(&self) -> bool {
        matches!(
            self.status,
            TaskStatus::Done | TaskStatus::Failed | TaskStatus::Cancelled
        )
    }

    /// Returns `true` when the task is actively being worked on.
    pub fn is_active(&self) -> bool {
        matches!(
            self.status,
            TaskStatus::Planning | TaskStatus::Running
        )
    }
}
