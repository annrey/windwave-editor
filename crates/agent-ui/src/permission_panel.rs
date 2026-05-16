//! Permission Configuration Panel - Phase 3.6
//!
//! Provides UI for configuring Agent permission policies and risk level settings.

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use agent_core::permission::{OperationRisk, PermissionPolicy};
use crate::layout::{LayoutManager, LayoutCommand, PanelPosition};
use crate::LayoutCommandQueue;

pub struct PermissionPanelPlugin;

impl Plugin for PermissionPanelPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PermissionState>()
            .add_systems(Update, render_permission_panel);
    }
}

/// Permission configuration state
#[derive(Resource)]
pub struct PermissionState {
    pub visible: bool,
    pub policy: PermissionPolicy,
}

impl Default for PermissionState {
    fn default() -> Self {
        Self {
            visible: false,
            policy: PermissionPolicy::default(),
        }
    }
}

impl PermissionState {
    /// Get the action type for a given risk level
    pub fn get_action_for_risk(&self, risk: OperationRisk) -> RiskAction {
        if self.policy.auto_allow.contains(&risk) {
            RiskAction::AutoAllow
        } else if self.policy.require_confirmation.contains(&risk) {
            RiskAction::NeedConfirmation
        } else if self.policy.forbidden.contains(&risk) {
            RiskAction::Forbidden
        } else {
            RiskAction::NeedConfirmation
        }
    }

    /// Set the action type for a given risk level
    pub fn set_action_for_risk(&mut self, risk: OperationRisk, action: RiskAction) {
        self.policy.auto_allow.retain(|&r| r != risk);
        self.policy.require_confirmation.retain(|&r| r != risk);
        self.policy.forbidden.retain(|&r| r != risk);

        match action {
            RiskAction::AutoAllow => self.policy.auto_allow.push(risk),
            RiskAction::NeedConfirmation => self.policy.require_confirmation.push(risk),
            RiskAction::Forbidden => self.policy.forbidden.push(risk),
        }
    }

    /// Reset policy to defaults
    pub fn reset_to_defaults(&mut self) {
        self.policy = PermissionPolicy::default();
    }

    /// Get a summary of the current policy
    pub fn policy_summary(&self) -> String {
        let mut summary = String::new();
        
        summary.push_str("Current Policy:\n");
        summary.push_str(&format!("- Auto-allow: {}\n", self.format_risks(&self.policy.auto_allow)));
        summary.push_str(&format!("- Require confirmation: {}\n", self.format_risks(&self.policy.require_confirmation)));
        summary.push_str(&format!("- Forbidden: {}", self.format_risks(&self.policy.forbidden)));
        
        summary
    }

    fn format_risks(&self, risks: &[OperationRisk]) -> String {
        if risks.is_empty() {
            "None".to_string()
        } else {
            risks.iter()
                .map(|&r| risk_to_string(r))
                .collect::<Vec<_>>()
                .join(", ")
        }
    }
}

/// Action types for risk levels
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RiskAction {
    AutoAllow,
    NeedConfirmation,
    Forbidden,
}

impl RiskAction {
    pub fn to_string(&self) -> &str {
        match self {
            RiskAction::AutoAllow => "Auto-Allow",
            RiskAction::NeedConfirmation => "Need Confirmation",
            RiskAction::Forbidden => "Forbidden",
        }
    }
}

fn risk_to_string(risk: OperationRisk) -> &'static str {
    match risk {
        OperationRisk::Safe => "Safe",
        OperationRisk::LowRisk => "Low Risk",
        OperationRisk::MediumRisk => "Medium Risk",
        OperationRisk::HighRisk => "High Risk",
        OperationRisk::Destructive => "Destructive",
    }
}

fn risk_description(risk: OperationRisk) -> &'static str {
    match risk {
        OperationRisk::Safe => "Read-only or purely informational",
        OperationRisk::LowRisk => "Minor mutation with well-understood scope",
        OperationRisk::MediumRisk => "Mutation that might affect gameplay",
        OperationRisk::HighRisk => "Significant mutation with wide impact",
        OperationRisk::Destructive => "Irreversible or dangerous operation",
    }
}

