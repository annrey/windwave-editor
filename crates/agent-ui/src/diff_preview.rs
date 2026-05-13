//! Diff Preview Panel - Phase 3.5
//!
//! Shows expected changes before user approves a plan.
//! Integrates with DirectorDesk approval flow.

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};

use crate::director_desk::{UserAction, DirectorDeskState};
use crate::layout::{LayoutManager, LayoutCommand, PanelPosition};
use crate::LayoutCommandQueue;

pub struct DiffPreviewPlugin;

impl Plugin for DiffPreviewPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DiffPreviewState>()
            .add_systems(Update, render_diff_preview);
    }
}

/// State for diff preview panel
#[derive(Resource, Default)]
pub struct DiffPreviewState {
    pub visible: bool,
    pub changes: Vec<ExpectedChange>,
    pub selected_change: Option<usize>,
    /// Plan ID for approval event dispatch
    pub pending_approval_id: Option<String>,
}

/// Represents an expected change
#[derive(Debug, Clone)]
pub struct ExpectedChange {
    pub entity_name: Option<String>,
    pub change_kind: ChangeKind,
    pub description: String,
    pub before: Option<serde_json::Value>,
    pub after: Option<serde_json::Value>,
}

/// Type of change
#[derive(Debug, Clone, PartialEq)]
pub enum ChangeKind {
    Create,
    Update,
    Delete,
    Move,
}

impl DiffPreviewState {
    /// Clear all changes
    pub fn clear(&mut self) {
        self.changes.clear();
        self.selected_change = None;
    }

    /// Add a change
    pub fn add_change(&mut self, change: ExpectedChange) {
        self.changes.push(change);
    }

    /// Set changes from a plan
    pub fn set_changes(&mut self, changes: Vec<ExpectedChange>) {
        let is_empty = changes.is_empty();
        self.changes = changes;
        self.visible = !is_empty;
    }

    /// Get summary text
    pub fn summary(&self) -> String {
        let creates = self.changes.iter().filter(|c| c.change_kind == ChangeKind::Create).count();
        let updates = self.changes.iter().filter(|c| c.change_kind == ChangeKind::Update).count();
        let deletes = self.changes.iter().filter(|c| c.change_kind == ChangeKind::Delete).count();
        let moves = self.changes.iter().filter(|c| c.change_kind == ChangeKind::Move).count();

        format!(
            "Changes: {} create, {} update, {} delete, {} move",
            creates, updates, deletes, moves
        )
    }
}

impl ExpectedChange {
    /// Create a new change
    pub fn new(
        entity_name: Option<&str>,
        kind: ChangeKind,
        description: &str,
    ) -> Self {
        Self {
            entity_name: entity_name.map(|s| s.to_string()),
            change_kind: kind,
            description: description.to_string(),
            before: None,
            after: None,
        }
    }

    /// Set before/after values for diff display
    pub fn with_diff(
        mut self,
        before: serde_json::Value,
        after: serde_json::Value,
    ) -> Self {
        self.before = Some(before);
        self.after = Some(after);
        self
    }

    /// Get display icon for change kind
    pub fn icon(&self) -> &'static str {
        match self.change_kind {
            ChangeKind::Create => "➕",
            ChangeKind::Update => "✏️",
            ChangeKind::Delete => "🗑️",
            ChangeKind::Move => "📍",
        }
    }

    /// Get color for change kind
    pub fn color(&self) -> egui::Color32 {
        match self.change_kind {
            ChangeKind::Create => egui::Color32::from_rgb(16, 185, 129),   // Green
            ChangeKind::Update => egui::Color32::from_rgb(59, 130, 246),   // Blue
            ChangeKind::Delete => egui::Color32::from_rgb(239, 68, 68),    // Red
            ChangeKind::Move => egui::Color32::from_rgb(245, 158, 11),     // Orange
        }
    }
}

