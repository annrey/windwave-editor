//! AgentEdit - AI Agent Driven Game Editor

mod resources;
use crate::resources::*;

use bevy::prelude::*;
use bevy::window::WindowResolution;
use std::collections::HashMap;
use std::sync::OnceLock;

use agent_core::{
    Message, BaseAgent, AgentConfig, ToolRegistry,
    register_scene_tools, register_code_tools,
    DirectorRuntime,
    SceneAgent, CodeAgent, ReviewAgent, PlannerAgent,
    AgentId, AgentRegistry,
    register_builtin_skills,
    create_empty_shared_bridge,
};
use agent_core::agent::AgentInstanceId;
use agent_core::EntityId;
use agent_ui::{ChatState, AgentRuntime, DirectorDeskState, UserAction, PendingApprovalInfo};
use agent_ui::AgentUiPlugin;
use bevy_adapter::{
    BevyAdapter, AgentTracked, AgentEntityId,
    EngineCommand,
    BevyAdapterPlugin, RuntimeAgentPlugin, PerceptionPlugin, LlmRuntimeAgentPlugin,
};
use bevy_adapter::scene_bridge_impl::BevySceneBridgePlugin;
use bevy_adapter::integration::{
    IntegrationState, SceneIndexCache, VisionState,
    SceneIndexSceneBridge,
    IntegrationPlugin,
    SceneIndexRebuildPlugin, SceneIndexIncrementalPlugin,
    VisionPlugin,
};

// ===========================================================================
// Plugin
// ===========================================================================

pub struct AgentCorePlugin;

impl Plugin for AgentCorePlugin {
    fn build(&self, app: &mut App) {
        let mut tools = ToolRegistry::new();
        let bridge = create_empty_shared_bridge();
        register_scene_tools(&mut tools, bridge);
        register_code_tools(&mut tools);

        let _config = AgentConfig::default();
        let agent = BaseAgent::new(AgentInstanceId(1), "Game Architect");

        let mut registry = AgentRegistry::new();
        registry.register(Box::new(SceneAgent::new(AgentId(100))));
        registry.register(Box::new(CodeAgent::new(AgentId(101))));
        registry.register(Box::new(ReviewAgent::new(AgentId(102))));
        registry.register(Box::new(PlannerAgent::new(AgentId(103))));

        let mut director = DirectorRuntime::new();
        register_builtin_skills(director.skill_registry_mut());

        app.init_resource::<AgentSelection>()
            .init_resource::<PendingCommands>()
            .init_resource::<CommandHistory>()
            .insert_resource(AgentRuntime::new(agent))
            .insert_resource(AgentRegistryResource(registry))
            .insert_resource(DirectorResource(director));

        app.add_systems(Startup, (setup, setup_integration).chain())
            .add_systems(Update, (handle_agent_input, update_agent_status, sync_integration_state, handle_user_actions))
            .add_systems(Update, apply_pending_commands);
    }
}

// ===========================================================================
// Startup systems
// ===========================================================================

fn setup(
    mut commands: Commands,
    mut adapter: ResMut<BevyAdapter>,
    mut chat_state: ResMut<ChatState>,
) {
    commands.spawn(Camera2d);

    let player = commands.spawn((
        Name::new("Player"),
        Transform::from_xyz(0.0, 0.0, 0.0),
        AgentTracked,
        Sprite::default(),
        Visibility::default(),
    )).id();

    let enemy1 = commands.spawn((
        Name::new("Enemy_01"),
        Transform::from_xyz(100.0, 50.0, 0.0),
        AgentTracked,
        Sprite::default(),
        Visibility::default(),
    )).id();

    let enemy2 = commands.spawn((
        Name::new("Enemy_02"),
        Transform::from_xyz(-100.0, 50.0, 0.0),
        AgentTracked,
        Sprite::default(),
        Visibility::default(),
    )).id();

    let player_agent_id = adapter.register_entity(player);
    commands.entity(player).insert(AgentEntityId(player_agent_id));

    let enemy1_agent_id = adapter.register_entity(enemy1);
    commands.entity(enemy1).insert(AgentEntityId(enemy1_agent_id));

    let enemy2_agent_id = adapter.register_entity(enemy2);
    commands.entity(enemy2).insert(AgentEntityId(enemy2_agent_id));

    chat_state.add_message(Message::new_agent(format!(
        "Scene ready: Player (id:{}), Enemy_01 (id:{}), Enemy_02 (id:{})",
        player_agent_id.0, enemy1_agent_id.0, enemy2_agent_id.0,
    )));
    chat_state.add_message(Message::new_agent("Say something or try: 'move player to left'".to_string()));
    info!("Startup: scene initialized with 3 entities");
}

