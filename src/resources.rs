//! Resources for the AgentEdit editor.

use agent_core::AgentRegistry;
use agent_core::DirectorRuntime;
use agent_core::EntityId;
use bevy::prelude::*;

pub use bevy_adapter::{CommandHistory, PendingCommands};

#[derive(Resource, Default)]
pub struct AgentSelection {
    #[allow(dead_code)]
    pub selected_entities: Vec<EntityId>,
}

#[derive(Resource)]
pub struct DirectorResource(pub DirectorRuntime);

#[derive(Resource)]
pub struct AgentRegistryResource(#[allow(dead_code)] pub AgentRegistry);
