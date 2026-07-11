//! AgentEdit - AI Agent Driven Game Editor

mod resources;
use crate::resources::*;

use bevy::prelude::*;
use bevy::window::WindowResolution;

use agent_core::agent::AgentInstanceId;
use agent_core::keyword_matcher::KeywordMatcher;
use agent_core::{
    create_empty_shared_bridge, register_builtin_skills, register_code_tools, register_file_tools,
    register_scene_tools, AgentConfig, AgentId, AgentRegistry, BaseAgent, CodeAgent,
    DirectorRuntime, EditorMode, HybridLlmStatus, Message, PlannerAgent, ReviewAgent, SceneAgent,
    ToolRegistry,
};
use agent_ui::AgentUiPlugin;
use agent_ui::{
    AgentRuntime, ChatState, DirectorDeskState, EditorSelection, GoalCheckResult,
    PendingApprovalInfo, UserAction, VgrcCycleSummary, VisualAnalysis, VisualUnderstandingState,
};
use bevy_adapter::integration::{
    IntegrationPlugin, SceneIndexCache, SceneIndexIncrementalPlugin, SceneIndexRebuildPlugin,
    SceneIndexSceneBridge, VisionPlugin, VisionState,
};
use bevy_adapter::scene_bridge_impl::BevySceneBridgePlugin;
use bevy_adapter::{
    AgentEntityId, AgentTracked, BevyAdapter, BevyAdapterPlugin, CommandProcessorPlugin,
    EngineCommand, LlmRuntimeAgentPlugin, PerceptionPlugin, RuntimeAgentPlugin, ScreenshotQueue,
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
            .add_systems(
                Update,
                (
                    handle_agent_input,
                    handle_user_actions,
                    vgrc_bridge_system,
                    sync_hybrid_status,
                ),
            );
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

    let player = commands
        .spawn((
            Name::new("Player"),
            Transform::from_xyz(0.0, 0.0, 0.0),
            AgentTracked,
            Sprite::default(),
            Visibility::default(),
        ))
        .id();

    let enemy1 = commands
        .spawn((
            Name::new("Enemy_01"),
            Transform::from_xyz(100.0, 50.0, 0.0),
            AgentTracked,
            Sprite::default(),
            Visibility::default(),
        ))
        .id();

    let enemy2 = commands
        .spawn((
            Name::new("Enemy_02"),
            Transform::from_xyz(-100.0, 50.0, 0.0),
            AgentTracked,
            Sprite::default(),
            Visibility::default(),
        ))
        .id();

    let player_agent_id = adapter.register_entity(player);
    commands
        .entity(player)
        .insert(AgentEntityId(player_agent_id));

    let enemy1_agent_id = adapter.register_entity(enemy1);
    commands
        .entity(enemy1)
        .insert(AgentEntityId(enemy1_agent_id));

    let enemy2_agent_id = adapter.register_entity(enemy2);
    commands
        .entity(enemy2)
        .insert(AgentEntityId(enemy2_agent_id));

    chat_state.add_message(Message::new_agent(format!(
        "Scene ready: Player (id:{}), Enemy_01 (id:{}), Enemy_02 (id:{})",
        player_agent_id.0, enemy1_agent_id.0, enemy2_agent_id.0,
    )));
    chat_state.add_message(Message::new_agent(
        "Say something or try: 'move player to left'".to_string(),
    ));
    info!("Startup: scene initialized with 3 entities");
}

