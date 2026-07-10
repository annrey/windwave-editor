use crate::error::{BridgeError, Result};
use crate::types::*;
use crate::ws_client::MulticaWebSocketClient;
use log::info;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Local task representation for WindWave
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalTask {
    pub id: TaskId,
    pub title: String,
    pub description: String,
    pub status: TaskStatus,
    /// Associated game scene
    pub scene_id: Option<String>,
    /// Associated game entities
    pub entity_ids: Vec<String>,
    /// Associated game resources
    pub resource_ids: Vec<String>,
    /// Scene snapshot when task was created
    pub scene_snapshot: Option<serde_json::Value>,
}

/// Task ID wrapper
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TaskId(pub u64);

impl std::fmt::Display for TaskId {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "task_{}", self.0)
    }
}

/// Task status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskStatus {
    Draft,
    Planning,
    InProgress,
    Done,
    Failed,
    Cancelled,
}

impl LocalTask {
    pub fn new(id: u64, title: String, description: String) -> Self {
        Self {
            id: TaskId(id),
            title,
            description,
            status: TaskStatus::InProgress,
            scene_id: None,
            entity_ids: Vec::new(),
            resource_ids: Vec::new(),
            scene_snapshot: None,
        }
    }

    /// Create a task linked to a game scene
    pub fn with_scene(
        id: u64,
        title: String,
        description: String,
        scene_id: String,
        scene_snapshot: Option<serde_json::Value>,
    ) -> Self {
        Self {
            id: TaskId(id),
            title,
            description,
            status: TaskStatus::InProgress,
            scene_id: Some(scene_id),
            entity_ids: Vec::new(),
            resource_ids: Vec::new(),
            scene_snapshot,
        }
    }

    /// Link an entity to this task
    pub fn add_entity(&mut self, entity_id: String) {
        if !self.entity_ids.contains(&entity_id) {
            self.entity_ids.push(entity_id);
        }
    }

    /// Link a resource to this task
    pub fn add_resource(&mut self, resource_id: String) {
        if !self.resource_ids.contains(&resource_id) {
            self.resource_ids.push(resource_id);
        }
    }
}

/// Task synchronizer between WindWave and Multica
pub struct TaskSync {
    ws_client: MulticaWebSocketClient,
    local_to_remote: HashMap<TaskId, String>,
    remote_to_local: HashMap<String, TaskId>,
    tasks: HashMap<TaskId, LocalTask>,
    scene_task_index: HashMap<String, Vec<TaskId>>,
    runtime_id: String,
}

impl TaskSync {
    /// Create new task sync
    pub fn new(config: BridgeConfig) -> Self {
        let runtime_id = uuid::Uuid::new_v4().to_string();
        Self {
            ws_client: MulticaWebSocketClient::new(config),
            local_to_remote: HashMap::new(),
            remote_to_local: HashMap::new(),
            tasks: HashMap::new(),
            scene_task_index: HashMap::new(),
            runtime_id,
        }
    }

    /// Connect and register daemon
    pub async fn connect(&mut self) -> Result<()> {
        match self.ws_client.connect().await {
            Ok(_) => self.ws_client.register_daemon().await,
            Err(e) => Err(e),
        }
    }

    /// Convert Multica TaskDispatch to WindWave Task
    fn dispatch_to_task(dispatch: TaskDispatchPayload) -> LocalTask {
        let task_id = uuid::Uuid::new_v4().as_u128() as u64;
        LocalTask::new(task_id, dispatch.title, dispatch.description)
    }

    /// Handle an incoming task dispatch from Multica
    pub fn handle_task_dispatch(&mut self, dispatch: TaskDispatchPayload) -> LocalTask {
        let task = Self::dispatch_to_task(dispatch.clone());

        self.local_to_remote
            .insert(task.id, dispatch.task_id.clone());
        self.remote_to_local.insert(dispatch.task_id, task.id);

        // Cache the task
        self.cache_task(task.clone());

        info!("Received and created task: {}", task.title);
        task
    }

