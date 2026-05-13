//! Project Panel - Project wizard, recent projects, open/save
//!
//! Provides the project management UI for AgentEdit.
//! Panels: project creation wizard, recent projects list, project info bar.

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use log::info;
use agent_core::project::{
    ProjectManager, ProjectTemplate, RecentProjectsList,
    PROJECT_MANIFEST_FILE,
};
use std::path::PathBuf;
use crate::layout::{LayoutManager, PanelPosition};

pub struct ProjectPanelPlugin;

impl Plugin for ProjectPanelPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ProjectPanelState>()
            .init_resource::<ProjectUiConfig>()
            .add_systems(Update, (
                render_project_panel,
                render_project_info_bar,
                auto_load_recent_projects,
            ));
    }
}

#[derive(Resource)]
pub struct ProjectPanelState {
    pub visible: bool,
    pub project_manager: ProjectManager,
    pub recent_projects: RecentProjectsList,
    pub active_tab: ProjectTab,
    // Wizard state
    pub wizard_name: String,
    pub wizard_path: String,
    pub wizard_template: ProjectTemplate,
    pub wizard_error: Option<String>,
}

impl Default for ProjectPanelState {
    fn default() -> Self {
        let recent = RecentProjectsList::load(RecentProjectsList::default_path()).unwrap_or_default();
        Self {
            visible: false,
            project_manager: ProjectManager::new(),
            recent_projects: recent,
            active_tab: ProjectTab::Recent,
            wizard_name: "MyGame".into(),
            wizard_path: default_project_path(),
            wizard_template: ProjectTemplate::Empty,
            wizard_error: None,
        }
    }
}

#[derive(Resource, Default)]
pub struct ProjectUiConfig {
    pub show_info_bar: bool,
}

#[derive(Default, PartialEq)]
pub enum ProjectTab {
    #[default]
    Recent,
    NewProject,
}

fn default_project_path() -> String {
    std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".into())
}

fn auto_load_recent_projects(
    mut state: ResMut<ProjectPanelState>,
) {
    // Called once on init - handled in Default
    _ = &mut state;
}

fn render_project_panel(
    mut contexts: EguiContexts,
    mut state: ResMut<ProjectPanelState>,
    layout_mgr: Res<LayoutManager>,
) {
    if !layout_mgr.is_visible("project") {
        return;
    }

    let ctx = match contexts.ctx_mut() {
        Ok(ctx) => ctx,
        Err(_) => return,
    };

    let has_project = state.project_manager.has_project_open();

    let (win_w, win_h) = layout_mgr
        .panel_config("project")
        .and_then(|c| match c.position {
            PanelPosition::Floating { width, height, .. } => Some((width, height)),
            _ => None,
        })
        .unwrap_or((520.0, 440.0));

    egui::Window::new(if has_project { "Project" } else { "Welcome" })
        .default_size([win_w, win_h])
        .resizable(true)
        .collapsible(true)
        .show(ctx, |ui| {
            if has_project {
                render_project_overview(ui, &state);
            } else {
                render_welcome_screen(ui, &mut state);
            }
        });
}

fn render_welcome_screen(ui: &mut egui::Ui, state: &mut ProjectPanelState) {
    ui.heading("Welcome to AgentEdit");
    ui.label("Open a recent project or create a new one.");
    ui.separator();

    // Tab bar
    ui.horizontal(|ui| {
        ui.selectable_value(&mut state.active_tab, ProjectTab::Recent, "Recent Projects");
        ui.selectable_value(&mut state.active_tab, ProjectTab::NewProject, "New Project");
    });
    ui.separator();

    match state.active_tab {
        ProjectTab::Recent => render_recent_projects(ui, state),
        ProjectTab::NewProject => render_new_project_wizard(ui, state),
    }
}

