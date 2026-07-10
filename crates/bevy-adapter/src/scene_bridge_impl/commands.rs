use agent_core::scene_bridge::ComponentPatch;
use bevy::prelude::*;
use std::collections::HashMap;

/// Commands that can be queued for execution in Bevy systems
#[derive(Debug, Clone)]
pub enum SceneCommand {
    CreateEntity {
        name: String,
        position: Option<[f64; 2]>,
        components: Vec<ComponentPatch>,
    },
    UpdateComponent {
        entity_id: u64,
        component: String,
        properties: HashMap<String, serde_json::Value>,
    },
    DeleteEntity {
        entity_id: u64,
    },
    InstantiatePrefab {
        prefab_path: String,
        position: Option<[f64; 3]>,
        rotation: Option<[f64; 4]>,
        scale: Option<[f64; 3]>,
    },
    SaveScene {
        path: String,
    },
    LoadScene {
        path: String,
    },
}

/// Resource holding pending scene commands
#[derive(Resource, Default, Debug)]
pub struct SceneCommandQueue {
    pub commands: Vec<SceneCommand>,
    pub results: HashMap<u64, SceneCommandResult>,
    pub next_id: u64,
}

impl SceneCommandQueue {
    pub fn push(&mut self, cmd: SceneCommand) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.commands.push(cmd);
        id
    }

    pub fn take_commands(&mut self) -> Vec<SceneCommand> {
        std::mem::take(&mut self.commands)
    }

    pub fn store_result(&mut self, id: u64, result: SceneCommandResult) {
        self.results.insert(id, result);
    }
}

/// Result of a scene command execution
#[derive(Debug, Clone)]
pub enum SceneCommandResult {
    Success { entity_id: Option<u64> },
    Error(String),
}
