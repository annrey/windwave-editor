use bevy::prelude::*;
use std::collections::HashMap;
use crate::{EngineCommand, EngineCommandResult, BevyAdapter, AgentEntityId};

/// Undo/redo history for engine commands.
#[derive(Resource, Default)]
pub struct CommandHistory {
    pub undo_stack: Vec<(Vec<EngineCommand>, Vec<EngineCommand>)>,
    pub redo_stack: Vec<(Vec<EngineCommand>, Vec<EngineCommand>)>,
}

/// Buffer of pending commands to be applied next frame.
#[derive(Resource, Default)]
pub struct PendingCommands {
    pub commands: Vec<EngineCommand>,
}

/// Processes buffered EngineCommands each frame: applies via BevyAdapter,
/// captures pre-state for undo, and maintains CommandHistory.
pub struct CommandProcessorPlugin;

impl Plugin for CommandProcessorPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PendingCommands>()
            .init_resource::<CommandHistory>()
            .add_systems(Update, process_pending_commands);
    }
}

fn process_pending_commands(world: &mut World) {
    let commands: Vec<EngineCommand> = {
        let mut pending = world.resource_mut::<PendingCommands>();
        std::mem::take(&mut pending.commands)
    };
    if commands.is_empty() { return; }

    let id_to_bevy: HashMap<u64, Entity> = {
        world.resource::<BevyAdapter>().entity_id_lookup()
    };

    let mut applied = 0u32;
    let mut failed = 0u32;
    let mut reverse_commands: Vec<EngineCommand> = Vec::new();
    let mut new_entity_ids: Vec<(Entity, String)> = Vec::new();

    for cmd in &commands {
        match cmd {
            EngineCommand::DeleteEntity { entity_id } => {
                let (name, sprite, transform, visibility) = id_to_bevy.get(entity_id)
                    .map(|&be| {
                        let n = world.get::<Name>(be).map(|n| n.to_string());
                        let s = world.get::<Sprite>(be).cloned();
                        let t = world.get::<Transform>(be).copied();
                        let v = world.get::<Visibility>(be).copied();
                        (n, s, t, v)
                    })
                    .unwrap_or_default();

                let result = world.resource_scope(|w, mut adapter: Mut<BevyAdapter>| {
                    adapter.apply_engine_command(cmd.clone(), w)
                });

                match result {
                    Ok(r) if r.success => {
                        applied += 1;
                        let name = name.unwrap_or_else(|| format!("entity_{}", entity_id));
                        let mut comps = Vec::new();
                        if let Some(s) = sprite {
                            let c = s.color.to_linear();
                            let props: HashMap<String, serde_json::Value> = [("color".into(), serde_json::json!([c.red, c.green, c.blue, c.alpha]))].into_iter().collect();
                            comps.push(crate::ComponentPatch { type_name: "Sprite".into(), value: serde_json::json!(props) });
                        }
                        reverse_commands.push(EngineCommand::CreateEntity { name, components: comps });

                        if let Some(t) = transform {
                            let (rx, ry, rz) = t.rotation.to_euler(EulerRot::XYZ);
                            reverse_commands.push(EngineCommand::SetTransform {
                                entity_id: *entity_id,
                                translation: Some(t.translation.to_array()),
                                rotation: Some([rx, ry, rz]),
                                scale: Some(t.scale.to_array()),
                            });
                        }
                        if let Some(v) = visibility {
                            reverse_commands.push(EngineCommand::SetVisibility {
                                entity_id: *entity_id,
                                visible: matches!(v, Visibility::Visible),
                            });
                        }
                    }
                    _ => { failed += 1; }
                }
            }

            EngineCommand::CreateEntity { .. } => {
                let result = world.resource_scope(|w, mut adapter: Mut<BevyAdapter>| {
                    adapter.apply_engine_command(cmd.clone(), w)
                });
                match result {
                    Ok(r) if r.success => {
                        applied += 1;
                        new_entity_ids.push((entity_for_create(&r), String::new()));
                        if let Some(eid) = r.entity_id {
                            reverse_commands.push(EngineCommand::DeleteEntity { entity_id: eid });
                        }
                    }
                    _ => { failed += 1; }
                }
            }

            EngineCommand::SetTransform { entity_id, .. } => {
                let old_transform = id_to_bevy.get(entity_id)
                    .and_then(|&be| world.get::<Transform>(be).copied());

                let result = world.resource_scope(|w, mut adapter: Mut<BevyAdapter>| {
                    adapter.apply_engine_command(cmd.clone(), w)
                });

                match result {
                    Ok(r) if r.success => {
                        applied += 1;
                        if let Some(old) = old_transform {
                            let (rx, ry, rz) = old.rotation.to_euler(EulerRot::XYZ);
                            reverse_commands.push(EngineCommand::SetTransform {
                                entity_id: *entity_id,
                                translation: Some(old.translation.to_array()),
                                rotation: Some([rx, ry, rz]),
                                scale: Some(old.scale.to_array()),
                            });
                        }
                    }
                    _ => { failed += 1; }
                }
            }

            EngineCommand::SetSpriteColor { entity_id, .. } => {
                let old_sprite = id_to_bevy.get(entity_id)
                    .and_then(|&be| world.get::<Sprite>(be).cloned());

                let result = world.resource_scope(|w, mut adapter: Mut<BevyAdapter>| {
                    adapter.apply_engine_command(cmd.clone(), w)
                });

                match result {
                    Ok(r) if r.success => {
                        applied += 1;
                        if let Some(old) = old_sprite {
                            let c = old.color.to_linear();
                            reverse_commands.push(EngineCommand::SetSpriteColor {
                                entity_id: *entity_id,
                                rgba: [c.red, c.green, c.blue, c.alpha],
                            });
                        }
                    }
                    _ => { failed += 1; }
                }
            }

            EngineCommand::SetVisibility { entity_id, .. } => {
                let old = id_to_bevy.get(entity_id)
                    .and_then(|&be| world.get::<Visibility>(be).copied());

                let result = world.resource_scope(|w, mut adapter: Mut<BevyAdapter>| {
                    adapter.apply_engine_command(cmd.clone(), w)
                });

                match result {
                    Ok(r) if r.success => {
                        applied += 1;
                        if let Some(v) = old {
                            reverse_commands.push(EngineCommand::SetVisibility {
                                entity_id: *entity_id,
                                visible: matches!(v, Visibility::Visible),
                            });
                        }
                    }
                    _ => { failed += 1; }
                }
            }

            EngineCommand::SetParent { child_entity_id, .. } => {
                let old_parent = id_to_bevy.get(child_entity_id)
                    .and_then(|&be| world.get::<ChildOf>(be).map(|co| co.parent()));

                let result = world.resource_scope(|w, mut adapter: Mut<BevyAdapter>| {
                    adapter.apply_engine_command(cmd.clone(), w)
                });

                match result {
                    Ok(r) if r.success => {
                        applied += 1;
                        if let Some(op) = old_parent {
                            let lookup = world.resource::<BevyAdapter>().entity_id_lookup();
                            if let Some((&aid, _)) = lookup.iter().find(|(_, &e)| e == op) {
                                reverse_commands.push(EngineCommand::SetParent {
                                    child_entity_id: *child_entity_id,
                                    parent_entity_id: aid,
                                });
                            }
                        } else {
                            reverse_commands.push(EngineCommand::RemoveFromParent {
                                entity_id: *child_entity_id,
                            });
                        }
                    }
                    _ => { failed += 1; }
                }
            }

            EngineCommand::RemoveFromParent { entity_id } => {
                let old_parent = id_to_bevy.get(entity_id)
                    .and_then(|&be| world.get::<ChildOf>(be).map(|co| co.parent()));

                let result = world.resource_scope(|w, mut adapter: Mut<BevyAdapter>| {
                    adapter.apply_engine_command(cmd.clone(), w)
                });

                match result {
                    Ok(r) if r.success => {
                        applied += 1;
                        if let Some(op) = old_parent {
                            let lookup = world.resource::<BevyAdapter>().entity_id_lookup();
                            if let Some((&aid, _)) = lookup.iter().find(|(_, &e)| e == op) {
                                reverse_commands.push(EngineCommand::SetParent {
                                    child_entity_id: *entity_id,
                                    parent_entity_id: aid,
                                });
                            }
                        }
                    }
                    _ => { failed += 1; }
                }
            }

            EngineCommand::ModifyComponent { entity_id, component_type, property, .. } => {
                let old_value = id_to_bevy.get(entity_id).and_then(|&be| {
                    match (component_type.as_str(), property.as_str()) {
                        ("Sprite", "color") => world.get::<Sprite>(be).map(|s| {
                            let c = s.color.to_linear();
                            serde_json::json!([c.red, c.green, c.blue, c.alpha])
                        }),
                        _ => None,
                    }
                });

                let result = world.resource_scope(|w, mut adapter: Mut<BevyAdapter>| {
                    adapter.apply_engine_command(cmd.clone(), w)
                });

                match result {
                    Ok(r) if r.success => {
                        applied += 1;
                        if let Some(ov) = old_value {
                            reverse_commands.push(EngineCommand::ModifyComponent {
                                entity_id: *entity_id,
                                component_type: component_type.clone(),
                                property: property.clone(),
                                value: ov,
                            });
                        }
                    }
                    _ => { failed += 1; }
                }
            }

            _ => {
                let result = world.resource_scope(|w, mut adapter: Mut<BevyAdapter>| {
                    adapter.apply_engine_command(cmd.clone(), w)
                });
                match result {
                    Ok(r) if r.success => {
                        applied += 1;
                        match cmd {
                            EngineCommand::ReparentChildren { source_parent_id, target_parent_id } => {
                                reverse_commands.push(EngineCommand::ReparentChildren {
                                    source_parent_id: *target_parent_id,
                                    target_parent_id: *source_parent_id,
                                });
                            }
                            _ => {}
                        }
                    }
                    _ => { failed += 1; }
                }
            }
        }
    }

    // Back-patch CreateEntity entity IDs in reverse commands for undo
    if !new_entity_ids.is_empty() {
        world.resource_scope(|w, mut adapter: Mut<BevyAdapter>| {
            let name_to_id: HashMap<String, u64> = new_entity_ids.iter()
                .filter_map(|(e, name)| {
                    if name.is_empty() { return None; }
                    let aid = adapter.register_entity(*e);
                    w.entity_mut(*e).insert(AgentEntityId(aid));
                    Some((name.to_string(), aid.0))
                })
                .collect();
            // DeleteEntity reverse commands are already pushed with the correct entity_id
            // from EngineCommandResult.entity_id, so no back-patching needed
        });
    }

    if !reverse_commands.is_empty() {
        let mut history = world.resource_mut::<CommandHistory>();
        history.undo_stack.push((commands.clone(), reverse_commands));
        history.redo_stack.clear();
    }

    info!("Engine commands: {} applied, {} failed. Undo entries: {}",
        applied, failed,
        world.resource::<CommandHistory>().undo_stack.len());
}

fn entity_for_create(_result: &EngineCommandResult) -> Entity {
    // The BevyAdapter has already registered the entity internally.
    // We don't have access to the Bevy Entity here — the result only carries
    // the agent_id. For undo tracking this is sufficient.
    Entity::PLACEHOLDER
}
