use agent_core::{AgentInstanceId, BaseAgent, Message, MessageType};
use agent_ui::*;
use bevy::prelude::IntoScheduleConfigs;
use bevy_adapter::{LootContainer, OpenWorldObject};
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct MemoryTaskPanelBackend {
    snapshot: TaskPanelSnapshot,
    commands: Vec<TaskPanelCommand>,
}

impl TaskPanelBackend for MemoryTaskPanelBackend {
    fn snapshot(&self) -> Result<TaskPanelSnapshot, TaskPanelBackendError> {
        Ok(self.snapshot.clone())
    }

    fn handle(&mut self, command: TaskPanelCommand) -> Result<(), TaskPanelBackendError> {
        self.commands.push(command);
        Ok(())
    }
}

#[derive(Clone, Default)]
struct SharedMemoryTaskPanelBackend {
    inner: Arc<Mutex<MemoryTaskPanelBackend>>,
    fail_first: bool,
}

#[derive(Clone)]
struct RejectDeleteBackend {
    snapshot: TaskPanelSnapshot,
}

impl TaskPanelBackend for RejectDeleteBackend {
    fn snapshot(&self) -> Result<TaskPanelSnapshot, TaskPanelBackendError> {
        Ok(self.snapshot.clone())
    }

    fn handle(&mut self, command: TaskPanelCommand) -> Result<(), TaskPanelBackendError> {
        if matches!(command, TaskPanelCommand::Delete { .. }) {
            return Err(TaskPanelBackendError {
                kind: TaskPanelBackendErrorKind::Rejected,
                message: "delete rejected".into(),
            });
        }
        Ok(())
    }
}

impl TaskPanelBackend for SharedMemoryTaskPanelBackend {
    fn snapshot(&self) -> Result<TaskPanelSnapshot, TaskPanelBackendError> {
        Ok(self.inner.lock().unwrap().snapshot.clone())
    }

    fn handle(&mut self, command: TaskPanelCommand) -> Result<(), TaskPanelBackendError> {
        let mut inner = self.inner.lock().unwrap();
        inner.commands.push(command);
        if self.fail_first && inner.commands.len() == 1 {
            return Err(TaskPanelBackendError {
                kind: TaskPanelBackendErrorKind::Rejected,
                message: "first command rejected".into(),
            });
        }
        Ok(())
    }
}

#[test]
fn task_panel_port_is_backend_agnostic() {
    let mut backend = MemoryTaskPanelBackend::default();
    backend.handle(TaskPanelCommand::Refresh).unwrap();
    assert_eq!(backend.commands, vec![TaskPanelCommand::Refresh]);
    assert!(backend.snapshot().unwrap().tasks.is_empty());
}

#[test]
fn task_panel_state_applies_backend_snapshot() {
    let mut task = TaskInfo::new("1".into(), "Quest".into(), "Find item".into());
    task.status = TaskStatus::InProgress;
    let done = TaskInfo {
        status: TaskStatus::Done,
        ..TaskInfo::new("2".into(), "Return".into(), "Report back".into())
    };
    let mut state = TaskPanelState::default();
    state.selected_task = Some("1".into());
    state.selected_ids = vec!["1".into(), "missing".into(), "2".into()];
    state.show_delete_confirm = Some("missing".into());
    state.apply_snapshot(TaskPanelSnapshot {
        tasks: vec![task.clone(), done.clone()],
        sync_status: SyncStatus::Synced,
    });
    assert_eq!(state.total_count, 2);
    assert_eq!(state.status_counts.get(&TaskStatus::InProgress), Some(&1));
    assert_eq!(state.status_counts.get(&TaskStatus::Done), Some(&1));
    assert_eq!(state.sync_status, SyncStatus::Synced);
    assert_eq!(state.selected_task.as_deref(), Some("1"));
    assert_eq!(state.selected_ids, vec!["1", "2"]);
    assert_eq!(state.show_delete_confirm, None);

    state.selected_task = Some("missing".into());
    state.show_delete_confirm = Some("2".into());
    state.apply_snapshot(TaskPanelSnapshot {
        tasks: vec![task, done],
        sync_status: SyncStatus::Synced,
    });
    assert_eq!(state.selected_task, None);
    assert_eq!(state.show_delete_confirm.as_deref(), Some("2"));
}

#[test]
fn task_panel_backend_resource_handles_commands_and_refreshes_snapshot() {
    let backend = SharedMemoryTaskPanelBackend::default();
    backend.inner.lock().unwrap().snapshot = TaskPanelSnapshot {
        tasks: vec![TaskInfo::new(
            "1".into(),
            "Quest".into(),
            "Find item".into(),
        )],
        sync_status: SyncStatus::Synced,
    };
    let resource = TaskPanelBackendResource::new(backend.clone());

    resource.handle(TaskPanelCommand::Refresh).unwrap();
    let snapshot = resource.snapshot().unwrap();

    assert_eq!(
        backend.inner.lock().unwrap().commands,
        vec![TaskPanelCommand::Refresh]
    );
    assert_eq!(snapshot.tasks.len(), 1);
    assert_eq!(snapshot.sync_status, SyncStatus::Synced);
}

#[test]
fn task_panel_state_merges_backend_snapshot_without_replacing_local_identity() {
    let local = TaskInfo::new("local-only".into(), "Local".into(), "Keep me".into());
    let existing = TaskInfo::new("panel-id".into(), "Old".into(), "Old description".into())
        .with_multica_id("42".into());
    let mut state = TaskPanelState::default();
    state.add_task(local);
    state.add_task(existing);

    let mut updated = TaskInfo::new(
        "task_bridge_42".into(),
        "Updated".into(),
        "Remote description".into(),
    )
    .with_multica_id("42".into());
    updated.status = TaskStatus::Done;
    let remote_new = TaskInfo::new("task_bridge_99".into(), "Remote".into(), "New".into())
        .with_multica_id("99".into());

    state.apply_backend_snapshot(TaskPanelSnapshot {
        tasks: vec![updated, remote_new],
        sync_status: SyncStatus::Synced,
    });

    assert!(state.tasks.contains_key("local-only"));
    assert!(!state.tasks.contains_key("task_bridge_42"));
    assert_eq!(state.tasks["panel-id"].title, "Updated");
    assert_eq!(state.tasks["panel-id"].status, TaskStatus::Done);
    assert!(state.tasks.contains_key("task_bridge_99"));
}

#[test]
fn remote_cancelled_task_remains_visible_without_local_delete_tombstone() {
    let mut cancelled = TaskInfo::new(
        "task_bridge_remote".into(),
        "Remote cancelled".into(),
        "Still visible".into(),
    )
    .with_multica_id("remote".into());
    cancelled.status = TaskStatus::Cancelled;
    let mut state = TaskPanelState::default();

    state.apply_backend_snapshot(TaskPanelSnapshot {
        tasks: vec![cancelled],
        sync_status: SyncStatus::Synced,
    });

    assert_eq!(
        state.tasks["task_bridge_remote"].status,
        TaskStatus::Cancelled
    );
}

#[test]
fn created_task_alias_prevents_duplicate_after_snapshot_merge() {
    let original = TaskInfo::new("panel-created".into(), "Quest".into(), "Find item".into());
    let mut state = TaskPanelState::default();
    state.add_task(original);

    let mut aliases = CreatedTaskAliases::default();
    aliases.record("7".into(), "panel-created".into());
    let remote = TaskInfo::new("task_bridge_7".into(), "Quest".into(), "Find item".into())
        .with_multica_id("7".into());
    let snapshot = aliases.apply(TaskPanelSnapshot {
        tasks: vec![remote],
        sync_status: SyncStatus::Synced,
    });

    state.apply_backend_snapshot(snapshot);

    assert_eq!(state.tasks.len(), 1);
    assert!(state.tasks.contains_key("panel-created"));
    assert_eq!(
        state.tasks["panel-created"].multica_id.as_deref(),
        Some("7")
    );
    assert!(!state.tasks.contains_key("task_bridge_7"));
}

