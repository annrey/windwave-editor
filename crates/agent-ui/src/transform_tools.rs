//! Transform Tools - Drag-to-move, rotate, scale via mouse interaction
//!
//! Hotkeys: G (translate), R (rotate), S (scale)
//! Click-drag on the viewport to transform the selected entity.
//! Builds on the existing Gizmo visual indicator.
//!
//! Architecture: reads EditorSelection for target, reads mouse input for drag,
//! writes directly to ECS Transform via mutable query.

use bevy::prelude::*;
use bevy::sprite::Sprite;

use crate::editor_selection::EditorSelection;
use crate::gizmo::{GizmoState, GizmoMode};

pub struct TransformToolsPlugin;

impl Plugin for TransformToolsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TransformDragState>()
            .init_resource::<SnapSettings>()
            .add_systems(Update, (handle_transform_drag, handle_snap_shortcuts));
    }
}

/// Grid/angle snap configuration
#[derive(Resource)]
pub struct SnapSettings {
    pub grid_size: f32,
    pub angle_snap: f32,  // degrees
    pub snap_enabled: bool,
}

impl Default for SnapSettings {
    fn default() -> Self {
        Self { grid_size: 16.0, angle_snap: 15.0, snap_enabled: false }
    }
}

fn handle_snap_shortcuts(keys: Res<ButtonInput<KeyCode>>, mut snap: ResMut<SnapSettings>) {
    if keys.just_pressed(KeyCode::KeyJ) {
        snap.snap_enabled = !snap.snap_enabled;
        info!("Snap: {}", if snap.snap_enabled { "ON" } else { "OFF" });
    }
}

#[derive(Resource, Default)]
pub struct TransformDragState {
    pub dragging: bool,
    pub drag_start_world: Option<Vec2>,
    pub drag_start_transform: Option<Transform>,
    pub drag_axis: Option<DragAxis>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DragAxis {
    X,
    Y,
    XY,
}

fn handle_transform_drag(
    mouse_button: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    camera_query: Query<(&Camera, &GlobalTransform)>,
    editor_selection: Res<EditorSelection>,
    gizmo_state: Res<GizmoState>,
    snap: Res<SnapSettings>,
    mut drag_state: ResMut<TransformDragState>,
    mut transform_query: Query<&mut Transform>,
    _sprite_query: Query<&Sprite>,
) {
    // Only active when transform gizmo mode is on and an entity is selected
    let Some(target_entity) = editor_selection.selected_entity else {
        drag_state.dragging = false;
        return;
    };

    if gizmo_state.mode == GizmoMode::None {
        drag_state.dragging = false;
        return;
    }

    // Get cursor world position
    let window = match windows.iter().find(|w| w.focused) {
        Some(w) => w,
        None => return,
    };
    let Some(cursor) = window.cursor_position() else { return };

    let (camera, camera_transform) = match camera_query.iter().next() {
        Some(c) => c,
        None => return,
    };
    let Ok(ray) = camera.viewport_to_world(camera_transform, cursor) else {
        return;
    };
    let cursor_world = ray.origin.truncate();

    if mouse_button.just_pressed(MouseButton::Left) {
        // Start drag - capture initial state
        if let Ok(current) = transform_query.get(target_entity) {
            drag_state.dragging = true;
            drag_state.drag_start_world = Some(cursor_world);
            drag_state.drag_start_transform = Some(*current);
            drag_state.drag_axis = match gizmo_state.mode {
                GizmoMode::Translation | GizmoMode::Scale => Some(DragAxis::XY),
                GizmoMode::Rotation => Some(DragAxis::Y),
                GizmoMode::None => None,
            };
        }
    }

    if !drag_state.dragging {
        return;
    }

    if !mouse_button.pressed(MouseButton::Left) {
        // End drag
        drag_state.dragging = false;
        return;
    }

    let Some(start_world) = drag_state.drag_start_world else { return };
    let Some(start_transform) = drag_state.drag_start_transform else { return };
    let Ok(mut transform) = transform_query.get_mut(target_entity) else { return };

    let delta = cursor_world - start_world;

    match gizmo_state.mode {
        GizmoMode::Translation => {
            let mut nx = start_transform.translation.x + delta.x;
            let mut ny = start_transform.translation.y + delta.y;
            if snap.snap_enabled {
                nx = (nx / snap.grid_size).round() * snap.grid_size;
                ny = (ny / snap.grid_size).round() * snap.grid_size;
            }
            transform.translation.x = nx;
            transform.translation.y = ny;
        }
        GizmoMode::Rotation => {
            let mut angle = delta.x * 0.01;
            if snap.snap_enabled {
                let snap_rad = snap.angle_snap.to_radians();
                angle = (angle / snap_rad).round() * snap_rad;
            }
            transform.rotation = start_transform.rotation * Quat::from_rotation_z(angle);
        }
        GizmoMode::Scale => {
            let scale_factor = 1.0 + delta.x * 0.005;
            let new_scale = start_transform.scale * scale_factor;
            transform.scale = new_scale.max(Vec3::splat(0.01));
        }
        GizmoMode::None => {}
    }
}
