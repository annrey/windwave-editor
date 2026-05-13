//! Plan types — the output of the Planner agent. An edit plan breaks a task
//! down into sequential steps, each targeting a specific module, with risk
//! annotations and validation requirements.

use serde::{Deserialize, Serialize};

use crate::permission::OperationRisk;

/// A structured edit plan that decomposes a task into actionable steps.
///
/// Plans go through a lifecycle (draft -> approval -> execution) and are the
/// primary artefact exchanged between Planner, Director, and Executor agents.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EditPlan {
    /// Unique plan identifier (e.g. `"plan_add_player_001"`).
    pub id: String,

    /// The task this plan was created for.
    pub task_id: u64,

    /// Short title for display in the UI.
    pub title: String,

    /// Narrative summary of what the plan will accomplish.
    pub summary: String,

    /// How the plan should be executed (Direct, Plan, or Team).
    pub mode: ExecutionMode,

    /// Ordered list of steps to execute.
    pub steps: Vec<EditPlanStep>,

    /// The highest risk level among all steps.
    pub risk_level: OperationRisk,

    /// Current state in the plan lifecycle.
    pub status: EditPlanStatus,
}

impl EditPlan {
    /// Create a new draft plan.
    pub fn new(
        id: impl Into<String>,
        task_id: u64,
        title: impl Into<String>,
        summary: impl Into<String>,
        mode: ExecutionMode,
    ) -> Self {
        Self {
            id: id.into(),
            task_id,
            title: title.into(),
            summary: summary.into(),
            mode,
            steps: Vec::new(),
            risk_level: OperationRisk::Safe,
            status: EditPlanStatus::Draft,
        }
    }

    /// Add a step and automatically update the overall `risk_level` to the max
    /// of all steps.
    pub fn add_step(&mut self, step: EditPlanStep) {
        // Update risk to the maximum seen so far.
        let step_risk = step.risk;
        if step_risk > self.risk_level {
            self.risk_level = step_risk;
        }
        self.steps.push(step);
    }

    /// Total number of steps.
    pub fn step_count(&self) -> usize {
        self.steps.len()
    }
}

/// The execution strategy for a plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExecutionMode {
    /// Execute immediately without formal planning (single-step, low risk).
    Direct,

    /// Execute sequentially through a list of steps.
    Plan,

    /// Delegate execution to a team of specialist agents.
    Team,
}

/// Plan lifecycle states.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EditPlanStatus {
    /// The plan is being authored.
    Draft,

    /// Submitted for user approval.
    WaitingForApproval,

    /// The user approved the plan.
    Approved,

    /// Steps are currently being executed.
    Running,

    /// All steps completed successfully.
    Completed,

    /// One or more steps failed and the plan cannot recover.
    Failed,

    /// The user rejected the plan before execution.
    Rejected,
}

/// A single step within an edit plan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EditPlanStep {
    /// Unique step identifier within the plan (e.g. `"step_01"`).
    pub id: String,

    /// Short title for display.
    pub title: String,

    /// Which subsystem this step affects.
    pub target_module: TargetModule,

    /// Natural-language description of the action to perform.
    pub action_description: String,

    /// How risky this individual step is.
    pub risk: OperationRisk,

    /// Conditions that must hold for this step to be considered successful.
    pub validation_requirements: Vec<String>,
}

impl EditPlanStep {
    pub fn new(
        id: impl Into<String>,
        title: impl Into<String>,
        target_module: TargetModule,
        action_description: impl Into<String>,
        risk: OperationRisk,
    ) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            target_module,
            action_description: action_description.into(),
            risk,
            validation_requirements: Vec::new(),
        }
    }
}

/// Which part of the game project a step operates on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TargetModule {
    /// Entity hierarchy, transforms, components.
    Scene,

    /// Source code files (Rust scripts, systems, etc.).
    Code,

    /// Binary or text assets (sprites, sounds, data files).
    Asset,

    /// Visual layout / camera / rendering.
    Vision,

    /// Gameplay rules, physics, state machines.
    Rule,

    /// Agent workflows, pipelines, tool chains.
    Workflow,
}

// ---------------------------------------------------------------------------
// Expected change previews — what the plan predicts will change.
// ---------------------------------------------------------------------------

/// A structured preview of what the plan will modify across the project.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExpectedChangeSet {
    pub files: Vec<FileChangePreview>,
    pub scene: Vec<SceneChangePreview>,
    pub assets: Vec<AssetChangePreview>,
    pub mechanics: Vec<MechanicChangePreview>,
}

impl ExpectedChangeSet {
    pub fn new() -> Self {
        Self {
            files: Vec::new(),
            scene: Vec::new(),
            assets: Vec::new(),
            mechanics: Vec::new(),
        }
    }

    /// Total number of changes across all categories.
    pub fn total_changes(&self) -> usize {
        self.files.len() + self.scene.len() + self.assets.len() + self.mechanics.len()
    }

    pub fn is_empty(&self) -> bool {
        self.total_changes() == 0
    }
}

impl Default for ExpectedChangeSet {
    fn default() -> Self {
        Self::new()
    }
}

/// Preview of a file-system change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileChangePreview {
    /// Relative path within the project.
    pub path: String,

    /// What kind of modification.
    pub change_kind: ChangeKind,

    /// Why this change is necessary.
    pub reason: String,
}

/// Preview of a scene-graph change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SceneChangePreview {
    /// Name of the affected entity, if known.
    pub entity_name: Option<String>,

    /// What kind of modification.
    pub change_kind: ChangeKind,

    /// Human-readable description.
    pub description: String,
}

/// Preview of an asset change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetChangePreview {
    /// Relative asset path.
    pub path: String,

    /// What kind of modification.
    pub change_kind: ChangeKind,

    /// Why this change is necessary.
    pub reason: String,
}

/// Preview of a game-mechanic change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanicChangePreview {
    /// Which layer of the game the mechanic belongs to.
    pub layer: MechanicLayer,

    /// Human-readable description.
    pub description: String,
}

/// Whether the change creates, modifies, deletes, moves, or merely inspects.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChangeKind {
    Create,
    Modify,
    Delete,
    Move,
    Inspect,
}

/// Which architectural layer a mechanic operates at.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MechanicLayer {
    /// Core simulation (physics, input, game state).
    World,

    /// Rendering / UI / audio.
    Presentation,

    /// Mechanics that span both World and Presentation.
    Mixed,
}