#[test]
fn task_panel_state_routes_panel_id_to_backend_identity() {
    let task =
        TaskInfo::new("panel-id".into(), "Quest".into(), "".into()).with_multica_id("42".into());
    let mut state = TaskPanelState::default();
    state.add_task(task);

    assert_eq!(
        state.route_backend_command(TaskPanelCommand::UpdateStatus {
            id: "panel-id".into(),
            status: TaskStatus::Done,
        }),
        TaskPanelCommand::UpdateStatus {
            id: "42".into(),
            status: TaskStatus::Done,
        }
    );
    assert_eq!(
        state.route_backend_command(TaskPanelCommand::Delete {
            id: "panel-id".into(),
        }),
        TaskPanelCommand::Delete { id: "42".into() }
    );
}

#[test]
fn task_panel_backend_resource_continues_after_command_failure() {
    let backend = SharedMemoryTaskPanelBackend {
        fail_first: true,
        ..Default::default()
    };
    let resource = TaskPanelBackendResource::new(backend.clone());
    let errors = resource.handle_all(vec![
        TaskPanelCommand::Refresh,
        TaskPanelCommand::Delete { id: "42".into() },
    ]);

    assert_eq!(errors.len(), 1);
    assert_eq!(
        backend.inner.lock().unwrap().commands,
        vec![
            TaskPanelCommand::Refresh,
            TaskPanelCommand::Delete { id: "42".into() }
        ]
    );
}

#[test]
fn legacy_pending_actions_and_messages_drive_backend_commands() {
    fn accepts_legacy_actions(_: &Vec<TaskAction>) {}

    let backend = SharedMemoryTaskPanelBackend::default();
    let mut app = bevy::prelude::App::new();
    app.add_plugins(TaskPanelPlugin)
        .insert_resource(TaskPanelBackendResource::new(backend.clone()));
    accepts_legacy_actions(&app.world().resource::<TaskPanelState>().pending_actions);
    app.world_mut()
        .resource_mut::<TaskPanelState>()
        .pending_actions
        .push(TaskAction::RefreshTasks);
    app.world_mut()
        .write_message(TaskAction::DeleteTask("42".into()));

    app.update();

    assert_eq!(
        backend.inner.lock().unwrap().commands,
        vec![
            TaskPanelCommand::Refresh,
            TaskPanelCommand::Delete { id: "42".into() },
        ]
    );
}

#[test]
fn backend_batch_handles_commands_and_snapshot_under_one_transaction() {
    let backend = SharedMemoryTaskPanelBackend::default();
    backend.inner.lock().unwrap().snapshot.sync_status = SyncStatus::Synced;
    let resource = TaskPanelBackendResource::new(backend.clone());

    let transaction = resource.handle_all_and_snapshot(vec![TaskPanelCommand::Refresh]);

    assert!(transaction.errors.is_empty());
    assert_eq!(
        transaction.snapshot.unwrap().sync_status,
        SyncStatus::Synced
    );
    assert_eq!(
        backend.inner.lock().unwrap().commands,
        vec![TaskPanelCommand::Refresh]
    );
}

#[test]
fn rejected_delete_restores_task_and_later_refresh_keeps_it_visible() {
    let remote = TaskInfo::new("panel-rejected".into(), "Keep".into(), "Rejected".into())
        .with_multica_id("42".into());
    let backend = RejectDeleteBackend {
        snapshot: TaskPanelSnapshot {
            tasks: vec![remote.clone()],
            sync_status: SyncStatus::Synced,
        },
    };
    let mut app = bevy::prelude::App::new();
    app.add_plugins(TaskPanelPlugin)
        .insert_resource(TaskPanelBackendResource::new(backend));
    app.world_mut()
        .resource_mut::<TaskPanelState>()
        .add_task(remote);
    app.world_mut()
        .resource_mut::<TaskPanelState>()
        .pending_commands
        .push(TaskPanelCommand::Delete {
            id: "panel-rejected".into(),
        });

    app.update();
    let state = app.world().resource::<TaskPanelState>();
    assert!(state.tasks.contains_key("panel-rejected"));
    assert!(matches!(state.sync_status, SyncStatus::SyncError(_)));

    app.world_mut()
        .resource_mut::<TaskPanelState>()
        .pending_commands
        .push(TaskPanelCommand::Refresh);
    app.update();
    assert!(app
        .world()
        .resource::<TaskPanelState>()
        .tasks
        .contains_key("panel-rejected"));
}

#[test]
fn task_view_model_preserves_status_semantics() {
    let mut task = TaskInfo::new("1".into(), "Build world".into(), "".into());
    task.status = TaskStatus::InProgress;
    assert_eq!(task.status.display(), "进行中");
}

fn sync_director_events_to_pending_ui(
    desk: &mut DirectorDeskState,
    events: &[agent_core::EditorEvent],
) {
    for event in events {
        if let agent_core::EditorEvent::PermissionRequested {
            plan_id,
            risk,
            reason,
        } = event
        {
            desk.sync_pending_approval(PendingApprovalInfo {
                plan_id: plan_id.clone(),
                title: plan_id.clone(),
                risk: risk.clone(),
                reason: reason.clone(),
                step_count: 0,
            });
        }
    }
}

fn make_open_world_timeline() -> agent_core::OpenWorldTimeline {
    let plan = agent_core::OpenWorldPlan::open_world_slice01_fixture();
    let scenario = agent_core::PlayableScenario::open_world_slice01_main_path(&plan);
    let mut state = agent_core::PlayableScenarioState::from_plan(&plan);
    let report = scenario.run(&mut state);
    let bundle =
        agent_core::OpenWorldVerificationBundle::from_playtest(&plan, &scenario, &state, &report);
    agent_core::OpenWorldTimeline::from_verification_bundle(&bundle)
}

// ============================================================================
// ChatState tests
// ============================================================================

#[test]
fn test_chat_state_defaults() {
    let state = ChatState::default();
    assert!(state.messages.is_empty());
    assert!(state.input_text.is_empty());
    assert!(!state.scroll_to_bottom);
    assert!(state.agent_identity.is_none());
    assert!(state.agent_state.is_empty());
    assert_eq!(state.current_step, 0);
    assert!(state.progress.is_none());
}

#[test]
fn test_chat_state_add_message() {
    let mut state = ChatState::default();
    let msg = Message::new_agent("Hello, World!");
    state.add_message(msg);

    assert_eq!(state.messages.len(), 1);
    assert_eq!(state.messages[0].content, "Hello, World!");
    assert!(state.scroll_to_bottom);
}

#[test]
fn test_chat_state_add_message_sets_scroll() {
    let mut state = ChatState::default();
    assert!(!state.scroll_to_bottom);

    state.add_message(Message::new_user("test"));
    assert!(state.scroll_to_bottom);
}

#[test]
fn test_chat_state_clear_input() {
    let mut state = ChatState {
        input_text: "some text".to_string(),
        ..Default::default()
    };
    assert!(!state.input_text.is_empty());

    state.clear_input();
    assert!(state.input_text.is_empty());
}

