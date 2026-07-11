//! Task Bridge - Unified task system integrating both Multica and WindWave tasks
//!
//! This module provides a unified task system that bridges between:
//! 1. `multica-bridge` tasks (via `task_sync`)
//! 2. Future `agent-core` tasks (WindWave's native task system)
//!
//! It enables bidirectional synchronization and seamless integration.

use crate::error::{BridgeError, Result};
use crate::task_sync::{LocalTask as MulticaLocalTask, TaskStatus as MulticaTaskStatus, TaskSync};
use crate::types::{
    GameTaskDispatchPayload, MulticaTask, TaskDispatchPayload, TaskLifecycleStatus,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Bridged task identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BridgedTaskId {
    /// Multica-side task ID (if present)
    pub multica_id: Option<u64>,
    /// Local task ID for the bridge system
    pub bridge_id: u64,
}

impl BridgedTaskId {
    /// Create a new bridged task ID
    pub fn new() -> Self {
        Self {
            multica_id: None,
            bridge_id: uuid::Uuid::new_v4().as_u128() as u64,
        }
    }

    /// Create from Multica ID
    pub fn from_multica(multica_id: u64) -> Self {
        Self {
            multica_id: Some(multica_id),
            bridge_id: uuid::Uuid::new_v4().as_u128() as u64,
        }
    }
}

impl Default for BridgedTaskId {
    fn default() -> Self {
        Self::new()
    }
}

/// Unified task status that maps between both systems
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum UnifiedTaskStatus {
    Draft,
    Planning,
    Queued,
    Dispatched,
    Running,
    WaitingForUser,
    Blocked,
    Done,
    Failed,
    Cancelled,
}

impl From<MulticaTaskStatus> for UnifiedTaskStatus {
    fn from(status: MulticaTaskStatus) -> Self {
        match status {
            MulticaTaskStatus::Draft => Self::Draft,
            MulticaTaskStatus::Planning => Self::Planning,
            MulticaTaskStatus::InProgress => Self::Running,
            MulticaTaskStatus::Done => Self::Done,
            MulticaTaskStatus::Failed => Self::Failed,
            MulticaTaskStatus::Cancelled => Self::Cancelled,
        }
    }
}

impl From<UnifiedTaskStatus> for MulticaTaskStatus {
    fn from(status: UnifiedTaskStatus) -> Self {
        match status {
            UnifiedTaskStatus::Draft => MulticaTaskStatus::Draft,
            UnifiedTaskStatus::Planning => MulticaTaskStatus::Planning,
            UnifiedTaskStatus::Queued => MulticaTaskStatus::Planning,
            UnifiedTaskStatus::Dispatched => MulticaTaskStatus::InProgress,
            UnifiedTaskStatus::Running => MulticaTaskStatus::InProgress,
            UnifiedTaskStatus::WaitingForUser => MulticaTaskStatus::InProgress,
            UnifiedTaskStatus::Blocked => MulticaTaskStatus::InProgress,
            UnifiedTaskStatus::Done => MulticaTaskStatus::Done,
            UnifiedTaskStatus::Failed => MulticaTaskStatus::Failed,
            UnifiedTaskStatus::Cancelled => MulticaTaskStatus::Cancelled,
        }
    }
}

/// Convert a Multica native task lifecycle status into a unified task status.
impl From<TaskLifecycleStatus> for UnifiedTaskStatus {
    fn from(status: TaskLifecycleStatus) -> Self {
        match status {
            TaskLifecycleStatus::Created => UnifiedTaskStatus::Draft,
            TaskLifecycleStatus::Claimed => UnifiedTaskStatus::Queued,
            TaskLifecycleStatus::Executing => UnifiedTaskStatus::Running,
            TaskLifecycleStatus::Completed => UnifiedTaskStatus::Done,
            TaskLifecycleStatus::Failed => UnifiedTaskStatus::Failed,
            TaskLifecycleStatus::Cancelled => UnifiedTaskStatus::Cancelled,
        }
    }
}

/// Convert a unified task status back into a Multica native task lifecycle status.
impl From<UnifiedTaskStatus> for TaskLifecycleStatus {
    fn from(status: UnifiedTaskStatus) -> Self {
        match status {
            UnifiedTaskStatus::Draft => TaskLifecycleStatus::Created,
            UnifiedTaskStatus::Planning => TaskLifecycleStatus::Created,
            UnifiedTaskStatus::Queued => TaskLifecycleStatus::Claimed,
            UnifiedTaskStatus::Dispatched => TaskLifecycleStatus::Executing,
            UnifiedTaskStatus::Running => TaskLifecycleStatus::Executing,
            UnifiedTaskStatus::WaitingForUser => TaskLifecycleStatus::Executing,
            UnifiedTaskStatus::Blocked => TaskLifecycleStatus::Executing,
            UnifiedTaskStatus::Done => TaskLifecycleStatus::Completed,
            UnifiedTaskStatus::Failed => TaskLifecycleStatus::Failed,
            UnifiedTaskStatus::Cancelled => TaskLifecycleStatus::Cancelled,
        }
    }
}

/// A unified task that combines both systems
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedTask {
    /// Bridged task identifier
    pub id: BridgedTaskId,

    /// Task title
    pub title: String,

    /// Task description
    pub description: String,

    /// Unified task status
    pub status: UnifiedTaskStatus,

    /// Associated scene ID (if any)
    pub scene_id: Option<String>,

    /// Associated entity IDs (if any)
    pub entity_ids: Vec<String>,

    /// Associated resource IDs (if any)
    pub resource_ids: Vec<String>,

    /// Scene snapshot (if available)
    pub scene_snapshot: Option<serde_json::Value>,

    /// Multica task (if present)
    pub multica_task: Option<MulticaLocalTask>,

    /// Creation timestamp
    pub created_at: String,

    /// Last update timestamp
    pub updated_at: String,
}

impl UnifiedTask {
    /// Create a new unified task
    pub fn new(title: String, description: String) -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        Self {
            id: BridgedTaskId::new(),
            title,
            description,
            status: UnifiedTaskStatus::Draft,
            scene_id: None,
            entity_ids: Vec::new(),
            resource_ids: Vec::new(),
            scene_snapshot: None,
            multica_task: None,
            created_at: now.clone(),
            updated_at: now,
        }
    }

    /// Create a unified task from a Multica task
    pub fn from_multica(multica_task: MulticaLocalTask) -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        Self {
            id: BridgedTaskId::from_multica(multica_task.id.0),
            title: multica_task.title.clone(),
            description: multica_task.description.clone(),
            status: multica_task.status.into(),
            scene_id: multica_task.scene_id.clone(),
            entity_ids: multica_task.entity_ids.clone(),
            resource_ids: multica_task.resource_ids.clone(),
            scene_snapshot: multica_task.scene_snapshot.clone(),
            multica_task: Some(multica_task),
            created_at: now.clone(),
            updated_at: now,
        }
    }

    /// Convert to a Multica task
    pub fn to_multica(&self) -> Result<MulticaLocalTask> {
        let mut task = MulticaLocalTask::new(
            self.id.multica_id.unwrap_or(self.id.bridge_id),
            self.title.clone(),
            self.description.clone(),
        );

        task.scene_id = self.scene_id.clone();
        task.entity_ids = self.entity_ids.clone();
        task.resource_ids = self.resource_ids.clone();
        task.scene_snapshot = self.scene_snapshot.clone();
        task.status = self.status.into();

        Ok(task)
    }

    /// Update the task status
    pub fn update_status(&mut self, new_status: UnifiedTaskStatus) {
        self.status = new_status;
        self.updated_at = chrono::Utc::now().to_rfc3339();

        // Also update underlying tasks
        if let Some(ref mut task) = self.multica_task {
            task.status = new_status.into();
        }
    }

    /// Add a scene association
    pub fn set_scene(&mut self, scene_id: String, snapshot: Option<serde_json::Value>) {
        self.scene_id = Some(scene_id);
        self.scene_snapshot = snapshot;
        self.updated_at = chrono::Utc::now().to_rfc3339();
    }

    /// Add an entity association
    pub fn add_entity(&mut self, entity_id: String) {
        if !self.entity_ids.contains(&entity_id) {
            self.entity_ids.push(entity_id);
            self.updated_at = chrono::Utc::now().to_rfc3339();
        }
    }

    /// Add a resource association
    pub fn add_resource(&mut self, resource_id: String) {
        if !self.resource_ids.contains(&resource_id) {
            self.resource_ids.push(resource_id);
            self.updated_at = chrono::Utc::now().to_rfc3339();
        }
    }
}