    /// Handle game task dispatch
    pub fn handle_game_task_dispatch(&mut self, dispatch: GameTaskDispatchPayload) -> LocalTask {
        let mut task = Self::dispatch_to_task(TaskDispatchPayload {
            task_id: dispatch.task_id.clone(),
            issue_id: dispatch.issue_id.clone(),
            title: dispatch.title.clone(),
            description: dispatch.description.clone(),
        });

        // Link game context
        if let Some(first_scene) = dispatch.target_scenes.first() {
            task.scene_id = Some(first_scene.id.clone());
        }
        for entity in dispatch.target_entities {
            task.add_entity(entity.id.clone());
        }
        task.scene_snapshot = dispatch.snapshot;

        self.local_to_remote
            .insert(task.id, dispatch.task_id.clone());
        self.remote_to_local.insert(dispatch.task_id, task.id);

        // Cache the task
        self.cache_task(task.clone());

        info!("Received and created game task: {}", task.title);
        task
    }

    /// Cache a task locally
    fn cache_task(&mut self, task: LocalTask) {
        // Insert into tasks map
        self.tasks.insert(task.id, task.clone());

        // Update scene index if task is linked to a scene
        if let Some(scene_id) = &task.scene_id {
            self.scene_task_index
                .entry(scene_id.clone())
                .or_default()
                .push(task.id);
        }
    }

    /// Create a new game task linked to a scene
    pub fn create_game_task(
        &mut self,
        title: String,
        description: String,
        scene_id: String,
    ) -> LocalTask {
        let task_id = uuid::Uuid::new_v4().as_u128() as u64;
        let task = LocalTask::with_scene(task_id, title, description, scene_id, None);

        // Cache the task with proper indexing
        self.cache_task(task.clone());
        info!("Created game task: {}", task.title);
        task
    }

    /// Get all tasks linked to a specific scene
    pub fn get_tasks_by_scene(&self, scene_id: &str) -> Vec<&LocalTask> {
        self.scene_task_index
            .get(scene_id)
            .map(|ids| ids.iter().filter_map(|id| self.tasks.get(id)).collect())
            .unwrap_or_default()
    }

    /// Get a specific task by id
    pub fn get_task(&self, task_id: TaskId) -> Option<&LocalTask> {
        self.tasks.get(&task_id)
    }

    /// Get a mutable task reference
    pub fn get_task_mut(&mut self, task_id: TaskId) -> Option<&mut LocalTask> {
        self.tasks.get_mut(&task_id)
    }

    /// Get all tasks
    pub fn get_all_tasks(&self) -> Vec<&LocalTask> {
        self.tasks.values().collect()
    }

    /// Update task status and sync with Multica
    pub fn update_task_status(&mut self, task_id: TaskId, new_status: TaskStatus) -> Result<()> {
        if let Some(task) = self.tasks.get_mut(&task_id) {
            task.status = new_status;
            info!("Updated task {} status to {:?}", task.title, new_status);
        }
        Ok(())
    }

    /// Send task progress update to Multica
    pub async fn send_task_progress(
        &mut self,
        local_task_id: TaskId,
        summary: &str,
        step: i32,
        total: i32,
    ) -> Result<()> {
        if !self.ws_client.is_connected() {
            // If not connected, just log and return Ok for simulation mode
            log::debug!(
                "Not connected to server - progress would be sent: {}",
                summary
            );
            return Ok(());
        }

        let remote_task_id = self
            .local_to_remote
            .get(&local_task_id)
            .ok_or_else(|| BridgeError::TaskNotFound(local_task_id.to_string()))?;

        let payload = TaskProgressPayload {
            task_id: remote_task_id.clone(),
            summary: summary.to_string(),
            step: Some(step),
            total: Some(total),
        };

        let message = Message {
            message_type: EVENT_TASK_PROGRESS.to_string(),
            payload: serde_json::to_value(payload)?,
        };

        self.ws_client.send_message(message).await
    }

    /// Mark a task as completed in Multica
    pub async fn complete_task(
        &mut self,
        local_task_id: TaskId,
        output: Option<String>,
    ) -> Result<()> {
        if !self.ws_client.is_connected() {
            log::debug!("Not connected to server - completion would be sent");
            return Ok(());
        }

        let remote_task_id = self
            .local_to_remote
            .get(&local_task_id)
            .ok_or_else(|| BridgeError::TaskNotFound(local_task_id.to_string()))?;

        let payload = TaskCompletedPayload {
            task_id: remote_task_id.clone(),
            pr_url: None,
            output,
        };

        let message = Message {
            message_type: EVENT_TASK_COMPLETED.to_string(),
            payload: serde_json::to_value(payload)?,
        };

        self.ws_client.send_message(message).await
    }

