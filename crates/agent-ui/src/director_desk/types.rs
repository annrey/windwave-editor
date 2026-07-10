//! DirectorDesk types — UI data types for the director interface

use bevy::prelude::*;
use std::collections::VecDeque;

/// Resource holding the director desk state
#[derive(Resource, Default)]
pub struct DirectorDeskState {
    pub current_plan: Option<PlanDisplayInfo>,
    pub events: VecDeque<EventDisplayInfo>,
    pub agent_statuses: Vec<AgentStatusInfo>,
    pub pending_approvals: Vec<PendingApprovalInfo>,
    pub execution_trace: Vec<String>,
    pub max_events: usize,
    pub tasks: Vec<TaskDisplayInfo>,
    pub goals: Vec<GoalDisplayInfo>,
    pub rollback_entries: Vec<RollbackEntry>,
    /// Pending user actions (approve/reject) waiting to be processed
    pub pending_actions: Vec<UserAction>,

    // --- HybridEditorController 状态显示 ---
    /// 当前 LLM 模式状态
    pub hybrid_mode: HybridModeDisplay,
    /// 横幅消息（用于降级/恢复提示）
    pub banner_message: Option<BannerMessage>,
    /// 上次状态更新时间
    pub last_mode_update: Option<f64>,
    /// Command palette visibility
    pub command_palette_open: bool,
}

/// HybridEditorController 模式显示
#[derive(Debug, Clone, Default)]
pub struct HybridModeDisplay {
    /// 当前模式（LLM / RuleBased）
    pub mode: String,
    /// LLM 状态（Available/Connecting/Unavailable/Disabled）
    pub status: String,
    /// 成功率（0-100）
    pub success_rate: f64,
    /// 平均响应时间（毫秒）
    pub avg_response_ms: f64,
    /// 连续失败次数
    pub consecutive_failures: u32,
    /// 降级原因（如果有）
    pub fallback_reason: Option<String>,
    /// 下次检测倒计时（秒）
    pub next_check_countdown: f64,
}

/// 横幅消息类型
#[derive(Debug, Clone)]
pub struct BannerMessage {
    /// 消息类型
    pub banner_type: BannerType,
    /// 消息内容
    pub message: String,
    /// 显示时间戳
    pub timestamp: f64,
    /// 显示持续时间（秒）
    pub duration: f64,
}

/// 横幅类型
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BannerType {
    /// 降级提示（红色）
    Degraded,
    /// 恢复提示（绿色）
    Recovered,
    /// 警告（黄色）
    Warning,
    /// 信息（蓝色）
    Info,
}

/// Info for a pending approval entry shown in UI
#[derive(Debug, Clone)]
pub struct PendingApprovalInfo {
    pub plan_id: String,
    pub title: String,
    pub risk: String,
    pub reason: String,
    pub step_count: usize,
}

/// User action types for permission handling and editor operations
#[derive(Debug, Clone)]
pub enum UserAction {
    Approve {
        plan_id: String,
    },
    Reject {
        plan_id: String,
        reason: Option<String>,
    },
    Undo,
    Redo,
    /// Delete the currently selected entity (via Ctrl+D or Delete key)
    DeleteSelected,
    /// Focus camera on selected entity (via F key)
    FocusSelected,
    /// Toggle command palette visibility (via Ctrl+P)
    ToggleCommandPalette,
    /// Recheck LLM connection status (manual trigger)
    RecheckLlm,
}

#[derive(Debug, Clone)]
pub struct PlanDisplayInfo {
    pub id: String,
    pub title: String,
    pub summary: String,
    pub mode: String,
    pub risk: String,
    pub status: String,
    pub steps: Vec<StepDisplayInfo>,
}

#[derive(Debug, Clone)]
pub struct StepDisplayInfo {
    pub id: String,
    pub title: String,
    pub status: StepStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StepStatus {
    Pending,
    Running,
    Completed,
    Failed,
}

#[derive(Debug, Clone)]
pub struct EventDisplayInfo {
    pub timestamp: String,
    pub event_type: String,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct AgentStatusInfo {
    pub name: String,
    pub status: String,
    pub active: bool,
}

#[derive(Debug, Clone)]
pub struct TaskDisplayInfo {
    pub id: String,
    pub title: String,
    pub status: String,
    pub progress: f32,
}

#[derive(Debug, Clone)]
pub struct GoalDisplayInfo {
    pub task_id: String,
    pub description: String,
    pub matched: bool,
    pub detail: String,
}

/// An entry in the rollback / undo-redo log.
#[derive(Debug, Clone)]
pub struct RollbackEntry {
    pub transaction_id: String,
    pub operation_description: String,
    pub status: RollbackStatus,
    pub timestamp: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RollbackStatus {
    Committed,
    RolledBack,
    UndoAvailable,
    RedoAvailable,
}