fn setup_integration(
    mut director: ResMut<DirectorResource>,
    mut vision: ResMut<VisionState>,
) {
    let screenshots_dir = std::env::temp_dir()
        .join("agentedit_screenshots")
        .to_string_lossy()
        .to_string();
    director.0.enable_goal_checker();
    director.0.init_builtin_skills();
    vision.with_real_providers(screenshots_dir);
    info!("Integration pipeline: GoalChecker + Skills enabled, BevyScreenshotProvider + SceneIndexVision active");
}

// ===========================================================================
// Update systems
// ===========================================================================

static LLM_RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();

fn llm_runtime() -> &'static tokio::runtime::Runtime {
    LLM_RUNTIME.get_or_init(|| {
        tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime")
    })
}

fn handle_agent_input(
    mut chat_state: ResMut<ChatState>,
    mut director: ResMut<DirectorResource>,
    _agent_runtime: ResMut<AgentRuntime>,
    mut pending: ResMut<PendingCommands>,
    mut desk_state: ResMut<DirectorDeskState>,
    cache: Res<SceneIndexCache>,
) {
    // Process user messages from chat
    let mut pending_chat: Vec<Message> = Vec::new();
    for msg in &mut chat_state.messages {
        if matches!(msg.message_type, agent_core::MessageType::User) {
            let text = msg.content.clone();
            info!("User input: '{}'", text);

            msg.message_type = agent_core::MessageType::Observation;
            let bridge = SceneIndexSceneBridge::from_cache(&cache);
            director.0.set_scene_bridge(Box::new(bridge));

            if director.0.has_llm() {
                pending_chat.push(Message::new_agent("😑 Entering LLM planning mode...".to_string()));
                let text_clone = text.clone();
                let result = llm_runtime().block_on(async {
                    director.0.handle_user_request_async(&text_clone).await
                });
                for event in result {
                    if let agent_core::director::EditorEvent::PermissionRequested { plan_id, risk, reason, .. } = &event {
                        let plan = director.0.get_plan(plan_id);
                        desk_state.sync_pending_approval(PendingApprovalInfo {
                            plan_id: plan_id.clone(),
                            title: plan.map(|p| p.title.clone()).unwrap_or_default(),
                            risk: format!("{:?}", risk),
                            reason: reason.clone(),
                            step_count: plan.map(|p| p.steps.len()).unwrap_or(0),
                        });
                    }
                }
            } else {
                pending_chat.push(Message::new_agent("📋 Using rule-based fallback...".to_string()));
                let events = director.0.handle_user_request(&text);
                for event in &events {
                    if let agent_core::director::EditorEvent::PermissionRequested { plan_id, risk, reason, .. } = event {
                        let plan = director.0.get_plan(plan_id);
                        desk_state.sync_pending_approval(PendingApprovalInfo {
                            plan_id: plan_id.clone(),
                            title: plan.map(|p| p.title.clone()).unwrap_or_default(),
                            risk: format!("{:?}", risk),
                            reason: reason.clone(),
                            step_count: plan.map(|p| p.steps.len()).unwrap_or(0),
                        });
                    }
                }
            }

            let raw_cmds = director.0.drain_bridge_commands();
            let cmds: Vec<EngineCommand> = raw_cmds.into_iter()
                .filter_map(|v| serde_json::from_value(v).ok())
                .collect();
            if !cmds.is_empty() {
                for cmd in cmds {
                    pending.commands.push(cmd);
                }
            }

            let mut feedback = String::new();
            for plan in director.0.list_plans().iter().rev().take(3) {
                if !feedback.is_empty() { feedback.push('\n'); }
                feedback.push_str(&format!("[{:?}] {} ({} steps)", plan.status, plan.title, plan.steps.len()));
            }
            if feedback.is_empty() {
                feedback = "Plan processed — check the Director Desk for details.".into();
            }
            pending_chat.push(Message::new_agent(feedback));
            break; // Process one message per frame
        }
    }
    for msg in pending_chat {
        chat_state.add_message(msg);
    }
}

