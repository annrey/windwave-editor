//! Inspector Panel - Entity property editor
//!
//! Provides a right-side panel for viewing and editing entity components.
//! Features:
//! - View selected entity's components
//! - Edit Transform (position, rotation, scale)
//! - Edit Sprite (color, visibility)
//! - Edit RuntimeAgentComponent properties

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use bevy_adapter::{RuntimeAgentComponent, RuntimeAgentStatus};
use crate::layout::LayoutManager;

pub struct InspectorPanelPlugin;

impl Plugin for InspectorPanelPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<InspectorState>()
            .add_systems(Update, render_inspector_panel);
    }
}

#[derive(Resource, Default)]
pub struct InspectorState {
    pub inspected_entity: Option<Entity>,
    pub active_tab: InspectorTab,
}

#[derive(Default, PartialEq)]
pub enum InspectorTab {
    #[default]
    Transform,
    Sprite,
    Agent,
    Components,
}

fn render_inspector_panel(
    mut contexts: EguiContexts,
    mut state: ResMut<InspectorState>,
    editor_selection: Res<crate::editor_selection::EditorSelection>,
    mut transform_query: Query<&mut Transform>,
    mut sprite_query: Query<&mut Sprite>,
    mut visibility_query: Query<&mut Visibility>,
    agent_query: Query<&RuntimeAgentComponent>,
    name_query: Query<(Entity, Option<&Name>)>,
    layout_mgr: Res<LayoutManager>,
) {
    state.inspected_entity = editor_selection.selected_entity;

    if !layout_mgr.is_visible("inspector") { return; }

    let ctx = contexts.ctx_mut();
    let Ok(ctx) = ctx else { return };

    egui::SidePanel::right("inspector_panel")
        .default_width(300.0)
        .resizable(true)
        .show(ctx, |ui| {
            ui.heading("Inspector");
            ui.separator();

            let Some(entity) = state.inspected_entity else {
                ui.label("Select an entity in the Hierarchy panel to inspect");
                return;
            };

            // Entity header
            let name = name_query
                .get(entity)
                .ok()
                .and_then(|(_, name)| name.map(|n| n.as_str().to_string()))
                .unwrap_or_else(|| format!("Entity {:?}", entity));

            ui.heading(&name);
            ui.label(format!("ID: {:?}", entity));
            ui.separator();

            // Tabs
            ui.horizontal(|ui| {
                ui.selectable_value(&mut state.active_tab, InspectorTab::Transform, "Transform");
                ui.selectable_value(&mut state.active_tab, InspectorTab::Sprite, "Sprite");
                ui.selectable_value(&mut state.active_tab, InspectorTab::Agent, "Agent");
                ui.selectable_value(&mut state.active_tab, InspectorTab::Components, "Components");
            });

            ui.separator();

            // Tab content
            match state.active_tab {
                InspectorTab::Transform => {
                    if let Ok(mut transform) = transform_query.get_mut(entity) {
                        render_transform_editor(ui, &mut transform);
                    } else {
                        ui.label("No Transform component");
                    }
                }
                InspectorTab::Sprite => {
                    let visibility = visibility_query.get_mut(entity).ok();
                    if let Ok(mut sprite) = sprite_query.get_mut(entity) {
                        render_sprite_editor(ui, &mut sprite, visibility);
                    } else {
                        ui.label("No Sprite component");
                    }
                }
                InspectorTab::Agent => {
                    if let Ok(agent) = agent_query.get(entity) {
                        render_agent_editor(ui, agent);
                    } else {
                        ui.label("No RuntimeAgentComponent");
                    }
                }
                InspectorTab::Components => {
                    ui.label("Component list (read-only)");
                    ui.label("- Transform (viewable)");
                    ui.label("- Sprite (viewable)");
                    ui.label("- Visibility (viewable)");
                    if agent_query.get(entity).is_ok() {
                        ui.label("- RuntimeAgentComponent (viewable)");
                    }
                }
            }
        });
}