fn render_recent_projects(ui: &mut egui::Ui, state: &mut ProjectPanelState) {
    let recent = &mut state.recent_projects;
    recent.prune_missing();

    if recent.projects.is_empty() {
        ui.label("No recent projects.");
        ui.add_space(8.0);
        if ui.button("Create New Project").clicked() {
            state.active_tab = ProjectTab::NewProject;
        }
        return;
    }

    ui.label(format!("{} recent project(s)", recent.projects.len()));
    ui.separator();

    let projects: Vec<_> = recent.projects.clone();

    egui::ScrollArea::vertical()
        .max_height(280.0)
        .show(ui, |ui| {
            for (_i, proj) in projects.iter().enumerate() {
                ui.horizontal(|ui| {
                    // Selection area
                    let response = ui.allocate_response(
                        egui::vec2(ui.available_width() - 30.0, 36.0),
                        egui::Sense::click(),
                    );

                    let rect = response.rect;
                    let is_hovered = response.hovered();

                    // Background
                    if is_hovered {
                        ui.painter().rect_filled(
                            rect, 4.0,
                            egui::Color32::from_rgb(50, 50, 70),
                        );
                    }

                    // Project name
                    ui.painter().text(
                        egui::pos2(rect.left() + 8.0, rect.top() + 8.0),
                        egui::Align2::LEFT_TOP,
                        &proj.name,
                        egui::FontId::proportional(13.0),
                        egui::Color32::WHITE,
                    );

                    // Project path
                    let path_str = proj.path.display().to_string();
                    ui.painter().text(
                        egui::pos2(rect.left() + 8.0, rect.top() + 24.0),
                        egui::Align2::LEFT_TOP,
                        &path_str,
                        egui::FontId::proportional(10.0),
                        egui::Color32::from_gray(140),
                    );

                    if response.clicked() {
                        if proj.path.exists() {
                            match state.project_manager.load_project(
                                proj.path.join(PROJECT_MANIFEST_FILE),
                            ) {
                                Ok(_) => {
                                    state.recent_projects.add(
                                        &proj.name,
                                        &proj.path,
                                        proj.template.clone(),
                                    );
                                    info!("Loaded project: {}", proj.name);
                                }
                                Err(e) => {
                                    error!("Failed to load project: {}", e);
                                    state.wizard_error = Some(format!("Load failed: {}", e));
                                }
                            }
                        } else {
                            state.recent_projects.remove(&proj.path);
                            info!("Removed missing project: {}", proj.name);
                        }
                    }

                    // Remove button
                    let close_btn_rect = egui::Rect::from_min_size(
                        egui::pos2(rect.right() - 26.0, rect.top() + 10.0),
                        egui::vec2(20.0, 20.0),
                    );
                    let close_resp = ui.allocate_rect(close_btn_rect, egui::Sense::click());
                    ui.painter().text(
                        close_btn_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        "✕",
                        egui::FontId::proportional(12.0),
                        if close_resp.hovered() {
                            egui::Color32::from_rgb(239, 68, 68)
                        } else {
                            egui::Color32::from_gray(120)
                        },
                    );
                    if close_resp.clicked() {
                        state.recent_projects.remove(&proj.path);
                    }
                });
                ui.add_space(2.0);
            }
        });
}

