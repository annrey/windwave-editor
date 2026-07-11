//! Backend-independent task panel view models.

use bevy::prelude::Resource;
use log::{info, warn};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::port::{TaskAction, TaskPanelCommand, TaskPanelSnapshot};

/// 任务状态枚举
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TaskStatus {
    /// 待处理
    Pending,
    /// 进行中
    InProgress,
    /// 已完成
    Done,
    /// 失败
    Failed,
    /// 已取消
    Cancelled,
}

impl TaskStatus {
    /// 获取状态显示颜色
    pub fn color(&self) -> [f32; 3] {
        match self {
            TaskStatus::Pending => [0.6, 0.6, 0.6],
            TaskStatus::InProgress => [1.0, 0.8, 0.0],
            TaskStatus::Done => [0.0, 0.8, 0.2],
            TaskStatus::Failed => [0.9, 0.2, 0.2],
            TaskStatus::Cancelled => [0.5, 0.5, 0.5],
        }
    }

    /// 获取状态显示文本
    pub fn display(&self) -> &str {
        match self {
            TaskStatus::Pending => "待处理",
            TaskStatus::InProgress => "进行中",
            TaskStatus::Done => "已完成",
            TaskStatus::Failed => "失败",
            TaskStatus::Cancelled => "已取消",
        }
    }
}

/// 任务数据结构
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskInfo {
    /// 任务唯一 ID
    pub id: String,
    /// 任务标题
    pub title: String,
    /// 任务描述
    pub description: String,
    /// 任务状态
    pub status: TaskStatus,
    /// 关联场景 ID
    pub scene_id: Option<String>,
    /// 创建时间
    pub created_at: String,
    /// 优先级 (1-5)
    pub priority: u8,
    /// 关联实体 IDs
    pub entity_ids: Vec<String>,
    /// Multica 任务 ID (如果存在)
    pub multica_id: Option<String>,
}

impl TaskInfo {
    /// 创建新任务
    pub fn new(id: String, title: String, description: String) -> Self {
        Self {
            id,
            title,
            description,
            status: TaskStatus::Pending,
            scene_id: None,
            created_at: chrono::Utc::now().to_rfc3339(),
            priority: 3,
            entity_ids: Vec::new(),
            multica_id: None,
        }
    }

    /// 设置关联场景
    pub fn with_scene(mut self, scene_id: String) -> Self {
        self.scene_id = Some(scene_id);
        self
    }

    /// 设置优先级
    pub fn with_priority(mut self, priority: u8) -> Self {
        self.priority = priority.clamp(1, 5);
        self
    }

    /// 设置 Multica 任务 ID
    pub fn with_multica_id(mut self, multica_id: String) -> Self {
        self.multica_id = Some(multica_id);
        self
    }

    /// 添加关联实体
    pub fn add_entity(mut self, entity_id: String) -> Self {
        if !self.entity_ids.contains(&entity_id) {
            self.entity_ids.push(entity_id);
        }
        self
    }
}

/// 任务筛选器
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum TaskFilter {
    /// 显示所有任务
    #[default]
    All,
    /// 按场景筛选
    ByScene(String),
    /// 按状态筛选
    ByStatus(TaskStatus),
    /// 按优先级筛选
    ByPriority(u8),
}

/// 任务排序方式
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SortBy {
    /// 按优先级排序 (高优先先)
    Priority,
    /// 按创建时间排序 (最新先)
    CreatedAt,
    /// 按状态分组排序
    Status,
    /// 按标题字母顺序
    Title,
}

/// 任务面板状态资源
#[derive(Resource)]
pub struct TaskPanelState {
    /// 任务列表
    pub tasks: HashMap<String, TaskInfo>,
    /// 当前选中的任务 ID
    pub selected_task: Option<String>,
    /// 当前筛选条件
    pub filter: TaskFilter,
    /// 删除确认对话框（Some 表示正在确认删除的任务ID）
    pub show_delete_confirm: Option<String>,
    /// 搜索关键词
    pub search_query: String,
    /// 任务总数统计
    pub total_count: usize,
    /// 按状态统计的任务数
    pub status_counts: HashMap<TaskStatus, usize>,
    /// 是否显示创建任务对话框
    pub show_create_dialog: bool,
    /// 新任务标题
    pub new_task_title: String,
    /// 新任务描述
    pub new_task_description: String,
    /// 新任务优先级
    pub new_task_priority: u8,
    /// 新任务场景 ID
    pub new_task_scene_id: String,
    /// 任务操作事件队列
    pub pending_actions: Vec<TaskAction>,
    /// Backend-neutral commands emitted by the current panel view.
    pub pending_commands: Vec<TaskPanelCommand>,
    deleted_backend_ids: HashMap<String, String>,
    /// 同步状态
    pub sync_status: SyncStatus,
    /// 排序方式
    pub sort_by: SortBy,
    /// 批量选中的任务 IDs
    pub selected_ids: Vec<String>,
    /// 是否开启批量选择模式
    pub select_mode: bool,
}

