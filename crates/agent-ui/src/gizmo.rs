//! Transform Gizmo System - Phase 2.2 (Simplified)
//!
//! Provides basic transformation controls.
//! Hotkeys: G (translate), R (rotate), S (scale)

use bevy::prelude::*;
use bevy::sprite::Sprite;

use crate::editor_selection::EditorSelection;
use crate::viewport_picking::PickingState;

pub struct GizmoPlugin;

impl Plugin for GizmoPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GizmoState>()
            .add_systems(Update, handle_gizmo_shortcuts)
            .add_systems(Update, spawn_gizmo_for_selection)
            .add_systems(Update, update_gizmo_position)
            .add_systems(Update, cleanup_gizmo);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GizmoMode {
    #[default]
    None,
    Translation,
    Rotation,
    Scale,
}

#[derive(Resource, Default)]
pub struct GizmoState {
    pub mode: GizmoMode,
    pub gizmo_entity: Option<Entity>,
    pub target_entity: Option<Entity>,
}

#[derive(Component)]
pub struct GizmoRoot;

pub fn set_gizmo_mode(state: &mut ResMut<GizmoState>, mode: GizmoMode) {
    state.mode = mode;
}

fn handle_gizmo_shortcuts(
    keys: Res<ButtonInput<KeyCode>>,
    mut gizmo_state: ResMut<GizmoState>,
    mut picking_state: ResMut<PickingState>,
) {
    if keys.just_pressed(KeyCode::KeyG) {
        let new_mode = if gizmo_state.mode == GizmoMode::Translation {
            GizmoMode::None
        } else {
            GizmoMode::Translation
        };
        gizmo_state.mode = new_mode;
        picking_state.enabled = new_mode == GizmoMode::None;
        info!("Gizmo mode: {:?}", new_mode);
    }

    if keys.just_pressed(KeyCode::KeyR) {
        info!("Rotation gizmo (placeholder)");
    }

    if keys.just_pressed(KeyCode::KeyS) {
        info!("Scale gizmo (placeholder)");
    }
}

fn spawn_gizmo_for_selection(
    mut commands: Commands,
    editor_selection: Res<EditorSelection>,
    mut gizmo_state: ResMut<GizmoState>,
    query: Query<&Transform>,
    existing_gizmo: Query<Entity, With<GizmoRoot>>,
) {
    if gizmo_state.mode == GizmoMode::None {
        // Despawn existing gizmo
        for entity in existing_gizmo.iter() {
            commands.entity(entity).despawn();
        }
        gizmo_state.gizmo_entity = None;
        return;
    }

    let Some(selected) = editor_selection.selected_entity else {
        for entity in existing_gizmo.iter() {
            commands.entity(entity).despawn();
        }
        gizmo_state.gizmo_entity = None;
        return;
    };

    // Check if gizmo already exists
    if gizmo_state.gizmo_entity.is_some() && gizmo_state.target_entity == Some(selected) {
        return;
    }

    // Despawn old gizmo
    for entity in existing_gizmo.iter() {
        commands.entity(entity).despawn();
    }

    let Ok(transform) = query.get(selected) else {
        return;
    };

    let position = transform.translation;

    // Spawn simplified gizmo visualization
    let gizmo = commands
        .spawn((
            Transform::from_translation(position),
            GlobalTransform::default(),
            GizmoRoot,
            Name::new("Gizmo"),
        ))
        .with_children(|parent| {
            // X axis indicator
            parent.spawn((
                Sprite {
                    custom_size: Some(Vec2::new(30.0, 8.0)),
                    color: Color::srgb(1.0, 0.2, 0.2),
                    ..default()
                },
                Transform::from_translation(Vec3::X * 20.0),
            ));

            // Y axis indicator
            parent.spawn((
                Sprite {
                    custom_size: Some(Vec2::new(8.0, 30.0)),
                    color: Color::srgb(0.2, 1.0, 0.2),
                    ..default()
                },
                Transform::from_translation(Vec3::Y * 20.0),
            ));
        })
        .id();

    gizmo_state.gizmo_entity = Some(gizmo);
    gizmo_state.target_entity = Some(selected);
}

fn update_gizmo_position(
    gizmo_state: Res<GizmoState>,
    mut gizmo_query: Query<&mut Transform, With<GizmoRoot>>,
    target_query: Query<&Transform, Without<GizmoRoot>>,
) {
    let Some(gizmo_entity) = gizmo_state.gizmo_entity else {
        return;
    };

    let Some(target_entity) = gizmo_state.target_entity else {
        return;
    };

    let Ok(target_transform) = target_query.get(target_entity) else {
        return;
    };

    let Ok(mut gizmo_transform) = gizmo_query.get_mut(gizmo_entity) else {
        return;
    };

    gizmo_transform.translation = target_transform.translation;
}

fn cleanup_gizmo(
    mut commands: Commands,
    editor_selection: Res<EditorSelection>,
    mut gizmo_state: ResMut<GizmoState>,
    existing_gizmo: Query<Entity, With<GizmoRoot>>,
) {
    if editor_selection.selected_entity.is_none() && gizmo_state.gizmo_entity.is_some() {
        for entity in existing_gizmo.iter() {
            commands.entity(entity).despawn();
        }
        gizmo_state.gizmo_entity = None;
        gizmo_state.target_entity = None;
    }
}

pub fn get_gizmo_mode_description(mode: GizmoMode) -> &'static str {
    match mode {
        GizmoMode::None => "None (G/R/S to activate)",
        GizmoMode::Translation => "Translation (G: toggle)",
        GizmoMode::Rotation => "Rotation (R: toggle) - placeholder",
        GizmoMode::Scale => "Scale (S: toggle) - placeholder",
    }
}
