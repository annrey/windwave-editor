//! Task Panel
//!
//! 任务面板组件，用于在 agent-ui 中展示和管理任务
//! 提供任务列表展示、筛选、创建、状态更新和 Multica 集成
//!
//! 与 multica-bridge 集成，支持统一任务系统和双向同步

use bevy::prelude::*;
use bevy_egui::EguiPrimaryContextPass;
use log::{debug, error, info, warn};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

// multica-bridge integration types
use multica_bridge::scene_event_bus::{
    SceneEvent, SceneEventSubscriber, SceneEventType, SubscriberId,
};
use multica_bridge::task_bridge::{TaskBridge, UnifiedTask, UnifiedTaskStatus};
use multica_bridge::task_sync_module::{SyncStatus as ModuleSyncStatus, TaskSynchronizer};

use super::model::{SyncStatus, TaskInfo, TaskPanelState, TaskStatus};
use super::port::TaskPanelCommand;
use super::view::render_task_panel;

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
    let actions: Vec<TaskPanelCommand> = std::mem::take(&mut task_state.pending_actions);
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
fn dispatch_task_action(
    action: &TaskPanelCommand,
    bridge: &TaskBridge,
    state: &mut TaskPanelState,
) {
    match action {
        TaskPanelCommand::Create(task_info) => {
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

        TaskPanelCommand::UpdateStatus {
            id: task_id,
            status: new_status,
        } => {
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

        TaskPanelCommand::Delete { id: task_id } => {
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

        TaskPanelCommand::Refresh => {
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
