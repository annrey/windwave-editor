//! AI Frameworks Panel
//!
//! Panel for managing and interacting with LangChain, LlamaIndex, and DSPy integrations

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts, EguiPrimaryContextPass};
use std::collections::VecDeque;

#[derive(Resource, Default)]
pub struct AIFrameworksState {
    pub openai_api_key: String,
    pub anthropic_api_key: String,
    pub model: String,
    pub temperature: f32,
    pub max_tokens: u32,
    pub selected_tab: AIFrameworkTab,
    pub logs: VecDeque<String>,
    pub initialized: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AIFrameworkTab {
    #[default]
    Config,
    Agents,
    KnowledgeBase,
    Workflows,
    Logs,
}

pub struct AIFrameworksPanelPlugin;

impl Plugin for AIFrameworksPanelPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AIFrameworksState>()
            .add_systems(EguiPrimaryContextPass, render_ai_frameworks_panel);
    }
}

pub fn toggle_ai_frameworks_panel(mut state: ResMut<LayoutCommandQueue>) {
    state.push(LayoutCommand::TogglePanel {
        panel_id: "ai_frameworks".to_string(),
    });
}

fn render_ai_frameworks_panel(
    mut contexts: EguiContexts,
    mut state: ResMut<AIFrameworksState>,
    layout_mgr: Res<LayoutManager>,
) {
    if !layout_mgr.is_visible("ai_frameworks") {
        return;
    }

    let ctx = contexts.ctx_mut();
    let Ok(ctx) = ctx else { return };

    egui::SidePanel::left("ai_frameworks_panel")
        .default_width(420.0)
        .resizable(true)
        .show(ctx, |ui| {
            ui.vertical(|ui| {
                render_header(ui);
                ui.separator();

                egui::ScrollArea::vertical()
                    .auto_shrink([false; 2])
                    .show(ui, |ui| {
                        render_tab_selector(ui, &mut state);
                        ui.separator();
                        render_tab_content(ui, &mut state);
                    });
            });
        });
}

fn render_header(ui: &mut egui::Ui) {
    let indigo = egui::Color32::from_rgb(99, 102, 241);

    ui.horizontal(|ui| {
        ui.add_sized(
            [40.0, 40.0],
            egui::Button::new("\u{1F9E0}")
                .fill(indigo)
                .corner_radius(10),
        );
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("AI Frameworks").strong().size(16.0));
                ui.label(
                    egui::RichText::new("Experimental")
                        .color(egui::Color32::from_rgb(251, 191, 36))
                        .size(10.0),
                );
            });
            ui.label(
                egui::RichText::new("LangChain • LlamaIndex • DSPy")
                    .color(egui::Color32::from_gray(160))
                    .size(11.0),
            );
        });
    });
    ui.add_space(8.0);
}

fn render_tab_selector(ui: &mut egui::Ui, state: &mut AIFrameworksState) {
    ui.horizontal_wrapped(|ui| {
        let tabs = [
            (AIFrameworkTab::Config, "\u{2699} Config"),
            (AIFrameworkTab::Agents, "\u{1F916} Agents"),
            (AIFrameworkTab::KnowledgeBase, "\u{1F4DA} Knowledge"),
            (AIFrameworkTab::Workflows, "\u{26A1} Workflows"),
            (AIFrameworkTab::Logs, "\u{1F4DD} Logs"),
        ];

        for (tab, label) in tabs {
            let is_selected = state.selected_tab == tab;
            if ui.selectable_label(is_selected, label).clicked() {
                state.selected_tab = tab;
            }
        }
    });
}

fn render_tab_content(ui: &mut egui::Ui, state: &mut AIFrameworksState) {
    match state.selected_tab {
        AIFrameworkTab::Config => render_config_tab(ui, state),
        AIFrameworkTab::Agents => render_agents_tab(ui, state),
        AIFrameworkTab::KnowledgeBase => render_knowledge_tab(ui, state),
        AIFrameworkTab::Workflows => render_workflows_tab(ui, state),
        AIFrameworkTab::Logs => render_logs_tab(ui, state),
    }
}

