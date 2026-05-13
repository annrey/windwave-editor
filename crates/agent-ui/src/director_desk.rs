//! DirectorDesk - Multi-panel director interface for the Agent team system

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use crate::layout::LayoutManager;
use std::collections::VecDeque;

// ---------------------------------------------------------------------------
// UI data types
// ---------------------------------------------------------------------------

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

/// User action types for permission handling
#[derive(Debug, Clone)]
pub enum UserAction {
    Approve { plan_id: String },
    Reject { plan_id: String, reason: Option<String> },
    Undo,
    Redo,
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
        if !self.pending_approvals.iter().any(|p| p.plan_id == info.plan_id) {
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

    pub fn add_goal(
        &mut self,
        task_id: &str,
        description: &str,
        matched: bool,
        detail: &str,
    ) {
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

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

pub struct DirectorDeskPlugin;

impl Plugin for DirectorDeskPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DirectorDeskState>()
            .add_systems(Update, render_left_panel)
            .add_systems(Update, render_bottom_panel);
    }
}

// ---------------------------------------------------------------------------
// Render functions
// ---------------------------------------------------------------------------

fn render_left_panel(
    mut contexts: EguiContexts,
    mut state: ResMut<DirectorDeskState>,
    layout_mgr: Res<LayoutManager>,
) {
    if !layout_mgr.is_visible("director_desk") { return; }

    let ctx = contexts.ctx_mut();
    let Ok(ctx) = ctx else {
        return;
    };

    egui::SidePanel::left("director_left_panel")
        .default_width(280.0)
        .resizable(true)
        .show(ctx, |ui| {
            ui.heading(
                egui::RichText::new("Director Desk")
                    .strong()
                    .size(16.0),
            );
            ui.separator();

            // --- Current Plan ---
            ui.collapsing(
                egui::RichText::new("Current Plan").strong(),
                |ui| {
                    if let Some(ref plan) = state.current_plan {
                        ui.label(format!("Title: {}", plan.title));
                        ui.label(format!(
                            "Mode: {}  |  Risk: {}  |  Status: {}",
                            plan.mode, plan.risk, plan.status
                        ));
                        ui.label(format!("Summary: {}", plan.summary));
                        ui.separator();
                        ui.label(egui::RichText::new("Steps:").strong());
                        for step in &plan.steps {
                            let status_icon = match step.status {
                                StepStatus::Pending => "\u{25CB}",
                                StepStatus::Running => "\u{25D0}",
                                StepStatus::Completed => "\u{25CF}",
                                StepStatus::Failed => "\u{2717}",
                            };
                            ui.label(format!(
                                "  {} {} - {}",
                                status_icon, step.id, step.title
                            ));
                        }
                    } else {
                        ui.label("No active plan.");
                    }
                },
            );

            ui.separator();

            // --- Agent Statuses ---
            ui.collapsing(
                egui::RichText::new("Agent Status").strong(),
                |ui| {
                    for agent in &state.agent_statuses {
                        let color = if agent.active {
                            egui::Color32::from_rgb(16, 185, 129)
                        } else {
                            egui::Color32::from_gray(160)
                        };
                        ui.label(
                            egui::RichText::new(format!(
                                "\u{25CF} {} - {}",
                                agent.name, agent.status
                            ))
                            .color(color)
                            .size(12.0),
                        );
                    }
                },
            );

            ui.separator();

            // --- Pending Approvals with Interactive Buttons ---
            ui.add_space(4.0);
            if state.pending_approvals.is_empty() {
                ui.label(
                    egui::RichText::new("Pending Approvals")
                        .strong()
                        .size(13.0),
                );
                ui.label(
                    egui::RichText::new("  No pending approvals")
                        .size(12.0)
                        .color(egui::Color32::from_gray(140)),
                );
            } else {
                ui.label(
                    egui::RichText::new(format!("\u{26A0} Pending Approvals ({})", state.pending_approvals.len()))
                        .strong()
                        .color(egui::Color32::from_rgb(245, 158, 11))
                        .size(13.0),
                );

                // Collect button clicks outside the borrow scope
                let mut clicked_approve: Option<String> = None;
                let mut clicked_reject: Option<String> = None;

                for info in &state.pending_approvals {
                    ui.add_space(4.0);
                    ui.group(|ui| {
                        // Risk color
                        let risk_color = match info.risk.to_lowercase().as_str() {
                            "highrisk" | "destructive" => egui::Color32::from_rgb(239, 68, 68),
                            "mediumrisk" => egui::Color32::from_rgb(245, 158, 11),
                            _ => egui::Color32::from_rgb(59, 130, 246),
                        };
                        ui.label(
                            egui::RichText::new(format!("\u{1F4CB} {}", info.title))
                                .strong()
                                .size(12.0),
                        );
                        ui.label(
                            egui::RichText::new(format!("  Risk: {}  |  Steps: {}", info.risk, info.step_count))
                                .color(risk_color)
                                .size(11.0),
                        );
                        ui.label(
                            egui::RichText::new(format!("  {}", info.reason))
                                .size(10.0)
                                .color(egui::Color32::from_gray(150)),
                        );
                        ui.add_space(2.0);
                        ui.horizontal(|ui| {
                            let approve_btn = ui.add_sized(
                                [60.0, 22.0],
                                egui::Button::new(egui::RichText::new("\u{2713} Approve").size(11.0))
                                    .fill(egui::Color32::from_rgb(16, 185, 129))
                                    .corner_radius(3),
                            );
                            let reject_btn = ui.add_sized(
                                [60.0, 22.0],
                                egui::Button::new(egui::RichText::new("\u{2717} Reject").size(11.0))
                                    .fill(egui::Color32::from_rgb(239, 68, 68))
                                    .corner_radius(3),
                            );

                            if approve_btn.clicked() {
                                clicked_approve = Some(info.plan_id.clone());
                            }
                            if reject_btn.clicked() {
                                clicked_reject = Some(info.plan_id.clone());
                            }
                        });
                    });
                }

                if let Some(plan_id) = clicked_approve {
                    state.pending_actions.push(UserAction::Approve { plan_id });
                }
                if let Some(plan_id) = clicked_reject {
                    state.pending_actions.push(UserAction::Reject {
                        plan_id,
                        reason: Some("User rejected".to_string()),
                    });
                }
            }

            ui.separator();

            // --- Tasks ---
            ui.collapsing(
                egui::RichText::new(format!("\u{1F4CB} Tasks ({})", state.tasks.len())).strong(),
                |ui| {
                    if state.tasks.is_empty() {
                        ui.label("No tasks.");
                    } else {
                        for task in &state.tasks {
                            let status_color = match task.status.as_str() {
                                "Completed" => egui::Color32::from_rgb(16, 185, 129),
                                "Running" => egui::Color32::from_rgb(59, 130, 246),
                                "Failed" => egui::Color32::from_rgb(239, 68, 68),
                                _ => egui::Color32::from_gray(160),
                            };
                            ui.label(
                                egui::RichText::new(format!(
                                    "\u{25CF} {} [{}]",
                                    task.title, task.status
                                ))
                                .color(status_color)
                                .size(12.0),
                            );
                            if task.progress > 0.0 && task.progress < 1.0 {
                                ui.add(
                                    egui::ProgressBar::new(task.progress)
                                        .desired_width(ui.available_width()),
                                );
                            }
                        }
                    }
                },
            );

            ui.separator();

            // --- Goals ---
            ui.collapsing(
                egui::RichText::new(format!("\u{1F3AF} Goals ({})", state.goals.len())).strong(),
                |ui| {
                    if state.goals.is_empty() {
                        ui.label("No goal checks.");
                    } else {
                        for goal in &state.goals {
                            let icon = if goal.matched { "\u{2705}" } else { "\u{274C}" };
                            ui.label(
                                egui::RichText::new(format!("{} {}", icon, goal.description))
                                    .size(12.0),
                            );
                            if !goal.detail.is_empty() {
                                ui.label(
                                    egui::RichText::new(format!("    {}", goal.detail))
                                        .size(11.0)
                                        .color(egui::Color32::from_gray(150)),
                                );
                            }
                        }
                    }
                },
            );

            ui.separator();

            // --- Rollback & Undo Log ---
            ui.collapsing(
                egui::RichText::new(format!("\u{21A9} Undo/Redo Log ({})", state.rollback_entries.len())).strong(),
                |ui| {
                    // Undo/Redo buttons
                    ui.horizontal(|ui| {
                        let undo_btn = ui.add_sized(
                            [60.0, 24.0],
                            egui::Button::new(egui::RichText::new("\u{2B05} Undo").size(11.0))
                                .fill(egui::Color32::from_rgb(59, 130, 246))
                                .corner_radius(4),
                        );
                        let redo_btn = ui.add_sized(
                            [60.0, 24.0],
                            egui::Button::new(egui::RichText::new("\u{27A1} Redo").size(11.0))
                                .fill(egui::Color32::from_rgb(139, 92, 246))
                                .corner_radius(4),
                        );

                        if undo_btn.clicked() {
                            state.pending_actions.push(UserAction::Undo);
                        }
                        if redo_btn.clicked() {
                            state.pending_actions.push(UserAction::Redo);
                        }
                    });
                    ui.add_space(8.0);

                    if state.rollback_entries.is_empty() {
                        ui.label("No rollback entries.");
                    } else {
                        egui::ScrollArea::vertical()
                            .max_height(160.0)
                            .auto_shrink([false; 2])
                            .show(ui, |ui| {
                                for entry in state.rollback_entries.iter().rev() {
                                    let (icon, color) = match entry.status {
                                        RollbackStatus::Committed => ("\u{2705}", egui::Color32::from_rgb(16, 185, 129)),
                                        RollbackStatus::RolledBack => ("\u{21A9}", egui::Color32::from_rgb(245, 158, 11)),
                                        RollbackStatus::UndoAvailable => ("\u{2B05}", egui::Color32::from_rgb(59, 130, 246)),
                                        RollbackStatus::RedoAvailable => ("\u{27A1}", egui::Color32::from_rgb(139, 92, 246)),
                                    };
                                    ui.label(
                                        egui::RichText::new(format!(
                                            "{} {}: {}",
                                            icon, entry.transaction_id, entry.operation_description
                                        ))
                                        .color(color)
                                        .size(11.0),
                                    );
                                }
                            });
                    }
                },
            );
        });
}

fn render_bottom_panel(mut contexts: EguiContexts, state: Res<DirectorDeskState>, layout_mgr: Res<LayoutManager>) {
    if !layout_mgr.is_visible("director_events") { return; }

    let ctx = contexts.ctx_mut();
    let Ok(ctx) = ctx else {
        return;
    };

    egui::TopBottomPanel::bottom("director_bottom_panel")
        .default_height(180.0)
        .resizable(true)
        .show(ctx, |ui| {
            ui.heading(
                egui::RichText::new("Events & Trace")
                    .strong()
                    .size(14.0),
            );
            ui.separator();

            egui::ScrollArea::vertical()
                .auto_shrink([false; 2])
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    for event in &state.events {
                        ui.label(
                            egui::RichText::new(format!(
                                "[{}] {}: {}",
                                event.timestamp, event.event_type, event.message
                            ))
                            .size(11.0)
                            .color(egui::Color32::from_gray(180)),
                        );
                    }
                    // Also show trace entries
                    for trace in &state.execution_trace {
                        ui.label(
                            egui::RichText::new(trace)
                                .size(10.0)
                                .color(egui::Color32::from_gray(130)),
                        );
                    }
                });
        });
}