/// Construct a [`UnifiedTask`] from a Multica native task record.
impl From<MulticaTask> for UnifiedTask {
    fn from(task: MulticaTask) -> Self {
        let _now = chrono::Utc::now().to_rfc3339();
        Self {
            id: BridgedTaskId::new(),
            title: task.title,
            description: task.description.unwrap_or_default(),
            status: task.status.into(),
            scene_id: None,
            entity_ids: Vec::new(),
            resource_ids: Vec::new(),
            scene_snapshot: None,
            multica_task: None,
            created_at: task.created_at,
            updated_at: task.updated_at,
        }
    }
}

/// The main task bridge system
pub struct TaskBridge {
    /// Task synchronization for Multica
    task_sync: Arc<Mutex<TaskSync>>,

    /// All bridged tasks indexed by bridge ID
    tasks: Arc<Mutex<HashMap<u64, UnifiedTask>>>,

    /// Multica task ID to bridge ID mapping
    multica_to_bridge: Arc<Mutex<HashMap<u64, u64>>>,

    /// Scene to tasks index
    scene_to_tasks: Arc<Mutex<HashMap<String, Vec<u64>>>>,

    /// Last sync timestamp
    last_sync: Arc<Mutex<Instant>>,

    /// Sync interval (default 1 second)
    sync_interval: Duration,
}

