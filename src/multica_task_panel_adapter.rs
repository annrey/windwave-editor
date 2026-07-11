use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use agent_ui::{
    CreatedTaskAliases, SyncStatus, TaskInfo, TaskPanelBackend, TaskPanelBackendError,
    TaskPanelBackendErrorKind, TaskPanelBackendResource, TaskPanelCommand, TaskPanelSnapshot,
    TaskPanelState, TaskPanelSystemSet, TaskStatus,
};
use bevy::log::{debug, error, warn};
use bevy::prelude::*;
use multica_bridge::error::BridgeError;
use multica_bridge::scene_event_bus::{
    create_shared_event_bus, SceneEvent, SceneEventSubscriber, SceneEventType, SharedSceneEventBus,
    SubscriberId,
};
use multica_bridge::task_bridge::{TaskBridge, UnifiedTask, UnifiedTaskStatus};
use multica_bridge::task_sync::TaskSync;
use multica_bridge::task_sync_module::{
    SyncStatus as ModuleSyncStatus, TaskSyncConfig, TaskSynchronizer,
};
use multica_bridge::types::BridgeConfig;

pub struct MulticaTaskPanelAdapter {
    bridge: Arc<TaskBridge>,
    created_aliases: CreatedTaskAliases,
}

impl MulticaTaskPanelAdapter {
    pub fn new(bridge: Arc<TaskBridge>) -> Self {
        Self {
            bridge,
            created_aliases: CreatedTaskAliases::default(),
        }
    }

    fn bridge_id(task_id: &str) -> Option<u64> {
        task_id
            .strip_prefix("task_bridge_")
            .unwrap_or(task_id)
            .parse()
            .ok()
    }
}

fn map_status(status: UnifiedTaskStatus) -> TaskStatus {
    match status {
        UnifiedTaskStatus::Draft | UnifiedTaskStatus::Planning | UnifiedTaskStatus::Queued => {
            TaskStatus::Pending
        }
        UnifiedTaskStatus::Dispatched
        | UnifiedTaskStatus::Running
        | UnifiedTaskStatus::WaitingForUser
        | UnifiedTaskStatus::Blocked => TaskStatus::InProgress,
        UnifiedTaskStatus::Done => TaskStatus::Done,
        UnifiedTaskStatus::Failed => TaskStatus::Failed,
        UnifiedTaskStatus::Cancelled => TaskStatus::Cancelled,
    }
}

fn panel_status_to_unified(status: TaskStatus) -> UnifiedTaskStatus {
    match status {
        TaskStatus::Pending => UnifiedTaskStatus::Draft,
        TaskStatus::InProgress => UnifiedTaskStatus::Running,
        TaskStatus::Done => UnifiedTaskStatus::Done,
        TaskStatus::Failed => UnifiedTaskStatus::Failed,
        TaskStatus::Cancelled => UnifiedTaskStatus::Cancelled,
    }
}

fn task_info_to_unified(task: &TaskInfo) -> UnifiedTask {
    let mut unified = UnifiedTask::new(task.title.clone(), task.description.clone());
    unified.status = panel_status_to_unified(task.status.clone());
    unified.created_at = task.created_at.clone();
    unified.updated_at = task.created_at.clone();
    if let Some(scene_id) = &task.scene_id {
        unified.set_scene(scene_id.clone(), None);
    }
    for entity_id in &task.entity_ids {
        unified.add_entity(entity_id.clone());
    }
    if let Some(bridge_id) = task
        .multica_id
        .as_deref()
        .and_then(|id| id.parse::<u64>().ok())
    {
        unified.id.bridge_id = bridge_id;
    }
    unified
}

fn backend_error(context: impl Into<String>, error: BridgeError) -> TaskPanelBackendError {
    let kind = match error {
        BridgeError::WebSocketError(_)
        | BridgeError::NetworkError(_)
        | BridgeError::ConnectionClosed => TaskPanelBackendErrorKind::Disconnected,
        BridgeError::TaskNotFound(_) | BridgeError::InvalidState(_) => {
            TaskPanelBackendErrorKind::Rejected
        }
        BridgeError::SerializationError(_)
        | BridgeError::UrlParseError(_)
        | BridgeError::AgentNotFound(_)
        | BridgeError::Other(_) => TaskPanelBackendErrorKind::Unavailable,
    };
    let message = format!("{}: {error}", context.into());
    error!("Task panel adapter failure: {message}");
    TaskPanelBackendError { kind, message }
}