#[test]
fn test_chat_state_message_limit_500() {
    let mut state = ChatState::default();
    for i in 0..510 {
        state.add_message(Message::new_agent(format!("msg {}", i)));
    }
    assert_eq!(state.messages.len(), 500);
    assert_eq!(state.messages[0].content, "msg 10");
    assert_eq!(state.messages[state.messages.len() - 1].content, "msg 509");
}

#[test]
fn test_chat_state_multiple_message_types() {
    let mut state = ChatState::default();
    state.add_message(Message::new_agent("agent msg"));
    state.add_message(Message::new_user("user msg"));
    state.add_message(Message::thought("thinking"));

    assert_eq!(state.messages.len(), 3);
    assert!(matches!(state.messages[0].message_type, MessageType::Agent));
    assert!(matches!(state.messages[1].message_type, MessageType::User));
    assert!(matches!(
        state.messages[2].message_type,
        MessageType::Thought
    ));
}

#[test]
fn test_chat_state_empty_input_not_sent() {
    let mut state = ChatState::default();
    let before = state.messages.len();
    state.clear_input();
    assert_eq!(state.messages.len(), before);
}

#[test]
fn test_world_timeline_panel_state_selects_ticks() {
    let mut state = WorldTimelinePanelState::default();
    state.load_timeline(make_open_world_timeline());

    assert_eq!(state.selected_tick, Some(0));
    assert!(state.selected_tick_entry().is_some());

    state.select_tick(8);
    let tick = state.selected_tick_entry().unwrap();
    assert_eq!(tick.tick, 8);
    assert!(tick
        .runtime_events
        .iter()
        .any(|event| event == "player defeated camp_enemy_01"));
}

#[test]
fn test_world_timeline_panel_state_loads_timeline_json_file() {
    let mut state = WorldTimelinePanelState::default();
    let timeline_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("docs/qa/open-world-slice01-timeline.json");

    state.load_timeline_json_file(timeline_path).unwrap();

    state.select_tick(8);
    let tick = state.selected_tick_entry().unwrap();
    assert!(tick
        .runtime_events
        .iter()
        .any(|event| event == "player defeated camp_enemy_01"));
    assert_eq!(
        state.screenshot_paths(),
        &["docs/qa/open-world-slice01-framebuffer.png".to_string()]
    );
    assert!(state.visual_check_evidence().iter().any(|evidence| {
        evidence.contains("screenshot_capture=bevy_framebuffer")
            && evidence.contains("docs/qa/open-world-slice01-framebuffer.png")
    }));
}

#[test]
fn test_world_timeline_default_qa_json_path_is_stable() {
    assert_eq!(
        WorldTimelinePanelState::default_open_world_slice01_timeline_path(),
        "docs/qa/open-world-slice01-timeline.json"
    );
}

#[test]
fn test_open_world_qa_default_artifact_paths_are_stable() {
    let paths = OpenWorldQaRequestQueue::default_open_world_slice01_artifact_paths();

    assert_eq!(
        paths.markdown_path,
        std::path::PathBuf::from("docs/qa/open-world-slice01.md")
    );
    assert_eq!(
        paths.timeline_json_path,
        Some(std::path::PathBuf::from(
            "docs/qa/open-world-slice01-timeline.json"
        ))
    );
    assert_eq!(
        paths.visual_snapshot_path,
        Some(std::path::PathBuf::from(
            "docs/qa/open-world-slice01-visual.png"
        ))
    );
}

#[test]
fn test_agent_ui_plugin_registers_screenshot_queue_for_open_world_qa() {
    // Full AgentUiPlugin pulls EguiPlugin (needs render/asset shaders). OpenWorld QA
    // only requires ScreenshotQueue; verify that resource can be registered headlessly.
    let mut app = bevy::prelude::App::new();
    app.init_resource::<bevy_adapter::ScreenshotQueue>();

    assert!(app
        .world()
        .get_resource::<bevy_adapter::ScreenshotQueue>()
        .is_some());
}

#[test]
fn test_world_timeline_panel_state_loads_from_director_runtime() {
    let mut director = agent_core::DirectorRuntime::new();
    let mut state = WorldTimelinePanelState::default();

    state.load_open_world_slice01_from_director(&mut director);

    state.select_tick(8);
    let tick = state.selected_tick_entry().unwrap();
    assert!(tick
        .runtime_events
        .iter()
        .any(|event| event == "player defeated camp_enemy_01"));
}

#[test]
fn test_world_timeline_panel_state_loads_director_runtime_with_engine_performance_evidence() {
    let mut director = agent_core::DirectorRuntime::new();
    let mut state = WorldTimelinePanelState::default();

    state.load_open_world_slice01_from_director_with_performance_evidence(
        &mut director,
        vec!["bevy_frame_time_avg_ms=16.667".to_string()],
    );

    assert!(state
        .timeline
        .as_ref()
        .unwrap()
        .performance_evidence
        .contains(&"bevy_frame_time_avg_ms=16.667".to_string()));
}

#[test]
fn test_world_timeline_update_queue_refreshes_panel_state() {
    let mut app = bevy::prelude::App::new();
    app.init_resource::<WorldTimelinePanelState>()
        .init_resource::<WorldTimelineUpdateQueue>()
        .add_systems(bevy::prelude::Update, apply_world_timeline_updates);

    app.world_mut()
        .resource_mut::<WorldTimelineUpdateQueue>()
        .request_update(make_open_world_timeline());

    app.update();

    let mut state = app.world_mut().resource_mut::<WorldTimelinePanelState>();
    assert_eq!(state.selected_tick, Some(0));

    state.select_tick(8);
    let tick = state.selected_tick_entry().unwrap();
    assert!(tick
        .runtime_events
        .iter()
        .any(|event| event == "player defeated camp_enemy_01"));
}

#[test]
fn test_world_timeline_qa_request_includes_integration_frame_metrics() {
    let mut app = bevy::prelude::App::new();
    app.init_resource::<WorldTimelinePanelState>()
        .init_resource::<WorldTimelineUpdateQueue>()
        .init_resource::<OpenWorldQaRequestQueue>()
        .insert_resource(bevy_adapter::integration::SceneIndexCache::default())
        .insert_resource(bevy_adapter::integration::IntegrationState::default())
        .add_systems(
            bevy::prelude::Update,
            (process_open_world_qa_requests, apply_world_timeline_updates).chain(),
        );

    {
        let mut integration = app
            .world_mut()
            .resource_mut::<bevy_adapter::integration::IntegrationState>();
        integration.frame_count = 2;
        integration.record_frame_delta_secs(0.016);
        integration.record_frame_delta_secs(0.020);
    }
    app.world_mut()
        .resource_mut::<OpenWorldQaRequestQueue>()
        .request_open_world_slice01();

    app.update();

    let state = app.world().resource::<WorldTimelinePanelState>();
    let timeline = state.timeline.as_ref().unwrap();
    assert!(timeline
        .performance_evidence
        .contains(&"bevy_frame_count=2".to_string()));
    assert!(timeline
        .performance_evidence
        .iter()
        .any(|row| row.starts_with("bevy_frame_time_avg_ms=")));
}