fn setup_integration(mut director: ResMut<DirectorResource>, mut vision: ResMut<VisionState>) {
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

fn sync_director_events_to_desk(
    desk_state: &mut DirectorDeskState,
    director: &DirectorRuntime,
    events: &[agent_core::EditorEvent],
) {
    for event in events {
        if let agent_core::director::EditorEvent::PermissionRequested {
            plan_id,
            risk,
            reason,
        } = event
        {
            let plan = director.get_plan(plan_id);
            desk_state.sync_pending_approval(PendingApprovalInfo {
                plan_id: plan_id.clone(),
                title: plan
                    .map(|p| p.title.clone())
                    .unwrap_or_else(|| plan_id.clone()),
                risk: format!("{:?}", risk),
                reason: reason.clone(),
                step_count: plan.map(|p| p.steps.len()).unwrap_or(0),
            });
        }
    }
}

fn sync_director_pending_ids_to_desk(
    desk_state: &mut DirectorDeskState,
    director: &DirectorRuntime,
) {
    for pending_id in director.pending_approval_ids() {
        let plan = director.get_plan(&pending_id);
        desk_state.sync_pending_approval(PendingApprovalInfo {
            plan_id: pending_id.clone(),
            title: plan
                .map(|p| p.title.clone())
                .unwrap_or_else(|| pending_id.clone()),
            risk: plan
                .map(|p| format!("{:?}", p.risk_level))
                .unwrap_or_else(|| "HighRisk".to_string()),
            reason: String::new(),
            step_count: plan.map(|p| p.steps.len()).unwrap_or(0),
        });
    }
}

fn handle_agent_input(
    mut chat_state: ResMut<ChatState>,
    mut director: ResMut<DirectorResource>,
    _agent_runtime: ResMut<AgentRuntime>,
    mut pending: ResMut<PendingCommands>,
    mut desk_state: ResMut<DirectorDeskState>,
    mut vis_state: ResMut<VisualUnderstandingState>,
    cache: Res<SceneIndexCache>,
) {
    let mut pending_chat: Vec<Message> = Vec::new();
    for msg in &mut chat_state.messages {
        if matches!(msg.message_type, agent_core::MessageType::User) {
            let text = msg.content.clone();
            info!("User input: '{}'", text);

            msg.message_type = agent_core::MessageType::Observation;
            let bridge = SceneIndexSceneBridge::from_cache(&cache);
            director.0.set_scene_bridge(Box::new(bridge));

            if KeywordMatcher::is_hr_request(&text) {
                pending_chat.push(Message::new_agent(
                    "\u{1F4CB} Routing HR request to team management...".to_string(),
                ));
                let events = director.0.dispatch_to_registered_agent(&text);
                sync_director_events_to_desk(&mut desk_state, &director.0, &events);
            } else if director.0.has_llm() {
                pending_chat.push(Message::new_agent(
                    "\u{1F4CB} Entering LLM planning mode...".to_string(),
                ));
                let _response = director.0.execute_with_llm(&text);
            } else {
                pending_chat.push(Message::new_agent(
                    "\u{1F4CB} Using rule-based fallback...".to_string(),
                ));
                let events = director.0.handle_user_request(&text);
                sync_director_events_to_desk(&mut desk_state, &director.0, &events);
            }

            // Update VisualUnderstandingState from VGRC snapshot
            if let Some((vgrc_result, observation)) = director.0.visual_snapshot() {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs_f64())
                    .unwrap_or(0.0);

                let analysis = VisualAnalysis {
                    timestamp: now,
                    goal: vgrc_result.message.clone(),
                    observation_summary: format!("VGRC cycle: {} attempts", vgrc_result.attempts),
                    detected_entities: observation
                        .visible_entities
                        .iter()
                        .map(|e| e.name.clone())
                        .collect(),
                    confidence: observation.confidence,
                    suggestions: vec![],
                };
                vis_state.add_analysis(analysis);

                if vgrc_result.success {
                    vis_state.add_goal_check(GoalCheckResult {
                        timestamp: now,
                        goal: vgrc_result.message.clone(),
                        passed: true,
                        details: "VGRC check passed".to_string(),
                        matches: vec![],
                    });
                }
            }

            // Sync pending approvals from director events (for async LLM path)
            sync_director_pending_ids_to_desk(&mut desk_state, &director.0);

            let raw_cmds = director.0.drain_bridge_commands();
            let cmds: Vec<EngineCommand> = raw_cmds
                .into_iter()
                .filter_map(|v| serde_json::from_value(v).ok())
                .collect();
            if !cmds.is_empty() {
                for cmd in cmds {
                    pending.commands.push(cmd);
                }
            }

            let mut feedback = String::new();
            for plan in director.0.list_plans().iter().rev().take(3) {
                if !feedback.is_empty() {
                    feedback.push('\n');
                }
                feedback.push_str(&format!(
                    "[{:?}] {} ({} steps)",
                    plan.status,
                    plan.title,
                    plan.steps.len()
                ));
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
                    if let agent_core::director::EditorEvent::PermissionResolved {
                        approved: true,
                        ..
                    } = event
                    {
                        desk_state.clear_pending_approval(&plan_id);
                        chat_state.add_message(Message::new_agent(format!(
                            "✅ Plan '{}' approved",
                            plan_id
                        )));
                    }
                    if let agent_core::director::EditorEvent::Error { message } = event {
                        chat_state
                            .add_message(Message::new_agent(format!("❌ Error: {}", message)));
                    }
                }
                let raw_cmds = director.0.drain_bridge_commands();
                for cmd in raw_cmds
                    .into_iter()
                    .filter_map(|v| serde_json::from_value::<EngineCommand>(v).ok())
                {
                    pending.commands.push(cmd);
                }
            }
            UserAction::Reject { plan_id, reason } => {
                let events = director.0.reject_plan(&plan_id, reason.as_deref());
                for event in &events {
                    if let agent_core::director::EditorEvent::PermissionResolved {
                        approved: false,
                        ..
                    } = event
                    {
                        desk_state.clear_pending_approval(&plan_id);
                        chat_state.add_message(Message::new_agent(format!(
                            "❌ Plan '{}' rejected",
                            plan_id
                        )));
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
                    pending
                        .commands
                        .push(EngineCommand::DeleteEntity { entity_id });
                    chat_state.add_message(Message::new_agent(format!(
                        "🗑 Deleted entity #{}",
                        entity_id
                    )));
                } else {
                    chat_state.add_message(Message::new_agent(
                        "⚠ No entity selected to delete".to_string(),
                    ));
                }
            }
            UserAction::FocusSelected => {
                // 聚焦到选中的实体
                if let Some(bevy_entity) = editor_selection.selected_entity {
                    let entity_id = bevy_entity.index() as u64;
                    chat_state.add_message(Message::new_agent(format!(
                        "🎯 Focused on entity #{}",
                        entity_id
                    )));
                } else {
                    chat_state.add_message(Message::new_agent(
                        "⚠ No entity selected to focus".to_string(),
                    ));
                }
            }
            UserAction::ToggleCommandPalette => {
                desk_state.command_palette_open = !desk_state.command_palette_open;
                if desk_state.command_palette_open {
                    chat_state.add_message(Message::new_agent(
                        "📋 Command palette opened (Ctrl+P or Esc to close)".to_string(),
                    ));
                }
            }
            UserAction::RecheckLlm => {
                if let Some(hc) = director.0.hybrid_controller() {
                    let status = hc.recheck();
                    let msg = match status {
                        HybridLlmStatus::Available => "✅ LLM connection restored".to_string(),
                        HybridLlmStatus::Connecting => "🔄 LLM connecting...".to_string(),
                        HybridLlmStatus::Unavailable => {
                            "❌ LLM still unavailable, using rule engine".to_string()
                        }
                        HybridLlmStatus::Disabled => "⚫ LLM is disabled".to_string(),
                    };
                    chat_state.add_message(Message::new_agent(msg));
                } else {
                    chat_state.add_message(Message::new_agent(
                        "⚠ Hybrid controller not initialized".to_string(),
                    ));
                }
            }
        }
    }
}

