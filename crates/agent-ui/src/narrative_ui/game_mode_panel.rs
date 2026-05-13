//! Game Mode Panel — Mode selection, configuration, preview.
//!
//! Shows 4 presets + custom mode option.
//! Displays current active mode with agent status.

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use agent_core::game_mode::{GameModeType, GameModeState, NarrativeAgentRole};
use crate::layout::{LayoutManager, PanelPosition};

pub struct GameModePanelPlugin;

impl Plugin for GameModePanelPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GameModePanel>()
            .add_systems(Update, render_game_mode_panel);
    }
}

#[derive(Resource)]
pub struct GameModePanel {
    pub visible: bool,
    pub game_state: GameModeState,
}

impl Default for GameModePanel {
    fn default() -> Self {
        Self { visible: false, game_state: GameModeState::default() }
    }
}

fn render_game_mode_panel(
    mut contexts: EguiContexts, mut panel: ResMut<GameModePanel>,
    layout_mgr: Res<LayoutManager>,
) {
    if !layout_mgr.is_visible("game_mode") { return; }
    let ctx = match contexts.ctx_mut() { Ok(c) => c, Err(_) => return };

    let (win_w, win_h) = layout_mgr
        .panel_config("game_mode")
        .and_then(|c| match c.position {
            PanelPosition::Floating { width, height, .. } => Some((width, height)),
            _ => None,
        })
        .unwrap_or((420.0, 380.0));

    egui::Window::new("Game Mode").default_size([win_w, win_h]).resizable(true).show(ctx, |ui| {
        if panel.game_state.is_active {
            render_active_mode(ui, &panel.game_state);
        } else {
            render_mode_selector(ui, &mut panel.game_state);
        }
    });

    if panel.game_state.is_active {
        egui::TopBottomPanel::top("game_mode_bar").min_height(24.0).show(ctx, |ui| {
            ui.horizontal_centered(|ui| {
                let name = panel.game_state.current.as_ref().map_or("Unknown", |m| m.name.as_str());
                ui.label(egui::RichText::new(format!("Game Mode: {} | Round {}", name, panel.game_state.round))
                    .size(11.0).color(egui::Color32::from_rgb(16, 185, 129)));
            });
        });
    }
}

fn render_mode_selector(ui: &mut egui::Ui, state: &mut GameModeState) {
    ui.heading("Select Game Mode");
    ui.separator();

    let presets = GameModeType::all_presets();
    egui::ScrollArea::vertical().show(ui, |ui| {
        for mode in &presets {
            let response = ui.selectable_label(false, "");
            let rect = response.rect;
            // Name + description
            ui.painter().text(egui::pos2(rect.left() + 6.0, rect.top() + 4.0),
                egui::Align2::LEFT_TOP, &mode.name, egui::FontId::proportional(13.0), egui::Color32::WHITE);
            ui.painter().text(egui::pos2(rect.left() + 6.0, rect.top() + 22.0),
                egui::Align2::LEFT_TOP, &mode.description, egui::FontId::proportional(10.0), egui::Color32::from_gray(150));
            // Agent count
            ui.painter().text(egui::pos2(rect.right() - 60.0, rect.top() + 14.0),
                egui::Align2::CENTER_CENTER,
                format!("{} agents", mode.enabled_agents.len()),
                egui::FontId::proportional(9.0), egui::Color32::from_gray(120));

            if response.clicked() {
                state.activate(mode.clone());
                info!("Activated game mode: {}", mode.name);
            }
        }
    });
}

fn render_active_mode(ui: &mut egui::Ui, state: &GameModeState) {
    if let Some(mode) = &state.current {
        ui.heading(&mode.name);
        ui.label(format!("Style: {}", mode.narrative_style));
        ui.label(format!("Round: {}", state.round));
        ui.separator();

        ui.label(egui::RichText::new("Active Agents").strong());
        for agent in &mode.enabled_agents {
            ui.label(format!("  {}", agent_label(*agent)));
        }

        ui.separator();

        if ui.button("Deactivate").clicked() {
            // state is behind ResMut, need a separate deactivation system
            info!("Deactivate requested");
        }
    }
}

fn agent_label(role: NarrativeAgentRole) -> &'static str {
    match role {
        NarrativeAgentRole::Narrator => "Narrator — Storytelling",
        NarrativeAgentRole::WorldKeeper => "WorldKeeper — Consistency",
        NarrativeAgentRole::NPCDirector => "NPCDirector — Characters",
        NarrativeAgentRole::RuleArbiter => "RuleArbiter — Rules",
        NarrativeAgentRole::DramaCurator => "DramaCurator — Drama",
    }
}

pub fn toggle_game_mode_panel(panel: &mut ResMut<GameModePanel>) {
    panel.visible = !panel.visible;
}
