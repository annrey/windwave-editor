use super::{SyncStatus, TaskInfo, TaskStatus};
use bevy::prelude::Message;

#[derive(Message, Clone, Debug, PartialEq, Eq)]
pub enum TaskAction {
    CreateTask(TaskInfo),
    UpdateTaskStatus(String, TaskStatus),
    DeleteTask(String),
    RefreshTasks,
}

impl From<TaskAction> for TaskPanelCommand {
    fn from(action: TaskAction) -> Self {
        match action {
            TaskAction::CreateTask(task) => Self::Create(task),
            TaskAction::UpdateTaskStatus(id, status) => Self::UpdateStatus { id, status },
            TaskAction::DeleteTask(id) => Self::Delete { id },
            TaskAction::RefreshTasks => Self::Refresh,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskPanelCommand {
    Refresh,
    Create(TaskInfo),
    UpdateStatus { id: String, status: TaskStatus },
    Delete { id: String },
}

#[derive(Debug, Clone, Default)]
pub struct TaskPanelSnapshot {
    pub tasks: Vec<TaskInfo>,
    pub sync_status: SyncStatus,
}

pub struct TaskPanelBackendTransaction {
    pub command_results: Vec<TaskPanelCommandResult>,
    pub errors: Vec<TaskPanelBackendError>,
    pub snapshot: Result<TaskPanelSnapshot, TaskPanelBackendError>,
}

pub struct TaskPanelCommandResult {
    pub command: TaskPanelCommand,
    pub result: Result<(), TaskPanelBackendError>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskPanelBackendErrorKind {
    Unavailable,
    Disconnected,
    Rejected,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskPanelBackendError {
    pub kind: TaskPanelBackendErrorKind,
    pub message: String,
}

pub trait TaskPanelBackend: Send + Sync + 'static {
    fn snapshot(&self) -> Result<TaskPanelSnapshot, TaskPanelBackendError>;
    fn handle(&mut self, command: TaskPanelCommand) -> Result<(), TaskPanelBackendError>;
}
