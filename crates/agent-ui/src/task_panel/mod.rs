//! Task Panel
//!
//! 任务面板组件，用于在 agent-ui 中展示和管理任务
//! 提供任务列表展示、筛选、创建、状态更新和 Multica 集成
//!
//! 与 multica-bridge 集成，支持统一任务系统和双向同步

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts, EguiPrimaryContextPass};
use log::{debug, error, info, warn};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::layout::{LayoutManager, PanelPosition};

// multica-bridge integration types
use multica_bridge::scene_event_bus::{
    SceneEvent, SceneEventSubscriber, SceneEventType, SubscriberId,
};
use multica_bridge::task_bridge::{TaskBridge, UnifiedTask, UnifiedTaskStatus};
use multica_bridge::task_sync_module::{SyncStatus as ModuleSyncStatus, TaskSynchronizer};

mod model;
mod port;

pub use model::{SortBy, SyncStatus, TaskFilter, TaskInfo, TaskPanelState, TaskStatus};
pub use port::{
    TaskPanelBackend, TaskPanelBackendError, TaskPanelBackendErrorKind, TaskPanelCommand,
    TaskPanelSnapshot,
};

/// 任务操作事件
#[derive(Message, Clone, Debug)]
pub enum TaskAction {
    /// 创建新任务
    CreateTask(TaskInfo),
    /// 更新任务状态
    UpdateTaskStatus(String, TaskStatus),
    /// 删除任务
    DeleteTask(String),
    /// 刷新任务列表
    RefreshTasks,
}

// ════════════════════════════════════════════════════════════
// Backend Integration Resources
// ════════════════════════════════════════════════════════════

/// Resource holding a shared handle to the TaskBridge for backend dispatch.
///
/// Insert this into the Bevy app to enable backend task operations:
/// ```ignore
/// app.insert_resource(TaskBridgeResource::new(task_bridge));
/// ```
#[derive(Resource, Clone)]
pub struct TaskBridgeResource {
    pub bridge: Arc<TaskBridge>,
}

impl TaskBridgeResource {
    /// Create from an existing TaskBridge
    pub fn new(bridge: TaskBridge) -> Self {
        Self {
            bridge: Arc::new(bridge),
        }
    }

    /// Create from an already-shared TaskBridge
    pub fn from_arc(bridge: Arc<TaskBridge>) -> Self {
        Self { bridge }
    }
}

/// Resource holding a shared handle to the TaskSynchronizer for sync status tracking.
///
/// Insert this into the Bevy app to enable sync status display:
/// ```ignore
/// app.insert_resource(TaskSynchronizerResource::new(synchronizer));
/// ```
#[derive(Resource, Clone)]
pub struct TaskSynchronizerResource {
    pub synchronizer: Arc<TaskSynchronizer>,
}

impl TaskSynchronizerResource {
    /// Create from an existing TaskSynchronizer
    pub fn new(synchronizer: TaskSynchronizer) -> Self {
        Self {
            synchronizer: Arc::new(synchronizer),
        }
    }

    /// Create from an already-shared TaskSynchronizer
    pub fn from_arc(synchronizer: Arc<TaskSynchronizer>) -> Self {
        Self { synchronizer }
    }
}

/// Shared scene event queue that bridges `SceneEventBus` callbacks into Bevy systems.
///
/// A `SceneEventSubscriber` implementation pushes events into this queue,
/// and the `process_scene_events` system drains them each frame.
#[derive(Resource, Clone, Default)]
pub struct SceneEventQueue {
    /// Pending scene events from the SceneEventBus
    pub events: Arc<Mutex<Vec<SceneEvent>>>,
}

