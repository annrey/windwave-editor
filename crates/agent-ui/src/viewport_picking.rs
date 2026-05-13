//! Viewport Picking System - Phase 2.1 (Simplified)
//!
//! Enables clicking in the viewport to select entities.

use bevy::prelude::*;
use bevy::sprite::Sprite;

use crate::editor_selection::EditorSelection;

pub struct ViewportPickingPlugin;

impl Plugin for ViewportPickingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PickingState>()
            .add_systems(Update, handle_viewport_click);
    }
}

#[derive(Resource)]
pub struct PickingState {
    pub enabled: bool,
}

impl Default for PickingState {
    fn default() -> Self {
        Self { enabled: true }
    }
}

#[derive(Component, Default)]
pub struct Pickable {
    pub selectable: bool,
}

fn handle_viewport_click(
    mouse_button: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    camera_query: Query<(&Camera, &GlobalTransform)>,
    pickable_query: Query<(Entity, &GlobalTransform, &Sprite, &Pickable)>,
    mut editor_selection: ResMut<EditorSelection>,
    picking_state: Res<PickingState>,
) {
    if !picking_state.enabled {
        return;
    }

    if !mouse_button.just_pressed(MouseButton::Left) {
        return;
    }

    // Get primary window
    let window = match windows.iter().find(|w| w.focused) {
        Some(w) => w,
        None => return,
    };

    let Some(cursor_position) = window.cursor_position() else {
        return;
    };

    // Get camera
    let (camera, camera_transform) = match camera_query.iter().next() {
        Some((cam, trans)) => (cam, trans),
        None => return,
    };

    let Ok(ray) = camera.viewport_to_world(camera_transform, cursor_position) else {
        return;
    };

    let click_point = ray.origin.truncate();

    // Find entity under cursor
    let mut best_pick: Option<Entity> = None;
    let mut best_z: f32 = f32::NEG_INFINITY;

    for (entity, transform, sprite, pickable) in pickable_query.iter() {
        if !pickable.selectable {
            continue;
        }

        let entity_pos = transform.translation().truncate();
        let sprite_size = match sprite.custom_size {
            Some(size) => size,
            None => Vec2::ONE * 50.0,
        };

        let min = entity_pos - sprite_size * 0.5;
        let max = entity_pos + sprite_size * 0.5;

        if click_point.x >= min.x && click_point.x <= max.x
            && click_point.y >= min.y && click_point.y <= max.y
        {
            let z = transform.translation().z;
            if z > best_z {
                best_z = z;
                best_pick = Some(entity);
            }
        }
    }

    // Update selection
    if let Some(entity) = best_pick {
        if editor_selection.selected_entity != Some(entity) {
            editor_selection.select(entity);
        }
    } else {
        editor_selection.clear();
    }
}

/// Toggle picking enabled/disabled
pub fn set_picking_enabled(state: &mut ResMut<PickingState>, enabled: bool) {
    state.enabled = enabled;
}

/// Helper to spawn a pickable entity
pub fn spawn_pickable_entity(
    commands: &mut Commands,
    position: Vec3,
    size: Vec2,
    color: Color,
    name: &str,
) -> Entity {
    commands
        .spawn((
            Sprite {
                custom_size: Some(size),
                color,
                ..default()
            },
            Transform::from_translation(position),
            Pickable { selectable: true },
            Name::new(name.to_string()),
        ))
        .id()
}