fn render_transform_editor(ui: &mut egui::Ui, transform: &mut Transform) {
    ui.label("📐 Transform");
    ui.separator();

    // Translation
    ui.label("Translation");
    let mut translation = transform.translation;
    ui.horizontal(|ui| {
        ui.label("X:");
        ui.add(egui::DragValue::new(&mut translation.x).speed(0.1));
        ui.label("Y:");
        ui.add(egui::DragValue::new(&mut translation.y).speed(0.1));
        ui.label("Z:");
        ui.add(egui::DragValue::new(&mut translation.z).speed(0.1));
    });
    transform.translation = translation;

    ui.separator();

    ui.label("Rotation");
    let (mut rot_x, mut rot_y, mut rot_z) = transform.rotation.to_euler(EulerRot::XYZ);
    ui.horizontal(|ui| {
        ui.label("X:");
        ui.add(egui::DragValue::new(&mut rot_x).speed(0.01));
        ui.label("Y:");
        ui.add(egui::DragValue::new(&mut rot_y).speed(0.01));
        ui.label("Z:");
        ui.add(egui::DragValue::new(&mut rot_z).speed(0.01));
    });
    transform.rotation = Quat::from_euler(EulerRot::XYZ, rot_x, rot_y, rot_z);

    ui.separator();

    // Scale
    ui.label("Scale");
    let mut scale = transform.scale;
    ui.horizontal(|ui| {
        ui.label("X:");
        ui.add(egui::DragValue::new(&mut scale.x).speed(0.01));
        ui.label("Y:");
        ui.add(egui::DragValue::new(&mut scale.y).speed(0.01));
        ui.label("Z:");
        ui.add(egui::DragValue::new(&mut scale.z).speed(0.01));
    });
    transform.scale = scale;

    ui.separator();

    if ui.button("Reset Transform").clicked() {
        *transform = Transform::default();
    }
}

fn render_sprite_editor(
    ui: &mut egui::Ui,
    sprite: &mut Sprite,
    visibility: Option<Mut<Visibility>>,
) {
    ui.label("🎨 Sprite");
    ui.separator();

    // Color
    ui.label("Color");
    let color = sprite.color.to_linear();
    let mut rgba = [
        color.red,
        color.green,
        color.blue,
        color.alpha,
    ];

    if ui.color_edit_button_rgba_unmultiplied(&mut rgba).changed() {
        sprite.color = Color::linear_rgba(rgba[0], rgba[1], rgba[2], rgba[3]);
    }

    ui.separator();

    ui.label("Size");
    let mut size = sprite.custom_size.unwrap_or(Vec2::splat(50.0));
    ui.horizontal(|ui| {
        ui.label("X:");
        ui.add(egui::DragValue::new(&mut size.x).speed(1.0).range(0.0..=10000.0));
        ui.label("Y:");
        ui.add(egui::DragValue::new(&mut size.y).speed(1.0).range(0.0..=10000.0));
    });
    sprite.custom_size = Some(size);

    ui.separator();

    // Flip
    ui.horizontal(|ui| {
        ui.label("Flip X:");
        ui.checkbox(&mut sprite.flip_x, "");
    });
    ui.horizontal(|ui| {
        ui.label("Flip Y:");
        ui.checkbox(&mut sprite.flip_y, "");
    });

    if let Some(mut visibility) = visibility {
        ui.separator();
        let mut visible = matches!(*visibility, Visibility::Visible | Visibility::Inherited);
        if ui.checkbox(&mut visible, "Visible").changed() {
            *visibility = if visible {
                Visibility::Visible
            } else {
                Visibility::Hidden
            };
        }
    }
}

fn render_agent_editor(ui: &mut egui::Ui, agent: &RuntimeAgentComponent) {
    ui.label("🤖 Runtime Agent");
    ui.separator();

    // Agent ID
    ui.horizontal(|ui| {
        ui.label("ID:");
        ui.label(&agent.id.0);
    });

    // Profile
    ui.horizontal(|ui| {
        ui.label("Profile:");
        ui.label(&agent.profile_id.0);
    });

    // Status
    ui.horizontal(|ui| {
        ui.label("Status:");
        let status_color = match agent.status {
            RuntimeAgentStatus::Idle => egui::Color32::GRAY,
            RuntimeAgentStatus::Thinking => egui::Color32::YELLOW,
            RuntimeAgentStatus::Acting => egui::Color32::GREEN,
            RuntimeAgentStatus::Waiting => egui::Color32::LIGHT_BLUE,
            RuntimeAgentStatus::Suspended => egui::Color32::DARK_GRAY,
            RuntimeAgentStatus::Error { .. } => egui::Color32::RED,
        };
        ui.colored_label(status_color, format!("{:?}", agent.status));
    });

    // Control Mode
    ui.horizontal(|ui| {
        ui.label("Mode:");
        ui.label(format!("{:?}", agent.control_mode));
    });

    ui.separator();

    // Blackboard preview
    ui.collapsing("Blackboard", |ui| {
        for (key, value) in agent.blackboard.snapshot().iter() {
            ui.horizontal(|ui| {
                ui.label(format!("{}:", key));
                ui.label(format!("{:?}", value));
            });
        }
    });

    // Pending actions
    ui.collapsing("Pending Actions", |ui| {
        for action in &agent.pending_actions {
            ui.label(format!("{:?}", action));
        }
    });
}
