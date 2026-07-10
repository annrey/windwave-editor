//! DirectorDesk state — DirectorDeskState implementation

use super::types::*;
use std::collections::VecDeque;

impl DirectorDeskState {
    pub fn new() -> Self {
        Self {
            current_plan: None,
            events: VecDeque::new(),
            agent_statuses: vec![
                AgentStatusInfo {
                    name: "Director".into(),
                    status: "Online".into(),
                    active: true,
                },
                AgentStatusInfo {
                    name: "SceneAgent".into(),
                    status: "Idle".into(),
                    active: false,
                },
                AgentStatusInfo {
                    name: "CodeAgent".into(),
                    status: "Idle".into(),
                    active: false,
                },
                AgentStatusInfo {
                    name: "AssetAgent".into(),
                    status: "Idle".into(),
                    active: false,
                },
                AgentStatusInfo {
                    name: "RuleAgent".into(),
                    status: "Idle".into(),
                    active: false,
                },
            ],
            pending_approvals: Vec::new(),
            execution_trace: Vec::new(),
            max_events: 200,
            tasks: Vec::new(),
            goals: Vec::new(),
            rollback_entries: Vec::new(),
            pending_actions: Vec::new(),
            hybrid_mode: HybridModeDisplay::default(),
            banner_message: None,
            last_mode_update: None,
            command_palette_open: false,
        }
    }

    pub fn add_event(&mut self, event_type: &str, message: &str) {
        while self.events.len() >= self.max_events {
            self.events.pop_front();
        }
        self.events.push_back(EventDisplayInfo {
            timestamp: format!("{:?}", std::time::Instant::now()),
            event_type: event_type.to_string(),
            message: message.to_string(),
        });
    }

    pub fn add_trace(&mut self, entry: &str) {
        if self.execution_trace.len() > 500 {
            self.execution_trace.remove(0);
        }
        self.execution_trace.push(entry.to_string());
    }

    pub fn set_plan(&mut self, plan: PlanDisplayInfo) {
        self.current_plan = Some(plan);
    }

    /// Sync pending approvals from director (called each frame by handle_agent_input)
    pub fn sync_pending_approval(&mut self, info: PendingApprovalInfo) {
        if !self
            .pending_approvals
            .iter()
            .any(|p| p.plan_id == info.plan_id)
        {
            self.pending_approvals.push(info);
        }
    }

    pub fn clear_pending_approval(&mut self, plan_id: &str) {
        self.pending_approvals.retain(|p| p.plan_id != plan_id);
    }

    pub fn clear_all_pending(&mut self) {
        self.pending_approvals.clear();
    }

    pub fn has_pending_approvals(&self) -> bool {
        !self.pending_approvals.is_empty()
    }

    pub fn add_task(&mut self, id: &str, title: &str, status: &str, progress: f32) {
        if let Some(existing) = self.tasks.iter_mut().find(|t| t.id == id) {
            existing.status = status.to_string();
            existing.progress = progress;
        } else {
            self.tasks.push(TaskDisplayInfo {
                id: id.to_string(),
                title: title.to_string(),
                status: status.to_string(),
                progress,
            });
        }
    }

    pub fn add_goal(&mut self, task_id: &str, description: &str, matched: bool, detail: &str) {
        self.goals.push(GoalDisplayInfo {
            task_id: task_id.to_string(),
            description: description.to_string(),
            matched,
            detail: detail.to_string(),
        });
    }

    /// Append a rollback / undo-redo entry.
    pub fn add_rollback_entry(
        &mut self,
        transaction_id: &str,
        operation_description: &str,
        status: RollbackStatus,
    ) {
        if self.rollback_entries.len() > 500 {
            self.rollback_entries.remove(0);
        }
        self.rollback_entries.push(RollbackEntry {
            transaction_id: transaction_id.to_string(),
            operation_description: operation_description.to_string(),
            status,
            timestamp: format!("{:?}", std::time::Instant::now()),
        });
    }
}