// ===========================================================================
// Hybrid Status Sync — syncs HybridEditorController state to UI
// ===========================================================================

fn sync_hybrid_status(
    director: Res<DirectorResource>,
    mut desk_state: ResMut<DirectorDeskState>,
    time: Res<Time>,
) {
    let Some(hc) = director.0.hybrid_controller() else {
        return;
    };

    let mode_display = &mut desk_state.hybrid_mode;
    let status = hc.llm_status();
    let mode = hc.current_mode();
    let stats = hc.stats();

    mode_display.mode = match mode {
        EditorMode::Llm => "LLM".to_string(),
        EditorMode::RuleBased => "RuleBased".to_string(),
    };
    mode_display.status = match status {
        HybridLlmStatus::Available => "Available".to_string(),
        HybridLlmStatus::Connecting => "Connecting".to_string(),
        HybridLlmStatus::Unavailable => "Unavailable".to_string(),
        HybridLlmStatus::Disabled => "Disabled".to_string(),
    };
    mode_display.success_rate = stats.llm_success_rate() * 100.0;
    mode_display.avg_response_ms = stats.avg_llm_response_ms;
    mode_display.consecutive_failures = stats.llm_failures as u32;

    // Set fallback reason from history
    let history = hc.fallback_history();
    mode_display.fallback_reason = history.last().map(|e| format!("{:?}", e.reason));

    // Update countdown (approximate based on last check + interval)
    mode_display.next_check_countdown = 0.0; // Will be set by controller internally

    desk_state.last_mode_update = Some(time.elapsed_secs_f64());
}

// ===========================================================================
// Visual Understanding Bridge — connects VGRC pipeline to UI state
// ===========================================================================