fn apply_pending_commands(world: &mut World) {
    let commands: Vec<EngineCommand> = {
        let mut pending = world.resource_mut::<PendingCommands>();
        std::mem::take(&mut pending.commands)
    };
    if commands.is_empty() { return; }

    let id_to_bevy: HashMap<u64, Entity> = {
        world.resource::<BevyAdapter>().entity_id_lookup()
    };

    let mut new_registrations: Vec<(Entity, String)> = Vec::new();
    let mut applied = 0;
    let mut failed = 0;
    let mut reverse_commands: Vec<EngineCommand> = Vec::new();
    let mut create_undo_queue: Vec<(usize, String)> = Vec::new();

    for cmd in &commands {
        match cmd {
            EngineCommand::CreateEntity { name, components } => {
                let mut ec = world.spawn((
                    Name::new(name.clone()),
                    Transform::default(),
                    Visibility::default(),
                    AgentTracked,
                ));
                for patch in components {
                    if patch.type_name.contains("Sprite") || patch.type_name.contains("sprite") {
                        let mut sprite = Sprite::default();
                        if let Some(color) = patch.value.get("color") {
                            if let Some(arr) = color.as_array() {
                                if arr.len() >= 4 {
                                    sprite.color = Color::linear_rgba(
                                        arr[0].as_f64().unwrap_or(1.0) as f32,
                                        arr[1].as_f64().unwrap_or(1.0) as f32,
                                        arr[2].as_f64().unwrap_or(1.0) as f32,
                                        arr[3].as_f64().unwrap_or(1.0) as f32,
                                    );
                                }
                            }
                        }
                        ec.insert(sprite);
                    }
                }
                let entity = ec.id();
                new_registrations.push((entity, name.clone()));
                applied += 1;
                let rev_idx = reverse_commands.len();
                reverse_commands.push(EngineCommand::DeleteEntity { entity_id: 0 });
                create_undo_queue.push((rev_idx, name.clone()));
            }
            EngineCommand::SetTransform { entity_id, translation, rotation, scale } => {
                if let Some(&bevy_entity) = id_to_bevy.get(entity_id) {
                    let old_transform = world.get::<Transform>(bevy_entity).copied();
                    if let Some(trans) = translation {
                        if let Some(mut transform) = world.get_mut::<Transform>(bevy_entity) {
                            transform.translation = Vec3::new(trans[0], trans[1], trans[2]);
                            applied += 1;
                        }
                    }
                    if let Some(rot) = rotation {
                        if let Some(mut transform) = world.get_mut::<Transform>(bevy_entity) {
                            transform.rotation = Quat::from_euler(EulerRot::XYZ, rot[0], rot[1], rot[2]);
                        }
                    }
                    if let Some(scl) = scale {
                        if let Some(mut transform) = world.get_mut::<Transform>(bevy_entity) {
                            transform.scale = Vec3::new(scl[0], scl[1], scl[2]);
                        }
                    }
                    if let Some(old) = old_transform {
                        let (rx, ry, rz) = old.rotation.to_euler(EulerRot::XYZ);
                        reverse_commands.push(EngineCommand::SetTransform {
                            entity_id: *entity_id,
                            translation: Some(old.translation.to_array()),
                            rotation: Some([rx, ry, rz]),
                            scale: Some(old.scale.to_array()),
                        });
                    }
                } else { failed += 1; }
            }
            EngineCommand::SetSpriteColor { entity_id, rgba } => {
                if let Some(&bevy_entity) = id_to_bevy.get(entity_id) {
                    let old_sprite = world.get::<Sprite>(bevy_entity).cloned();
                    if let Some(mut sprite) = world.get_mut::<Sprite>(bevy_entity) {
                        sprite.color = Color::linear_rgba(rgba[0], rgba[1], rgba[2], rgba[3]);
                        applied += 1;
                    }
                    if let Some(old) = old_sprite {
                        let c = old.color.to_linear();
                        reverse_commands.push(EngineCommand::SetSpriteColor {
                            entity_id: *entity_id,
                            rgba: [c.red, c.green, c.blue, c.alpha],
                        });
                    }
                } else { failed += 1; }
            }
            EngineCommand::DeleteEntity { entity_id } => {
                if let Some(&bevy_entity) = id_to_bevy.get(entity_id) {
                    let saved_name = world.get::<Name>(bevy_entity).map(|n| n.to_string());
                    let saved_sprite = world.get::<Sprite>(bevy_entity).cloned();
                    let saved_transform = world.get::<Transform>(bevy_entity).copied();
                    let saved_vis = world.get::<Visibility>(bevy_entity).copied();
                    world.despawn(bevy_entity);
                    applied += 1;

                    let name = saved_name.unwrap_or_else(|| format!("entity_{}", entity_id));
                    let mut comps: Vec<bevy_adapter::ComponentPatch> = Vec::new();
                    if let Some(s) = saved_sprite {
                        let c = s.color.to_linear();
                        let props: HashMap<String, serde_json::Value> = [("color".into(), serde_json::json!([c.red, c.green, c.blue, c.alpha]))].into_iter().collect();
                        comps.push(bevy_adapter::ComponentPatch { type_name: "Sprite".into(), value: serde_json::json!(props) });
                    }
                    let saved_tf = saved_transform.map(|t| {
                        let (x, y, z) = t.rotation.to_euler(EulerRot::XYZ);
                        (t.translation.to_array(), [x, y, z], t.scale.to_array())
                    });
                    reverse_commands.push(EngineCommand::CreateEntity { name, components: comps });
                    if let Some((trans, rot, scl)) = saved_tf {
                        reverse_commands.push(EngineCommand::SetTransform {
                            entity_id: *entity_id, translation: Some(trans), rotation: Some(rot), scale: Some(scl),
                        });
                    }
                    if let Some(vis) = saved_vis {
                        reverse_commands.push(EngineCommand::SetVisibility {
                            entity_id: *entity_id, visible: matches!(vis, Visibility::Visible),
                        });
                    }
                } else { failed += 1; }
            }
            EngineCommand::SetVisibility { entity_id, visible } => {
                if let Some(&bevy_entity) = id_to_bevy.get(entity_id) {
                    let old = world.get::<Visibility>(bevy_entity).copied();
                    if let Some(mut vis) = world.get_mut::<Visibility>(bevy_entity) {
                        *vis = if *visible { Visibility::Visible } else { Visibility::Hidden };
                        applied += 1;
                    }
                    if let Some(o) = old {
                        reverse_commands.push(EngineCommand::SetVisibility {
                            entity_id: *entity_id,
                            visible: matches!(o, Visibility::Visible),
                        });
                    }
                } else { failed += 1; }
            }
            EngineCommand::AddComponent { entity_id, component } => {
                if let Some(&_bevy_entity) = id_to_bevy.get(entity_id) {
                    // Capture pre-state for undo: remove the component
                    let _cmd = cmd.clone();
                    world.resource_scope(|world, mut adapter: Mut<BevyAdapter>| {
                        match adapter.apply_engine_command(_cmd, world) {
                            Ok(result) if result.success => { /* applied */ }
                            Ok(_) | Err(_) => { /* failed */ }
                        }
                    });
                    applied += 1;
                    reverse_commands.push(EngineCommand::RemoveComponent {
                        entity_id: *entity_id,
                        component_type: component.type_name.clone(),
                    });
                } else { failed += 1; }
            }
            EngineCommand::RemoveComponent { entity_id, component_type: _ } => {
                if let Some(&_bevy_entity) = id_to_bevy.get(entity_id) {
                    let _cmd = cmd.clone();
                    world.resource_scope(|world, mut adapter: Mut<BevyAdapter>| {
                        match adapter.apply_engine_command(_cmd, world) {
                            Ok(result) if result.success => { /* applied */ }
                            Ok(_) | Err(_) => { /* failed */ }
                        }
                    });
                    applied += 1;
                    // Undo: no way to re-add the removed component without full snapshot
                    // so we skip undo for RemoveComponent
                } else { failed += 1; }
            }
            EngineCommand::ModifyComponent { entity_id, component_type, property, value: _ } => {
                if let Some(&bevy_entity) = id_to_bevy.get(entity_id) {
                    // Capture old property value for undo (simplified: snapshot value from entity)
                    let old_value = match component_type.as_str() {
                        "Sprite" if property == "color" => world.get::<Sprite>(bevy_entity)
                            .map(|s| {
                                let c = s.color.to_linear();
                                serde_json::json!([c.red, c.green, c.blue, c.alpha])
                            }),
                        _ => None,
                    };

                    let _cmd = cmd.clone();
                    world.resource_scope(|world, mut adapter: Mut<BevyAdapter>| {
                        match adapter.apply_engine_command(_cmd, world) {
                            Ok(result) if result.success => { /* applied */ }
                            Ok(_) | Err(_) => { /* failed */ }
                        }
                    });
                    applied += 1;

                    if let Some(old_val) = old_value {
                        reverse_commands.push(EngineCommand::ModifyComponent {
                            entity_id: *entity_id,
                            component_type: component_type.clone(),
                            property: property.clone(),
                            value: old_val,
                        });
                    }
                } else { failed += 1; }
            }
            EngineCommand::SetParent { child_entity_id, parent_entity_id: _ } => {
                if let Some(&bevy_child) = id_to_bevy.get(child_entity_id) {
                    // Capture old parent (if any) for undo
                    let old_parent = world.get::<ChildOf>(bevy_child)
                        .map(|co| co.parent());

                    let _cmd = cmd.clone();
                    world.resource_scope(|world, mut adapter: Mut<BevyAdapter>| {
                        match adapter.apply_engine_command(_cmd, world) {
                            Ok(result) if result.success => { /* applied */ }
                            Ok(_) | Err(_) => { /* failed */ }
                        }
                    });
                    applied += 1;

                    // Reverse: restore old parent or remove from parent
                    if let Some(op) = old_parent {
                        // Look up the agent_id for old parent entity
                        let reverse: Option<EngineCommand> = {
                            let adapter = world.resource::<BevyAdapter>();
                            let lookup = adapter.entity_id_lookup();
                            lookup.iter()
                                .find(|(_, &e)| e == op)
                                .map(|(&aid, _)| EngineCommand::SetParent {
                                    child_entity_id: *child_entity_id,
                                    parent_entity_id: aid,
                                })
                        };
                        if let Some(r) = reverse {
                            reverse_commands.push(r);
                        } else {
                            reverse_commands.push(EngineCommand::RemoveFromParent {
                                entity_id: *child_entity_id,
                            });
                        }
                    } else {
                        reverse_commands.push(EngineCommand::RemoveFromParent {
                            entity_id: *child_entity_id,
                        });
                    }
                } else { failed += 1; }
            }
            EngineCommand::RemoveFromParent { entity_id } => {
                if let Some(&bevy_entity) = id_to_bevy.get(entity_id) {
                    // Capture old parent for undo
                    let old_parent = world.get::<ChildOf>(bevy_entity)
                        .map(|co| co.parent());

                    let _cmd = cmd.clone();
                    world.resource_scope(|world, mut adapter: Mut<BevyAdapter>| {
                        match adapter.apply_engine_command(_cmd, world) {
                            Ok(result) if result.success => { /* applied */ }
                            Ok(_) | Err(_) => { /* failed */ }
                        }
                    });
                    applied += 1;

                    if let Some(op) = old_parent {
                        let adapter = world.resource::<BevyAdapter>();
                        let lookup = adapter.entity_id_lookup();
                        if let Some((&aid, _)) = lookup.iter().find(|(_, &e)| e == op) {
                            reverse_commands.push(EngineCommand::SetParent {
                                child_entity_id: *entity_id,
                                parent_entity_id: aid,
                            });
                        }
                    }
                } else { failed += 1; }
            }
            _ => {
                world.resource_scope(|world, mut adapter: Mut<BevyAdapter>| {
                    match adapter.apply_engine_command(cmd.clone(), world) {
                        Ok(result) if result.success => {
                            applied += 1;
                            // Generate reverse commands for undo support
                            match cmd {
                                EngineCommand::ReparentChildren { source_parent_id, target_parent_id } => {
                                    // Reverse: reparent back to original parent
                                    reverse_commands.push(EngineCommand::ReparentChildren {
                                        source_parent_id: *target_parent_id,
                                        target_parent_id: *source_parent_id,
                                    });
                                }
                                EngineCommand::LoadAsset { .. } => {
                                    // LoadAsset is read-only, no undo needed
                                }
                                EngineCommand::SetSpriteTexture { entity_id, .. } => {
                                    // Capture old texture for undo (simplified: store current texture name)
                                    if let Some(&bevy_entity) = id_to_bevy.get(entity_id) {
                                        if let Some(_sprite) = world.get::<Sprite>(bevy_entity) {
                                            // Bevy 0.17 Sprite.image is Option<Handle<Image>>
                                            // Undo not fully implemented for texture swap
                                            let _ = entity_id;
                                        }
                                    }
                                }
                                EngineCommand::SpawnPrefab { asset_handle, transform } => {
                                    // SpawnPrefab creates new entities; undo is handled by DeleteEntity
                                    // We record the asset_handle for potential cleanup
                                    // Note: actual entity deletion requires tracking spawned entity IDs
                                    // For now, mark as needing manual cleanup
                                    bevy::log::warn!("SpawnPrefab undo not fully supported — entity tracking needed");
                                }
                                _ => {}
                            }
                        }
                        Ok(_) | Err(_) => { failed += 1; }
                    }
                });
            }
        }
    }

    world.resource_scope(|world, mut adapter: Mut<BevyAdapter>| {
        let new_id_map: HashMap<String, u64> = new_registrations.iter()
            .map(|(e, name)| {
                let agent_id = adapter.register_entity(*e);
                world.entity_mut(*e).insert(AgentEntityId(agent_id));
                (name.clone(), agent_id.0)
            })
            .collect();
        for (rev_idx, name) in &create_undo_queue {
            if let Some(&agent_id) = new_id_map.get(name) {
                reverse_commands[*rev_idx] = EngineCommand::DeleteEntity { entity_id: agent_id };
            }
        }
        for cmd in &commands {
            if let EngineCommand::DeleteEntity { entity_id } = cmd {
                adapter.unregister_entity(EntityId(*entity_id));
            }
        }
    });

    if !reverse_commands.is_empty() {
        let mut history = world.resource_mut::<CommandHistory>();
        history.undo_stack.push((commands.clone(), reverse_commands));
        history.redo_stack.clear();
    }

    info!("Engine commands: {} applied, {} failed, {} new. Undo entries: {}",
        applied, failed, new_registrations.len(),
        world.resource::<CommandHistory>().undo_stack.len());
}