impl TaskPanelBackend for MulticaTaskPanelAdapter {
    fn snapshot(&self) -> Result<TaskPanelSnapshot, TaskPanelBackendError> {
        let tasks = self
            .bridge
            .get_all_tasks()
            .into_iter()
            .map(|unified| {
                let bridge_id = unified.id.bridge_id.to_string();
                let mut task = TaskInfo::new(
                    format!("task_bridge_{bridge_id}"),
                    unified.title,
                    unified.description,
                );
                task.status = map_status(unified.status);
                task.scene_id = unified.scene_id;
                task.created_at = unified.created_at;
                task.entity_ids = unified.entity_ids;
                task.multica_id = Some(bridge_id);
                task
            })
            .collect();
        Ok(self.created_aliases.apply(TaskPanelSnapshot {
            tasks,
            sync_status: SyncStatus::Synced,
        }))
    }

    fn handle(&mut self, command: TaskPanelCommand) -> Result<(), TaskPanelBackendError> {
        match command {
            TaskPanelCommand::Refresh => Ok(()),
            TaskPanelCommand::Create(task) => {
                let panel_id = task.id.clone();
                let title = task.title.clone();
                let registered = self
                    .bridge
                    .register_task(task_info_to_unified(&task))
                    .map_err(|error| {
                        backend_error(format!("failed to create task '{title}'"), error)
                    })?;
                self.created_aliases
                    .record(registered.id.bridge_id.to_string(), panel_id);
                Ok(())
            }
            TaskPanelCommand::UpdateStatus { id, status } => {
                let Some(bridge_id) = Self::bridge_id(&id) else {
                    debug!("Task '{id}' has no bridge ID; status update applied locally only");
                    return Ok(());
                };
                self.bridge
                    .update_task_status(bridge_id, panel_status_to_unified(status))
                    .map_err(|error| {
                        backend_error(format!("failed to update task '{id}' status"), error)
                    })?;
                Ok(())
            }
            TaskPanelCommand::Delete { id } => {
                let Some(bridge_id) = Self::bridge_id(&id) else {
                    debug!("Task '{id}' has no bridge ID; deleted locally only");
                    return Ok(());
                };
                self.bridge
                    .update_task_status(bridge_id, UnifiedTaskStatus::Cancelled)
                    .map_err(|error| {
                        backend_error(format!("failed to cancel task '{id}'"), error)
                    })?;
                Ok(())
            }
        }
    }
}

#[derive(Resource, Clone)]
pub struct TaskSynchronizerResource {
    synchronizer: Arc<TaskSynchronizer>,
}

#[derive(Resource, Clone, Default)]
pub struct SceneEventQueue {
    events: Arc<Mutex<Vec<SceneEvent>>>,
}

impl SceneEventQueue {
    fn push(&self, event: SceneEvent) {
        if let Ok(mut queue) = self.events.lock() {
            if queue.len() >= 1000 {
                warn!("Scene event queue full, dropping oldest event");
                queue.remove(0);
            }
            queue.push(event);
        }
    }

    fn drain(&self) -> Vec<SceneEvent> {
        self.events
            .lock()
            .map(|mut queue| std::mem::take(&mut *queue))
            .unwrap_or_default()
    }
}

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

#[derive(Resource)]
pub struct SceneEventSubscriberId {
    _subscriber_id: SubscriberId,
}

#[derive(Resource, Clone)]
pub struct SceneEventBusResource {
    _bus: SharedSceneEventBus,
}

pub struct MulticaTaskPanelPlugin;

