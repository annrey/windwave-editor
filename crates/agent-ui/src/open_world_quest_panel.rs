use agent_core::application::{OpenWorldReplayWorldState, OpenWorldRuntimeState};
use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts, EguiPrimaryContextPass};

use crate::world_timeline_panel::WorldTimelinePanelState;

pub struct OpenWorldQuestPanelPlugin;

impl Plugin for OpenWorldQuestPanelPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<OpenWorldQuestPanelState>()
            .add_systems(Update, sync_open_world_quest_panel_from_timeline_replay)
            .add_systems(EguiPrimaryContextPass, render_open_world_quest_panel);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenWorldQuestRow {
    pub quest_id: String,
    pub state: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenWorldQuestObjectiveRow {
    pub objective_id: String,
    pub state: String,
}

#[derive(Resource, Debug, Clone, Default, PartialEq, Eq)]
pub struct OpenWorldQuestPanelState {
    pub visible: bool,
    pub quest_rows: Vec<OpenWorldQuestRow>,
    pub objective_rows: Vec<OpenWorldQuestObjectiveRow>,
}

impl OpenWorldQuestPanelState {
    pub fn sync_from_runtime(&mut self, runtime: &OpenWorldRuntimeState) {
        self.quest_rows = runtime
            .quest_states
            .iter()
            .map(|(quest_id, state)| OpenWorldQuestRow {
                quest_id: quest_id.clone(),
                state: format!("{state:?}"),
            })
            .collect();

        self.objective_rows = runtime
            .quest_objectives
            .iter()
            .map(|(objective_id, objective)| OpenWorldQuestObjectiveRow {
                objective_id: objective_id.clone(),
                state: format!("{:?}", objective.state),
            })
            .collect();
    }

    pub fn sync_from_replay_world_state(&mut self, world_state: &OpenWorldReplayWorldState) {
        self.quest_rows = world_state
            .quest_states
            .iter()
            .map(|(quest_id, state)| OpenWorldQuestRow {
                quest_id: quest_id.clone(),
                state: state.clone(),
            })
            .collect();

        self.objective_rows = world_state
            .quest_objectives
            .iter()
            .map(|(objective_id, state)| OpenWorldQuestObjectiveRow {
                objective_id: objective_id.clone(),
                state: state.clone(),
            })
            .collect();
    }
}

pub fn toggle_open_world_quest_panel(mut state: ResMut<OpenWorldQuestPanelState>) {
    state.visible = !state.visible;
}

pub fn sync_open_world_quest_panel_from_timeline_replay(
    timeline: Res<WorldTimelinePanelState>,
    mut quest_panel: ResMut<OpenWorldQuestPanelState>,
) {
    let Some(world_state) = timeline.replay_cursor_world_state() else {
        return;
    };

    quest_panel.sync_from_replay_world_state(world_state);
}

fn render_open_world_quest_panel(mut contexts: EguiContexts, state: Res<OpenWorldQuestPanelState>) {
    if !state.visible {
        return;
    }

    let ctx = match contexts.ctx_mut() {
        Ok(ctx) => ctx,
        Err(_) => return,
    };

    egui::Window::new("Open World Quest")
        .default_size([360.0, 420.0])
        .resizable(true)
        .show(ctx, |ui| {
            ui.label(egui::RichText::new("Quests").strong());
            if state.quest_rows.is_empty() {
                ui.label("none");
            } else {
                for row in &state.quest_rows {
                    ui.label(format!("{}: {}", row.quest_id, row.state));
                }
            }

            ui.separator();
            ui.label(egui::RichText::new("Objectives").strong());
            if state.objective_rows.is_empty() {
                ui.label("none");
            } else {
                for row in &state.objective_rows {
                    ui.label(format!("{}: {}", row.objective_id, row.state));
                }
            }
        });
}