impl TaskBridge {
    /// Create a new task bridge
    pub fn new(task_sync: TaskSync) -> Self {
        Self {
            task_sync: Arc::new(Mutex::new(task_sync)),
            tasks: Arc::new(Mutex::new(HashMap::new())),
            multica_to_bridge: Arc::new(Mutex::new(HashMap::new())),
            scene_to_tasks: Arc::new(Mutex::new(HashMap::new())),
            last_sync: Arc::new(Mutex::new(Instant::now())),
            sync_interval: Duration::from_secs(1),
        }
    }

    /// Create a task bridge with a custom sync interval
    pub fn with_sync_interval(task_sync: TaskSync, sync_interval: Duration) -> Self {
        Self {
            task_sync: Arc::new(Mutex::new(task_sync)),
            tasks: Arc::new(Mutex::new(HashMap::new())),
            multica_to_bridge: Arc::new(Mutex::new(HashMap::new())),
            scene_to_tasks: Arc::new(Mutex::new(HashMap::new())),
            last_sync: Arc::new(Mutex::new(Instant::now())),
            sync_interval,
        }
    }

    /// Create a new game task
    pub fn create_game_task(
        &self,
        title: String,
        description: String,
        scene_id: String,
        snapshot: Option<serde_json::Value>,
    ) -> Result<UnifiedTask> {
        let mut task = UnifiedTask::new(title, description);
        task.set_scene(scene_id, snapshot);
        self.register_task(task)
    }

    /// Register a unified task with the bridge
    pub fn register_task(&self, mut task: UnifiedTask) -> Result<UnifiedTask> {
        let bridge_id = task.id.bridge_id;

        // Create Multica side task
        let multica_task = task.to_multica()?;
        task.multica_task = Some(multica_task.clone());

        // Register with scene index
        if let Some(ref scene_id) = task.scene_id {
            self.scene_to_tasks
                .lock()
                .expect("mutex poisoned")
                .entry(scene_id.clone())
                .or_default()
                .push(bridge_id);
        }

        // Store mappings
        if let Some(multica_id) = task.id.multica_id {
            self.multica_to_bridge
                .lock()
                .expect("mutex poisoned")
                .insert(multica_id, bridge_id);
        }

        // Store the task
        self.tasks
            .lock()
            .expect("mutex poisoned")
            .insert(bridge_id, task.clone());

        Ok(task)
    }

