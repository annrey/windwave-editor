//! History Panel - Audit log and edit history visualization
//!
//! Provides a panel for viewing audit logs and undo/redo history.
//! Features:
//! - Two tabs: Audit Log and Edit History
//! - Audit log: shows timestamped operations with results and risk levels
//! - Edit history: visual timeline with undo/redo capabilities

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use crate::layout::LayoutManager;
use agent_core::audit::{AuditLog, AuditEntry};
use agent_core::edit_history::EditHistory;

pub struct HistoryPanelPlugin;

impl Plugin for HistoryPanelPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<HistoryState>()
            .add_systems(Update, render_history_panel);
    }
}

#[derive(Resource)]
pub struct HistoryState {
    pub active_tab: HistoryTab,
    pub audit_log: AuditLog,
    pub edit_history: EditHistory,
    pub selected_history_index: Option<usize>,
    pub filter_audit_by_result: AuditResultFilter,
    pub filter_audit_by_agent: AgentFilter,
    pub mock_edit_history: Vec<String>, // Simple mock for display since we can't access real stack
}

impl Default for HistoryState {
    fn default() -> Self {
        Self {
            active_tab: HistoryTab::AuditLog,
            audit_log: AuditLog::new(1000),
            edit_history: EditHistory::new(50),
            selected_history_index: None,
            filter_audit_by_result: AuditResultFilter::All,
            filter_audit_by_agent: AgentFilter::All,
            mock_edit_history: Vec::new(),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Default)]
pub enum HistoryTab {
    #[default]
    AuditLog,
    EditHistory,
}

#[derive(Clone, Copy, PartialEq, Default)]
pub enum AuditResultFilter {
    #[default]
    All,
    Success,
    Failure,
    Forbidden,
    JailbreakBlocked,
}

#[derive(Clone, Copy, PartialEq, Default)]
pub enum AgentFilter {
    #[default]
    All,
    Director,
    Agent,
}

fn render_history_panel(
    mut contexts: EguiContexts,
    mut state: ResMut<HistoryState>,
    layout_mgr: Res<LayoutManager>,
) {
    if !layout_mgr.is_visible("history") {
        return;
    }

    let ctx = contexts.ctx_mut();
    let Ok(ctx) = ctx else { return };

    egui::TopBottomPanel::bottom("history_panel")
        .default_height(250.0)
        .resizable(true)
        .show(ctx, |ui| {
            // Tab selection
            ui.horizontal(|ui| {
                ui.heading("History");
                ui.separator();
                
                ui.selectable_value(&mut state.active_tab, HistoryTab::AuditLog, "📝 Audit Log");
                ui.selectable_value(&mut state.active_tab, HistoryTab::EditHistory, "↩️ Edit History");

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Clear").clicked() {
                        match state.active_tab {
                            HistoryTab::AuditLog => state.audit_log = AuditLog::new(1000),
                            HistoryTab::EditHistory => {
                                state.edit_history.clear();
                                state.mock_edit_history.clear();
                            }
                        }
                    }
                });
            });

            ui.separator();

            match state.active_tab {
                HistoryTab::AuditLog => render_audit_log_tab(ui, &mut state),
                HistoryTab::EditHistory => render_edit_history_tab(ui, &mut state),
            }
        });
}

fn render_audit_log_tab(ui: &mut egui::Ui, state: &mut HistoryState) {
    // Filter toolbar
    ui.horizontal(|ui| {
        ui.label("Filter by:");
        
        ui.selectable_value(
            &mut state.filter_audit_by_result, 
            AuditResultFilter::All, 
            "All"
        );
        ui.selectable_value(
            &mut state.filter_audit_by_result, 
            AuditResultFilter::Success, 
            "✅ Success"
        );
        ui.selectable_value(
            &mut state.filter_audit_by_result, 
            AuditResultFilter::Failure, 
            "❌ Failure"
        );
        ui.selectable_value(
            &mut state.filter_audit_by_result, 
            AuditResultFilter::Forbidden, 
            "🚫 Forbidden"
        );
        
        ui.separator();
        
        ui.selectable_value(
            &mut state.filter_audit_by_agent, 
            AgentFilter::All, 
            "All"
        );
        ui.selectable_value(
            &mut state.filter_audit_by_agent, 
            AgentFilter::Director, 
            "👤 Director"
        );
        ui.selectable_value(
            &mut state.filter_audit_by_agent, 
            AgentFilter::Agent, 
            "🤖 Agent"
        );
    });

    ui.separator();

    // Filter entries
    let mut filtered_entries: Vec<&AuditEntry> = state
        .audit_log
        .iter()
        .filter(|entry| {
            // Result filter
            let result_match = match state.filter_audit_by_result {
                AuditResultFilter::All => true,
                AuditResultFilter::Success => entry.result == "success",
                AuditResultFilter::Failure => entry.result == "failure",
                AuditResultFilter::Forbidden => entry.result == "forbidden",
                AuditResultFilter::JailbreakBlocked => entry.result == "jailbreak_blocked",
            };

            // Agent filter
            let agent_match = match state.filter_audit_by_agent {
                AgentFilter::All => true,
                AgentFilter::Director => entry.agent_id == 0,
                AgentFilter::Agent => entry.agent_id > 0,
            };

            result_match && agent_match
        })
        .collect();
    // Show newest first
    filtered_entries.reverse();

    // Table header
    ui.horizontal(|ui| {
        ui.heading("Index");
        ui.add_space(20.0);
        ui.heading("Timestamp");
        ui.add_space(80.0);
        ui.heading("Agent");
        ui.add_space(40.0);
        ui.heading("Action");
        ui.add_space(100.0);
        ui.heading("Target");
        ui.add_space(80.0);
        ui.heading("Result");
        ui.add_space(40.0);
        ui.heading("Risk");
        ui.add_space(20.0);
        ui.heading("Approved");
    });

    ui.separator();

    // Scrollable entries
    let text_style = egui::TextStyle::Monospace;
    let row_height = ui.text_style_height(&text_style) + 4.0;

    egui::ScrollArea::vertical()
        .auto_shrink([false; 2])
        .show_rows(ui, row_height, filtered_entries.len(), |ui, row_range| {
            for i in row_range {
                if let Some(entry) = filtered_entries.get(i) {
                    render_audit_entry(ui, entry);
                }
            }
        });
}

