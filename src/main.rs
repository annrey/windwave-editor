//! AgentEdit - AI Agent Driven Game Editor

mod resources;
use crate::resources::*;

use bevy::prelude::*;
use bevy::window::WindowResolution;
use std::sync::OnceLock;

use agent_core::{
    Message, BaseAgent, AgentConfig, ToolRegistry,
    register_scene_tools, register_code_tools, register_file_tools,
    DirectorRuntime,
    SceneAgent, CodeAgent, ReviewAgent, PlannerAgent,
    AgentId, AgentRegistry,
    register_builtin_skills,
    create_empty_shared_bridge,
};
use agent_core::agent::AgentInstanceId;
use agent_ui::{ChatState, AgentRuntime, DirectorDeskState, UserAction, PendingApprovalInfo, EditorSelection, VisualUnderstandingState, VisualAnalysis, GoalCheckResult, VgrcCycleSummary};
use agent_ui::AgentUiPlugin;
use bevy_adapter::{
    BevyAdapter, AgentTracked, AgentEntityId,
    EngineCommand,
    BevyAdapterPlugin, RuntimeAgentPlugin, PerceptionPlugin, LlmRuntimeAgentPlugin,
    ScreenshotPlugin, ScreenshotQueue,
    CommandProcessorPlugin,
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
        register_file_tools(&mut tools);

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
            .insert_resource(AgentRuntime::new(agent))
            .insert_resource(AgentRegistryResource(registry))
            .insert_resource(DirectorResource(director));

        app.add_systems(Startup, (setup, setup_integration).chain())
            .add_systems(Update, (handle_agent_input, update_agent_status, sync_integration_state, handle_user_actions, vgrc_bridge_system));
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
    editor_selection: Res<EditorSelection>,
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
            UserAction::DeleteSelected => {
                // 获取当前选中的实体
                if let Some(bevy_entity) = editor_selection.selected_entity {
                    let entity_id = bevy_entity.index() as u64;
                    pending.commands.push(EngineCommand::DeleteEntity {
                        entity_id,
                    });
                    chat_state.add_message(Message::new_agent(format!("🗑 Deleted entity #{}", entity_id)));
                } else {
                    chat_state.add_message(Message::new_agent("⚠ No entity selected to delete".to_string()));
                }
            }
            UserAction::FocusSelected => {
                // 聚焦到选中的实体
                if let Some(bevy_entity) = editor_selection.selected_entity {
                    let entity_id = bevy_entity.index() as u64;
                    chat_state.add_message(Message::new_agent(format!("🎯 Focused on entity #{}", entity_id)));
                } else {
                    chat_state.add_message(Message::new_agent("⚠ No entity selected to focus".to_string()));
                }
            }
            UserAction::ToggleCommandPalette => {
                chat_state.add_message(Message::new_agent("📋 Command palette toggled".to_string()));
            }
            UserAction::RecheckLlm => {
                chat_state.add_message(Message::new_agent("🔄 Rechecking LLM connection...".to_string()));
            }
        }
    }
}

// ===========================================================================
// Visual Understanding Bridge — connects VGRC pipeline to UI state
// ===========================================================================

fn vgrc_bridge_system(
    mut director: ResMut<DirectorResource>,
    mut vis_state: ResMut<VisualUnderstandingState>,
    screenshot_queue: Res<ScreenshotQueue>,
    mut frame_count: Local<u64>,
) {
    *frame_count += 1;

    // Pop screenshot results and feed into visual state
    if let Some(result) = screenshot_queue.pop_result() {
        match result {
            bevy_adapter::ScreenshotResult::Success { path, dimensions, base64 } => {
                vis_state.update_screenshot(base64, dimensions);
                vis_state.add_vgrc_cycle(VgrcCycleSummary {
                    cycle_id: *frame_count as u32,
                    goal: "Verify scene state".into(),
                    vision_count: 1,
                    realize_attempts: 0,
                    check_passed: true,
                    total_duration_ms: 0,
                });
            }
            bevy_adapter::ScreenshotResult::Failure { error } => {
                bevy::log::warn!("Screenshot failed: {}", error);
            }
        }
    }

    // Check for new DirectorRuntime events and create analysis entries
    let events = director.0.drain_events();
    for event in &events {
        match event {
            agent_core::director::EditorEvent::StepCompleted { plan_id, step_id, title, result: step_result } => {
                vis_state.add_goal_check(GoalCheckResult {
                    timestamp: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_secs_f64())
                        .unwrap_or(0.0),
                    goal: format!("{} / {}", plan_id, title),
                    passed: true,
                    details: step_result.clone(),
                    matches: vec![],
                });
            }
            agent_core::director::EditorEvent::StepFailed { plan_id, step_id, title, error } => {
                vis_state.add_goal_check(GoalCheckResult {
                    timestamp: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_secs_f64())
                        .unwrap_or(0.0),
                    goal: format!("{} / {}", plan_id, title),
                    passed: false,
                    details: format!("Failed: {}", error),
                    matches: vec![],
                });
            }
            agent_core::director::EditorEvent::DirectExecutionCompleted { success, .. } => {
                if *success {
                    vis_state.add_goal_check(GoalCheckResult {
                        timestamp: std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .map(|d| d.as_secs_f64())
                            .unwrap_or(0.0),
                        goal: "Direct execution".into(),
                        passed: true,
                        details: "Execution completed".into(),
                        matches: vec![],
                    });
                }
            }
            _ => {}
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
            ScreenshotPlugin,
            CommandProcessorPlugin,
        ))
        .run();
}