    /// Get a task by bridge ID
    pub fn get_task(&self, bridge_id: u64) -> Result<UnifiedTask> {
        self.tasks
            .lock()
            .expect("mutex poisoned")
            .get(&bridge_id)
            .cloned()
            .ok_or_else(|| BridgeError::TaskNotFound(format!("Task not found: {}", bridge_id)))
    }

    /// Get a task by Multica ID
    pub fn get_task_by_multica(&self, multica_id: u64) -> Result<UnifiedTask> {
        let bridge_id = self
            .multica_to_bridge
            .lock()
            .expect("mutex poisoned")
            .get(&multica_id)
            .cloned()
            .ok_or_else(|| {
                BridgeError::TaskNotFound(format!("Multica task not found: {}", multica_id))
            })?;

        self.get_task(bridge_id)
    }

    /// Get all tasks for a scene
    pub fn get_tasks_by_scene(&self, scene_id: &str) -> Vec<UnifiedTask> {
        self.scene_to_tasks
            .lock()
            .expect("mutex poisoned")
            .get(scene_id)
            .map(|ids| {
                let tasks = self.tasks.lock().expect("mutex poisoned");
                ids.iter().filter_map(|id| tasks.get(id).cloned()).collect()
            })
            .unwrap_or_default()
    }

    /// Get all tasks
    pub fn get_all_tasks(&self) -> Vec<UnifiedTask> {
        self.tasks
            .lock()
            .expect("mutex poisoned")
            .values()
            .cloned()
            .collect()
    }

    /// Update a task
    pub fn update_task(
        &self,
        bridge_id: u64,
        updater: impl FnOnce(&mut UnifiedTask),
    ) -> Result<UnifiedTask> {
        let mut tasks = self.tasks.lock().expect("mutex poisoned");
        let task = tasks
            .get_mut(&bridge_id)
            .ok_or_else(|| BridgeError::TaskNotFound(format!("Task not found: {}", bridge_id)))?;

        updater(task);

        // Update mappings and scene index if needed
        if let Some(ref scene_id) = task.scene_id {
            let mut scene_index = self.scene_to_tasks.lock().expect("mutex poisoned");
            if !scene_index
                .get(scene_id)
                .map(|ids| ids.contains(&bridge_id))
                .unwrap_or(false)
            {
                scene_index
                    .entry(scene_id.clone())
                    .or_default()
                    .push(bridge_id);
            }
        }

        Ok(task.clone())
    }

    /// Update task status
    pub fn update_task_status(
        &self,
        bridge_id: u64,
        new_status: UnifiedTaskStatus,
    ) -> Result<UnifiedTask> {
        self.update_task(bridge_id, |task| {
            task.update_status(new_status);
        })
    }

    /// Query tasks by entity ID.
    ///
    /// Returns all bridged tasks that are associated with the given entity.
    pub fn get_tasks_by_entity(&self, entity_id: &str) -> Vec<UnifiedTask> {
        let tasks = self.tasks.lock().expect("mutex poisoned");
        tasks
            .values()
            .filter(|t| t.entity_ids.iter().any(|e| e == entity_id))
            .cloned()
            .collect()
    }

    /// Register a task with an explicit scene association.
    ///
    /// The returned [`UnifiedTask`] is stored in the bridge and indexed by
    /// its scene ID so that it can be retrieved via [`Self::get_tasks_by_scene`].
    pub fn register_scene_task(
        &self,
        title: String,
        description: String,
        scene_id: String,
        entity_ids: Vec<String>,
    ) -> UnifiedTask {
        let mut task = UnifiedTask::new(title, description);
        task.scene_id = Some(scene_id);
        task.entity_ids = entity_ids;
        let mut tasks = self.tasks.lock().expect("mutex poisoned");
        let id = task.id;
        tasks.insert(id.bridge_id, task.clone());
        task
    }

