//! Debug Panel - Entity list, component inspector, performance monitor
//!
//! Provides runtime debugging tools for the Bevy ECS scene.
//! Useful for both manual inspection and agent observability.

use bevy::prelude::*;
use bevy::sprite::Sprite;
use bevy_egui::{egui, EguiContexts};
use crate::layout::{LayoutManager, PanelPosition};

pub struct DebugPanelPlugin;

impl Plugin for DebugPanelPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DebugPanelState>()
            .add_systems(Update, render_debug_panel);
    }
}

#[derive(Resource)]
pub struct DebugPanelState {
    pub visible: bool,
    pub active_tab: DebugTab,
    pub entity_search: String,
    pub selected_entity: Option<Entity>,
    pub fps_history: Vec<f32>,
    pub frame_count: u64,
}

impl Default for DebugPanelState {
    fn default() -> Self {
        Self {
            visible: false,
            active_tab: DebugTab::Entities,
            entity_search: String::new(),
            selected_entity: None,
            fps_history: Vec::with_capacity(120),
            frame_count: 0,
        }
    }
}

#[derive(Default, PartialEq)]
pub enum DebugTab {
    #[default]
    Entities,
    Performance,
    Log,
}

fn render_debug_panel(
    mut contexts: EguiContexts,
    mut state: ResMut<DebugPanelState>,
    time: Res<Time>,
    entity_query: Query<(Entity, Option<&Name>)>,
    transform_query: Query<&Transform>,
    sprite_query: Query<&Sprite>,
    visibility_query: Query<&Visibility>,
    layout_mgr: Res<LayoutManager>,
) {
    // Track FPS
    state.frame_count += 1;
    let fps = 1.0 / time.delta_secs().max(0.001);
    state.fps_history.push(fps);
    if state.fps_history.len() > 120 {
        state.fps_history.remove(0);
    }

    if !layout_mgr.is_visible("debug") {
        return;
    }

    let ctx = match contexts.ctx_mut() {
        Ok(ctx) => ctx,
        Err(_) => return,
    };

    let (win_w, win_h) = layout_mgr
        .panel_config("debug")
        .and_then(|c| match c.position {
            PanelPosition::Floating { width, height, .. } => Some((width, height)),
            _ => None,
        })
        .unwrap_or((420.0, 500.0));

    egui::Window::new("Debug")
        .default_size([win_w, win_h])
        .resizable(true)
        .show(ctx, |ui| {
            // Tab bar
            ui.horizontal(|ui| {
                ui.selectable_value(&mut state.active_tab, DebugTab::Entities, "Entities");
                ui.selectable_value(&mut state.active_tab, DebugTab::Performance, "Performance");
                ui.selectable_value(&mut state.active_tab, DebugTab::Log, "Log");
            });
            ui.separator();

            match state.active_tab {
                DebugTab::Entities => render_entity_list(ui, &mut state, &entity_query),
                DebugTab::Performance => render_performance_tab(ui, &state),
                DebugTab::Log => render_log_tab(ui),
            }

            // Selected entity details
            if let Some(entity) = state.selected_entity {
                ui.separator();
                render_entity_details(
                    ui, entity,
                    &transform_query, &sprite_query, &visibility_query,
                );
            }
        });
}

fn render_entity_list(
    ui: &mut egui::Ui,
    state: &mut DebugPanelState,
    query: &Query<(Entity, Option<&Name>)>,
) {
    // Search
    ui.horizontal(|ui| {
        ui.label("Filter:");
        ui.text_edit_singleline(&mut state.entity_search);
    });

    ui.add_space(4.0);
    let entity_count = query.iter().count();
    ui.label(format!("Total entities: {}", entity_count));
    ui.separator();

    let search = state.entity_search.to_lowercase();

    egui::ScrollArea::vertical()
        .max_height(200.0)
        .show(ui, |ui| {
            for (entity, name) in query.iter() {
                let display_name = name
                    .map(|n| n.as_str().to_string())
                    .unwrap_or_else(|| format!("Entity {:?}", entity));

                if !search.is_empty() && !display_name.to_lowercase().contains(&search) {
                    continue;
                }

                let is_selected = state.selected_entity == Some(entity);
                if ui.selectable_label(is_selected, &display_name).clicked() {
                    state.selected_entity = Some(entity);
                }
            }
        });
}