fn render_config_tab(ui: &mut egui::Ui, state: &mut AIFrameworksState) {
    ui.heading("Configuration");
    ui.add_space(16.0);

    ui.collapsing("OpenAI API", |ui| {
        ui.add_space(8.0);
        ui.label("API Key:");
        ui.add(
            egui::TextEdit::singleline(&mut state.openai_api_key)
                .password(true)
                .hint_text("sk-..."),
        );
        ui.add_space(8.0);
    });

    ui.collapsing("Anthropic API (Optional)", |ui| {
        ui.add_space(8.0);
        ui.label("API Key:");
        ui.add(
            egui::TextEdit::singleline(&mut state.anthropic_api_key)
                .password(true)
                .hint_text("sk-ant-..."),
        );
        ui.add_space(8.0);
    });

    ui.collapsing("Model Settings", |ui| {
        ui.add_space(8.0);
        ui.label("Model:");
        egui::ComboBox::from_label("")
            .selected_text(&state.model)
            .show_ui(ui, |ui| {
                for model in &["gpt-4", "gpt-4-turbo", "gpt-3.5-turbo", "claude-3"] {
                    ui.selectable_value(&mut state.model, model.to_string(), *model);
                }
            });

        ui.add_space(8.0);
        ui.label(format!("Temperature: {:.1}", state.temperature));
        ui.add(egui::Slider::new(&mut state.temperature, 0.0..=2.0));

        ui.add_space(8.0);
        ui.label("Max Tokens:");
        ui.add(
            egui::DragValue::new(&mut state.max_tokens)
                .range(256..=128000)
                .speed(64),
        );
        ui.add_space(8.0);
    });

    ui.add_space(16.0);

    ui.horizontal(|ui| {
        if ui.button("\u{1F4BE} Save Config").clicked() {
            state
                .logs
                .push_back("[Config] Configuration saved".to_string());
        }
        if ui.button("\u{1F680} Initialize").clicked() {
            state.initialized = true;
            state
                .logs
                .push_back("[System] AI frameworks initialized".to_string());
        }
    });
}

fn render_agents_tab(ui: &mut egui::Ui, state: &mut AIFrameworksState) {
    ui.heading("AI Agents");
    ui.add_space(12.0);

    ui.label(
        egui::RichText::new("AI Agent management coming soon!")
            .color(egui::Color32::from_gray(160))
            .size(12.0),
    );

    ui.add_space(12.0);
    ui.horizontal(|ui| {
        if ui.button("\u{2795} New Agent").clicked() {
            state
                .logs
                .push_back("[Agent] Agent creation queued".to_string());
        }
    });
}

fn render_knowledge_tab(ui: &mut egui::Ui, state: &mut AIFrameworksState) {
    ui.heading("Knowledge Bases");
    ui.add_space(12.0);

    ui.label(
        egui::RichText::new("Knowledge base management coming soon!")
            .color(egui::Color32::from_gray(160))
            .size(12.0),
    );

    ui.add_space(12.0);
    ui.horizontal(|ui| {
        if ui.button("\u{2795} Create Base").clicked() {
            state
                .logs
                .push_back("[Knowledge] Knowledge base creation queued".to_string());
        }
        if ui.button("\u{1F4C2} Add Documents").clicked() {
            state
                .logs
                .push_back("[Knowledge] Document addition queued".to_string());
        }
    });
}

fn render_workflows_tab(ui: &mut egui::Ui, state: &mut AIFrameworksState) {
    ui.heading("AI Workflows");
    ui.add_space(12.0);

    ui.label(
        egui::RichText::new("Workflow management coming soon!")
            .color(egui::Color32::from_gray(160))
            .size(12.0),
    );

    ui.add_space(12.0);
    ui.horizontal(|ui| {
        if ui.button("\u{2795} New Workflow").clicked() {
            state
                .logs
                .push_back("[Workflow] Workflow creation queued".to_string());
        }
    });
}

fn render_logs_tab(ui: &mut egui::Ui, state: &mut AIFrameworksState) {
    ui.heading("Activity Logs");
    ui.add_space(12.0);

    if state.logs.is_empty() {
        ui.label(
            egui::RichText::new("No logs yet. Start by initializing AI frameworks!")
                .color(egui::Color32::from_gray(160))
                .size(12.0),
        );
    } else {
        for log in state.logs.iter().rev() {
            ui.label(egui::RichText::new(log).size(11.0));
        }
    }

    ui.add_space(16.0);

    if ui.button("\u{1F5D1} Clear Logs").clicked() {
        state.logs.clear();
    }
}

use crate::layout::{LayoutCommand, LayoutCommandQueue, LayoutManager};
