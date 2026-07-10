//! DirectorDesk plugin

use super::hybrid_ui::render_command_palette;
use super::panels::{render_bottom_panel, render_left_panel};
use super::types::DirectorDeskState;
use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiPrimaryContextPass};

pub struct DirectorDeskPlugin;

impl Plugin for DirectorDeskPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DirectorDeskState>()
            .add_systems(EguiPrimaryContextPass, render_left_panel)
            .add_systems(EguiPrimaryContextPass, render_bottom_panel)
            .add_systems(EguiPrimaryContextPass, render_command_palette_system);
    }
}

fn render_command_palette_system(mut contexts: EguiContexts, mut state: ResMut<DirectorDeskState>) {
    let ctx = contexts.ctx_mut();
    let Ok(ctx) = ctx else { return };
    render_command_palette(ctx, &mut state);
}
