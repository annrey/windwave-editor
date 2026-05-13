//! Editor command types — the top-level commands issued by the Director / User
//! to drive the editing workflow. These commands are dispatched to the agent
//! team and translated into lower-level engine operations.

use serde::{Deserialize, Serialize};

use crate::types::UserRequest;

/// High-level editor commands that orchestrate the agent pipeline.
///
/// These represent user intents ("create an edit plan", "approve this plan")
/// as well as system-level actions ("apply an engine command", "roll back").
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EditorCommand {
    /// Ask the Planner agent to produce an edit plan for the given request.
    CreateEditPlan { request: UserRequest },

    /// Approve a previously drafted edit plan.
    ApprovePlan { plan_id: String },

    /// Reject a plan, optionally providing a reason for the rejection.
    RejectPlan {
        plan_id: String,
        reason: Option<String>,
    },

    /// Execute all steps of a plan in sequence.
    ExecutePlan { plan_id: String },

    /// Execute a single step within a plan.
    ExecutePlanStep {
        plan_id: String,
        step_id: String,
    },

    /// Apply a low-level engine command inside a tracked transaction.
    ApplyEngineCommand {
        transaction_id: String,
        command: EngineCommand,
    },

    /// Roll back a previously committed / in-progress transaction.
    RollbackTransaction { transaction_id: String },

    /// Ask the Reviewer to check whether the current task goal is satisfied.
    CheckGoal { task_id: u64 },
}

/// Low-level engine command DSL — the canonical set of operations that
/// the Bevy adapter (and other engine adapters) expose to Agents.
///
/// Each variant represents a discrete, undoable mutation of engine state.
/// This enum is kept in sync with `bevy_adapter::EngineCommand`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EngineCommand {
    /// Create a new entity with the given name and optional component patches.
    CreateEntity {
        name: String,
        components: Vec<ComponentPatch>,
    },

    /// Delete an existing entity by its Agent entity ID.
    DeleteEntity { entity_id: u64 },

    /// Set the Transform (translation, rotation, scale) of an entity.
    /// Each field is optional so that partial updates are possible
    /// (e.g. only changing translation).
    SetTransform {
        entity_id: u64,
        translation: Option<[f32; 3]>,
        rotation: Option<[f32; 3]>,
        scale: Option<[f32; 3]>,
    },

    /// Set the Sprite color (RGBA) of an entity.
    SetSpriteColor {
        entity_id: u64,
        rgba: [f32; 4],
    },

    /// Set the visibility of an entity.
    SetVisibility {
        entity_id: u64,
        visible: bool,
    },
}

/// A patch that describes how to initialise or update a single component on an
/// entity. The `value` is a free-form JSON blob that will be interpreted by the
/// engine adapter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentPatch {
    /// The fully-qualified type name of the component (e.g. `"bevy_sprite::Sprite"`).
    pub type_name: String,

    /// Serialised component data.
    pub value: serde_json::Value,
}