fn update_agent_status(
    _time: Res<Time>,
    _state: Res<IntegrationState>,
    _cache: Res<SceneIndexCache>,
    _director: Res<DirectorResource>,
) {
    // Status updates handled in handle_agent_input
}

fn sync_integration_state(_state: Res<IntegrationState>) {
    // Sync handled by integration plugins
}

fn handle_user_actions(
    mut desk_state: ResMut<DirectorDeskState>,
    mut director: ResMut<DirectorResource>,
    mut chat_state: ResMut<ChatState>,
    mut pending: ResMut<PendingCommands>,
    mut history: ResMut<CommandHistory>,
) {
    let actions: Vec<UserAction> = std::mem::take(&mut desk_state.pending_actions);

    for action in actions {
        match action {
            UserAction::Approve { plan_id } => {
                let events = director.0.approve_plan(&plan_id);
                for event in &events {
                    if let agent_core::director::EditorEvent::PermissionResolved { approved: true, .. } = event {
                        desk_state.clear_pending_approval(&plan_id);
                        chat_state.add_message(Message::new_agent(format!("✅ Plan '{}' approved", plan_id)));
                    }
                    if let agent_core::director::EditorEvent::Error { message } = event {
                        chat_state.add_message(Message::new_agent(format!("❌ Error: {}", message)));
                    }
                }
                let raw_cmds = director.0.drain_bridge_commands();
                for cmd in raw_cmds.into_iter().filter_map(|v| serde_json::from_value::<EngineCommand>(v).ok()) {
                    pending.commands.push(cmd);
                }
            }
            UserAction::Reject { plan_id, reason } => {
                let events = director.0.reject_plan(&plan_id, reason.as_deref());
                for event in &events {
                    if let agent_core::director::EditorEvent::PermissionResolved { approved: false, .. } = event {
                        desk_state.clear_pending_approval(&plan_id);
                        chat_state.add_message(Message::new_agent(format!("❌ Plan '{}' rejected", plan_id)));
                    }
                }
            }
            UserAction::Undo => {
                if let Some((forward, reverse)) = history.undo_stack.pop() {
                    pending.commands.extend(reverse);
                    history.redo_stack.push((forward, vec![]));
                    chat_state.add_message(Message::new_agent("⬅ Undo successful".to_string()));
                } else {
                    chat_state.add_message(Message::new_agent("⚠ Nothing to undo".to_string()));
                }
            }
            UserAction::Redo => {
                if let Some((forward, _reverse)) = history.redo_stack.pop() {
                    pending.commands.extend(forward);
                    chat_state.add_message(Message::new_agent("➡ Redo successful".to_string()));
                } else {
                    chat_state.add_message(Message::new_agent("⚠ Nothing to redo".to_string()));
                }
            }
        }
    }
}

// ===========================================================================
// Main
// ===========================================================================

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "AgentEdit - AI Agent Driven Game Editor".into(),
                        resolution: WindowResolution::new(1600, 900),
                        ..default()
                    }),
                    ..default()
                }),
            AgentCorePlugin,
            AgentUiPlugin,
            BevyAdapterPlugin,
            BevySceneBridgePlugin,
            RuntimeAgentPlugin,
            PerceptionPlugin,
            LlmRuntimeAgentPlugin,
            SceneIndexRebuildPlugin::default(),
            SceneIndexIncrementalPlugin { fallback_interval: 300, full_rebuild_interval: 600 },
            IntegrationPlugin,
            VisionPlugin,
        ))
        .run();
}
