//! Goal types — the specification language used by the Reviewer agent to
//! determine whether a task has been completed successfully. Goals are
//! composed of concrete requirements that can be checked against engine state.

use serde::{Deserialize, Serialize};

/// A named collection of requirements that together define the desired outcome
/// for a task.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GoalState {
    /// Unique identifier for this goal state (e.g. `"goal_add_player"`).
    pub id: String,

    /// The individual conditions that must all be satisfied.
    pub requirements: Vec<GoalRequirement>,

    /// Observations collected by agents while checking this goal.
    pub observations: Vec<String>,

    /// Historical check results (most recent last).
    pub checks: Vec<GoalCheckResult>,

    /// The task this goal is attached to, if any.
    pub task_id: Option<u64>,
}

impl GoalState {
    /// Create an empty goal state with the given ID.
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            requirements: Vec::new(),
            observations: Vec::new(),
            checks: Vec::new(),
            task_id: None,
        }
    }

    /// Attach this goal to a specific task.
    pub fn with_task(mut self, task_id: u64) -> Self {
        self.task_id = Some(task_id);
        self
    }

    /// Add a requirement to this goal.
    pub fn add_requirement(&mut self, requirement: GoalRequirement) {
        self.requirements.push(requirement);
    }

    /// Record a check result.
    pub fn record_check(&mut self, result: GoalCheckResult) {
        self.checks.push(result);
    }

    /// Get the most recent check result, if any.
    pub fn latest_check(&self) -> Option<&GoalCheckResult> {
        self.checks.last()
    }
}

/// The kind of condition that must be satisfied.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum GoalRequirementKind {
    /// An entity with this name must exist in the scene.
    EntityExists { name: String },

    /// An entity must have a component of the given type.
    HasComponent {
        entity_name: String,
        component: String,
    },

    /// An entity's Transform translation must be within `tolerance` of the given
    /// `[x, y, z]` coordinates.
    TransformNear {
        entity_name: String,
        translation: [f32; 3],
        tolerance: f32,
    },

    /// An entity's Sprite colour must exactly match the given RGBA values.
    SpriteColorIs {
        entity_name: String,
        rgba: [f32; 4],
    },
}

/// A single requirement inside a `GoalState`, combining a concrete check with a
/// human-readable description.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GoalRequirement {
    /// Unique ID for this requirement within the goal (e.g. `"req_player_exists"`).
    pub id: String,

    /// The concrete condition to evaluate.
    pub kind: GoalRequirementKind,

    /// Human-readable explanation of what this requirement checks.
    pub description: String,
}

impl GoalRequirement {
    pub fn new(
        id: impl Into<String>,
        kind: GoalRequirementKind,
        description: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            kind,
            description: description.into(),
        }
    }
}

/// The aggregated result of checking all requirements in a goal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GoalCheckResult {
    /// Per-requirement pass/fail results.
    pub requirement_results: Vec<GoalRequirementResult>,

    /// `true` only when every requirement matched.
    pub all_matched: bool,
}

impl GoalCheckResult {
    /// Create a check result from individual requirement results.
    pub fn new(requirement_results: Vec<GoalRequirementResult>) -> Self {
        let all_matched = requirement_results.iter().all(|r| r.matched);
        Self {
            requirement_results,
            all_matched,
        }
    }
}

/// The outcome of checking a single `GoalRequirement`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GoalRequirementResult {
    /// The ID of the requirement that was checked.
    pub requirement_id: String,

    /// `true` if the condition was satisfied.
    pub matched: bool,

    /// Human-readable description of the requirement.
    pub description: String,

    /// Optional diagnostic message (e.g. "expected translation [0,0,0], got [1,2,3]").
    pub message: Option<String>,
}

impl GoalRequirementResult {
    pub fn matched(requirement_id: impl Into<String>) -> Self {
        Self {
            requirement_id: requirement_id.into(),
            matched: true,
            description: String::new(),
            message: None,
        }
    }

    pub fn mismatched(
        requirement_id: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            requirement_id: requirement_id.into(),
            matched: false,
            description: String::new(),
            message: Some(message.into()),
        }
    }
}

/// Quick-lookup status for a single requirement during a check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GoalCheckStatus {
    /// The requirement has not been checked yet.
    Unknown,

    /// The requirement was satisfied.
    Matched,

    /// The requirement was not satisfied.
    Mismatched,
}