/// 同步状态
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum SyncStatus {
    #[default]
    /// 未同步
    NotSynced,
    /// 同步中
    Syncing,
    /// 已同步
    Synced,
    /// 同步失败
    SyncError(String),
}

/// Panel identities assigned to tasks created during a backend batch.
#[derive(Debug, Clone, Default)]
pub struct CreatedTaskAliases {
    panel_id_by_backend_id: HashMap<String, String>,
}

impl CreatedTaskAliases {
    pub fn record(&mut self, backend_id: String, panel_id: String) {
        self.panel_id_by_backend_id.insert(backend_id, panel_id);
    }

    pub fn apply(&self, mut snapshot: TaskPanelSnapshot) -> TaskPanelSnapshot {
        for task in &mut snapshot.tasks {
            if let Some(panel_id) = task
                .multica_id
                .as_ref()
                .and_then(|backend_id| self.panel_id_by_backend_id.get(backend_id))
            {
                task.id = panel_id.clone();
            }
        }
        snapshot
    }
}

impl Default for TaskPanelState {
    fn default() -> Self {
        Self {
            tasks: HashMap::new(),
            selected_task: None,
            filter: TaskFilter::All,
            show_delete_confirm: None,
            search_query: String::new(),
            total_count: 0,
            status_counts: HashMap::new(),
            show_create_dialog: false,
            new_task_title: String::new(),
            new_task_description: String::new(),
            new_task_priority: 3,
            new_task_scene_id: String::new(),
            pending_actions: Vec::new(),
            pending_commands: Vec::new(),
            deleted_backend_ids: HashMap::new(),
            sync_status: SyncStatus::NotSynced,
            sort_by: SortBy::Priority,
            selected_ids: Vec::new(),
            select_mode: false,
        }
    }
}

impl TaskPanelState {
    fn update_statistics(&mut self) {
        self.total_count = self.tasks.len();
        self.status_counts.clear();
        for task in self.tasks.values() {
            *self.status_counts.entry(task.status.clone()).or_insert(0) += 1;
        }
    }

    pub fn apply_snapshot(&mut self, snapshot: TaskPanelSnapshot) {
        self.tasks = snapshot
            .tasks
            .into_iter()
            .map(|task| (task.id.clone(), task))
            .collect();
        self.sync_status = snapshot.sync_status;
        self.update_statistics();
        if self
            .selected_task
            .as_ref()
            .is_some_and(|id| !self.tasks.contains_key(id))
        {
            self.selected_task = None;
        }
        self.selected_ids.retain(|id| self.tasks.contains_key(id));
        if self
            .show_delete_confirm
            .as_ref()
            .is_some_and(|id| !self.tasks.contains_key(id))
        {
            self.show_delete_confirm = None;
        }
    }

    /// Merge a backend snapshot without replacing panel-local task identity.
    pub fn apply_backend_snapshot(&mut self, snapshot: TaskPanelSnapshot) {
        let mut merged = self.tasks.clone();
        for mut remote in snapshot.tasks {
            if self
                .deleted_backend_ids
                .iter()
                .any(|(panel_id, backend_id)| {
                    remote.id == *panel_id || remote.multica_id.as_deref() == Some(backend_id)
                })
            {
                continue;
            }
            let existing_id = remote.multica_id.as_ref().and_then(|remote_id| {
                merged
                    .iter()
                    .find(|(_, task)| task.multica_id.as_ref() == Some(remote_id))
                    .map(|(id, _)| id.clone())
            });
            if let Some(existing_id) = existing_id {
                remote.id = existing_id.clone();
                merged.insert(existing_id, remote);
            } else {
                merged.insert(remote.id.clone(), remote);
            }
        }
        self.apply_snapshot(TaskPanelSnapshot {
            tasks: merged.into_values().collect(),
            sync_status: snapshot.sync_status,
        });
    }

    /// Translate panel-local identity to the backend identity carried by the task.
    pub fn route_backend_command(&self, command: TaskPanelCommand) -> TaskPanelCommand {
        let backend_id = |id: String| {
            self.tasks
                .get(&id)
                .and_then(|task| task.multica_id.clone())
                .or_else(|| self.deleted_backend_ids.get(&id).cloned())
                .unwrap_or(id)
        };
        match command {
            TaskPanelCommand::UpdateStatus { id, status } => TaskPanelCommand::UpdateStatus {
                id: backend_id(id),
                status,
            },
            TaskPanelCommand::Delete { id } => TaskPanelCommand::Delete { id: backend_id(id) },
            other => other,
        }
    }

    /// 添加任务
    pub fn add_task(&mut self, task: TaskInfo) {
        let status = task.status.clone();
        let id = task.id.clone();
        let title = task.title.clone();
        self.status_counts
            .entry(status)
            .and_modify(|c| *c += 1)
            .or_insert(1);
        self.tasks.insert(id.clone(), task);
        self.total_count = self.tasks.len();
        info!("Task added: {} ({})", title, id);
    }