#[test]
fn test_world_timeline_qa_request_writes_configured_artifacts() {
    let mut app = bevy::prelude::App::new();
    app.init_resource::<WorldTimelinePanelState>()
        .init_resource::<WorldTimelineUpdateQueue>()
        .init_resource::<OpenWorldQaRequestQueue>()
        .insert_resource(bevy_adapter::integration::IntegrationState::default())
        .add_systems(
            bevy::prelude::Update,
            (process_open_world_qa_requests, apply_world_timeline_updates).chain(),
        );

    {
        let mut integration = app
            .world_mut()
            .resource_mut::<bevy_adapter::integration::IntegrationState>();
        integration.frame_count = 4;
        integration.record_frame_delta_secs(0.010);
        integration.record_frame_delta_secs(0.030);
    }

    let artifact_dir =
        std::env::temp_dir().join(format!("windwave-open-world-qa-{}", std::process::id()));
    let markdown_path = artifact_dir.join("open-world-slice01.md");
    let timeline_json_path = artifact_dir.join("open-world-slice01-timeline.json");
    let visual_snapshot_path = artifact_dir.join("open-world-slice01-visual.png");

    app.world_mut()
        .resource_mut::<OpenWorldQaRequestQueue>()
        .request_open_world_slice01_artifacts(
            markdown_path.clone(),
            Some(timeline_json_path.clone()),
            Some(visual_snapshot_path.clone()),
        );

    app.update();

    let state = app.world().resource::<WorldTimelinePanelState>();
    let timeline = state.timeline.as_ref().unwrap();
    assert!(timeline
        .performance_evidence
        .contains(&"bevy_frame_count=4".to_string()));
    assert!(timeline
        .screenshot_paths
        .contains(&visual_snapshot_path.to_string_lossy().to_string()));

    let markdown = std::fs::read_to_string(&markdown_path).unwrap();
    assert!(markdown.contains("bevy_frame_time_avg_ms=20.000"));

    let timeline_json = std::fs::read_to_string(&timeline_json_path).unwrap();
    assert!(timeline_json.contains("bevy_frame_count=4"));

    let png_bytes = std::fs::read(&visual_snapshot_path).unwrap();
    assert!(png_bytes.starts_with(b"\x89PNG\r\n\x1a\n"));
}

