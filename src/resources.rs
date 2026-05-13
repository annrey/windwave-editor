//! Resources for the AgentEdit editor.

use bevy::prelude::*;
use agent_core::EntityId;
use agent_core::DirectorRuntime;
use agent_core::AgentRegistry;
use bevy_adapter::{EngineCommand, EngineCommandResult};

#[derive(Resource, Default)]
pub struct AgentSelection {
    pub selected_entities: Vec<EntityId>,
}

#[derive(Resource)]
pub struct DirectorResource(pub DirectorRuntime);

#[derive(Resource)]
pub struct AgentRegistryResource(pub AgentRegistry);

#[derive(Resource, Default)]
pub struct PendingCommands {
    pub commands: Vec<EngineCommand>,
    pub results: Vec<EngineCommandResult>,
}

#[derive(Resource, Default)]
pub struct CommandHistory {
    pub undo_stack: Vec<(Vec<EngineCommand>, Vec<EngineCommand>)>,
    pub redo_stack: Vec<(Vec<EngineCommand>, Vec<EngineCommand>)>,
}