    /// 更新任务状态
    pub fn update_task_status(&mut self, task_id: &str, new_status: TaskStatus) -> bool {
        if let Some(task) = self.tasks.get_mut(task_id) {
            let old_status = task.status.clone();
            task.status = new_status.clone();

            self.status_counts
                .entry(old_status)
                .and_modify(|c| *c = c.saturating_sub(1));

            self.status_counts
                .entry(new_status.clone())
                .and_modify(|c| *c += 1)
                .or_insert(1);

            info!("Task {} status changed to {:?}", task_id, new_status);
            true
        } else {
            warn!("Task {} not found for status update", task_id);
            false
        }
    }

    /// 删除任务
    pub fn delete_task(&mut self, task_id: &str) -> bool {
        if let Some(task) = self.tasks.remove(task_id) {
            if let Some(backend_id) = &task.multica_id {
                self.deleted_backend_ids
                    .insert(task_id.to_string(), backend_id.clone());
            }
            self.status_counts
                .entry(task.status)
                .and_modify(|c| *c = c.saturating_sub(1));
            self.total_count = self.tasks.len();
            if self.selected_task.as_deref() == Some(task_id) {
                self.selected_task = None;
            }
            info!("Task deleted: {}", task_id);
            true
        } else {
            false
        }
    }

    /// 创建任务并加入操作队列
    pub fn create_task(
        &mut self,
        title: String,
        description: String,
        scene_id: Option<String>,
        priority: u8,
    ) {
        let id = format!("task_{}", chrono::Utc::now().timestamp_millis());
        let mut task = TaskInfo::new(id, title.clone(), description);
        task.scene_id = scene_id;
        task.priority = priority.clamp(1, 5);

        let task_for_queue = task.clone();
        self.pending_commands
            .push(TaskPanelCommand::Create(task_for_queue));
        self.add_task(task);
        info!("Task creation queued: {}", title);
    }

    /// 获取筛选后的任务列表
    pub fn get_filtered_tasks(&self) -> Vec<&TaskInfo> {
        let mut tasks: Vec<&TaskInfo> = self
            .tasks
            .values()
            .filter(|task| {
                let matches_filter = match &self.filter {
                    TaskFilter::All => true,
                    TaskFilter::ByScene(scene_id) => task.scene_id.as_deref() == Some(scene_id),
                    TaskFilter::ByStatus(status) => task.status == *status,
                    TaskFilter::ByPriority(min_priority) => task.priority >= *min_priority,
                };

                let matches_search = self.search_query.is_empty()
                    || task
                        .title
                        .to_lowercase()
                        .contains(&self.search_query.to_lowercase())
                    || task
                        .description
                        .to_lowercase()
                        .contains(&self.search_query.to_lowercase());

                matches_filter && matches_search
            })
            .collect();

        // Sort by the selected sort criteria
        tasks.sort_by(|a, b| {
            match &self.sort_by {
                SortBy::Priority => {
                    // Descending: higher priority first
                    b.priority.cmp(&a.priority)
                }
                SortBy::CreatedAt => {
                    // Descending: newest first (rfc3339 strings are lexicographically ordered)
                    b.created_at.cmp(&a.created_at)
                }
                SortBy::Status => {
                    // Group by status order: Pending > InProgress > Done > Failed > Cancelled
                    fn status_order(s: &TaskStatus) -> u8 {
                        match s {
                            TaskStatus::Pending => 0,
                            TaskStatus::InProgress => 1,
                            TaskStatus::Done => 2,
                            TaskStatus::Failed => 3,
                            TaskStatus::Cancelled => 4,
                        }
                    }
                    status_order(&a.status).cmp(&status_order(&b.status))
                }
                SortBy::Title => {
                    // Alphabetical ascending
                    a.title.to_lowercase().cmp(&b.title.to_lowercase())
                }
            }
        });

        tasks
    }

    /// 设置筛选条件
    pub fn set_filter(&mut self, filter: TaskFilter) {
        self.filter = filter;
    }

    /// 打开创建任务对话框
    pub fn open_create_dialog(&mut self) {
        self.show_create_dialog = true;
        self.new_task_title.clear();
        self.new_task_description.clear();
        self.new_task_priority = 3;
        self.new_task_scene_id.clear();
    }

    /// 关闭创建任务对话框
    pub fn close_create_dialog(&mut self) {
        self.show_create_dialog = false;
    }

    /// 提交新任务创建
    pub fn submit_new_task(&mut self) {
        if !self.new_task_title.is_empty() {
            let scene_id = if self.new_task_scene_id.is_empty() {
                None
            } else {
                Some(self.new_task_scene_id.clone())
            };
            self.create_task(
                self.new_task_title.clone(),
                self.new_task_description.clone(),
                scene_id,
                self.new_task_priority,
            );
            self.close_create_dialog();
        }
    }

    /// Update status counts when a task transitions between statuses.
    /// Used by the bridge sync system to avoid duplicating the count logic.
    pub fn update_task_status_counts(&mut self, old_status: &TaskStatus, new_status: &TaskStatus) {
        self.status_counts
            .entry(old_status.clone())
            .and_modify(|c| *c = c.saturating_sub(1));

        self.status_counts
            .entry(new_status.clone())
            .and_modify(|c| *c += 1)
            .or_insert(1);
    }
}