fn render_diff_preview(
    mut contexts: EguiContexts,
    mut state: ResMut<DiffPreviewState>,
    mut desk_state: ResMut<DirectorDeskState>,
    layout_mgr: Res<LayoutManager>,
    mut layout_queue: ResMut<LayoutCommandQueue>,
) {
    let ctx = match contexts.ctx_mut() {
        Ok(ctx) => ctx,
        Err(_) => return,
    };

    if !layout_mgr.is_visible("diff_preview") || state.changes.is_empty() {
        return;
    }

    let (win_w, win_h) = layout_mgr
        .panel_config("diff_preview")
        .and_then(|c| match c.position {
            PanelPosition::Floating { width, height, .. } => Some((width, height)),
            _ => None,
        })
        .unwrap_or((500.0, 400.0));

    egui::Window::new("📋 Diff Preview")
        .default_size([win_w, win_h])
        .resizable(true)
        .collapsible(true)
        .show(ctx, |ui| {
            ui.heading("Expected Changes");
            ui.label(state.summary());
            ui.separator();

            // Change list
            let changes_count = state.changes.len();
            let selected = state.selected_change;

            egui::ScrollArea::vertical()
                .max_height(250.0)
                .show(ui, |ui| {
                    for idx in 0..changes_count {
                        let is_selected = selected == Some(idx);
                        let change = &state.changes[idx];

                        let response = ui.horizontal(|ui| {
                            // Icon with color
                            ui.colored_label(change.color(), change.icon());

                            // Entity name
                            let name = change.entity_name.as_deref().unwrap_or("Unknown");
                            ui.label(
                                egui::RichText::new(name)
                                    .strong()
                                    .color(if is_selected {
                                        egui::Color32::WHITE
                                    } else {
                                        egui::Color32::LIGHT_GRAY
                                    }),
                            );

                            // Description
                            ui.label(&change.description);
                        });

                        // Clone data needed for detail view before mutating state
                        let change_details = if is_selected {
                            change.before.clone().zip(change.after.clone())
                        } else {
                            None
                        };

                        if response.response.clicked() {
                            state.selected_change = Some(idx);
                        }

                        if is_selected {
                            ui.indent("details", |ui| {
                                // Show diff details
                                if let Some((before, after)) = change_details {
                                    ui.colored_label(
                                        egui::Color32::from_rgb(239, 68, 68),
                                        format!("- {}", serde_json::to_string_pretty(&before).unwrap_or_default()),
                                    );
                                    ui.colored_label(
                                        egui::Color32::from_rgb(16, 185, 129),
                                        format!("+ {}", serde_json::to_string_pretty(&after).unwrap_or_default()),
                                    );
                                }
                            });
                        }

                        ui.separator();
                    }
                });

            ui.add_space(16.0);

            // Action buttons
            ui.horizontal(|ui| {
                if ui.button("✅ Approve All").clicked() {
                    if let Some(plan_id) = state.pending_approval_id.take() {
                        desk_state.pending_actions.push(UserAction::Approve { plan_id });
                    }
                    layout_queue.push(LayoutCommand::HidePanel { panel_id: "diff_preview".to_string() });
                }

                if ui.button("❌ Cancel").clicked() {
                    if let Some(plan_id) = state.pending_approval_id.take() {
                        desk_state.pending_actions.push(UserAction::Reject {
                            plan_id,
                            reason: Some("User cancelled".into()),
                        });
                    }
                    state.clear();
                    layout_queue.push(LayoutCommand::HidePanel { panel_id: "diff_preview".to_string() });
                }
            });
        });
}

/// Helper to create a change set from a plan description
pub fn create_change_set_from_description(description: &str) -> Vec<ExpectedChange> {
    // Simple parsing - in production, this would parse structured plan data
    let mut changes = Vec::new();

    // Example: look for keywords like "create", "update", "delete", "move"
    let lower = description.to_lowercase();

    if lower.contains("create") || lower.contains("spawn") {
        changes.push(ExpectedChange::new(
            Some("New Entity"),
            ChangeKind::Create,
            "Create new entity",
        ));
    }

    if lower.contains("update") || lower.contains("modify") || lower.contains("change") {
        changes.push(ExpectedChange::new(
            Some("Existing Entity"),
            ChangeKind::Update,
            "Update entity properties",
        ));
    }

    if lower.contains("delete") || lower.contains("remove") {
        changes.push(ExpectedChange::new(
            Some("Entity to Delete"),
            ChangeKind::Delete,
            "Delete entity",
        ));
    }

    if lower.contains("move") || lower.contains("position") {
        changes.push(ExpectedChange::new(
            Some("Entity to Move"),
            ChangeKind::Move,
            "Move entity to new position",
        ));
    }

    if changes.is_empty() {
        // Default: assume update
        changes.push(ExpectedChange::new(
            None,
            ChangeKind::Update,
            "Apply plan changes",
        ));
    }

    changes
}

/// Show diff preview for a plan
pub fn show_diff_preview(state: &mut ResMut<DiffPreviewState>, changes: Vec<ExpectedChange>) {
    state.set_changes(changes);
}

/// Hide diff preview
pub fn hide_diff_preview(state: &mut ResMut<DiffPreviewState>) {
    state.visible = false;
    state.clear();
}