fn render_entity_details(
    ui: &mut egui::Ui,
    entity: Entity,
    transform_query: &Query<&Transform>,
    sprite_query: &Query<&Sprite>,
    visibility_query: &Query<&Visibility>,
) {
    ui.label(egui::RichText::new(format!("Entity {:?}", entity)).strong());
    ui.separator();

    // Transform
    if let Ok(transform) = transform_query.get(entity) {
        ui.collapsing("Transform", |ui| {
            let t = transform.translation;
            ui.label(format!("Translation: ({:.2}, {:.2}, {:.2})", t.x, t.y, t.z));
            let (rx, ry, rz) = transform.rotation.to_euler(EulerRot::XYZ);
            ui.label(format!("Rotation: ({:.2}, {:.2}, {:.2})", rx, ry, rz));
            let s = transform.scale;
            ui.label(format!("Scale: ({:.2}, {:.2}, {:.2})", s.x, s.y, s.z));
        });
    }

    // Sprite
    if let Ok(sprite) = sprite_query.get(entity) {
        ui.collapsing("Sprite", |ui| {
            let c = sprite.color.to_linear();
            ui.label(format!("Color: rgba({:.2}, {:.2}, {:.2}, {:.2})", c.red, c.green, c.blue, c.alpha));
            if let Some(size) = sprite.custom_size {
                ui.label(format!("Size: ({:.1}, {:.1})", size.x, size.y));
            }
            ui.label(format!("Flip X: {}, Flip Y: {}", sprite.flip_x, sprite.flip_y));
        });
    }

    // Visibility
    if let Ok(vis) = visibility_query.get(entity) {
        ui.collapsing("Visibility", |ui| {
            ui.label(format!("{:?}", vis));
        });
    }
}

fn render_performance_tab(ui: &mut egui::Ui, state: &DebugPanelState) {
    let current_fps = state.fps_history.last().copied().unwrap_or(0.0);
    let avg_fps = if state.fps_history.is_empty() {
        0.0
    } else {
        state.fps_history.iter().sum::<f32>() / state.fps_history.len() as f32
    };

    ui.label(format!("Current FPS: {:.0}", current_fps));
    ui.label(format!("Average FPS: {:.0}", avg_fps));
    ui.label(format!("Frame: {}", state.frame_count));

    // FPS bar visualization
    if current_fps > 0.0 {
        let ratio = (current_fps / 120.0).clamp(0.0, 1.0);
        ui.add(egui::ProgressBar::new(ratio)
            .text(format!("{:.0} FPS", current_fps))
            .fill(if current_fps >= 55.0 {
                egui::Color32::from_rgb(16, 185, 129)
            } else if current_fps >= 30.0 {
                egui::Color32::from_rgb(245, 158, 11)
            } else {
                egui::Color32::from_rgb(239, 68, 68)
            }),
        );
    }

    // Recent FPS values as inline sparkline
    if !state.fps_history.is_empty() {
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            let recent: Vec<_> = state.fps_history.iter().rev().take(60).copied().collect();
            for &fps in recent.iter() {
                let t = (fps / 120.0).clamp(0.0, 1.0);
                let c = if fps >= 55.0 {
                    egui::Color32::from_rgb(16, 185, 129)
                } else if fps >= 30.0 {
                    egui::Color32::from_rgb(245, 158, 11)
                } else {
                    egui::Color32::from_rgb(239, 68, 68)
                };
                let height = (t * 20.0).max(2.0);
                let (rect, _) = ui.allocate_exact_size(egui::vec2(2.0, height), egui::Sense::hover());
                ui.painter().rect_filled(rect, 0.0, c);
            }
        });
    }

    ui.separator();
    ui.label(egui::RichText::new("Memory").strong());
    ui.label("(install sysinfo crate for detailed metrics)");
}

fn render_log_tab(ui: &mut egui::Ui) {
    ui.label("Agent execution log");
    ui.separator();

    // In future: integrate with director trace entries
    ui.label("(connect to DirectorRuntime.trace_entries for real log)");

    ui.add_space(8.0);
    if ui.button("Clear Log").clicked() {
        // Placeholder
    }
}

pub fn toggle_debug_panel(state: &mut ResMut<DebugPanelState>) {
    state.visible = !state.visible;
}
