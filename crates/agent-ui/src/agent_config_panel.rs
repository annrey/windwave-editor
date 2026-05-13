//! Agent Configuration Panel - Phase 3.6
//!
//! Provides UI for configuring Agent LLM parameters and behavior.

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use crate::layout::{LayoutManager, LayoutCommand, PanelPosition};
use crate::LayoutCommandQueue;

pub struct AgentConfigPanelPlugin;

impl Plugin for AgentConfigPanelPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AgentConfigState>()
            .add_systems(Update, render_agent_config_panel);
    }
}

/// Configuration state for the Agent
#[derive(Resource)]
pub struct AgentConfigState {
    pub visible: bool,
    pub llm_provider: String,
    pub llm_model: String,
    pub llm_status: String,
    pub temperature: f32,
    pub max_steps: usize,
    pub max_tokens: u32,
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
}

impl Default for AgentConfigState {
    fn default() -> Self {
        Self {
            visible: false,
            llm_provider: "openai".to_string(),
            llm_model: "gpt-4o".to_string(),
            llm_status: "Ready".to_string(),
            temperature: 0.7,
            max_steps: 10,
            max_tokens: 4096,
            prompt_tokens: 0,
            completion_tokens: 0,
        }
    }
}

/// Token usage display
#[derive(Default, Debug, Clone)]
pub struct TokenUsageDisplay {
    pub prompt: u32,
    pub completion: u32,
    pub total: u32,
}

impl AgentConfigState {
    /// Calculate total tokens used
    pub fn total_tokens(&self) -> u32 {
        self.prompt_tokens + self.completion_tokens
    }

    /// Get provider display name
    pub fn provider_display(&self) -> &str {
        match self.llm_provider.as_str() {
            "openai" => "OpenAI",
            "claude" => "Anthropic Claude",
            "local" => "Local/Ollama",
            _ => &self.llm_provider,
        }
    }

    /// Update token usage
    pub fn update_usage(&mut self, prompt: u32, completion: u32) {
        self.prompt_tokens = prompt;
        self.completion_tokens = completion;
    }

    /// Reset token counters
    pub fn reset_usage(&mut self) {
        self.prompt_tokens = 0;
        self.completion_tokens = 0;
    }
}

fn render_agent_config_panel(
    mut contexts: EguiContexts,
    mut state: ResMut<AgentConfigState>,
    layout_mgr: Res<LayoutManager>,
    mut layout_queue: ResMut<LayoutCommandQueue>,
) {
    let ctx = match contexts.ctx_mut() {
        Ok(ctx) => ctx,
        Err(_) => return,
    };

    if !layout_mgr.is_visible("agent_config") {
        return;
    }

    let (win_w, win_h) = layout_mgr
        .panel_config("agent_config")
        .and_then(|c| match c.position {
            PanelPosition::Floating { width, height, .. } => Some((width, height)),
            _ => None,
        })
        .unwrap_or((400.0, 500.0));

    egui::Window::new("🔧 Agent Configuration")
        .default_size([win_w, win_h])
        .resizable(true)
        .collapsible(true)
        .show(ctx, |ui| {
            ui.heading("LLM Settings");
            ui.separator();

            // Provider selection
            ui.horizontal(|ui| {
                ui.label("Provider:");
                egui::ComboBox::from_id_salt("llm_provider")
                    .selected_text(state.provider_display())
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut state.llm_provider, "openai".to_string(), "OpenAI");
                        ui.selectable_value(&mut state.llm_provider, "claude".to_string(), "Claude");
                        ui.selectable_value(&mut state.llm_provider, "local".to_string(), "Local");
                    });
            });

            ui.add_space(8.0);

            // Model input
            ui.horizontal(|ui| {
                ui.label("Model:");
                ui.text_edit_singleline(&mut state.llm_model);
            });

            ui.add_space(8.0);

            // Temperature slider
            ui.horizontal(|ui| {
                ui.label("Temperature:");
                ui.add(
                    egui::Slider::new(&mut state.temperature, 0.0..=2.0)
                        .show_value(true)
                        .text(""),
                );
            });

            ui.add_space(8.0);

            // Max tokens
            ui.horizontal(|ui| {
                ui.label("Max Tokens:");
                ui.add(egui::DragValue::new(&mut state.max_tokens).speed(100));
            });

            ui.add_space(8.0);

            // Max steps
            ui.horizontal(|ui| {
                ui.label("Max Steps:");
                ui.add(egui::DragValue::new(&mut state.max_steps).speed(1));
            });

            ui.add_space(16.0);
            ui.separator();
            ui.heading("Token Usage");
            ui.separator();

            // Token usage display
            ui.horizontal(|ui| {
                ui.label("Prompt:");
                ui.label(format!("{}", state.prompt_tokens));
            });
            ui.horizontal(|ui| {
                ui.label("Completion:");
                ui.label(format!("{}", state.completion_tokens));
            });
            ui.horizontal(|ui| {
                ui.label("Total:");
                ui.label(format!("{}", state.total_tokens()));
            });

            ui.add_space(8.0);

            if ui.button("Reset Counters").clicked() {
                state.reset_usage();
            }

            ui.add_space(16.0);
            ui.separator();
            ui.heading("Status");
            ui.separator();

            // Status display
            let status_color = match state.llm_status.as_str() {
                "Ready" => egui::Color32::GREEN,
                "Error" => egui::Color32::RED,
                "Thinking" => egui::Color32::YELLOW,
                _ => egui::Color32::GRAY,
            };
            ui.colored_label(status_color, format!("Status: {}", state.llm_status));

            ui.add_space(16.0);

            // Close button
            if ui.button("Close").clicked() {
                layout_queue.push(LayoutCommand::HidePanel { panel_id: "agent_config".to_string() });
            }
        });
}

/// Toggle config panel visibility
pub fn toggle_agent_config(state: &mut ResMut<AgentConfigState>) {
    state.visible = !state.visible;
}

/// Update token usage from LLM response
pub fn update_token_usage(state: &mut ResMut<AgentConfigState>, prompt: u32, completion: u32) {
    state.update_usage(prompt, completion);
}

/// Set LLM status
pub fn set_llm_status(state: &mut ResMut<AgentConfigState>, status: &str) {
    state.llm_status = status.to_string();
}
