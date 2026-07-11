use crate::interactable::InteractionCompleteEvent;
use agent_core::application::{OpenWorldRuntimeError, OpenWorldRuntimeState};
use bevy::prelude::*;
use bevy_adapter::{
    process_open_world_combat_interaction, process_open_world_loot_interaction, OpenWorldObject,
};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpenWorldInteractionCommand {
    Loot {
        actor_id: String,
        container_id: String,
        reward_id: String,
    },
    Combat {
        actor_id: String,
        enemy_id: String,
    },
}

#[derive(Resource, Debug, Clone, Default, PartialEq, Eq)]
pub struct OpenWorldInteractionCommandQueue {
    pub pending: Vec<OpenWorldInteractionCommand>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpenWorldInteractionBinding {
    Loot { reward_id: String },
    Combat,
}

#[derive(Resource, Debug, Clone, Default, PartialEq, Eq)]
pub struct OpenWorldInteractionBindings {
    bindings: BTreeMap<String, OpenWorldInteractionBinding>,
}

impl OpenWorldInteractionBindings {
    pub fn bind_loot(
        &mut self,
        target_id: impl Into<String>,
        reward_id: impl Into<String>,
    ) -> &mut Self {
        self.bindings.insert(
            target_id.into(),
            OpenWorldInteractionBinding::Loot {
                reward_id: reward_id.into(),
            },
        );
        self
    }

    pub fn bind_combat(&mut self, target_id: impl Into<String>) -> &mut Self {
        self.bindings
            .insert(target_id.into(), OpenWorldInteractionBinding::Combat);
        self
    }

    fn binding_for(&self, target_id: &str) -> Option<&OpenWorldInteractionBinding> {
        self.bindings.get(target_id)
    }
}

pub fn enqueue_open_world_interaction_commands(
    mut events: MessageReader<InteractionCompleteEvent>,
    objects: Query<&OpenWorldObject>,
    bindings: Res<OpenWorldInteractionBindings>,
    mut queue: ResMut<OpenWorldInteractionCommandQueue>,
) {
    for event in events.read() {
        if !event.success {
            continue;
        }

        let Ok(actor) = objects.get(event.player) else {
            continue;
        };
        let Ok(target) = objects.get(event.target) else {
            continue;
        };

        match bindings.binding_for(&target.object_id) {
            Some(OpenWorldInteractionBinding::Loot { reward_id }) => {
                queue.pending.push(OpenWorldInteractionCommand::Loot {
                    actor_id: actor.object_id.clone(),
                    container_id: target.object_id.clone(),
                    reward_id: reward_id.clone(),
                });
            }
            Some(OpenWorldInteractionBinding::Combat) => {
                queue.pending.push(OpenWorldInteractionCommand::Combat {
                    actor_id: actor.object_id.clone(),
                    enemy_id: target.object_id.clone(),
                });
            }
            None => {}
        }
    }
}

pub fn drain_open_world_interaction_commands(
    world: &mut World,
    runtime: &mut OpenWorldRuntimeState,
    queue: &mut OpenWorldInteractionCommandQueue,
) -> Vec<Result<(), OpenWorldRuntimeError>> {
    let pending = std::mem::take(&mut queue.pending);
    pending
        .into_iter()
        .map(|command| match command {
            OpenWorldInteractionCommand::Loot {
                actor_id,
                container_id,
                reward_id,
            } => process_open_world_loot_interaction(
                world,
                runtime,
                &actor_id,
                &container_id,
                &reward_id,
            ),
            OpenWorldInteractionCommand::Combat { actor_id, enemy_id } => {
                process_open_world_combat_interaction(world, runtime, &actor_id, &enemy_id)
            }
        })
        .collect()
}
