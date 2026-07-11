//! Task Panel
//!
//! 任务面板组件，用于在 agent-ui 中展示和管理任务
//! 提供任务列表展示、筛选、创建和状态更新

use bevy::prelude::*;
use bevy_egui::EguiPrimaryContextPass;
use log::{debug, error};
use std::sync::{Arc, Mutex};

use super::model::{SyncStatus, TaskPanelState};
use super::port::{
    TaskAction, TaskPanelBackend, TaskPanelBackendError, TaskPanelBackendErrorKind,
    TaskPanelBackendTransaction, TaskPanelCommand, TaskPanelSnapshot,
};
use super::view::render_task_panel;

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

    pub fn handle_all_and_snapshot(
        &self,
        commands: impl IntoIterator<Item = TaskPanelCommand>,
    ) -> TaskPanelBackendTransaction {
        let mut backend = match self.backend.lock() {
            Ok(backend) => backend,
            Err(_) => {
                return TaskPanelBackendTransaction {
                    errors: Vec::new(),
                    snapshot: Err(backend_unavailable("task panel backend lock poisoned")),
                }
            }
        };
        let errors = commands
            .into_iter()
            .filter_map(|command| backend.handle(command).err())
            .collect();
        let snapshot = backend.snapshot();
        TaskPanelBackendTransaction { errors, snapshot }
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
    mut legacy_messages: MessageReader<TaskAction>,
    backend: Option<Res<TaskPanelBackendResource>>,
) {
    let mut actions: Vec<TaskPanelCommand> = std::mem::take(&mut task_state.pending_commands);
    actions.extend(
        std::mem::take(&mut task_state.pending_actions)
            .into_iter()
            .map(TaskPanelCommand::from),
    );
    actions.extend(legacy_messages.read().cloned().map(TaskPanelCommand::from));
    if actions.is_empty() {
        return;
    }

    let Some(backend) = backend else {
        debug!(
            "No TaskPanelBackendResource found; {} task actions remain unprocessed",
            actions.len()
        );
        task_state.pending_commands = actions;
        return;
    };

    let commands: Vec<_> = actions
        .into_iter()
        .map(|command| {
            let routed = task_state.route_backend_command(command.clone());
            if let TaskPanelCommand::Delete { id } = command {
                task_state.delete_task(&id);
            }
            routed
        })
        .collect();
    let transaction = backend.handle_all_and_snapshot(commands);
    let errors = transaction.errors;
    for backend_error in &errors {
        error!(
            "Task panel backend command failed: {}",
            backend_error.message
        );
    }

    match transaction.snapshot {
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