fn risk_color(risk: OperationRisk) -> egui::Color32 {
    match risk {
        OperationRisk::Safe => egui::Color32::from_rgb(34, 197, 94),
        OperationRisk::LowRisk => egui::Color32::from_rgb(13, 162, 58),
        OperationRisk::MediumRisk => egui::Color32::from_rgb(245, 158, 11),
        OperationRisk::HighRisk => egui::Color32::from_rgb(249, 115, 22),
        OperationRisk::Destructive => egui::Color32::from_rgb(239, 68, 68),
    }
}

fn render_permission_panel(
    mut contexts: EguiContexts,
    mut state: ResMut<PermissionState>,
    layout_mgr: Res<LayoutManager>,
    mut layout_queue: ResMut<LayoutCommandQueue>,
) {
    let ctx = match contexts.ctx_mut() {
        Ok(ctx) => ctx,
        Err(_) => return,
    };

    if !layout_mgr.is_visible("permission") {
        return;
    }

    let (win_w, win_h) = layout_mgr
        .panel_config("permission")
        .and_then(|c| match c.position {
            PanelPosition::Floating { width, height, .. } => Some((width, height)),
            _ => None,
        })
        .unwrap_or((450.0, 550.0));

    egui::Window::new("🛡️ Permission Configuration")
        .default_size([win_w, win_h])
        .resizable(true)
        .collapsible(true)
        .show(ctx, |ui| {
            ui.heading("Risk Level Configuration");
            ui.separator();
            ui.add_space(8.0);

            // Render each risk level
            let risks = [
                OperationRisk::Safe,
                OperationRisk::LowRisk,
                OperationRisk::MediumRisk,
                OperationRisk::HighRisk,
                OperationRisk::Destructive,
            ];

            for risk in risks.iter() {
                render_risk_row(ui, &mut state, *risk);
                ui.add_space(8.0);
            }

            ui.add_space(16.0);
            ui.separator();
            ui.heading("Policy Summary");
            ui.separator();
            ui.add_space(8.0);

            // Policy summary
            ui.label(egui::RichText::new(state.policy_summary()).size(12.0));

            ui.add_space(16.0);
            ui.separator();

            // Reset to defaults button
            ui.horizontal(|ui| {
                if ui.button("Reset to Defaults").clicked() {
                    state.reset_to_defaults();
                }
                ui.add_space(8.0);
                if ui.button("Close").clicked() {
                    layout_queue.push(LayoutCommand::HidePanel { panel_id: "permission".to_string() });
                }
            });
        });
}

fn render_risk_row(ui: &mut egui::Ui, state: &mut PermissionState, risk: OperationRisk) {
    let mut current_action = state.get_action_for_risk(risk);
    
    ui.group(|ui| {
        ui.vertical(|ui| {
            // Risk level name and description
            ui.horizontal(|ui| {
                ui.colored_label(risk_color(risk), egui::RichText::new(risk_to_string(risk)).strong());
            });
            ui.add_space(2.0);
            ui.label(egui::RichText::new(risk_description(risk)).size(10.0).color(egui::Color32::from_gray(160)));
            ui.add_space(4.0);
            
            // Action selection
            ui.horizontal(|ui| {
                ui.label("Action:");
                
                let changed = egui::ComboBox::from_id_salt(format!("risk_action_{:?}", risk))
                    .selected_text(current_action.to_string())
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut current_action, RiskAction::AutoAllow, "Auto-Allow");
                        ui.selectable_value(&mut current_action, RiskAction::NeedConfirmation, "Need Confirmation");
                        ui.selectable_value(&mut current_action, RiskAction::Forbidden, "Forbidden");
                    })
                    .response
                    .changed();
                
                if changed {
                    state.set_action_for_risk(risk, current_action);
                }
            });
        });
    });
}

/// Toggle permission panel visibility
pub fn toggle_permission_panel(state: &mut ResMut<PermissionState>) {
    state.visible = !state.visible;
}
