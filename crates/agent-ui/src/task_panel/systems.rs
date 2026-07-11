//! Task Panel
//!
//! 任务面板组件，用于在 agent-ui 中展示和管理任务
//! 提供任务列表展示、筛选、创建和状态更新

use bevy::prelude::*;
use bevy_egui::EguiPrimaryContextPass;
use log::{debug, error};
use std::sync::{Arc, Mutex};

use super::model::{SyncStatus, TaskInfo, TaskPanelState, TaskStatus};
use super::port::{
    TaskPanelBackend, TaskPanelBackendError, TaskPanelBackendErrorKind, TaskPanelCommand,
    TaskPanelSnapshot,
};
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

/// Public scheduling boundary for composition-root task backend integrations.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum TaskPanelSystemSet {
    ProcessBackendActions,
}

// ════════════════════════════════════════════════════════════
// Backend Integration Resources
// ════════════════════════════════════════════════════════════

/// Backend-agnostic task panel access used by Bevy systems.
#[derive(Resource, Clone)]
pub struct TaskPanelBackendResource {
    backend: Arc<Mutex<Box<dyn TaskPanelBackend>>>,
}

impl TaskPanelBackendResource {
    pub fn new(backend: impl TaskPanelBackend) -> Self {
        Self {
            backend: Arc::new(Mutex::new(Box::new(backend))),
        }
    }

    pub fn handle(&self, command: TaskPanelCommand) -> Result<(), TaskPanelBackendError> {
        self.backend
            .lock()
            .map_err(|_| backend_unavailable("task panel backend lock poisoned"))?
            .handle(command)
    }

    pub fn handle_all(
        &self,
        commands: impl IntoIterator<Item = TaskPanelCommand>,
    ) -> Vec<TaskPanelBackendError> {
        commands
            .into_iter()
            .filter_map(|command| self.handle(command).err())
            .collect()
    }

    pub fn snapshot(&self) -> Result<TaskPanelSnapshot, TaskPanelBackendError> {
        self.backend
            .lock()
            .map_err(|_| backend_unavailable("task panel backend lock poisoned"))?
            .snapshot()
    }
}

fn backend_unavailable(message: impl Into<String>) -> TaskPanelBackendError {
    TaskPanelBackendError {
        kind: TaskPanelBackendErrorKind::Unavailable,
        message: message.into(),
    }
}

/// 任务面板插件
pub struct TaskPanelPlugin;

impl Plugin for TaskPanelPlugin {
    fn build(&self, app: &mut App) {
        app
            // Core state
            .init_resource::<TaskPanelState>()
            // Legacy task action message channel retained for public compatibility
            .add_message::<TaskAction>()
            // UI rendering
            .add_systems(EguiPrimaryContextPass, render_task_panel)
            // Backend integration system (runs after UI to process queued commands)
            .add_systems(
                Update,
                process_task_actions.in_set(TaskPanelSystemSet::ProcessBackendActions),
            );
    }
}

// ════════════════════════════════════════════════════════════
// Backend Integration Systems
// ════════════════════════════════════════════════════════════

/// Route pending panel commands through the backend port, then merge its snapshot.
fn process_task_actions(
    mut task_state: ResMut<TaskPanelState>,
    backend: Option<Res<TaskPanelBackendResource>>,
) {
    let actions: Vec<TaskPanelCommand> = std::mem::take(&mut task_state.pending_actions);
    if actions.is_empty() {
        return;
    }

    let Some(backend) = backend else {
        debug!(
            "No TaskPanelBackendResource found; {} task actions remain unprocessed",
            actions.len()
        );
        task_state.pending_actions = actions;
        return;
    };

    let commands = actions
        .into_iter()
        .map(|command| task_state.route_backend_command(command));
    let errors = backend.handle_all(commands);
    for backend_error in &errors {
        error!(
            "Task panel backend command failed: {}",
            backend_error.message
        );
    }

    match backend.snapshot() {
        Ok(snapshot) => {
            task_state.apply_backend_snapshot(snapshot);
            if !errors.is_empty() {
                task_state.sync_status = SyncStatus::SyncError(
                    errors
                        .iter()
                        .map(|error| error.message.as_str())
                        .collect::<Vec<_>>()
                        .join("; "),
                );
            }
        }
        Err(snapshot_error) => {
            error!(
                "Task panel backend snapshot failed: {}",
                snapshot_error.message
            );
            task_state.sync_status = SyncStatus::SyncError(snapshot_error.message);
        }
    }
}
