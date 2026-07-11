use std::collections::HashMap;
use std::panic::{catch_unwind, AssertUnwindSafe};
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
    integration: MulticaTaskPanelIntegration,
    created_aliases: Mutex<CreatedTaskAliases>,
}

impl MulticaTaskPanelAdapter {
    pub fn new(integration: MulticaTaskPanelIntegration) -> Self {
        Self {
            integration,
            created_aliases: Mutex::new(CreatedTaskAliases::default()),
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
    let kind = error_kind(&error);
    let message = format!("{}: {error}", context.into());
    error!("Task panel adapter failure: {message}");
    TaskPanelBackendError { kind, message }
}

fn error_kind(error: &BridgeError) -> TaskPanelBackendErrorKind {
    match error {
        BridgeError::WebSocketError(_)
        | BridgeError::NetworkError(_)
        | BridgeError::ConnectionClosed => TaskPanelBackendErrorKind::Disconnected,
        BridgeError::TaskNotFound(_)
        | BridgeError::AgentNotFound(_)
        | BridgeError::InvalidState(_) => TaskPanelBackendErrorKind::Rejected,
        BridgeError::SerializationError(_)
        | BridgeError::UrlParseError(_)
        | BridgeError::Other(_) => TaskPanelBackendErrorKind::Unavailable,
    }
}

fn catch_bridge_panic<T>(
    context: &str,
    operation: impl FnOnce() -> Result<T, BridgeError>,
) -> Result<T, TaskPanelBackendError> {
    match catch_unwind(AssertUnwindSafe(operation)) {
        Ok(result) => result.map_err(|error| backend_error(context, error)),
        Err(_) => {
            let message = format!("{context}: backend lock or operation panicked");
            error!("Task panel adapter failure: {message}");
            Err(TaskPanelBackendError {
                kind: TaskPanelBackendErrorKind::Unavailable,
                message,
            })
        }
    }
}

impl TaskPanelBackend for MulticaTaskPanelAdapter {
    fn snapshot(&self) -> Result<TaskPanelSnapshot, TaskPanelBackendError> {
        let unified_tasks = catch_bridge_panic("failed to read task snapshot", || {
            Ok(self.integration.bridge.get_all_tasks())
        })?;
        let tasks = unified_tasks
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
        let snapshot = TaskPanelSnapshot {
            tasks,
            sync_status: SyncStatus::Synced,
        };
        let mut aliases = self
            .created_aliases
            .lock()
            .map_err(|_| TaskPanelBackendError {
                kind: TaskPanelBackendErrorKind::Unavailable,
                message: "task alias lock poisoned".into(),
            })?;
        let snapshot = aliases.apply(snapshot);
        *aliases = CreatedTaskAliases::default();
        Ok(snapshot)
    }

    fn handle(&mut self, command: TaskPanelCommand) -> Result<(), TaskPanelBackendError> {
        match command {
            TaskPanelCommand::Refresh => Ok(()),
            TaskPanelCommand::Create(task) => {
                let panel_id = task.id.clone();
                let title = task.title.clone();
                let registered =
                    catch_bridge_panic(&format!("failed to create task '{title}'"), || {
                        self.integration
                            .bridge
                            .register_task(task_info_to_unified(&task))
                    })?;
                catch_bridge_panic("failed to expose created task to synchronizer", || {
                    self.integration
                        .synchronizer
                        .add_local_task(registered.clone());
                    self.integration
                        .synchronizer
                        .add_pending_sync(registered.id);
                    Ok(())
                })?;
                self.created_aliases
                    .lock()
                    .map_err(|_| TaskPanelBackendError {
                        kind: TaskPanelBackendErrorKind::Unavailable,
                        message: "task alias lock poisoned".into(),
                    })?
                    .record(registered.id.bridge_id.to_string(), panel_id);
                Ok(())
            }
            TaskPanelCommand::UpdateStatus { id, status } => {
                let Some(bridge_id) = Self::bridge_id(&id) else {
                    debug!("Task '{id}' has no bridge ID; status update applied locally only");
                    return Ok(());
                };
                let unified_status = panel_status_to_unified(status);
                catch_bridge_panic(&format!("failed to update task '{id}' status"), || {
                    self.integration
                        .bridge
                        .update_task_status(bridge_id, unified_status)
                })?;
                catch_bridge_panic("failed to expose task update to synchronizer", || {
                    self.integration
                        .synchronizer
                        .update_local_task_status(bridge_id, unified_status)?;
                    let task = self.integration.bridge.get_task(bridge_id)?;
                    self.integration.synchronizer.add_pending_sync(task.id);
                    Ok(())
                })?;
                Ok(())
            }
            TaskPanelCommand::Delete { id } => {
                let Some(bridge_id) = Self::bridge_id(&id) else {
                    debug!("Task '{id}' has no bridge ID; deleted locally only");
                    return Ok(());
                };
                catch_bridge_panic(&format!("failed to cancel task '{id}'"), || {
                    self.integration
                        .bridge
                        .update_task_status(bridge_id, UnifiedTaskStatus::Cancelled)
                })?;
                catch_bridge_panic("failed to expose task cancellation to synchronizer", || {
                    self.integration
                        .synchronizer
                        .update_local_task_status(bridge_id, UnifiedTaskStatus::Cancelled)?;
                    let task = self.integration.bridge.get_task(bridge_id)?;
                    self.integration.synchronizer.add_pending_sync(task.id);
                    Ok(())
                })?;
                Ok(())
            }
        }
    }
}

#[derive(Resource, Clone, Default)]
pub struct SceneEventQueue {
    events: Arc<Mutex<Vec<SceneEvent>>>,
    last_error: Arc<Mutex<Option<String>>>,
}

impl SceneEventQueue {
    fn record_error(&self, message: String) {
        error!("Task panel scene event queue failure: {message}");
        if let Ok(mut error) = self.last_error.lock() {
            *error = Some(message);
        }
    }

    fn push(&self, event: SceneEvent) -> Result<(), String> {
        let mut queue = self.events.lock().map_err(|_| {
            let message = "scene event queue lock poisoned while publishing".to_string();
            self.record_error(message.clone());
            message
        })?;
        if queue.len() >= 1000 {
            warn!("Scene event queue full, dropping oldest event");
            queue.remove(0);
        }
        queue.push(event);
        Ok(())
    }

    fn drain(&self) -> Result<Vec<SceneEvent>, String> {
        self.events
            .lock()
            .map(|mut queue| std::mem::take(&mut *queue))
            .map_err(|_| {
                let message = "scene event queue lock poisoned while draining".to_string();
                self.record_error(message.clone());
                message
            })
    }

    #[allow(dead_code)] // Root producers use this diagnostic API when integrating scene sources.
    fn pending_len(&self) -> Result<usize, String> {
        self.events.lock().map(|queue| queue.len()).map_err(|_| {
            let message = "scene event queue lock poisoned while inspecting".to_string();
            self.record_error(message.clone());
            message
        })
    }

    #[allow(dead_code)] // Read by root publish callers; no producer is installed in this binary yet.
    fn last_error(&self) -> Option<String> {
        self.last_error.lock().ok().and_then(|error| error.clone())
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
        if let Err(message) = self.queue.push(event.clone()) {
            error!("Task panel scene event subscriber failed: {message}");
        }
    }
}

#[derive(Resource)]
pub struct SceneEventSubscriberId {
    _subscriber_id: Option<SubscriberId>,
}

#[derive(Resource, Default)]
struct PublishedSceneIndexState {
    entity_fingerprint_by_id: HashMap<u64, String>,
}

#[derive(Resource, Clone)]
#[allow(dead_code)] // Composition-root integration surface for scene/task producers.
pub struct MulticaTaskPanelIntegration {
    bridge: Arc<TaskBridge>,
    synchronizer: Arc<TaskSynchronizer>,
    scene_event_bus: SharedSceneEventBus,
    scene_event_queue: SceneEventQueue,
}

#[allow(dead_code)] // Methods are consumed as root producers are composed into the binary.
impl MulticaTaskPanelIntegration {
    pub(crate) fn publish_scene_event(
        &self,
        event: SceneEvent,
    ) -> Result<(), TaskPanelBackendError> {
        let mut bus = self
            .scene_event_bus
            .lock()
            .map_err(|_| TaskPanelBackendError {
                kind: TaskPanelBackendErrorKind::Unavailable,
                message: "scene event bus lock poisoned while publishing".into(),
            })?;
        bus.publish(event);
        if let Some(message) = self.scene_event_queue.last_error() {
            return Err(TaskPanelBackendError {
                kind: TaskPanelBackendErrorKind::Unavailable,
                message,
            });
        }
        Ok(())
    }

    pub(crate) fn pending_scene_event_count(&self) -> Result<usize, TaskPanelBackendError> {
        self.scene_event_queue
            .pending_len()
            .map_err(|message| TaskPanelBackendError {
                kind: TaskPanelBackendErrorKind::Unavailable,
                message,
            })
    }

    pub(crate) fn synchronized_tasks_for_scene(
        &self,
        scene_id: &str,
    ) -> Result<Vec<UnifiedTask>, TaskPanelBackendError> {
        catch_bridge_panic("failed to inspect synchronized tasks", || {
            Ok(self.synchronizer.get_scene_tasks(scene_id))
        })
    }
}

pub struct MulticaTaskPanelPlugin;

impl Plugin for MulticaTaskPanelPlugin {
    fn build(&self, app: &mut App) {
        let bridge = Arc::new(TaskBridge::new(TaskSync::new(BridgeConfig::default())));
        let synchronizer = Arc::new(TaskSynchronizer::new(TaskSyncConfig::default()));
        let scene_event_queue = SceneEventQueue::default();
        let scene_event_bus = create_shared_event_bus();
        let subscriber_id = match scene_event_bus.lock() {
            Ok(mut bus) => Some(bus.subscribe(Arc::new(PanelSceneEventSubscriber::new(
                scene_event_queue.clone(),
            )))),
            Err(_) => {
                error!("Task panel scene event bus lock poisoned during setup");
                None
            }
        };
        let integration = MulticaTaskPanelIntegration {
            bridge,
            synchronizer,
            scene_event_bus,
            scene_event_queue: scene_event_queue.clone(),
        };
        app.insert_resource(TaskPanelBackendResource::new(MulticaTaskPanelAdapter::new(
            integration.clone(),
        )))
        .insert_resource(integration)
        .insert_resource(scene_event_queue)
        .insert_resource(SceneEventSubscriberId {
            _subscriber_id: subscriber_id,
        })
        .init_resource::<PublishedSceneIndexState>()
        .add_systems(
            Update,
            (
                publish_scene_index_changes,
                process_scene_events,
                update_sync_status,
            )
                .chain()
                .after(TaskPanelSystemSet::ProcessBackendActions),
        );
    }
}

fn publish_scene_index_changes(
    cache: Option<Res<bevy_adapter::integration::SceneIndexCache>>,
    integration: Res<MulticaTaskPanelIntegration>,
    mut published: ResMut<PublishedSceneIndexState>,
) {
    let Some(cache) = cache else {
        return;
    };
    let current: HashMap<u64, String> = cache
        .get()
        .entities_by_name
        .iter()
        .map(|(name, id)| {
            let fingerprint = cache
                .get()
                .get_entity_by_name(name)
                .and_then(|node| serde_json::to_string(node).ok())
                .unwrap_or_else(|| name.clone());
            (*id, fingerprint)
        })
        .collect();
    let timestamp = format!("{:?}", std::time::SystemTime::now());
    for (entity_id, name) in &current {
        let event_type = match published.entity_fingerprint_by_id.get(entity_id) {
            None => Some(SceneEventType::EntityCreated),
            Some(previous_name) if previous_name != name => Some(SceneEventType::EntityUpdated),
            Some(_) => None,
        };
        if let Some(event_type) = event_type {
            if let Err(error) = integration.publish_scene_event(SceneEvent {
                scene_id: "default".into(),
                event_type,
                entity_id: *entity_id,
                entity_before: None,
                entity_after: None,
                component: None,
                timestamp: timestamp.clone(),
            }) {
                error!("Failed to publish SceneIndex change: {}", error.message);
            }
        }
    }
    for entity_id in published
        .entity_fingerprint_by_id
        .keys()
        .filter(|entity_id| !current.contains_key(entity_id))
    {
        if let Err(error) = integration.publish_scene_event(SceneEvent {
            scene_id: "default".into(),
            event_type: SceneEventType::EntityDeleted,
            entity_id: *entity_id,
            entity_before: None,
            entity_after: None,
            component: None,
            timestamp: timestamp.clone(),
        }) {
            error!("Failed to publish SceneIndex deletion: {}", error.message);
        }
    }
    published.entity_fingerprint_by_id = current;
}

fn process_scene_events(queue: Res<SceneEventQueue>, mut task_state: ResMut<TaskPanelState>) {
    let events = match queue.drain() {
        Ok(events) => events,
        Err(message) => {
            task_state.sync_status = SyncStatus::SyncError(message);
            return;
        }
    };
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
    integration: Res<MulticaTaskPanelIntegration>,
    mut task_state: ResMut<TaskPanelState>,
) {
    let module_status =
        match catch_unwind(AssertUnwindSafe(|| integration.synchronizer.get_status())) {
            Ok(status) => status,
            Err(_) => {
                task_state.sync_status =
                    SyncStatus::SyncError("task synchronizer lock poisoned".into());
                return;
            }
        };
    if let Some(status) = map_sync_status(module_status) {
        if task_state.sync_status != status {
            task_state.sync_status = status;
        }
    }
}

fn map_sync_status(status: ModuleSyncStatus) -> Option<SyncStatus> {
    match status {
        ModuleSyncStatus::Idle => None,
        ModuleSyncStatus::Syncing => Some(SyncStatus::Syncing),
        ModuleSyncStatus::Success | ModuleSyncStatus::Conflict(_) => Some(SyncStatus::Synced),
        ModuleSyncStatus::Failed(message) => Some(SyncStatus::SyncError(message)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_ui::{TaskPanelBackendResource, TaskPanelCommand, TaskStatus};
    use multica_bridge::task_bridge::UnifiedTaskStatus;
    use multica_bridge::task_sync_module::SyncStatus as ModuleSyncStatus;

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

    #[test]
    fn maps_ui_status_back_to_multica_status() {
        assert_eq!(
            panel_status_to_unified(TaskStatus::Pending),
            UnifiedTaskStatus::Draft
        );
        assert_eq!(
            panel_status_to_unified(TaskStatus::InProgress),
            UnifiedTaskStatus::Running
        );
        assert_eq!(
            panel_status_to_unified(TaskStatus::Done),
            UnifiedTaskStatus::Done
        );
        assert_eq!(
            panel_status_to_unified(TaskStatus::Failed),
            UnifiedTaskStatus::Failed
        );
        assert_eq!(
            panel_status_to_unified(TaskStatus::Cancelled),
            UnifiedTaskStatus::Cancelled
        );
    }

    #[test]
    fn classifies_bridge_errors_consistently() {
        assert_eq!(
            error_kind(&BridgeError::ConnectionClosed),
            TaskPanelBackendErrorKind::Disconnected
        );
        assert_eq!(
            error_kind(&BridgeError::NetworkError("offline".into())),
            TaskPanelBackendErrorKind::Disconnected
        );
        assert_eq!(
            error_kind(&BridgeError::TaskNotFound("task".into())),
            TaskPanelBackendErrorKind::Rejected
        );
        assert_eq!(
            error_kind(&BridgeError::AgentNotFound("agent".into())),
            TaskPanelBackendErrorKind::Rejected
        );
        assert_eq!(
            error_kind(&BridgeError::Other("internal".into())),
            TaskPanelBackendErrorKind::Unavailable
        );
    }

    #[test]
    fn maps_active_and_failed_sync_states_but_not_idle() {
        assert_eq!(map_sync_status(ModuleSyncStatus::Idle), None);
        assert_eq!(
            map_sync_status(ModuleSyncStatus::Syncing),
            Some(SyncStatus::Syncing)
        );
        assert_eq!(
            map_sync_status(ModuleSyncStatus::Success),
            Some(SyncStatus::Synced)
        );
        assert_eq!(
            map_sync_status(ModuleSyncStatus::Failed("offline".into())),
            Some(SyncStatus::SyncError("offline".into()))
        );
    }

    #[test]
    fn plugin_crud_updates_the_shared_synchronizer_and_preserves_success_status() {
        let mut app = App::new();
        app.init_resource::<TaskPanelState>()
            .add_plugins(MulticaTaskPanelPlugin);

        let task = TaskInfo::new("panel-1".into(), "Quest".into(), "Find item".into())
            .with_scene("scene-1".into());
        app.world()
            .resource::<TaskPanelBackendResource>()
            .handle(TaskPanelCommand::Create(task))
            .unwrap();
        let integration = app.world().resource::<MulticaTaskPanelIntegration>();
        let tasks = integration.synchronized_tasks_for_scene("scene-1").unwrap();
        assert_eq!(tasks.len(), 1);
        let bridge_id = tasks[0].id.bridge_id;

        app.world()
            .resource::<TaskPanelBackendResource>()
            .handle(TaskPanelCommand::UpdateStatus {
                id: bridge_id.to_string(),
                status: TaskStatus::Done,
            })
            .unwrap();
        let tasks = app
            .world()
            .resource::<MulticaTaskPanelIntegration>()
            .synchronized_tasks_for_scene("scene-1")
            .unwrap();
        assert_eq!(tasks[0].status, UnifiedTaskStatus::Done);

        let snapshot = app
            .world()
            .resource::<TaskPanelBackendResource>()
            .snapshot()
            .unwrap();
        assert_eq!(snapshot.sync_status, SyncStatus::Synced);
        app.world_mut()
            .resource_mut::<TaskPanelState>()
            .apply_backend_snapshot(snapshot);
        app.update();
        assert_eq!(
            app.world().resource::<TaskPanelState>().sync_status,
            SyncStatus::Synced
        );
    }

    #[test]
    fn published_scene_event_is_consumed_and_updates_ui_task_state() {
        let mut app = App::new();
        app.init_resource::<TaskPanelState>()
            .add_plugins(MulticaTaskPanelPlugin);
        app.world_mut().resource_mut::<TaskPanelState>().add_task(
            TaskInfo::new("panel-1".into(), "Quest".into(), "Find item".into())
                .with_scene("scene-1".into()),
        );
        app.world()
            .resource::<MulticaTaskPanelIntegration>()
            .publish_scene_event(SceneEvent {
                scene_id: "scene-1".into(),
                event_type: SceneEventType::EntityCreated,
                entity_id: 42,
                entity_before: None,
                entity_after: None,
                component: None,
                timestamp: "now".into(),
            })
            .unwrap();
        app.update();
        assert_eq!(
            app.world().resource::<TaskPanelState>().tasks["panel-1"].entity_ids,
            vec!["42"]
        );
        assert_eq!(
            app.world()
                .resource::<MulticaTaskPanelIntegration>()
                .pending_scene_event_count()
                .unwrap(),
            0
        );
    }

    #[test]
    fn scene_index_rebuild_publishes_entity_creation_to_linked_tasks() {
        let mut app = App::new();
        app.init_resource::<TaskPanelState>().add_plugins((
            bevy_adapter::BevyAdapterPlugin,
            bevy_adapter::integration::SceneIndexRebuildPlugin::every(1),
            MulticaTaskPanelPlugin,
        ));
        app.world_mut().resource_mut::<TaskPanelState>().add_task(
            TaskInfo::new(
                "panel-real-scene".into(),
                "Quest".into(),
                "Track scene".into(),
            )
            .with_scene("default".into()),
        );
        let entity = app
            .world_mut()
            .spawn((Name::new("Real Producer Entity"), Transform::default()))
            .id();
        let entity_id = app
            .world_mut()
            .resource_mut::<bevy_adapter::BevyAdapter>()
            .register_entity(entity)
            .0;

        app.update();
        app.update();

        assert_eq!(
            app.world().resource::<TaskPanelState>().tasks["panel-real-scene"].entity_ids,
            vec![entity_id.to_string()]
        );
    }

    #[test]
    fn scene_queue_lock_failure_is_observable() {
        let queue = SceneEventQueue::default();
        let events = queue.events.clone();
        let _ = std::thread::spawn(move || {
            let _guard = events.lock().unwrap();
            panic!("poison queue");
        })
        .join();
        assert!(queue.pending_len().is_err());
        assert!(queue.last_error().unwrap().contains("poison"));
    }

    #[test]
    fn adapter_converts_backend_panics_to_unavailable_errors() {
        let error = catch_bridge_panic("snapshot", || -> Result<Vec<UnifiedTask>, BridgeError> {
            panic!("poisoned bridge lock")
        })
        .unwrap_err();
        assert_eq!(error.kind, TaskPanelBackendErrorKind::Unavailable);
        assert!(error.message.contains("snapshot"));
    }

    #[test]
    fn crud_roundtrip_preserves_alias_once_and_cancels_the_shared_task() {
        let bridge = Arc::new(TaskBridge::new(TaskSync::new(BridgeConfig::default())));
        let integration = MulticaTaskPanelIntegration {
            bridge,
            synchronizer: Arc::new(TaskSynchronizer::new(TaskSyncConfig::default())),
            scene_event_bus: create_shared_event_bus(),
            scene_event_queue: SceneEventQueue::default(),
        };
        let mut adapter = MulticaTaskPanelAdapter::new(integration.clone());
        let task = TaskInfo::new("panel-created".into(), "Quest".into(), "Find item".into())
            .with_scene("scene-1".into())
            .add_entity("7".into());
        let created_at = task.created_at.clone();
        adapter.handle(TaskPanelCommand::Create(task)).unwrap();

        let first = adapter.snapshot().unwrap();
        assert_eq!(first.tasks[0].id, "panel-created");
        assert_eq!(first.tasks[0].created_at, created_at);
        assert_eq!(first.tasks[0].scene_id.as_deref(), Some("scene-1"));
        assert_eq!(first.tasks[0].entity_ids, vec!["7"]);
        let bridge_id = first.tasks[0].multica_id.clone().unwrap();
        let second = adapter.snapshot().unwrap();
        assert_eq!(second.tasks[0].id, format!("task_bridge_{bridge_id}"));

        adapter
            .handle(TaskPanelCommand::Delete { id: bridge_id })
            .unwrap();
        assert_eq!(
            integration.synchronized_tasks_for_scene("scene-1").unwrap()[0].status,
            UnifiedTaskStatus::Cancelled
        );
    }

    #[test]
    fn deleted_remote_linked_task_stays_absent_after_cancelled_snapshots() {
        let mut app = App::new();
        app.add_plugins((agent_ui::TaskPanelPlugin, MulticaTaskPanelPlugin));
        let task = TaskInfo::new("panel-delete".into(), "Quest".into(), "Delete me".into())
            .with_scene("scene-1".into());
        app.world()
            .resource::<TaskPanelBackendResource>()
            .handle(TaskPanelCommand::Create(task))
            .unwrap();
        let snapshot = app
            .world()
            .resource::<TaskPanelBackendResource>()
            .snapshot()
            .unwrap();
        app.world_mut()
            .resource_mut::<TaskPanelState>()
            .apply_backend_snapshot(snapshot);
        let panel_id = app
            .world()
            .resource::<TaskPanelState>()
            .tasks
            .keys()
            .next()
            .unwrap()
            .clone();
        app.world_mut()
            .resource_mut::<TaskPanelState>()
            .pending_commands
            .push(TaskPanelCommand::Delete {
                id: panel_id.clone(),
            });

        app.update();
        assert!(!app
            .world()
            .resource::<TaskPanelState>()
            .tasks
            .contains_key(&panel_id));
        app.world_mut()
            .resource_mut::<TaskPanelState>()
            .pending_commands
            .push(TaskPanelCommand::Refresh);
        app.update();
        assert!(!app
            .world()
            .resource::<TaskPanelState>()
            .tasks
            .contains_key(&panel_id));
        assert_eq!(
            app.world()
                .resource::<MulticaTaskPanelIntegration>()
                .synchronized_tasks_for_scene("scene-1")
                .unwrap()[0]
                .status,
            UnifiedTaskStatus::Cancelled
        );
    }
}