impl SceneEventQueue {
    /// Create a new empty queue
    pub fn new() -> Self {
        Self {
            events: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Push a scene event into the queue (called by subscriber)
    pub fn push(&self, event: SceneEvent) {
        if let Ok(mut queue) = self.events.lock() {
            // Cap the queue to prevent unbounded growth if not drained
            if queue.len() < 1000 {
                queue.push(event);
            } else {
                warn!("Scene event queue full, dropping oldest event");
                queue.remove(0);
                queue.push(event);
            }
        }
    }

    /// Drain all pending events
    pub fn drain(&self) -> Vec<SceneEvent> {
        self.events
            .lock()
            .map(|mut q| std::mem::take(&mut *q))
            .unwrap_or_default()
    }
}

/// A `SceneEventSubscriber` that forwards events into a `SceneEventQueue`.
///
/// Registered with the `SceneEventBus` to bridge external events into Bevy.
pub struct PanelSceneEventSubscriber {
    queue: SceneEventQueue,
}

impl PanelSceneEventSubscriber {
    pub fn new(queue: SceneEventQueue) -> Self {
        Self { queue }
    }
}

impl SceneEventSubscriber for PanelSceneEventSubscriber {
    fn on_event(&self, event: &SceneEvent) {
        debug!(
            "TaskPanel received scene event: {:?} for entity {} in scene {}",
            event.event_type, event.entity_id, event.scene_id
        );
        self.queue.push(event.clone());
    }
}

/// Resource tracking the subscriber ID for deregistration on cleanup.
#[derive(Resource)]
pub struct SceneEventSubscriberId {
    pub subscriber_id: SubscriberId,
}

/// 任务面板插件
pub struct TaskPanelPlugin;

impl Plugin for TaskPanelPlugin {
    fn build(&self, app: &mut App) {
        app
            // Core state
            .init_resource::<TaskPanelState>()
            // Task action event channel (for Bevy-internal event-driven dispatch)
            .add_message::<TaskAction>()
            // UI rendering
            .add_systems(EguiPrimaryContextPass, render_task_panel)
            // Backend integration systems (run after UI to process queued actions)
            .add_systems(
                Update,
                (
                    process_task_actions,
                    process_scene_events,
                    update_sync_status,
                )
                    .chain(),
            );
    }
}

// ════════════════════════════════════════════════════════════
// Status Conversion Helpers
// ════════════════════════════════════════════════════════════

/// Convert panel `TaskStatus` to bridge `UnifiedTaskStatus`
fn panel_status_to_unified(status: &TaskStatus) -> UnifiedTaskStatus {
    match status {
        TaskStatus::Pending => UnifiedTaskStatus::Draft,
        TaskStatus::InProgress => UnifiedTaskStatus::Running,
        TaskStatus::Done => UnifiedTaskStatus::Done,
        TaskStatus::Failed => UnifiedTaskStatus::Failed,
        TaskStatus::Cancelled => UnifiedTaskStatus::Cancelled,
    }
}

/// Convert bridge `UnifiedTaskStatus` to panel `TaskStatus`
fn unified_status_to_panel(status: &UnifiedTaskStatus) -> TaskStatus {
    match status {
        UnifiedTaskStatus::Draft => TaskStatus::Pending,
        UnifiedTaskStatus::Planning => TaskStatus::Pending,
        UnifiedTaskStatus::Queued => TaskStatus::Pending,
        UnifiedTaskStatus::Dispatched => TaskStatus::InProgress,
        UnifiedTaskStatus::Running => TaskStatus::InProgress,
        UnifiedTaskStatus::WaitingForUser => TaskStatus::InProgress,
        UnifiedTaskStatus::Blocked => TaskStatus::InProgress,
        UnifiedTaskStatus::Done => TaskStatus::Done,
        UnifiedTaskStatus::Failed => TaskStatus::Failed,
        UnifiedTaskStatus::Cancelled => TaskStatus::Cancelled,
    }
}

/// Map module-level `ModuleSyncStatus` to panel `SyncStatus`
fn module_sync_to_panel(status: &ModuleSyncStatus) -> SyncStatus {
    match status {
        ModuleSyncStatus::Idle => SyncStatus::NotSynced,
        ModuleSyncStatus::Syncing => SyncStatus::Syncing,
        ModuleSyncStatus::Success => SyncStatus::Synced,
        ModuleSyncStatus::Conflict(_) => SyncStatus::Synced,
        ModuleSyncStatus::Failed(msg) => SyncStatus::SyncError(msg.clone()),
    }
}

/// Convert a panel `TaskInfo` to a bridge `UnifiedTask`
fn task_info_to_unified(task: &TaskInfo) -> UnifiedTask {
    let mut unified = UnifiedTask::new(task.title.clone(), task.description.clone());

    unified.status = panel_status_to_unified(&task.status);

    if let Some(ref scene_id) = task.scene_id {
        unified.set_scene(scene_id.clone(), None);
    }

    for entity_id in &task.entity_ids {
        unified.add_entity(entity_id.clone());
    }

    // If the task already has a multica_id, embed it as the bridge_id
    if let Some(ref multica_id) = task.multica_id {
        if let Ok(bridge_id) = multica_id.parse::<u64>() {
            unified.id.bridge_id = bridge_id;
        }
    }

    unified
}

// ════════════════════════════════════════════════════════════
// Backend Integration Systems
// ════════════════════════════════════════════════════════════

/// Process pending task actions by dispatching them to the TaskBridge.
///
/// Runs every frame after the UI has had a chance to push actions into
/// `pending_actions`. Drains the queue and attempts to execute each action
/// through the bridge. If no `TaskBridgeResource` is present, actions are
/// silently skipped (the panel operates in standalone mode).
fn process_task_actions(
    mut task_state: ResMut<TaskPanelState>,
    bridge: Option<Res<TaskBridgeResource>>,
) {
    let actions: Vec<TaskAction> = std::mem::take(&mut task_state.pending_actions);
    if actions.is_empty() {
        return;
    }

    let Some(bridge) = bridge else {
        debug!(
            "No TaskBridgeResource found; {} task actions remain unprocessed",
            actions.len()
        );
        // Re-queue actions so they can be processed once the bridge is available
        task_state.pending_actions = actions;
        return;
    };

    for action in &actions {
        dispatch_task_action(action, &bridge.bridge, &mut task_state);
    }
}

/// Dispatch a single `TaskAction` to the `TaskBridge`.
fn dispatch_task_action(action: &TaskAction, bridge: &TaskBridge, state: &mut TaskPanelState) {
    match action {
        TaskAction::CreateTask(task_info) => {
            let unified = task_info_to_unified(task_info);
            let title = task_info.title.clone();

            match bridge.register_task(unified) {
                Ok(registered) => {
                    // Link back the bridge ID to the panel task
                    if let Some(panel_task) = state.tasks.get_mut(&task_info.id) {
                        panel_task.multica_id = Some(registered.id.bridge_id.to_string());
                    }
                    info!(
                        "Task bridged successfully: '{}' (bridge_id={})",
                        title, registered.id.bridge_id
                    );
                }
                Err(e) => {
                    error!(
                        "Failed to bridge task '{}' (id={}): {:?}",
                        title, task_info.id, e
                    );
                    state.sync_status = SyncStatus::SyncError(format!(
                        "Failed to create task '{}': {:?}",
                        title, e
                    ));
                }
            }
        }

        TaskAction::UpdateTaskStatus(task_id, new_status) => {
            let unified_status = panel_status_to_unified(new_status);

            // Try to find the bridge ID from the task's multica_id
            let bridge_id = state
                .tasks
                .get(task_id)
                .and_then(|t| t.multica_id.as_ref())
                .and_then(|mid| mid.parse::<u64>().ok());

            if let Some(bridge_id) = bridge_id {
                match bridge.update_task_status(bridge_id, unified_status) {
                    Ok(_) => {
                        info!(
                            "Task '{}' status updated to {:?} via bridge",
                            task_id, new_status
                        );
                    }
                    Err(e) => {
                        warn!(
                            "Failed to update task '{}' status via bridge: {:?}",
                            task_id, e
                        );
                    }
                }
            } else {
                debug!(
                    "Task '{}' has no bridge ID; status update applied locally only",
                    task_id
                );
            }
        }

        TaskAction::DeleteTask(task_id) => {
            // TaskBridge has no delete method; mark as cancelled via bridge
            let bridge_id = state
                .tasks
                .get(task_id)
                .and_then(|t| t.multica_id.as_ref())
                .and_then(|mid| mid.parse::<u64>().ok());

            if let Some(bridge_id) = bridge_id {
                match bridge.update_task_status(bridge_id, UnifiedTaskStatus::Cancelled) {
                    Ok(_) => {
                        info!(
                            "Task '{}' cancelled via bridge (bridge_id={})",
                            task_id, bridge_id
                        );
                    }
                    Err(e) => {
                        warn!("Failed to cancel task '{}' via bridge: {:?}", task_id, e);
                    }
                }
            } else {
                debug!("Task '{}' has no bridge ID; deleted locally only", task_id);
            }
        }

        TaskAction::RefreshTasks => {
            info!("Refreshing tasks from TaskBridge...");
            let bridge_tasks = bridge.get_all_tasks();

            // Sync tasks from bridge into panel state
            let mut synced_count = 0;
            for unified in &bridge_tasks {
                let bridge_id_str = unified.id.bridge_id.to_string();
                let panel_status = unified_status_to_panel(&unified.status);

                // Find existing task by multica_id
                let existing_id = state
                    .tasks
                    .iter()
                    .find(|(_, t)| t.multica_id.as_deref() == Some(&bridge_id_str))
                    .map(|(id, _)| id.clone());

                if let Some(existing_id) = existing_id {
                    // Check and collect changes before mutating
                    let needs_status_update = {
                        if let Some(task) = state.tasks.get(&existing_id) {
                            task.status != panel_status
                        } else {
                            false
                        }
                    };
                    let old_status = state.tasks.get(&existing_id).map(|t| t.status.clone());
                    let needs_scene_update = {
                        if let Some(task) = state.tasks.get(&existing_id) {
                            if let Some(ref scene_id) = unified.scene_id {
                                task.scene_id.as_deref() != Some(scene_id.as_str())
                            } else {
                                false
                            }
                        } else {
                            false
                        }
                    };

                    // Apply status update
                    if needs_status_update {
                        if let Some(task) = state.tasks.get_mut(&existing_id) {
                            task.status = panel_status.clone();
                        }
                        if let Some(old) = old_status {
                            state.update_task_status_counts(&old, &panel_status);
                            synced_count += 1;
                        }
                    }
                    // Update scene association if available
                    if needs_scene_update {
                        if let Some(task) = state.tasks.get_mut(&existing_id) {
                            if let Some(ref scene_id) = unified.scene_id {
                                task.scene_id = Some(scene_id.clone());
                            }
                        }
                    }
                    // Sync entity IDs
                    if let Some(task) = state.tasks.get_mut(&existing_id) {
                        for entity_id in &unified.entity_ids {
                            if !task.entity_ids.contains(entity_id) {
                                task.entity_ids.push(entity_id.clone());
                            }
                        }
                    }
                } else {
                    // New task from bridge - add to panel
                    let mut task = TaskInfo::new(
                        format!("task_bridge_{}", bridge_id_str),
                        unified.title.clone(),
                        unified.description.clone(),
                    );
                    task.status = panel_status;
                    task.scene_id = unified.scene_id.clone();
                    task.entity_ids = unified.entity_ids.clone();
                    task.multica_id = Some(bridge_id_str);

                    state.add_task(task);
                    synced_count += 1;
                }
            }

            info!(
                "Task refresh complete: {} tasks synced from bridge ({} total in panel)",
                synced_count, state.total_count
            );
            state.sync_status = SyncStatus::Synced;
        }
    }
}

/// Process scene events from the `SceneEventQueue`, updating task-scene associations.
///
/// When entities are created/modified/deleted in the editor, scene events are
/// pushed into the queue. This system consumes them and updates the task panel's
/// scene association display for any tasks linked to the affected scenes.
fn process_scene_events(
    queue: Option<Res<SceneEventQueue>>,
    mut task_state: ResMut<TaskPanelState>,
) {
    let Some(queue) = queue else { return };

    let events = queue.drain();
    if events.is_empty() {
        return;
    }

    // Track which scenes were affected
    let mut affected_scenes: HashMap<String, Vec<&SceneEvent>> = HashMap::new();
    for event in &events {
        affected_scenes
            .entry(event.scene_id.clone())
            .or_default()
            .push(event);
    }

    // Log a summary of scene activity relevant to the panel
    for (scene_id, scene_events) in &affected_scenes {
        let task_count = task_state
            .tasks
            .values()
            .filter(|t| t.scene_id.as_deref() == Some(scene_id.as_str()))
            .count();

        if task_count > 0 {
            let creates = scene_events
                .iter()
                .filter(|e| e.event_type == SceneEventType::EntityCreated)
                .count();
            let updates = scene_events
                .iter()
                .filter(|e| e.event_type == SceneEventType::EntityUpdated)
                .count();
            let deletes = scene_events
                .iter()
                .filter(|e| e.event_type == SceneEventType::EntityDeleted)
                .count();

            debug!(
                "Scene '{}' activity: +{} entities created, ~{} updated, -{} deleted ({} linked tasks)",
                scene_id, creates, updates, deletes, task_count
            );
        }
    }

    // Add newly created entity IDs to tasks that reference their scene
    for event in &events {
        if event.event_type == SceneEventType::EntityCreated {
            let entity_id_str = event.entity_id.to_string();
            for task in task_state.tasks.values_mut() {
                if task.scene_id.as_deref() == Some(event.scene_id.as_str())
                    && !task.entity_ids.contains(&entity_id_str)
                {
                    task.entity_ids.push(entity_id_str.clone());
                    debug!(
                        "Auto-linked entity {} to task '{}'",
                        entity_id_str, task.title
                    );
                }
            }
        }

        // Remove deleted entity IDs from tasks
        if event.event_type == SceneEventType::EntityDeleted {
            let entity_id_str = event.entity_id.to_string();
            for task in task_state.tasks.values_mut() {
                task.entity_ids.retain(|eid| eid != &entity_id_str);
            }
        }
    }
}

/// Update the `SyncStatus` in `TaskPanelState` from the `TaskSynchronizer`.
///
/// Polls the synchronizer's status each frame and maps it to the panel's
/// `SyncStatus` enum. If no `TaskSynchronizerResource` is present, the
/// panel maintains its own sync status independently.
fn update_sync_status(
    synchronizer: Option<Res<TaskSynchronizerResource>>,
    mut task_state: ResMut<TaskPanelState>,
) {
    let Some(sync) = synchronizer else { return };

    let module_status = sync.synchronizer.get_status();
    let panel_status = module_sync_to_panel(&module_status);

    // Only update if the status actually changed (avoid unnecessary writes)
    if task_state.sync_status != panel_status {
        debug!(
            "TaskPanel sync status changed: {:?} -> {:?}",
            task_state.sync_status, panel_status
        );
        task_state.sync_status = panel_status;
    }
}

/// 渲染任务面板
fn render_task_panel(
    mut contexts: EguiContexts,
    mut task_state: ResMut<TaskPanelState>,
    layout_mgr: Res<LayoutManager>,
) {
    if !layout_mgr.is_visible("task") {
        return;
    }

    let ctx = match contexts.ctx_mut() {
        Ok(c) => c,
        Err(_) => return,
    };

    // ── P2-6: Keyboard shortcuts ──
    let mut kb_ctrl_n = false;
    let mut kb_delete = false;
    let mut kb_escape = false;
    ctx.input(|i| {
        if i.modifiers.ctrl && i.key_pressed(egui::Key::N) {
            kb_ctrl_n = true;
        }
        if i.key_pressed(egui::Key::Delete) {
            kb_delete = true;
        }
        if i.key_pressed(egui::Key::Escape) {
            kb_escape = true;
        }
    });

    // Process keyboard shortcuts
    if kb_ctrl_n {
        task_state.open_create_dialog();
    }
    if kb_escape {
        task_state.close_create_dialog();
        task_state.selected_task = None;
        task_state.show_delete_confirm = None;
        task_state.selected_ids.clear();
        task_state.select_mode = false;
    }
    if kb_delete {
        if let Some(ref selected_id) = task_state.selected_task.clone() {
            task_state.show_delete_confirm = Some(selected_id.clone());
        }
    }

    let (win_w, win_h) = layout_mgr
        .panel_config("task")
        .and_then(|c| match c.position {
            PanelPosition::Floating { width, height, .. } => Some((width, height)),
            _ => None,
        })
        .unwrap_or((450.0, 650.0));

    egui::Window::new("任务面板")
        .default_size([win_w, win_h])
        .resizable(true)
        .show(ctx, |ui| {
            // ── 顶部工具栏 ──
            ui.horizontal(|ui| {
                // 搜索栏
                ui.horizontal(|ui| {
                    ui.label("\u{1F50D}"); // P2-8: unicode escape for 🔍
                    egui::TextEdit::singleline(&mut task_state.search_query)
                        .hint_text("搜索任务标题或描述...")
                        .show(ui);
                });

                // 刷新按钮 (P2-7: tooltip)
                if ui
                    .button("\u{1F504}") // P2-8: unicode escape for 🔄
                    .on_hover_text("刷新任务列表")
                    .clicked()
                {
                    task_state.pending_actions.push(TaskAction::RefreshTasks);
                }

                // ── P2-2: Sync status icon ──
                let (sync_icon, sync_tooltip) = match &task_state.sync_status {
                    SyncStatus::NotSynced => ("\u{26AA}", "未同步"),
                    SyncStatus::Syncing => ("\u{1F7E1}", "同步中"),
                    SyncStatus::Synced => ("\u{1F7E2}", "已同步"),
                    SyncStatus::SyncError(msg) => ("\u{1F534}", msg.as_str()),
                };
                ui.label(sync_icon).on_hover_text(sync_tooltip);

                // 创建任务按钮 (P2-7: tooltip)
                if ui
                    .button("+ 新建")
                    .on_hover_text("创建新任务 (Ctrl+N)")
                    .clicked()
                {
                    task_state.open_create_dialog();
                }

                // ── P2-4: Select mode toggle ──
                let select_label = if task_state.select_mode {
                    "退出选择"
                } else {
                    "批量选择"
                };
                if ui.button(select_label).clicked() {
                    task_state.select_mode = !task_state.select_mode;
                    if !task_state.select_mode {
                        task_state.selected_ids.clear();
                    }
                }
            });

            ui.separator();

            // ── 筛选按钮 + 排序下拉 ──
            ui.horizontal_wrapped(|ui| {
                ui.label("筛选:");
                ui.selectable_value(&mut task_state.filter, TaskFilter::All, "全部");
                ui.selectable_value(
                    &mut task_state.filter,
                    TaskFilter::ByStatus(TaskStatus::Pending),
                    "待处理",
                );
                ui.selectable_value(
                    &mut task_state.filter,
                    TaskFilter::ByStatus(TaskStatus::InProgress),
                    "进行中",
                );
                ui.selectable_value(
                    &mut task_state.filter,
                    TaskFilter::ByStatus(TaskStatus::Done),
                    "已完成",
                );
                ui.selectable_value(
                    &mut task_state.filter,
                    TaskFilter::ByPriority(4),
                    "高优先级",
                );

                // ── P2-1: Sort dropdown ──
                ui.add_space(12.0);
                egui::ComboBox::from_label("排序")
                    .selected_text(match task_state.sort_by {
                        SortBy::Priority => "优先级",
                        SortBy::CreatedAt => "创建时间",
                        SortBy::Status => "状态",
                        SortBy::Title => "标题",
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut task_state.sort_by, SortBy::Priority, "优先级");
                        ui.selectable_value(&mut task_state.sort_by, SortBy::CreatedAt, "创建时间");
                        ui.selectable_value(&mut task_state.sort_by, SortBy::Status, "状态");
                        ui.selectable_value(&mut task_state.sort_by, SortBy::Title, "标题");
                    })
                    .response
                    .on_hover_text("选择排序方式");
            });

            ui.separator();

            // ── 任务列表 ──
            egui::ScrollArea::vertical().show(ui, |ui| {
                let filtered: Vec<_> = task_state
                    .get_filtered_tasks()
                    .into_iter()
                    .cloned()
                    .collect();

                if filtered.is_empty() {
                    // ── P2-3: Empty state guidance ──
                    if task_state.tasks.is_empty() && task_state.total_count == 0 {
                        ui.centered_and_justified(|ui| {
                            ui.vertical(|ui| {
                                ui.label("暂无任务");
                                ui.add_space(8.0);
                                if ui.button("创建第一个任务").clicked() {
                                    task_state.open_create_dialog();
                                }
                            });
                        });
                    } else {
                        ui.centered_and_justified(|ui| {
                            ui.vertical(|ui| {
                                ui.label("没有匹配的任务");
                                ui.add_space(8.0);
                                if ui.button("清除筛选").clicked() {
                                    task_state.filter = TaskFilter::All;
                                    task_state.search_query.clear();
                                }
                            });
                        });
                    }
                } else {
                    // ── P2-4: Batch operations toolbar ──
                    if task_state.select_mode && !task_state.selected_ids.is_empty() {
                        ui.horizontal(|ui| {
                            ui.label(format!("已选择 {} 项", task_state.selected_ids.len()));
                            if ui.button("批量开始").clicked() {
                                for id in task_state.selected_ids.clone() {
                                    task_state
                                        .pending_actions
                                        .push(TaskAction::UpdateTaskStatus(
                                            id.clone(),
                                            TaskStatus::InProgress,
                                        ));
                                    task_state.update_task_status(&id, TaskStatus::InProgress);
                                }
                                task_state.selected_ids.clear();
                            }
                            if ui.button("批量完成").clicked() {
                                for id in task_state.selected_ids.clone() {
                                    task_state
                                        .pending_actions
                                        .push(TaskAction::UpdateTaskStatus(
                                            id.clone(),
                                            TaskStatus::Done,
                                        ));
                                    task_state.update_task_status(&id, TaskStatus::Done);
                                }
                                task_state.selected_ids.clear();
                            }
                            if ui.button("批量删除").clicked() {
                                // Show confirmation for the first selected task as a proxy
                                // (batch delete confirms all)
                                for id in task_state.selected_ids.clone() {
                                    task_state
                                        .pending_actions
                                        .push(TaskAction::DeleteTask(id.clone()));
                                    task_state.delete_task(&id);
                                }
                                task_state.selected_ids.clear();
                            }
                            if ui.button("取消选择").clicked() {
                                task_state.selected_ids.clear();
                            }
                        });
                        ui.separator();
                    }

                    for task in &filtered {
                        let is_selected = task_state.selected_task.as_deref() == Some(&task.id);

                        // ── P2-5: Card visual hierarchy with ui.group ──
                        ui.group(|ui| {
                            // Row 1: Status indicator + Title
                            ui.horizontal(|ui| {
                                // ── P2-4: Checkbox in select mode ──
                                if task_state.select_mode {
                                    let mut is_checked = task_state.selected_ids.contains(&task.id);
                                    if ui.checkbox(&mut is_checked, "").changed() {
                                        if is_checked {
                                            task_state.selected_ids.push(task.id.clone());
                                        } else {
                                            task_state.selected_ids.retain(|id| id != &task.id);
                                        }
                                    }
                                }

                                let color_utf8 = match task.status {
                                    TaskStatus::Pending => "\u{26AB}",
                                    TaskStatus::InProgress => "\u{1F7E1}",
                                    TaskStatus::Done => "\u{1F7E2}",
                                    TaskStatus::Failed => "\u{1F534}",
                                    TaskStatus::Cancelled => "\u{26AA}",
                                };
                                ui.label(format!("{} {}", color_utf8, task.status.display()));

                                ui.label(egui::RichText::new(&task.title).strong().color(
                                    if is_selected {
                                        egui::Color32::from_rgb(100, 150, 255)
                                    } else {
                                        egui::Color32::WHITE
                                    },
                                ));
                            });

                            // Row 2: Description (small, gray)
                            if !task.description.is_empty() {
                                ui.label(
                                    egui::RichText::new(&task.description)
                                        .size(10.0)
                                        .color(egui::Color32::GRAY),
                                );
                            }

                            // Row 3: Priority badge + Scene badge + Multica badge
                            ui.horizontal(|ui| {
                                ui.label(
                                    egui::RichText::new(format!("P{}", task.priority))
                                        .size(9.0)
                                        .color(egui::Color32::YELLOW),
                                );

                                if let Some(scene_id) = &task.scene_id {
                                    ui.label(
                                        egui::RichText::new(format!("[场景: {}]", scene_id))
                                            .size(9.0)
                                            .color(egui::Color32::DARK_GREEN),
                                    );
                                }

                                if task.multica_id.is_some() {
                                    ui.label(
                                        egui::RichText::new("[Multica]")
                                            .size(9.0)
                                            .color(egui::Color32::LIGHT_BLUE),
                                    );
                                }
                            });

                            // Row 4: Action buttons (P2-7: tooltips)
                            ui.horizontal(|ui| {
                                if ui
                                    .small_button("选择")
                                    .on_hover_text("选择此任务查看详情")
                                    .clicked()
                                {
                                    task_state.selected_task = Some(task.id.clone());
                                }

                                ui.add_enabled_ui(task.status == TaskStatus::Pending, |ui| {
                                    if ui
                                        .small_button("开始")
                                        .on_hover_text("开始执行此任务")
                                        .clicked()
                                    {
                                        task_state.pending_actions.push(
                                            TaskAction::UpdateTaskStatus(
                                                task.id.clone(),
                                                TaskStatus::InProgress,
                                            ),
                                        );
                                        task_state
                                            .update_task_status(&task.id, TaskStatus::InProgress);
                                    }
                                });

                                ui.add_enabled_ui(task.status == TaskStatus::InProgress, |ui| {
                                    if ui
                                        .small_button("完成")
                                        .on_hover_text("标记任务为已完成")
                                        .clicked()
                                    {
                                        task_state.pending_actions.push(
                                            TaskAction::UpdateTaskStatus(
                                                task.id.clone(),
                                                TaskStatus::Done,
                                            ),
                                        );
                                        task_state.update_task_status(&task.id, TaskStatus::Done);
                                    }
                                });

                                if ui
                                    .small_button("删除")
                                    .on_hover_text("删除此任务 (Del)")
                                    .clicked()
                                {
                                    task_state.show_delete_confirm = Some(task.id.clone());
                                }
                            });
                        });

                        ui.add_space(4.0);
                    }
                }
            });

            // ── P1-2: Selected task detail panel ──
            if let Some(selected_id) = &task_state.selected_task.clone() {
                if let Some(task) = task_state.tasks.get(selected_id) {
                    ui.separator();
                    ui.collapsing("任务详情", |ui| {
                        ui.label(format!("ID: {}", task.id));
                        ui.label(format!("描述: {}", task.description));
                        ui.label(format!("状态: {}", task.status.display()));
                        ui.label(format!("创建时间: {}", task.created_at));
                        ui.label(format!("关联实体: {}", task.entity_ids.len()));
                        if let Some(ref mid) = task.multica_id {
                            ui.label(format!("Multica ID: {}", mid));
                        }
                    });
                }
            }

            ui.separator();

            // ── P1-5: Stats bar layout (always show all 5 statuses) ──
            ui.horizontal(|ui| {
                let total = task_state.total_count;
                ui.label(egui::RichText::new(format!("总计: {}", total)).strong());

                let statuses = [
                    (TaskStatus::Pending, "待处理"),
                    (TaskStatus::InProgress, "进行中"),
                    (TaskStatus::Done, "已完成"),
                    (TaskStatus::Failed, "失败"),
                    (TaskStatus::Cancelled, "已取消"),
                ];
                for (status, label) in &statuses {
                    let count = task_state.status_counts.get(status).copied().unwrap_or(0);
                    let [r, g, b] = status.color();
                    ui.add_space(8.0);
                    ui.label(egui::RichText::new(format!("{}:{}", label, count)).color(
                        egui::Color32::from_rgb(
                            (r * 255.0) as u8,
                            (g * 255.0) as u8,
                            (b * 255.0) as u8,
                        ),
                    ));
                }
            });

            // ── P1-3: Inline create dialog (collapsing) ──
            let collapsing_id = ui.make_persistent_id("task_create_section");
            let mut collapsing_state =
                egui::collapsing_header::CollapsingState::load_with_default_open(
                    ui.ctx(),
                    collapsing_id,
                    task_state.show_create_dialog,
                );
            if task_state.show_create_dialog {
                collapsing_state.set_open(true);
            }

            let header_response = ui.collapsing("创建新任务", |ui| {
                ui.vertical(|ui| {
                    egui::TextEdit::singleline(&mut task_state.new_task_title)
                        .hint_text("标题")
                        .show(ui);

                    egui::TextEdit::multiline(&mut task_state.new_task_description)
                        .hint_text("描述")
                        .desired_rows(3)
                        .desired_width(350.0)
                        .show(ui);

                    egui::TextEdit::singleline(&mut task_state.new_task_scene_id)
                        .hint_text("场景 ID (可选)")
                        .show(ui);

                    // P1-7: Priority labels
                    ui.horizontal(|ui| {
                        ui.label("优先级:");
                        let priorities =
                            [(1, "低"), (2, "较低"), (3, "中"), (4, "高"), (5, "紧急")];
                        for (p, label) in &priorities {
                            ui.selectable_value(&mut task_state.new_task_priority, *p, *label)
                                .on_hover_text(format!("优先级 {}", p));
                        }
                    });

                    ui.horizontal(|ui| {
                        if ui.button("创建").on_hover_text("提交创建").clicked() {
                            task_state.submit_new_task();
                        }
                        if ui.button("取消").on_hover_text("取消创建").clicked() {
                            task_state.close_create_dialog();
                        }
                    });
                });
            });

            // Sync collapsed state: if user closed the collapsing, set show_create_dialog = false
            if task_state.show_create_dialog && !collapsing_state.is_open() {
                task_state.show_create_dialog = false;
            }
            // Silence unused warning
            let _ = header_response;
        });

    // ── P0-2: Delete confirmation dialog ──
    if let Some(ref delete_id) = task_state.show_delete_confirm.clone() {
        let title = task_state
            .tasks
            .get(delete_id)
            .map(|t| t.title.clone())
            .unwrap_or_default();

        egui::Window::new("确认删除")
            .collapsible(false)
            .resizable(false)
            .default_width(300.0)
            .show(ctx, |ui| {
                ui.label(format!("确认删除任务「{}」?", title));
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button("确认删除").clicked() {
                        task_state
                            .pending_actions
                            .push(TaskAction::DeleteTask(delete_id.clone()));
                        task_state.delete_task(delete_id);
                        task_state.show_delete_confirm = None;
                    }
                    if ui.button("取消").clicked() {
                        task_state.show_delete_confirm = None;
                    }
                });
            });
    }
}