fn vgrc_bridge_system(
    mut director: ResMut<DirectorResource>,
    mut vis_state: ResMut<VisualUnderstandingState>,
    mut screenshot_queue: ResMut<ScreenshotQueue>,
    qa_requests: Option<Res<agent_ui::OpenWorldQaRequestQueue>>,
    mut frame_count: Local<u64>,
) {
    *frame_count += 1;

    // OpenWorld QA owns ScreenshotQueue results while WriteArtifacts is pending.
    let qa_waiting_for_framebuffer = qa_requests
        .as_ref()
        .is_some_and(|queue| queue.has_pending_write_artifacts());

    if !qa_waiting_for_framebuffer {
        // Request screenshot every 30 frames (~0.5s at 60fps) for VGRC
        if (*frame_count).is_multiple_of(30) {
            screenshot_queue.request_capture();
        }

        // Pop screenshot results and feed into visual state
        if let Some(result) = screenshot_queue.pop_result() {
            match result {
                bevy_adapter::ScreenshotResult::Success {
                    path: _,
                    dimensions,
                    base64,
                } => {
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
    }

    // Check for new DirectorRuntime events and create analysis entries
    let events = director.0.drain_events();
    for event in &events {
        match event {
            agent_core::director::EditorEvent::StepCompleted {
                plan_id,
                step_id: _,
                title,
                result: step_result,
            } => {
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
            agent_core::director::EditorEvent::StepFailed {
                plan_id,
                step_id: _,
                title,
                error,
            } => {
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
            agent_core::director::EditorEvent::DirectExecutionCompleted {
                success: true,
                ..
            } => {
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
            DefaultPlugins.set(WindowPlugin {
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
            SceneIndexIncrementalPlugin {
                fallback_interval: 300,
                full_rebuild_interval: 600,
            },
            IntegrationPlugin,
            VisionPlugin,
            CommandProcessorPlugin,
        ))
        .run();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_hr_smoke_app() -> App {
        let mut director = DirectorRuntime::new();
        director.disable_llm();

        let mut app = App::new();
        app.insert_resource(ChatState::default())
            .insert_resource(DirectorResource(director))
            .insert_resource(AgentRuntime::new(BaseAgent::new(
                AgentInstanceId(99),
                "Smoke Agent",
            )))
            .insert_resource(PendingCommands::default())
            .insert_resource(DirectorDeskState::new())
            .insert_resource(VisualUnderstandingState::default())
            .insert_resource(SceneIndexCache::default())
            .insert_resource(CommandHistory::default())
            .insert_resource(EditorSelection::default())
            .add_systems(Update, (handle_agent_input, handle_user_actions).chain());
        app
    }

    fn submit_ui_request(app: &mut App, text: &str) {
        app.world_mut()
            .resource_mut::<ChatState>()
            .add_message(Message::new_user(text));
        app.update();
    }

    fn first_pending_plan_id(app: &mut App) -> String {
        app.world()
            .resource::<DirectorDeskState>()
            .pending_approvals
            .first()
            .expect("expected a pending approval in Director Desk")
            .plan_id
            .clone()
    }

    fn hr_roster_event_text(app: &mut App) -> String {
        let events = app
            .world_mut()
            .resource_mut::<DirectorResource>()
            .0
            .dispatch_to_registered_agent("list team");
        format!("{:?}", events)
    }

    #[test]
    fn ui_smoke_hr_request_approve_clears_desk_and_updates_roster() {
        let mut app = make_hr_smoke_app();

        submit_ui_request(&mut app, "hire agent");
        let plan_id = first_pending_plan_id(&mut app);
        assert!(app
            .world()
            .resource::<DirectorResource>()
            .0
            .has_pending_approvals());

        app.world_mut()
            .resource_mut::<DirectorDeskState>()
            .pending_actions
            .push(UserAction::Approve { plan_id });
        app.update();

        assert!(!app
            .world()
            .resource::<DirectorDeskState>()
            .has_pending_approvals());
        assert!(!app
            .world()
            .resource::<DirectorResource>()
            .0
            .has_pending_approvals());
        assert!(app
            .world()
            .resource::<ChatState>()
            .messages
            .iter()
            .any(|msg| msg.content.contains("approved")));
        assert!(hr_roster_event_text(&mut app).contains("1 members"));
    }

    #[test]
    fn ui_smoke_hr_request_reject_clears_desk_without_updating_roster() {
        let mut app = make_hr_smoke_app();

        submit_ui_request(&mut app, "hire agent");
        let plan_id = first_pending_plan_id(&mut app);

        app.world_mut()
            .resource_mut::<DirectorDeskState>()
            .pending_actions
            .push(UserAction::Reject {
                plan_id,
                reason: Some("Not now".into()),
            });
        app.update();

        assert!(!app
            .world()
            .resource::<DirectorDeskState>()
            .has_pending_approvals());
        assert!(!app
            .world()
            .resource::<DirectorResource>()
            .0
            .has_pending_approvals());
        assert!(app
            .world()
            .resource::<ChatState>()
            .messages
            .iter()
            .any(|msg| msg.content.contains("rejected")));
        assert!(hr_roster_event_text(&mut app).contains("0 members"));
    }
}