#[test]
fn test_world_timeline_qa_request_uses_bevy_framebuffer_screenshot_result() {
    let mut app = bevy::prelude::App::new();
    app.init_resource::<WorldTimelinePanelState>()
        .init_resource::<WorldTimelineUpdateQueue>()
        .init_resource::<OpenWorldQaRequestQueue>()
        .insert_resource(bevy_adapter::integration::IntegrationState::default())
        .init_resource::<bevy_adapter::ScreenshotQueue>()
        .add_systems(
            bevy::prelude::Update,
            (process_open_world_qa_requests, apply_world_timeline_updates).chain(),
        );

    let artifact_dir = std::env::temp_dir().join(format!(
        "windwave-open-world-qa-framebuffer-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&artifact_dir).unwrap();
    let markdown_path = artifact_dir.join("open-world-slice01.md");
    let timeline_json_path = artifact_dir.join("open-world-slice01-timeline.json");
    let proxy_visual_path = artifact_dir.join("open-world-slice01-proxy.png");
    let engine_screenshot_path = artifact_dir.join("bevy-framebuffer.png");
    let durable_framebuffer_path = artifact_dir.join("open-world-slice01-framebuffer.png");
    std::fs::write(&engine_screenshot_path, b"fake-png-bytes").unwrap();

    {
        let screenshot_queue = app.world().resource::<bevy_adapter::ScreenshotQueue>();
        screenshot_queue.record_result(bevy_adapter::ScreenshotResult::Success {
            path: engine_screenshot_path.clone(),
            dimensions: (1280, 720),
            base64: "encoded-framebuffer".to_string(),
        });
    }

    app.world_mut()
        .resource_mut::<OpenWorldQaRequestQueue>()
        .request_open_world_slice01_artifacts(
            markdown_path.clone(),
            Some(timeline_json_path.clone()),
            Some(proxy_visual_path.clone()),
        );

    app.update();

    let state = app.world().resource::<WorldTimelinePanelState>();
    let timeline = state.timeline.as_ref().unwrap();
    assert_eq!(
        timeline.screenshot_paths,
        vec![durable_framebuffer_path.to_string_lossy().to_string()]
    );
    assert!(timeline
        .visual_check_evidence
        .iter()
        .any(|row| row.contains("screenshot_capture=bevy_framebuffer")));
    assert!(timeline
        .visual_check_evidence
        .iter()
        .any(|row| row.contains("dimensions=1280x720")));
    assert!(timeline
        .performance_evidence
        .contains(&"bevy_screenshot_success_total=1".to_string()));
    assert!(timeline.performance_evidence.iter().any(|row| {
        row.contains("bevy_screenshot_last_result=success") && row.contains("bevy-framebuffer.png")
    }));

    let markdown = std::fs::read_to_string(&markdown_path).unwrap();
    assert!(markdown.contains("screenshot_capture=bevy_framebuffer"));
    assert!(markdown.contains("open-world-slice01-framebuffer.png"));
    assert!(markdown.contains("bevy_screenshot_success_total=1"));

    let timeline_json = std::fs::read_to_string(&timeline_json_path).unwrap();
    assert!(timeline_json.contains("screenshot_capture=bevy_framebuffer"));
    assert!(timeline_json.contains("open-world-slice01-framebuffer.png"));
    assert!(timeline_json.contains("bevy_screenshot_success_total=1"));
    assert!(!proxy_visual_path.exists());
    assert!(durable_framebuffer_path.exists());
}

#[test]
fn test_world_timeline_qa_request_waits_for_bevy_framebuffer_capture() {
    let mut app = bevy::prelude::App::new();
    app.init_resource::<WorldTimelinePanelState>()
        .init_resource::<WorldTimelineUpdateQueue>()
        .init_resource::<OpenWorldQaRequestQueue>()
        .insert_resource(bevy_adapter::integration::IntegrationState::default())
        .init_resource::<bevy_adapter::ScreenshotQueue>()
        .add_systems(
            bevy::prelude::Update,
            (process_open_world_qa_requests, apply_world_timeline_updates).chain(),
        );

    let artifact_dir = std::env::temp_dir().join(format!(
        "windwave-open-world-qa-wait-framebuffer-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&artifact_dir).unwrap();
    let markdown_path = artifact_dir.join("open-world-slice01.md");
    let timeline_json_path = artifact_dir.join("open-world-slice01-timeline.json");
    let proxy_visual_path = artifact_dir.join("open-world-slice01-proxy.png");
    let engine_screenshot_path = artifact_dir.join("bevy-framebuffer.png");
    let durable_framebuffer_path = artifact_dir.join("open-world-slice01-framebuffer.png");

    app.world_mut()
        .resource_mut::<OpenWorldQaRequestQueue>()
        .request_open_world_slice01_artifacts(
            markdown_path.clone(),
            Some(timeline_json_path.clone()),
            Some(proxy_visual_path.clone()),
        );

    app.update();

    assert!(
        app.world()
            .resource::<bevy_adapter::ScreenshotQueue>()
            .requested
    );
    assert!(app
        .world()
        .resource::<WorldTimelinePanelState>()
        .timeline
        .is_none());
    assert!(!markdown_path.exists());
    assert!(!proxy_visual_path.exists());

    std::fs::write(&engine_screenshot_path, b"fake-png-bytes").unwrap();
    {
        let screenshot_queue = app.world().resource::<bevy_adapter::ScreenshotQueue>();
        screenshot_queue
            .results
            .lock()
            .unwrap()
            .push(bevy_adapter::ScreenshotResult::Success {
                path: engine_screenshot_path.clone(),
                dimensions: (1024, 576),
                base64: "encoded-framebuffer".to_string(),
            });
    }

    app.update();

    let state = app.world().resource::<WorldTimelinePanelState>();
    let timeline = state.timeline.as_ref().unwrap();
    assert_eq!(
        timeline.screenshot_paths,
        vec![durable_framebuffer_path.to_string_lossy().to_string()]
    );
    assert!(markdown_path.exists());
    assert!(!proxy_visual_path.exists());
    assert!(durable_framebuffer_path.exists());

    let markdown = std::fs::read_to_string(&markdown_path).unwrap();
    assert!(markdown.contains("screenshot_capture=bevy_framebuffer"));
    assert!(markdown.contains("dimensions=1024x576"));
    assert!(markdown.contains("open-world-slice01-framebuffer.png"));
}

#[test]
fn test_world_timeline_qa_request_falls_back_to_proxy_after_framebuffer_failure() {
    let mut app = bevy::prelude::App::new();
    app.init_resource::<WorldTimelinePanelState>()
        .init_resource::<WorldTimelineUpdateQueue>()
        .init_resource::<OpenWorldQaRequestQueue>()
        .insert_resource(bevy_adapter::integration::IntegrationState::default())
        .init_resource::<bevy_adapter::ScreenshotQueue>()
        .add_systems(
            bevy::prelude::Update,
            (process_open_world_qa_requests, apply_world_timeline_updates).chain(),
        );

    let artifact_dir = std::env::temp_dir().join(format!(
        "windwave-open-world-qa-framebuffer-failure-{}",
        std::process::id()
    ));
    let markdown_path = artifact_dir.join("open-world-slice01.md");
    let timeline_json_path = artifact_dir.join("open-world-slice01-timeline.json");
    let proxy_visual_path = artifact_dir.join("open-world-slice01-proxy.png");

    {
        let screenshot_queue = app.world().resource::<bevy_adapter::ScreenshotQueue>();
        screenshot_queue
            .results
            .lock()
            .unwrap()
            .push(bevy_adapter::ScreenshotResult::Failure {
                error: "primary window unavailable".to_string(),
            });
    }

    app.world_mut()
        .resource_mut::<OpenWorldQaRequestQueue>()
        .request_open_world_slice01_artifacts(
            markdown_path.clone(),
            Some(timeline_json_path.clone()),
            Some(proxy_visual_path.clone()),
        );

    app.update();

    let state = app.world().resource::<WorldTimelinePanelState>();
    let timeline = state.timeline.as_ref().unwrap();
    assert_eq!(
        timeline.screenshot_paths,
        vec![proxy_visual_path.to_string_lossy().to_string()]
    );
    assert!(timeline
        .performance_evidence
        .contains(&"bevy_framebuffer_screenshot_error=primary window unavailable".to_string()));
    assert!(timeline
        .visual_check_evidence
        .iter()
        .any(|row| row.contains("screenshot_capture=scene_index_proxy_png")));
    assert!(
        !app.world()
            .resource::<bevy_adapter::ScreenshotQueue>()
            .requested
    );

    let markdown = std::fs::read_to_string(&markdown_path).unwrap();
    assert!(markdown.contains("bevy_framebuffer_screenshot_error=primary window unavailable"));
    assert!(markdown.contains("screenshot_capture=scene_index_proxy_png"));

    let timeline_json = std::fs::read_to_string(&timeline_json_path).unwrap();
    assert!(timeline_json.contains("bevy_framebuffer_screenshot_error"));

    let png_bytes = std::fs::read(&proxy_visual_path).unwrap();
    assert!(png_bytes.starts_with(b"\x89PNG\r\n\x1a\n"));
}

#[test]
fn test_world_timeline_replay_controls_step_between_ticks() {
    let mut state = WorldTimelinePanelState::default();
    let timeline = make_open_world_timeline();
    let first_tick = timeline.ticks[0].tick;
    let second_tick = timeline.ticks[1].tick;
    let third_tick = timeline.ticks[2].tick;
    state.load_timeline(timeline);

    state.start_replay();
    assert!(state.replay_enabled);
    assert_eq!(state.replay_cursor_tick, Some(first_tick));

    state.step_replay_forward();
    assert_eq!(state.replay_cursor_tick, Some(second_tick));

    state.step_replay_forward();
    assert_eq!(state.replay_cursor_tick, Some(third_tick));

    state.step_replay_backward();
    assert_eq!(state.replay_cursor_tick, Some(second_tick));
    assert_eq!(state.replay_cursor_entry().unwrap().tick, second_tick);
}

#[test]
fn test_world_timeline_replay_can_seek_selected_tick() {
    let mut state = WorldTimelinePanelState::default();
    state.load_timeline(make_open_world_timeline());

    state.select_tick(8);
    state.start_replay_from_selected_tick();

    assert_eq!(state.replay_cursor_tick, Some(8));
    assert!(state
        .replay_cursor_entry()
        .unwrap()
        .runtime_events
        .iter()
        .any(|event| event == "player defeated camp_enemy_01"));
}

#[test]
fn test_world_timeline_replay_exposes_world_state_snapshot() {
    let mut state = WorldTimelinePanelState::default();
    state.load_timeline(make_open_world_timeline());

    state.select_tick(8);
    state.start_replay_from_selected_tick();
    assert_eq!(
        state
            .replay_cursor_world_state()
            .unwrap()
            .enemy_states
            .get("camp_enemy_01"),
        Some(&"Dead".to_string())
    );

    while state.replay_cursor_tick
        != state
            .timeline
            .as_ref()
            .unwrap()
            .ticks
            .last()
            .map(|tick| tick.tick)
    {
        state.step_replay_forward();
    }

    let world_state = state.replay_cursor_world_state().unwrap();
    assert_eq!(
        world_state.quest_states.get("main_quest"),
        Some(&"Completed".to_string())
    );
    assert!(world_state
        .inventory
        .get("player")
        .is_some_and(|items| items.contains(&"reward_item".to_string())));
}

#[test]
fn test_open_world_interaction_events_enqueue_runtime_commands() {
    let mut app = bevy::prelude::App::new();
    app.add_message::<InteractionCompleteEvent>()
        .init_resource::<OpenWorldInteractionBindings>()
        .init_resource::<OpenWorldInteractionCommandQueue>()
        .add_systems(
            bevy::prelude::Update,
            enqueue_open_world_interaction_commands,
        );

    app.world_mut()
        .resource_mut::<OpenWorldInteractionBindings>()
        .bind_loot("reward_chest", "reward_item")
        .bind_combat("camp_enemy_01");

    let player = app
        .world_mut()
        .spawn(OpenWorldObject {
            object_id: "player".into(),
            kind: "Player".into(),
            zone_id: "spawn_zone".into(),
        })
        .id();
    let chest = app
        .world_mut()
        .spawn(OpenWorldObject {
            object_id: "reward_chest".into(),
            kind: "LootContainer".into(),
            zone_id: "reward_zone".into(),
        })
        .id();
    let enemy = app
        .world_mut()
        .spawn(OpenWorldObject {
            object_id: "camp_enemy_01".into(),
            kind: "Enemy".into(),
            zone_id: "camp_zone".into(),
        })
        .id();

    app.world_mut().write_message(InteractionCompleteEvent {
        player,
        target: chest,
        interaction_type: InteractionType::Pickup,
        success: true,
    });
    app.world_mut().write_message(InteractionCompleteEvent {
        player,
        target: enemy,
        interaction_type: InteractionType::Trigger,
        success: true,
    });
    app.update();

    let queue = app.world().resource::<OpenWorldInteractionCommandQueue>();
    assert_eq!(
        queue.pending,
        vec![
            OpenWorldInteractionCommand::Loot {
                actor_id: "player".into(),
                container_id: "reward_chest".into(),
                reward_id: "reward_item".into(),
            },
            OpenWorldInteractionCommand::Combat {
                actor_id: "player".into(),
                enemy_id: "camp_enemy_01".into(),
            },
        ]
    );
}

#[test]
fn test_open_world_interaction_command_queue_executes_against_runtime() {
    let plan = agent_core::OpenWorldPlan::open_world_slice01_fixture();
    let mut runtime = agent_core::OpenWorldRuntimeState::from_plan(&plan);
    runtime.enter_zone("player", "puzzle_zone").unwrap();
    runtime.interact("player", "puzzle_switch").unwrap();

    let mut world = bevy::prelude::World::new();
    world.spawn(OpenWorldObject {
        object_id: "player".into(),
        kind: "Player".into(),
        zone_id: "spawn_zone".into(),
    });
    world.spawn((
        OpenWorldObject {
            object_id: "reward_chest".into(),
            kind: "LootContainer".into(),
            zone_id: "reward_zone".into(),
        },
        LootContainer,
    ));

    let mut queue = OpenWorldInteractionCommandQueue {
        pending: vec![OpenWorldInteractionCommand::Loot {
            actor_id: "player".into(),
            container_id: "reward_chest".into(),
            reward_id: "reward_item".into(),
        }],
    };

    let results = drain_open_world_interaction_commands(&mut world, &mut runtime, &mut queue);

    assert!(queue.pending.is_empty());
    assert_eq!(results.len(), 1);
    assert!(results[0].is_ok());
    assert!(runtime.inventory_contains("player", "reward_item"));
}

#[test]
fn test_open_world_quest_panel_state_syncs_from_runtime() {
    let plan = agent_core::OpenWorldPlan::open_world_slice01_fixture();
    let mut runtime = agent_core::OpenWorldRuntimeState::from_plan(&plan);
    runtime.enter_zone("player", "puzzle_zone").unwrap();
    runtime.interact("player", "puzzle_switch").unwrap();

    let mut state = OpenWorldQuestPanelState::default();
    state.sync_from_runtime(&runtime);

    assert_eq!(state.quest_rows.len(), 1);
    assert_eq!(state.quest_rows[0].quest_id, "main_quest");
    assert_eq!(state.quest_rows[0].state, "PuzzleSolved");
    assert!(state.objective_rows.iter().any(|objective| {
        objective.objective_id == "solve_puzzle_switch" && objective.state == "Completed"
    }));
    assert!(state.objective_rows.iter().any(|objective| {
        objective.objective_id == "defeat_camp_enemy" && objective.state == "Pending"
    }));
}

#[test]
fn test_open_world_quest_panel_state_syncs_from_replay_world_state() {
    let timeline = make_open_world_timeline();
    let final_tick = timeline.ticks.last().unwrap();

    let mut state = OpenWorldQuestPanelState::default();
    state.sync_from_replay_world_state(&final_tick.world_state);

    assert_eq!(state.quest_rows.len(), 1);
    assert_eq!(state.quest_rows[0].quest_id, "main_quest");
    assert_eq!(state.quest_rows[0].state, "Completed");
    assert!(state.objective_rows.iter().any(|objective| {
        objective.objective_id == "collect_reward_item" && objective.state == "Completed"
    }));
}

#[test]
fn test_open_world_quest_panel_syncs_from_world_timeline_replay_cursor() {
    let mut app = bevy::prelude::App::new();
    app.init_resource::<WorldTimelinePanelState>()
        .init_resource::<OpenWorldQuestPanelState>()
        .add_systems(
            bevy::prelude::Update,
            sync_open_world_quest_panel_from_timeline_replay,
        );

    let mut timeline_state = WorldTimelinePanelState::default();
    timeline_state.load_timeline(make_open_world_timeline());
    timeline_state.select_tick(8);
    timeline_state.start_replay_from_selected_tick();
    app.world_mut().insert_resource(timeline_state);
    app.update();

    let quest_state = app.world().resource::<OpenWorldQuestPanelState>();
    assert!(quest_state.objective_rows.iter().any(|objective| {
        objective.objective_id == "defeat_camp_enemy" && objective.state == "Completed"
    }));
    assert!(quest_state
        .quest_rows
        .iter()
        .any(|quest| { quest.quest_id == "main_quest" && quest.state == "EnemyDefeated" }));
}

#[test]
fn test_director_desk_hr_approval_smoke() {
    let mut director = agent_core::DirectorRuntime::new();
    let mut registry = agent_core::AgentRegistry::new();
    registry.register(Box::new(agent_core::hr_agent::HrAgent::new(
        agent_core::AgentId(5),
        agent_core::team_structure::TeamRoster::new(),
    )));
    let mut desk = DirectorDeskState::new();

    let events = director.dispatch_to_agent("hire agent", &mut registry);
    sync_director_events_to_pending_ui(&mut desk, &events);
    assert!(desk.has_pending_approvals());

    director.set_agent_registry(registry);
    let plan_id = desk.pending_approvals[0].plan_id.clone();
    desk.pending_actions.push(UserAction::Approve {
        plan_id: plan_id.clone(),
    });

    let actions = std::mem::take(&mut desk.pending_actions);
    for action in actions {
        if let UserAction::Approve { plan_id } = action {
            let events = director.approve_plan(&plan_id);
            if events.iter().any(|event| {
                matches!(
                    event,
                    agent_core::EditorEvent::PermissionResolved { approved: true, .. }
                )
            }) {
                desk.clear_pending_approval(&plan_id);
            }
            assert!(events.iter().any(|event| {
                matches!(
                    event,
                    agent_core::EditorEvent::StepCompleted { result, .. }
                    if result.contains("Added agent")
                )
            }));
        }
    }

    assert!(!desk.has_pending_approvals());
    assert!(!director.has_pending_approvals());
}

#[test]
fn test_director_desk_hr_reject_smoke() {
    let mut director = agent_core::DirectorRuntime::new();
    let mut registry = agent_core::AgentRegistry::new();
    registry.register(Box::new(agent_core::hr_agent::HrAgent::new(
        agent_core::AgentId(5),
        agent_core::team_structure::TeamRoster::new(),
    )));
    let mut desk = DirectorDeskState::new();

    let events = director.dispatch_to_agent("hire agent", &mut registry);
    sync_director_events_to_pending_ui(&mut desk, &events);
    assert!(desk.has_pending_approvals());

    director.set_agent_registry(registry);
    let plan_id = desk.pending_approvals[0].plan_id.clone();
    desk.pending_actions.push(UserAction::Reject {
        plan_id: plan_id.clone(),
        reason: Some("Not now".into()),
    });

    let actions = std::mem::take(&mut desk.pending_actions);
    for action in actions {
        if let UserAction::Reject { plan_id, reason } = action {
            let events = director.reject_plan(&plan_id, reason.as_deref());
            if events.iter().any(|event| {
                matches!(
                    event,
                    agent_core::EditorEvent::PermissionResolved {
                        approved: false,
                        ..
                    }
                )
            }) {
                desk.clear_pending_approval(&plan_id);
            }
        }
    }

    assert!(!desk.has_pending_approvals());
    assert!(!director.has_pending_approvals());
}

// ============================================================================
// AgentRuntime tests
// ============================================================================

fn make_test_agent() -> BaseAgent {
    BaseAgent::new(AgentInstanceId(1), "TestAgent")
}

#[test]
fn test_agent_runtime_creation() {
    let agent = make_test_agent();
    let runtime = AgentRuntime::new(agent);

    let state = runtime.current_state();
    assert_eq!(state, "Idle");

    let step = runtime.current_step();
    assert_eq!(step, 0);
}

#[test]
fn test_agent_runtime_current_state() {
    let agent = make_test_agent();
    let runtime = AgentRuntime::new(agent);

    assert_eq!(runtime.current_state(), "Idle");
}

#[test]
fn test_agent_runtime_current_step() {
    let agent = make_test_agent();
    let runtime = AgentRuntime::new(agent);

    assert_eq!(runtime.current_step(), 0);
}

#[test]
fn test_agent_runtime_progress_none_when_idle() {
    let agent = make_test_agent();
    let runtime = AgentRuntime::new(agent);

    assert!(runtime.progress().is_none());
}

// ============================================================================
// UiConfig tests
// ============================================================================

#[test]
fn test_ui_config_defaults() {
    let config = UiConfig::default();
    assert_eq!(config.panel_width, 380.0);
    assert_eq!(config.message_spacing, 12.0);
}

// ============================================================================
// LayoutCommandQueue tests
// ============================================================================

#[test]
fn test_layout_command_queue_push() {
    let mut queue = LayoutCommandQueue::default();
    assert!(queue.commands.is_empty());

    queue.push(LayoutCommand::ResetLayout);
    assert_eq!(queue.commands.len(), 1);
}

#[test]
fn test_layout_command_queue_multiple_commands() {
    let mut queue = LayoutCommandQueue::default();
    queue.push(LayoutCommand::ShowPanel {
        panel_id: "chat".into(),
    });
    queue.push(LayoutCommand::HidePanel {
        panel_id: "hierarchy".into(),
    });
    queue.push(LayoutCommand::SwitchToCompact);

    assert_eq!(queue.commands.len(), 3);
}

// ============================================================================
// LayoutManager tests
// ============================================================================

#[test]
fn test_layout_manager_default_has_panels() {
    let layout = LayoutDefinition::default();
    let mgr = LayoutManager::new(layout);

    assert!(!mgr.all_panel_ids().is_empty());
    assert!(mgr.panel_config("chat").is_some());
    assert!(mgr.panel_config("hierarchy").is_some());
}

#[test]
fn test_layout_manager_default_presets() {
    let mgr = LayoutManager::new(LayoutDefinition::default());
    assert!(mgr.preset_names.contains(&"Standard".to_string()));
    assert!(mgr.preset_names.contains(&"Compact".to_string()));
    assert!(mgr.preset_names.contains(&"Wide".to_string()));
    assert!(mgr.preset_names.contains(&"Minimal".to_string()));
}

#[test]
fn test_layout_manager_is_visible() {
    let mgr = LayoutManager::new(LayoutDefinition::default());
    assert!(mgr.is_visible("chat"));
    assert!(!mgr.is_visible("game_mode_bar"));
}

#[test]
fn test_layout_manager_show_hide_panel() {
    let mut mgr = LayoutManager::new(LayoutDefinition::default());

    assert!(mgr.is_visible("chat"));
    mgr.hide_panel("chat");
    assert!(!mgr.is_visible("chat"));
    assert!(mgr.dirty);

    mgr.show_panel("chat");
    assert!(mgr.is_visible("chat"));
}

#[test]
fn test_layout_manager_toggle_panel() {
    let mut mgr = LayoutManager::new(LayoutDefinition::default());
    let initial = mgr.is_visible("chat");

    let result = mgr.toggle_panel("chat");
    assert_eq!(result, !initial);
    assert!(mgr.dirty);

    let result2 = mgr.toggle_panel("chat");
    assert_eq!(result2, initial);
}

#[test]
fn test_layout_manager_panel_size() {
    let mgr = LayoutManager::new(LayoutDefinition::default());
    let chat_size = mgr.panel_size("chat");
    assert!(chat_size.is_some());
    assert!(chat_size.unwrap() > 0.0);
}

#[test]
fn test_layout_manager_resize_panel() {
    let mut mgr = LayoutManager::new(LayoutDefinition::default());

    assert!(mgr.resize_panel("chat", 500.0));
    assert_eq!(mgr.panel_size("chat"), Some(500.0));

    assert!(!mgr.resize_panel("nonexistent_panel", 100.0));
}

#[test]
fn test_layout_manager_resize_clamps() {
    let mut mgr = LayoutManager::new(LayoutDefinition::default());

    mgr.resize_panel("chat", 5.0);
    assert_eq!(mgr.panel_size("chat"), Some(50.0));

    mgr.resize_panel("chat", 5000.0);
    assert_eq!(mgr.panel_size("chat"), Some(2000.0));
}

#[test]
fn test_layout_manager_move_panel() {
    let mut mgr = LayoutManager::new(LayoutDefinition::default());

    assert!(mgr.move_panel("chat", PanelPosition::Left));
    assert!(!mgr.move_panel("nonexistent", PanelPosition::Right));
}

#[test]
fn test_layout_manager_visible_panels_in_position() {
    let mgr = LayoutManager::new(LayoutDefinition::default());

    let right_panels = mgr.visible_panels_in(PanelPosition::Right);
    assert!(right_panels.iter().any(|p| p.id == "chat"));
    assert!(right_panels.iter().any(|p| p.id == "inspector"));

    let left_panels = mgr.visible_panels_in(PanelPosition::Left);
    assert!(left_panels.iter().any(|p| p.id == "hierarchy"));
}

#[test]
fn test_layout_manager_floating_panels() {
    let mgr = LayoutManager::new(LayoutDefinition::default());
    let floating = mgr.floating_panels();
    assert!(floating.is_empty());
}

#[test]
fn test_layout_manager_all_panel_ids() {
    let mgr = LayoutManager::new(LayoutDefinition::default());
    let ids = mgr.all_panel_ids();
    assert!(ids.contains(&"chat"));
    assert!(ids.contains(&"hierarchy"));
    assert!(ids.contains(&"inspector"));
    assert!(ids.contains(&"console"));
}

#[test]
fn test_layout_manager_visible_panel_ids() {
    let mgr = LayoutManager::new(LayoutDefinition::default());
    let visible = mgr.visible_panel_ids();
    assert!(visible.contains(&"chat"));
    assert!(!visible.contains(&"game_mode_bar"));
}

#[test]
fn test_layout_manager_set_order() {
    let mut mgr = LayoutManager::new(LayoutDefinition::default());

    assert!(mgr.set_order("chat", 5));
    assert_eq!(mgr.panel_config("chat").unwrap().order, 5);

    assert!(!mgr.set_order("nonexistent", 1));
}

// ============================================================================
// LayoutCommand execute tests
// ============================================================================

#[test]
fn test_layout_command_show_panel() {
    let mut mgr = LayoutManager::new(LayoutDefinition::default());
    mgr.hide_panel("hierarchy");
    assert!(!mgr.is_visible("hierarchy"));

    let cmd = LayoutCommand::ShowPanel {
        panel_id: "hierarchy".into(),
    };
    let desc = cmd.execute(&mut mgr);
    assert!(mgr.is_visible("hierarchy"));
    assert!(desc.contains("shown"));
}

#[test]
fn test_layout_command_hide_panel() {
    let mut mgr = LayoutManager::new(LayoutDefinition::default());
    assert!(mgr.is_visible("chat"));

    let cmd = LayoutCommand::HidePanel {
        panel_id: "chat".into(),
    };
    let desc = cmd.execute(&mut mgr);
    assert!(!mgr.is_visible("chat"));
    assert!(desc.contains("hidden"));
}

#[test]
fn test_layout_command_toggle_panel() {
    let mut mgr = LayoutManager::new(LayoutDefinition::default());
    let initial = mgr.is_visible("chat");

    let cmd = LayoutCommand::TogglePanel {
        panel_id: "chat".into(),
    };
    let desc = cmd.execute(&mut mgr);
    assert_eq!(mgr.is_visible("chat"), !initial);
    assert!(desc.contains("chat"));
}

#[test]
fn test_layout_command_resize_panel() {
    let mut mgr = LayoutManager::new(LayoutDefinition::default());

    let cmd = LayoutCommand::ResizePanel {
        panel_id: "chat".into(),
        size: 600.0,
    };
    let desc = cmd.execute(&mut mgr);
    assert_eq!(mgr.panel_size("chat"), Some(600.0));
    assert!(desc.contains("ok"));
}

#[test]
fn test_layout_command_move_panel() {
    let mut mgr = LayoutManager::new(LayoutDefinition::default());

    let cmd = LayoutCommand::MovePanel {
        panel_id: "chat".into(),
        position: PanelPosition::Left,
    };
    let desc = cmd.execute(&mut mgr);
    assert!(desc.contains("ok"));
}

#[test]
fn test_layout_command_apply_preset() {
    let mut mgr = LayoutManager::new(LayoutDefinition::default());
    let orig_size = mgr.panel_size("chat");

    let cmd = LayoutCommand::ApplyPreset {
        name: "compact".into(),
    };
    let desc = cmd.execute(&mut mgr);
    assert!(desc.contains("ok"));

    let compact_size = mgr.panel_size("chat");
    assert!(compact_size.is_some());
    assert_ne!(orig_size, compact_size);
}

#[test]
fn test_layout_command_reset_layout() {
    let mut mgr = LayoutManager::new(LayoutDefinition::default());
    mgr.hide_panel("chat");
    mgr.resize_panel("hierarchy", 999.0);
    assert!(mgr.dirty);

    let cmd = LayoutCommand::ResetLayout;
    let desc = cmd.execute(&mut mgr);
    assert!(desc.contains("reset"));

    assert!(mgr.is_visible("chat"));
}

#[test]
fn test_layout_command_switch_to_compact() {
    let mut mgr = LayoutManager::new(LayoutDefinition::default());

    let cmd = LayoutCommand::SwitchToCompact;
    let desc = cmd.execute(&mut mgr);
    assert!(desc.contains("compact"));
}

#[test]
fn test_layout_command_switch_to_wide() {
    let mut mgr = LayoutManager::new(LayoutDefinition::default());

    let cmd = LayoutCommand::SwitchToWide;
    let desc = cmd.execute(&mut mgr);
    assert!(desc.contains("wide"));
}

#[test]
fn test_layout_command_switch_to_minimal() {
    let mut mgr = LayoutManager::new(LayoutDefinition::default());

    let cmd = LayoutCommand::SwitchToMinimal;
    let desc = cmd.execute(&mut mgr);
    assert!(desc.contains("minimal"));
    assert!(mgr.is_visible("chat"));
    assert!(!mgr.is_visible("hierarchy"));
}

#[test]
fn test_layout_command_focus_panel() {
    let mut mgr = LayoutManager::new(LayoutDefinition::default());

    let cmd = LayoutCommand::FocusPanel {
        panel_id: "chat".into(),
    };
    let desc = cmd.execute(&mut mgr);
    assert!(desc.contains("chat"));
}

#[test]
fn test_layout_command_show_only() {
    let mut mgr = LayoutManager::new(LayoutDefinition::default());

    let cmd = LayoutCommand::ShowOnly {
        panel_ids: vec!["chat".into(), "inspector".into()],
    };
    let desc = cmd.execute(&mut mgr);
    assert!(mgr.is_visible("chat"));
    assert!(mgr.is_visible("inspector"));
    assert!(!mgr.is_visible("hierarchy"));
    assert!(!mgr.is_visible("console"));
    assert!(desc.contains("chat"));
}

#[test]
fn test_layout_command_apply_from_json() {
    let mut mgr = LayoutManager::new(LayoutDefinition::default());

    let json = r#"{"name":"JSON Layout","version":1,"panels":[
        {"id":"chat","title":"Chat","position":{"Right":null},"order":0,"size":400.0,"visible":true,"tab_group":null},
        {"id":"hierarchy","title":"Hierarchy","position":{"Left":null},"order":0,"size":250.0,"visible":false,"tab_group":null}
    ]}"#;

    let cmd = LayoutCommand::ApplyLayoutFromJson {
        json: json.to_string(),
    };
    let desc = cmd.execute(&mut mgr);
    assert!(desc.contains("JSON"));
    assert!(mgr.is_visible("chat"));
    assert!(!mgr.is_visible("hierarchy"));
}

// ============================================================================
// LayoutDefinition tests
// ============================================================================

#[test]
fn test_layout_definition_default_has_all_panels() {
    let def = LayoutDefinition::default();
    assert_eq!(def.name, "Standard Editor Layout");
    assert_eq!(def.version, 1);
    assert!(!def.panels.is_empty());

    let panel_ids: Vec<&str> = def.panels.iter().map(|p| p.id.as_str()).collect();
    assert!(panel_ids.contains(&"chat"));
    assert!(panel_ids.contains(&"hierarchy"));
    assert!(panel_ids.contains(&"inspector"));
    assert!(panel_ids.contains(&"console"));
    assert!(panel_ids.contains(&"director_desk"));
}

#[test]
fn test_layout_definition_compact() {
    let def = LayoutDefinition::compact();
    assert_eq!(def.name, "Compact Layout");
    let chat = def.panels.iter().find(|p| p.id == "chat").unwrap();
    assert_eq!(chat.size, 320.0);
}

#[test]
fn test_layout_definition_wide() {
    let def = LayoutDefinition::wide();
    assert_eq!(def.name, "Wide Layout");
    let chat = def.panels.iter().find(|p| p.id == "chat").unwrap();
    assert_eq!(chat.size, 420.0);
}

#[test]
fn test_layout_definition_minimal() {
    let def = LayoutDefinition::minimal();
    assert_eq!(def.name, "Minimal Layout");
    assert!(def.panels.iter().any(|p| p.id == "chat" && p.visible));
    assert!(def.panels.iter().any(|p| p.id == "hierarchy" && !p.visible));
}

#[test]
fn test_layout_definition_focus_right() {
    let def = LayoutDefinition::focus_right();
    assert_eq!(def.name, "Focus Right");
    let chat = def.panels.iter().find(|p| p.id == "chat").unwrap();
    assert_eq!(chat.size, 500.0);
    assert!(chat.visible);

    let inspector = def.panels.iter().find(|p| p.id == "inspector").unwrap();
    assert!(!inspector.visible);
}

// ============================================================================
// PanelConfig tests
// ============================================================================

#[test]
fn test_panel_config_new() {
    let cfg = PanelConfig::new("test", "Test Panel", PanelPosition::Right, 1, 300.0);
    assert_eq!(cfg.id, "test");
    assert_eq!(cfg.title, "Test Panel");
    assert!(cfg.visible);
    assert_eq!(cfg.order, 1);
    assert_eq!(cfg.size, 300.0);
    assert_eq!(cfg.tab_group, None);
}

#[test]
fn test_panel_config_hidden() {
    let cfg = PanelConfig::hidden("test", "Test", PanelPosition::Left, 0, 200.0);
    assert!(!cfg.visible);
    assert_eq!(cfg.id, "test");
    assert_eq!(cfg.size, 200.0);
}