fn render_audit_entry(ui: &mut egui::Ui, entry: &AuditEntry) {
    ui.horizontal(|ui| {
        // Index
        ui.label(
            egui::RichText::new(format!("#{}", entry.index))
                .monospace()
                .color(egui::Color32::GRAY)
        );

        // Timestamp
        let timestamp_str = format_timestamp(entry.timestamp);
        ui.label(
            egui::RichText::new(&timestamp_str)
                .monospace()
                .color(egui::Color32::LIGHT_GRAY)
        );

        // Agent
        let agent_name = if entry.agent_id == 0 { "Director" } else { "Agent" };
        ui.label(
            egui::RichText::new(agent_name)
                .monospace()
                .color(egui::Color32::LIGHT_BLUE)
        );

        // Action
        ui.label(
            egui::RichText::new(&entry.action)
                .monospace()
        );

        // Target
        ui.label(
            egui::RichText::new(&entry.target)
                .monospace()
                .color(egui::Color32::LIGHT_GREEN)
        );

        // Result
        let (result_icon, result_color) = match entry.result.as_str() {
            "success" => ("✅", egui::Color32::GREEN),
            "failure" => ("❌", egui::Color32::RED),
            "forbidden" => ("🚫", egui::Color32::ORANGE),
            "jailbreak_blocked" => ("🛡️", egui::Color32::RED),
            _ => ("❓", egui::Color32::GRAY),
        };
        ui.label(
            egui::RichText::new(format!("{} {}", result_icon, entry.result))
                .color(result_color)
        );

        // Risk level
        let risk_color = match entry.risk_level.as_str() {
            "LowRisk" => egui::Color32::GREEN,
            "MediumRisk" => egui::Color32::YELLOW,
            "HighRisk" => egui::Color32::RED,
            _ => egui::Color32::GRAY,
        };
        ui.label(
            egui::RichText::new(&entry.risk_level)
                .color(risk_color)
        );

        // User approved
        let approved_text = if entry.user_approved { "✓" } else { "✗" };
        let approved_color = if entry.user_approved { egui::Color32::GREEN } else { egui::Color32::GRAY };
        ui.label(
            egui::RichText::new(approved_text)
                .color(approved_color)
        );
    });
}