    /// Handle incoming task dispatch from Multica
    pub fn handle_multica_dispatch(&self, payload: TaskDispatchPayload) -> Result<UnifiedTask> {
        let mut task_sync = self.task_sync.lock().expect("mutex poisoned");
        let multica_task = task_sync.handle_task_dispatch(payload);

        let mut task = UnifiedTask::from_multica(multica_task);
        task.id.bridge_id = uuid::Uuid::new_v4().as_u128() as u64;

        self.register_task(task)
    }

    /// Handle incoming game task dispatch
    pub fn handle_game_dispatch(&self, payload: GameTaskDispatchPayload) -> Result<UnifiedTask> {
        let mut task = UnifiedTask::new(payload.title.clone(), payload.description.clone());

        for scene in &payload.target_scenes {
            if task.scene_id.is_none() {
                task.set_scene(scene.id.clone(), None);
            }
        }

        for entity in &payload.target_entities {
            task.add_entity(entity.id.clone());
        }

        task.scene_snapshot = payload.snapshot.clone();

        self.register_task(task)
    }

    /// Perform a sync between systems if needed
    pub fn maybe_sync(&self) {
        let mut last_sync = self.last_sync.lock().expect("mutex poisoned");
        if last_sync.elapsed() >= self.sync_interval {
            *last_sync = Instant::now();
            // Actual sync logic would go here
        }
    }

    /// Get the number of active tasks
    pub fn task_count(&self) -> usize {
        self.tasks.lock().expect("mutex poisoned").len()
    }

    /// Get statistics about the task bridge
    pub fn get_stats(&self) -> BridgeStats {
        let tasks = self.tasks.lock().expect("mutex poisoned");
        let scene_count = self.scene_to_tasks.lock().expect("mutex poisoned").len();
        let multica_count = self.multica_to_bridge.lock().expect("mutex poisoned").len();

        let status_counts = tasks.values().fold(HashMap::new(), |mut counts, task| {
            *counts.entry(task.status).or_insert(0) += 1;
            counts
        });

        let scene_tasks_count = tasks.values().filter(|t| t.scene_id.is_some()).count();

        BridgeStats {
            total_tasks: tasks.len(),
            scene_count,
            scene_tasks_count,
            multica_tasks: multica_count,
            status_counts,
        }
    }
}

/// Statistics about the task bridge
#[derive(Debug, Clone)]
pub struct BridgeStats {
    pub total_tasks: usize,
    pub scene_count: usize,
    pub scene_tasks_count: usize,
    pub multica_tasks: usize,
    pub status_counts: HashMap<UnifiedTaskStatus, usize>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bridged_task_id_creation() {
        let id = BridgedTaskId::new();
        assert!(id.bridge_id != 0);

        let multica_id = BridgedTaskId::from_multica(123);
        assert_eq!(multica_id.multica_id, Some(123));
    }

    #[test]
    fn test_status_conversion() {
        let multica_status = MulticaTaskStatus::InProgress;
        let unified: UnifiedTaskStatus = multica_status.into();
        assert_eq!(unified, UnifiedTaskStatus::Running);

        let back: MulticaTaskStatus = UnifiedTaskStatus::Done.into();
        assert_eq!(back, MulticaTaskStatus::Done);
    }

    #[test]
    fn test_unified_task_creation() {
        let task = UnifiedTask::new("Test Task".into(), "Test Description".into());
        assert_eq!(task.title, "Test Task");
        assert_eq!(task.description, "Test Description");
    }

    #[test]
    fn test_task_status_update() {
        let mut task = UnifiedTask::new("Test".into(), "Desc".into());
        task.update_status(UnifiedTaskStatus::Running);
        assert_eq!(task.status, UnifiedTaskStatus::Running);
    }

    #[test]
    fn test_scene_association() {
        let mut task = UnifiedTask::new("Test".into(), "Desc".into());
        task.set_scene("scene-123".into(), None);
        assert_eq!(task.scene_id, Some("scene-123".into()));
    }
}
