use super::{SyncStatus, TaskInfo, TaskStatus};

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
