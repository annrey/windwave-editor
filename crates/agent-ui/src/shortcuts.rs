//! Keyboard Shortcuts System
//!
//! Provides global keyboard shortcuts for editor operations.
//! Shortcuts:
//! - Ctrl+Z: Undo
//! - Ctrl+Y / Ctrl+Shift+Z: Redo
//! - Delete: Delete selected entity
//! - F: Focus camera on selected entity
//! - Ctrl+P: Open Command Palette

use bevy::prelude::*;

pub struct ShortcutsPlugin;

impl Plugin for ShortcutsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ShortcutState>()
            .add_systems(Update, handle_shortcuts);
    }
}

#[derive(Resource, Default)]
pub struct ShortcutState {
    pub undo_stack: Vec<UndoAction>,
    pub redo_stack: Vec<UndoAction>,
    pub last_shortcut: Option<String>,
}

pub enum UndoAction {
    DeleteEntity { entity: Entity, name: String },
    SetTransform { entity: Entity, old_transform: Transform },
    SetVisibility { entity: Entity, old_visible: bool },
}

fn handle_shortcuts(
    keys: Res<ButtonInput<KeyCode>>,
    mut shortcut_state: ResMut<ShortcutState>,
) {
    // Check for Ctrl modifier
    let ctrl_pressed = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);

    // Ctrl+Z: Undo
    if ctrl_pressed && keys.just_pressed(KeyCode::KeyZ) && !keys.pressed(KeyCode::ShiftLeft) && !keys.pressed(KeyCode::ShiftRight) {
        shortcut_state.last_shortcut = Some("Undo".to_string());
        info!("Shortcut: Undo (Ctrl+Z)");
        // TODO: Trigger undo action
    }

    // Ctrl+Y or Ctrl+Shift+Z: Redo
    if (ctrl_pressed && keys.just_pressed(KeyCode::KeyY))
        || (ctrl_pressed && (keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight)) && keys.just_pressed(KeyCode::KeyZ))
    {
        shortcut_state.last_shortcut = Some("Redo".to_string());
        info!("Shortcut: Redo (Ctrl+Y)");
        // TODO: Trigger redo action
    }

    // Delete: Delete selected entity
    if keys.just_pressed(KeyCode::Delete) {
        shortcut_state.last_shortcut = Some("Delete".to_string());
        info!("Shortcut: Delete selected entity");
        // TODO: Trigger delete action
    }

    // F: Focus on selected entity
    if keys.just_pressed(KeyCode::KeyF) {
        shortcut_state.last_shortcut = Some("Focus".to_string());
        info!("Shortcut: Focus on selected entity");
        // TODO: Trigger focus action
    }

    // Ctrl+P: Command Palette
    if ctrl_pressed && keys.just_pressed(KeyCode::KeyP) {
        shortcut_state.last_shortcut = Some("CommandPalette".to_string());
        info!("Shortcut: Open Command Palette");
        // TODO: Toggle command palette visibility
    }

    // G/R/S: Gizmo modes (when no text input is focused)
    // These are single-key shortcuts that should only work when not typing
    if !ctrl_pressed && keys.just_pressed(KeyCode::KeyG) {
        shortcut_state.last_shortcut = Some("GizmoTranslate".to_string());
        info!("Shortcut: Gizmo Translate mode");
    }

    if !ctrl_pressed && keys.just_pressed(KeyCode::KeyR) {
        shortcut_state.last_shortcut = Some("GizmoRotate".to_string());
        info!("Shortcut: Gizmo Rotate mode");
    }

    if !ctrl_pressed && keys.just_pressed(KeyCode::KeyS) {
        shortcut_state.last_shortcut = Some("GizmoScale".to_string());
        info!("Shortcut: Gizmo Scale mode");
    }
}

/// System to handle undo action
pub fn trigger_undo(
    shortcut_state: &mut ResMut<ShortcutState>,
) -> Option<UndoAction> {
    shortcut_state.undo_stack.pop()
}

/// System to handle redo action
pub fn trigger_redo(
    shortcut_state: &mut ResMut<ShortcutState>,
) -> Option<UndoAction> {
    shortcut_state.redo_stack.pop()
}

/// Push an action to the undo stack
pub fn push_undo_action(
    shortcut_state: &mut ResMut<ShortcutState>,
    action: UndoAction,
) {
    shortcut_state.undo_stack.push(action);
    // Clear redo stack when new action is performed
    shortcut_state.redo_stack.clear();
}