    /// Send a task message to Multica
    pub async fn send_task_message(
        &mut self,
        local_task_id: TaskId,
        seq: i32,
        content: &str,
    ) -> Result<()> {
        if !self.ws_client.is_connected() {
            log::debug!(
                "Not connected to server - message would be sent: {}",
                content
            );
            return Ok(());
        }

        let remote_task_id = self
            .local_to_remote
            .get(&local_task_id)
            .ok_or_else(|| BridgeError::TaskNotFound(local_task_id.to_string()))?;

        let payload = TaskMessagePayload {
            task_id: remote_task_id.clone(),
            issue_id: None,
            seq,
            message_type: "text".to_string(),
            tool: None,
            content: Some(content.to_string()),
            input: None,
            output: None,
        };

        let message = Message {
            message_type: EVENT_TASK_MESSAGE.to_string(),
            payload: serde_json::to_value(payload)?,
        };

        self.ws_client.send_message(message).await
    }

    /// Send heartbeat
    pub async fn send_heartbeat(&mut self) -> Result<()> {
        self.ws_client.send_heartbeat(self.runtime_id.clone()).await
    }

    /// Receive a message from Multica
    pub async fn receive_message(&mut self) -> Result<Option<Message>> {
        self.ws_client.receive_message().await
    }

    /// Get runtime id
    pub fn runtime_id(&self) -> &str {
        &self.runtime_id
    }

    /// Check connection status
    pub fn is_connected(&self) -> bool {
        self.ws_client.is_connected()
    }

    /// Get remote task id
    pub fn get_remote_id(&self, local_id: TaskId) -> Option<&String> {
        self.local_to_remote.get(&local_id)
    }

    /// Get local task id
    pub fn get_local_id(&self, remote_id: &str) -> Option<TaskId> {
        self.remote_to_local.get(remote_id).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_local_task_creation() {
        let task = LocalTask::new(1, "Test Task".into(), "Test Description".into());
        assert_eq!(task.id, TaskId(1));
        assert_eq!(task.title, "Test Task");
        assert_eq!(task.status, TaskStatus::InProgress);
        assert!(task.scene_id.is_none());
    }

    #[test]
    fn test_local_task_with_scene() {
        let task = LocalTask::with_scene(
            1,
            "Test Task".into(),
            "Test Description".into(),
            "scene-123".into(),
            None,
        );
        assert_eq!(task.scene_id, Some("scene-123".into()));
    }

    #[test]
    fn test_local_task_add_entity() {
        let mut task = LocalTask::new(1, "Test".into(), "Desc".into());
        task.add_entity("entity-1".into());
        task.add_entity("entity-2".into());
        assert_eq!(task.entity_ids.len(), 2);
    }

    #[test]
    fn test_local_task_add_resource() {
        let mut task = LocalTask::new(1, "Test".into(), "Desc".into());
        task.add_resource("res-1".into());
        assert_eq!(task.resource_ids.len(), 1);
    }

    #[test]
    fn test_task_sync_create_game_task() {
        let config = BridgeConfig::default();
        let mut sync = TaskSync::new(config);

        let task = sync.create_game_task(
            "Test Task".into(),
            "Test Description".into(),
            "MainScene".into(),
        );
        assert_eq!(task.title, "Test Task");
        assert_eq!(task.scene_id, Some("MainScene".into()));

        let tasks = sync.get_tasks_by_scene("MainScene");
        assert_eq!(tasks.len(), 1);
    }

    #[test]
    fn test_task_sync_get_all_tasks() {
        let config = BridgeConfig::default();
        let mut sync = TaskSync::new(config);

        sync.create_game_task("Task 1".into(), "Desc 1".into(), "Scene 1".into());
        sync.create_game_task("Task 2".into(), "Desc 2".into(), "Scene 2".into());

        let all_tasks = sync.get_all_tasks();
        assert_eq!(all_tasks.len(), 2);
    }

    #[test]
    fn test_task_sync_update_status() {
        let config = BridgeConfig::default();
        let mut sync = TaskSync::new(config);

        let task = sync.create_game_task("Test".into(), "Desc".into(), "Scene".into());
        let task_id = task.id;

        sync.update_task_status(task_id, TaskStatus::Done).unwrap();
        let updated_task = sync.get_task(task_id).unwrap();
        assert_eq!(updated_task.status, TaskStatus::Done);
    }

    #[test]
    fn test_task_id_display() {
        let id = TaskId(42);
        assert_eq!(format!("{}", id), "task_42");
    }
}