fn render_new_project_wizard(ui: &mut egui::Ui, state: &mut ProjectPanelState) {
    ui.heading("Create New Project");
    ui.separator();

    // Project name
    ui.horizontal(|ui| {
        ui.label("Name:");
        ui.text_edit_singleline(&mut state.wizard_name);
    });

    // Project path
    ui.horizontal(|ui| {
        ui.label("Path:");
        ui.text_edit_singleline(&mut state.wizard_path);
    });
    let full_path = PathBuf::from(&state.wizard_path).join(&state.wizard_name);

    ui.label(
        egui::RichText::new(format!("Will create: {}", full_path.display()))
            .size(11.0)
            .color(egui::Color32::from_gray(140)),
    );

    ui.add_space(12.0);

    // Template selection
    ui.label("Template:");
    ui.add_space(4.0);

    egui::ScrollArea::vertical()
        .max_height(180.0)
        .show(ui, |ui| {
            let templates = ProjectTemplate::all();
            for (template, name, desc) in templates.iter() {
                let is_selected = state.wizard_template == *template;
                let response = ui.selectable_label(is_selected, "");

                // Template name and description
                let rect = response.rect;
                ui.painter().text(
                    egui::pos2(rect.left() + 6.0, rect.top() + 4.0),
                    egui::Align2::LEFT_TOP,
                    name,
                    egui::FontId::proportional(13.0),
                    egui::Color32::WHITE,
                );
                ui.painter().text(
                    egui::pos2(rect.left() + 6.0, rect.top() + 22.0),
                    egui::Align2::LEFT_TOP,
                    desc,
                    egui::FontId::proportional(10.0),
                    egui::Color32::from_gray(150),
                );

                // Resolution info
                let (w, h) = template.default_resolution();
                ui.painter().text(
                    egui::pos2(rect.right() - 80.0, rect.top() + 14.0),
                    egui::Align2::CENTER_CENTER,
                    format!("{}x{}", w, h),
                    egui::FontId::proportional(9.0),
                    egui::Color32::from_gray(120),
                );

                if response.clicked() {
                    state.wizard_template = *template;
                }
            }
        });

    ui.add_space(12.0);

    // Error display
    if let Some(ref error) = state.wizard_error {
        ui.colored_label(egui::Color32::from_rgb(239, 68, 68), error);
    }

    // Create button
    let can_create = !state.wizard_name.is_empty() && !state.wizard_path.is_empty();
    ui.add_enabled_ui(can_create, |ui| {
        if ui.button("Create Project").clicked() {
            state.wizard_error = None;
            let path = PathBuf::from(&state.wizard_path);
            match state.project_manager.create_project(
                &state.wizard_name,
                path.join(&state.wizard_name),
            ) {
                Ok(manifest) => {
                    let template_name = format!("{:?}", state.wizard_template);
                    state.recent_projects.add(
                        &manifest.name,
                        path.join(&state.wizard_name),
                        Some(template_name),
                    );
                    info!("Created project: {}", manifest.name);
                    state.visible = false;
                }
                Err(e) => {
                    state.wizard_error = Some(format!("Create failed: {}", e));
                }
            }
        }
    });
}

fn render_project_overview(ui: &mut egui::Ui, state: &ProjectPanelState) {
    let Some(project) = state.project_manager.current_project() else {
        return;
    };

    ui.heading(&project.name);
    ui.label(format!("Engine: {} v{}", project.engine, project.engine_version));
    ui.label(format!("Version: {}", project.version));
    ui.separator();

    // Scenes
    ui.label(egui::RichText::new("Scenes").strong());
    if project.scenes.is_empty() {
        ui.label("  (none)");
    } else {
        for scene in &project.scenes {
            ui.label(format!("  {}", scene));
        }
    }

    ui.add_space(8.0);

    // Agent config
    ui.label(egui::RichText::new("Agent Configuration").strong());
    ui.label(format!("  Provider: {}", project.agent_config.default_llm_provider));
    ui.label(format!("  Model: {}", project.agent_config.default_model));
    ui.label(format!("  Confirmation: {}", project.agent_config.confirmation_level));
    ui.label(format!("  Max Steps: {}", project.agent_config.max_steps));

    ui.add_space(8.0);

    // Template info
    if let Some(template) = project.metadata.get("template") {
        ui.label(format!("Template: {}", template));
    }
    if let Some(res) = project.metadata.get("resolution") {
        ui.label(format!("Resolution: {}", res));
    }
}

fn render_project_info_bar(
    mut contexts: EguiContexts,
    state: Res<ProjectPanelState>,
    config: Res<ProjectUiConfig>,
) {
    if !config.show_info_bar || !state.project_manager.has_project_open() {
        return;
    }

    let ctx = match contexts.ctx_mut() {
        Ok(ctx) => ctx,
        Err(_) => return,
    };

    egui::TopBottomPanel::bottom("project_info_bar")
        .min_height(24.0)
        .show(ctx, |ui| {
            ui.horizontal_centered(|ui| {
                let name = state.project_manager.project_name();
                ui.label(
                    egui::RichText::new(format!("Project: {} | Bevy 0.17", name))
                        .size(11.0)
                        .color(egui::Color32::from_gray(160)),
                );
            });
        });
}

/// Toggle project panel visibility
pub fn toggle_project_panel(state: &mut ResMut<ProjectPanelState>) {
    state.visible = !state.visible;
}

/// Open the project panel
pub fn open_project_panel(state: &mut ResMut<ProjectPanelState>) {
    state.visible = true;
}

/// Show the info bar
pub fn show_project_info_bar(config: &mut ResMut<ProjectUiConfig>, show: bool) {
    config.show_info_bar = show;
}