fn render_edit_history_tab(ui: &mut egui::Ui, state: &mut HistoryState) {
    // Add demo buttons for testing
    ui.horizontal(|ui| {
        if ui.button("Add Demo Edit").clicked() {
            let demo_ops = [
                "CreateEntity", "SetTransform", "SetColor", 
                "DeleteEntity", "SetVisibility", "MultiOp"
            ];
            let op_name = demo_ops[state.mock_edit_history.len() % demo_ops.len()];
            state.mock_edit_history.push(op_name.to_string());
        }
    });

    ui.separator();

    // Undo/Redo buttons
    ui.horizontal(|ui| {
        let undo_enabled = state.edit_history.can_undo() || !state.mock_edit_history.is_empty();
        let redo_enabled = state.edit_history.can_redo();

        if ui
            .add_enabled(undo_enabled, egui::Button::new("↩️ Undo"))
            .clicked()
        {
            // Mock undo
            if !state.mock_edit_history.is_empty() {
                state.mock_edit_history.pop();
            }
            // TODO: Need access to SceneBridge to actually perform undo on real history
        }

        if let Some(name) = state.edit_history.top_undo_name() {
            ui.label(format!("Next: {}", name));
        } else if let Some(name) = state.mock_edit_history.last() {
            ui.label(format!("Next: {}", name));
        }

        ui.separator();

        if ui
            .add_enabled(redo_enabled, egui::Button::new("↪️ Redo"))
            .clicked()
        {
            // TODO: Need access to SceneBridge to actually perform redo
        }

        if let Some(name) = state.edit_history.top_redo_name() {
            ui.label(format!("Next: {}", name));
        }
    });

    ui.separator();

    // History counts
    ui.horizontal(|ui| {
        ui.label(format!("Undo stack: {}", state.edit_history.undo_count()));
        ui.label(format!("Redo stack: {}", state.edit_history.redo_count()));
        ui.label(format!("Mock history: {}", state.mock_edit_history.len()));
    });

    ui.separator();

    // Visual timeline
    ui.heading("Edit Timeline");
    ui.add_space(8.0);

    // Render mock timeline
    if state.mock_edit_history.is_empty() && state.edit_history.undo_count() == 0 {
        ui.label("No edit history yet. Make some changes to see them here.");
        ui.label("Click 'Add Demo Edit' to see the timeline in action.");
    } else {
        // Clone data to avoid borrow issues
        let history_items = state.mock_edit_history.clone();
        let total_count = history_items.len();
        let mut selected_idx = state.selected_history_index;
        
        egui::ScrollArea::horizontal().show(ui, |ui| {
            ui.horizontal(|ui| {
                // Show mock history items
                for (i, name) in history_items.iter().enumerate() {
                    let is_selected = selected_idx == Some(i);
                    let (new_selected, hover_text) = render_timeline_item(
                        ui, 
                        name, 
                        i, 
                        true, 
                        false, 
                        is_selected, 
                        total_count
                    );
                    
                    if new_selected {
                        selected_idx = if is_selected { None } else { Some(i) };
                    }
                    
                    if let Some(text) = hover_text {
                        ui.label(text);
                    }
                }
            });
        });
        
        // Update state after the borrow
        state.selected_history_index = selected_idx;
    }

    // Selected item details
    if let Some(selected_idx) = state.selected_history_index {
        if selected_idx < state.mock_edit_history.len() {
            ui.separator();
            ui.heading("Selected Operation");
            ui.label(format!("Name: {}", state.mock_edit_history[selected_idx]));
            ui.label("Status: Applied");
        }
    }
}

fn render_timeline_item(
    ui: &mut egui::Ui,
    name: &str,
    idx: usize,
    _is_applied: bool,
    is_redo: bool,
    is_selected: bool,
    total_count: usize,
) -> (bool, Option<String>) {
    let (fill_color, text_color, border_color) = if is_redo {
        (
            egui::Color32::from_gray(60),
            egui::Color32::LIGHT_GRAY,
            egui::Color32::from_gray(80),
        )
    } else if is_selected {
        (
            egui::Color32::from_rgb(99, 102, 241),
            egui::Color32::WHITE,
            egui::Color32::from_rgb(139, 92, 246),
        )
    } else {
        (
            egui::Color32::from_gray(80),
            egui::Color32::WHITE,
            egui::Color32::from_gray(100),
        )
    };

    let mut clicked = false;
    let mut hover_text = None;

    ui.vertical(|ui| {
        // Dot
        let (rect, response) = ui.allocate_exact_size(egui::Vec2::new(20.0, 20.0), egui::Sense::click());
        let painter = ui.painter();
        painter.circle_filled(rect.center(), 8.0, fill_color);
        painter.circle_stroke(rect.center(), 8.0, egui::Stroke::new(2.0, border_color));

        // Line to next item
        if idx < total_count.saturating_sub(1) {
            let next_x = rect.max.x + 40.0;
            painter.line_segment(
                [rect.right_center(), egui::pos2(next_x, rect.center().y)],
                egui::Stroke::new(2.0, border_color),
            );
        }

        // Label
        ui.add_space(4.0);
        ui.label(
            egui::RichText::new(name)
                .size(11.0)
                .color(text_color)
        );
        ui.label(
            egui::RichText::new(format!("#{}", idx + 1))
                .size(9.0)
                .color(egui::Color32::GRAY)
        );

        if response.clicked() {
            clicked = true;
        }

        if response.hovered() {
            hover_text = Some(format!("Operation: {}\nClick to select", name));
        }
    });

    ui.add_space(20.0);

    (clicked, hover_text)
}

fn format_timestamp(unix_seconds: u64) -> String {
    // Simple formatting without chrono
    let hours = (unix_seconds / 3600) % 24;
    let minutes = (unix_seconds / 60) % 60;
    let seconds = unix_seconds % 60;
    format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
}
