//! DirectorDesk panels — Left and bottom panel rendering

use super::hybrid_ui::{render_banner_message, render_hybrid_status_bar};
use super::types::*;
use crate::layout::LayoutManager;
use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};

pub(super) fn render_left_panel(
    mut contexts: EguiContexts,
    mut state: ResMut<DirectorDeskState>,
    layout_mgr: Res<LayoutManager>,
) {
    if !layout_mgr.is_visible("director_desk") {
        return;
    }

    let ctx = contexts.ctx_mut();
    let Ok(ctx) = ctx else {
        return;
    };

    egui::SidePanel::left("director_left_panel")
        .default_width(280.0)
        .resizable(true)
        .show(ctx, |ui| {
            ui.heading(egui::RichText::new("Director Desk").strong().size(16.0));
            ui.separator();

            // --- HybridController 状态栏 ---
            render_hybrid_status_bar(ui, &mut state);

            // --- 横幅消息（降级/恢复提示）---
            render_banner_message(ui, &mut state);

            // --- Current Plan ---
            ui.collapsing(egui::RichText::new("Current Plan").strong(), |ui| {
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
                        ui.label(format!("  {} {} - {}", status_icon, step.id, step.title));
                    }
                } else {
                    ui.label("No active plan.");
                }
            });

            ui.separator();

            // --- Agent Statuses ---
            ui.collapsing(egui::RichText::new("Agent Status").strong(), |ui| {
                for agent in &state.agent_statuses {
                    let color = if agent.active {
                        egui::Color32::from_rgb(16, 185, 129)
                    } else {
                        egui::Color32::from_gray(160)
                    };
                    ui.label(
                        egui::RichText::new(format!("\u{25CF} {} - {}", agent.name, agent.status))
                            .color(color)
                            .size(12.0),
                    );
                }
            });

            ui.separator();

            // --- Pending Approvals with Interactive Buttons ---
            ui.add_space(4.0);
            if state.pending_approvals.is_empty() {
                ui.label(egui::RichText::new("Pending Approvals").strong().size(13.0));
                ui.label(
                    egui::RichText::new("  No pending approvals")
                        .size(12.0)
                        .color(egui::Color32::from_gray(140)),
                );
            } else {
                ui.label(
                    egui::RichText::new(format!(
                        "\u{26A0} Pending Approvals ({})",
                        state.pending_approvals.len()
                    ))
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
                            egui::RichText::new(format!(
                                "  Risk: {}  |  Steps: {}",
                                info.risk, info.step_count
                            ))
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
                                egui::Button::new(
                                    egui::RichText::new("\u{2713} Approve").size(11.0),
                                )
                                .fill(egui::Color32::from_rgb(16, 185, 129))
                                .corner_radius(3),
                            );
                            let reject_btn = ui.add_sized(
                                [60.0, 22.0],
                                egui::Button::new(
                                    egui::RichText::new("\u{2717} Reject").size(11.0),
                                )
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
                egui::RichText::new(format!(
                    "\u{21A9} Undo/Redo Log ({})",
                    state.rollback_entries.len()
                ))
                .strong(),
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
                                        RollbackStatus::Committed => {
                                            ("\u{2705}", egui::Color32::from_rgb(16, 185, 129))
                                        }
                                        RollbackStatus::RolledBack => {
                                            ("\u{21A9}", egui::Color32::from_rgb(245, 158, 11))
                                        }
                                        RollbackStatus::UndoAvailable => {
                                            ("\u{2B05}", egui::Color32::from_rgb(59, 130, 246))
                                        }
                                        RollbackStatus::RedoAvailable => {
                                            ("\u{27A1}", egui::Color32::from_rgb(139, 92, 246))
                                        }
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

pub(super) fn render_bottom_panel(
    mut contexts: EguiContexts,
    state: Res<DirectorDeskState>,
    layout_mgr: Res<LayoutManager>,
) {
    if !layout_mgr.is_visible("director_events") {
        return;
    }

    let ctx = contexts.ctx_mut();
    let Ok(ctx) = ctx else {
        return;
    };

    egui::TopBottomPanel::bottom("director_bottom_panel")
        .default_height(180.0)
        .resizable(true)
        .show(ctx, |ui| {
            ui.heading(egui::RichText::new("Events & Trace").strong().size(14.0));
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