impl Plugin for MulticaTaskPanelPlugin {
    fn build(&self, app: &mut App) {
        let bridge = Arc::new(TaskBridge::new(TaskSync::new(BridgeConfig::default())));
        let scene_event_queue = SceneEventQueue::default();
        let scene_event_bus = create_shared_event_bus();
        let subscriber_id = scene_event_bus
            .lock()
            .expect("scene event bus lock poisoned during setup")
            .subscribe(Arc::new(PanelSceneEventSubscriber::new(
                scene_event_queue.clone(),
            )));
        app.insert_resource(TaskPanelBackendResource::new(MulticaTaskPanelAdapter::new(
            bridge,
        )))
        .insert_resource(TaskSynchronizerResource {
            synchronizer: Arc::new(TaskSynchronizer::new(TaskSyncConfig::default())),
        })
        .insert_resource(scene_event_queue)
        .insert_resource(SceneEventBusResource {
            _bus: scene_event_bus,
        })
        .insert_resource(SceneEventSubscriberId {
            _subscriber_id: subscriber_id,
        })
        .add_systems(
            Update,
            (process_scene_events, update_sync_status)
                .chain()
                .after(TaskPanelSystemSet::ProcessBackendActions),
        );
    }
}

fn process_scene_events(queue: Res<SceneEventQueue>, mut task_state: ResMut<TaskPanelState>) {
    let events = queue.drain();
    if events.is_empty() {
        return;
    }
    let mut affected_scenes: HashMap<String, Vec<&SceneEvent>> = HashMap::new();
    for event in &events {
        affected_scenes
            .entry(event.scene_id.clone())
            .or_default()
            .push(event);
    }
    for (scene_id, scene_events) in &affected_scenes {
        let task_count = task_state
            .tasks
            .values()
            .filter(|task| task.scene_id.as_deref() == Some(scene_id.as_str()))
            .count();
        if task_count > 0 {
            let count = |kind| {
                scene_events
                    .iter()
                    .filter(|event| event.event_type == kind)
                    .count()
            };
            debug!(
                "Scene '{}' activity: +{} entities created, ~{} updated, -{} deleted ({} linked tasks)",
                scene_id,
                count(SceneEventType::EntityCreated),
                count(SceneEventType::EntityUpdated),
                count(SceneEventType::EntityDeleted),
                task_count
            );
        }
    }
    for event in &events {
        let entity_id = event.entity_id.to_string();
        if event.event_type == SceneEventType::EntityCreated {
            for task in task_state.tasks.values_mut() {
                if task.scene_id.as_deref() == Some(event.scene_id.as_str())
                    && !task.entity_ids.contains(&entity_id)
                {
                    task.entity_ids.push(entity_id.clone());
                }
            }
        } else if event.event_type == SceneEventType::EntityDeleted {
            for task in task_state.tasks.values_mut() {
                task.entity_ids.retain(|id| id != &entity_id);
            }
        }
    }
}

fn update_sync_status(
    synchronizer: Res<TaskSynchronizerResource>,
    mut task_state: ResMut<TaskPanelState>,
) {
    let status = match synchronizer.synchronizer.get_status() {
        ModuleSyncStatus::Idle => SyncStatus::NotSynced,
        ModuleSyncStatus::Syncing => SyncStatus::Syncing,
        ModuleSyncStatus::Success | ModuleSyncStatus::Conflict(_) => SyncStatus::Synced,
        ModuleSyncStatus::Failed(message) => SyncStatus::SyncError(message),
    };
    if task_state.sync_status != status {
        task_state.sync_status = status;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_ui::TaskStatus;
    use multica_bridge::task_bridge::UnifiedTaskStatus;

    #[test]
    fn maps_multica_status_to_ui_status() {
        assert_eq!(map_status(UnifiedTaskStatus::Draft), TaskStatus::Pending);
        assert_eq!(map_status(UnifiedTaskStatus::Planning), TaskStatus::Pending);
        assert_eq!(map_status(UnifiedTaskStatus::Queued), TaskStatus::Pending);
        assert_eq!(
            map_status(UnifiedTaskStatus::Dispatched),
            TaskStatus::InProgress
        );
        assert_eq!(
            map_status(UnifiedTaskStatus::Running),
            TaskStatus::InProgress
        );
        assert_eq!(
            map_status(UnifiedTaskStatus::WaitingForUser),
            TaskStatus::InProgress
        );
        assert_eq!(
            map_status(UnifiedTaskStatus::Blocked),
            TaskStatus::InProgress
        );
        assert_eq!(map_status(UnifiedTaskStatus::Done), TaskStatus::Done);
        assert_eq!(map_status(UnifiedTaskStatus::Failed), TaskStatus::Failed);
        assert_eq!(
            map_status(UnifiedTaskStatus::Cancelled),
            TaskStatus::Cancelled
        );
    }
}
