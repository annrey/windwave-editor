//! Agent UI - Chat interface and conversation rendering
//!
//! Provides the visual interface for Agent interaction using egui/bevy_egui.

pub mod director_desk;
pub mod runtime_agent_panel;
pub mod hierarchy_panel;
pub mod inspector_panel;
pub mod console_panel;
pub mod shortcuts;
pub mod editor_selection;
pub mod viewport_picking;
pub mod gizmo;
pub mod memory_persistence;
pub mod agent_config_panel;
pub mod diff_preview;
pub mod prefab_browser;
pub mod asset_browser;
pub mod project_panel;
pub mod debug_panel;
pub mod transform_tools;
pub mod play_mode;
pub mod narrative_ui;
pub mod layout;

pub use director_desk::{DirectorDeskState, DirectorDeskPlugin, UserAction, PendingApprovalInfo};
pub use runtime_agent_panel::{RuntimeAgentPanelPlugin, RuntimeAgentPanelState, toggle_runtime_agent_panel};
pub use hierarchy_panel::{HierarchyPanelPlugin, HierarchyState};
pub use inspector_panel::{InspectorPanelPlugin, InspectorState};
pub use console_panel::{ConsolePanelPlugin, ConsoleState, log_console, LogLevel};
pub use shortcuts::{ShortcutsPlugin, ShortcutState};
pub use editor_selection::{EditorSelectionPlugin, EditorSelection, SelectionContext, SelectionChangedEvent, selection_shortcuts};
pub use viewport_picking::{ViewportPickingPlugin, PickingState, Pickable, spawn_pickable_entity};
pub use gizmo::{GizmoPlugin, GizmoState, GizmoMode, set_gizmo_mode, get_gizmo_mode_description};
pub use memory_persistence::{MemoryPersistenceConfig, load_persistent_memory, save_persistent_memory, default_memory_path};
pub use agent_config_panel::{AgentConfigPanelPlugin, AgentConfigState, TokenUsageDisplay, toggle_agent_config, update_token_usage, set_llm_status};
pub use diff_preview::{DiffPreviewPlugin, DiffPreviewState, ExpectedChange, ChangeKind, show_diff_preview, hide_diff_preview, create_change_set_from_description};
pub use prefab_browser::{PrefabBrowserPlugin, PrefabBrowserState, PrefabEntry, PrefabRegistry, toggle_prefab_browser, create_prefab_from_entity};
pub use asset_browser::{AssetBrowserPlugin, AssetBrowserState, AssetEntry, AssetType, toggle_asset_browser, refresh_assets};
pub use project_panel::{ProjectPanelPlugin, ProjectPanelState, ProjectUiConfig, toggle_project_panel, open_project_panel, show_project_info_bar};
pub use debug_panel::{DebugPanelPlugin, DebugPanelState, DebugTab, toggle_debug_panel};
pub use transform_tools::{TransformToolsPlugin, TransformDragState, DragAxis, SnapSettings};
pub use play_mode::{PlayModePlugin, PlayModeState, is_play_mode};
pub use layout::{LayoutManager, LayoutDefinition, PanelConfig, PanelPosition, LayoutCommand};

use agent_core::{Message, MessageType, AgentIdentity, AgentStatus, BaseAgent};
use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts, EguiPlugin};
use std::sync::{Arc, Mutex};

pub struct AgentUiPlugin;

impl Plugin for AgentUiPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(EguiPlugin::default())
            .add_plugins(DirectorDeskPlugin)
            .add_plugins(RuntimeAgentPanelPlugin)
            .add_plugins(HierarchyPanelPlugin)
            .add_plugins(InspectorPanelPlugin)
            .add_plugins(ConsolePanelPlugin)
            .add_plugins(ShortcutsPlugin)
            .add_plugins(EditorSelectionPlugin)
            .add_plugins(ViewportPickingPlugin)
            .add_plugins(GizmoPlugin)
            .add_plugins(AgentConfigPanelPlugin)
            .add_plugins(DiffPreviewPlugin)
            .add_plugins(PrefabBrowserPlugin)
            .add_plugins(AssetBrowserPlugin)
            .add_plugins(ProjectPanelPlugin)
            .add_plugins(DebugPanelPlugin)
            .add_plugins(TransformToolsPlugin)
            .add_plugins(PlayModePlugin)
            .add_plugins(narrative_ui::game_mode_panel::GameModePanelPlugin)
            .init_resource::<ChatState>()
            .init_resource::<UiConfig>()
            .init_resource::<LayoutCommandQueue>()
            .insert_resource(LayoutManager::new(LayoutDefinition::default()))
            .add_systems(Update, (update_agent_state, render_agent_panel).chain())
            .add_systems(Update, (process_layout_commands, render_layout_settings).chain())
            .add_systems(Update, selection_shortcuts);
    }
}

fn update_agent_state(
    runtime: Option<Res<AgentRuntime>>,
    mut chat_state: ResMut<ChatState>,
) {
    if let Some(runtime) = runtime {
        chat_state.agent_state = runtime.current_state();
        chat_state.current_step = runtime.current_step();
        chat_state.progress = runtime.progress();
    }
}

#[derive(Resource, Default)]
pub struct ChatState {
    pub messages: Vec<Message>,
    pub input_text: String,
    pub scroll_to_bottom: bool,
    pub agent_identity: Option<AgentIdentity>,
    pub agent_state: String,
    pub current_step: usize,
    pub progress: Option<f32>,
}

#[derive(Resource)]
pub struct AgentRuntime {
    pub agent: Arc<Mutex<BaseAgent>>,
}

impl AgentRuntime {
    pub fn new(agent: BaseAgent) -> Self {
        Self {
            agent: Arc::new(Mutex::new(agent)),
        }
    }

    pub fn current_state(&self) -> String {
        if let Ok(agent) = self.agent.lock() {
            agent.state_name().to_string()
        } else {
            "Locked".to_string()
        }
    }

    pub fn current_step(&self) -> usize {
        if let Ok(agent) = self.agent.lock() {
            agent.current_step
        } else {
            0
        }
    }

    pub fn progress(&self) -> Option<f32> {
        if let Ok(agent) = self.agent.lock() {
            agent.progress()
        } else {
            None
        }
    }
}

impl ChatState {
    pub fn add_message(&mut self, message: Message) {
        self.messages.push(message);
        if self.messages.len() > 500 {
            self.messages.remove(0);
        }
        self.scroll_to_bottom = true;
    }

    pub fn clear_input(&mut self) {
        self.input_text.clear();
    }
}

#[derive(Resource)]
pub struct UiConfig {
    pub panel_width: f32,
    pub message_spacing: f32,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            panel_width: 380.0,
            message_spacing: 12.0,
        }
    }
}

// ──────────────────────────────────────────────────────────
// Chat panel rendering
// ──────────────────────────────────────────────────────────

fn render_agent_panel(
    mut contexts: EguiContexts,
    mut chat_state: ResMut<ChatState>,
    config: Res<UiConfig>,
    layout_mgr: Res<LayoutManager>,
) {
    if !layout_mgr.is_visible("chat") {
        return;
    }

    let ctx = contexts.ctx_mut();
    let Ok(ctx) = ctx else { return };

    let panel_width = layout_mgr.panel_size("chat").unwrap_or(config.panel_width);

    egui::SidePanel::right("agent_panel")
        .default_width(panel_width)
        .resizable(true)
        .show(ctx, |ui| {
            ui.vertical(|ui| {
                render_agent_header(ui, &chat_state);
                ui.separator();
                render_chat_messages(ui, &chat_state);
                ui.separator();
                render_input_area(ui, &mut chat_state);
            });
        });
}

fn render_agent_header(ui: &mut egui::Ui, chat_state: &ChatState) {
    let indigo = egui::Color32::from_rgb(99, 102, 241);

    ui.horizontal(|ui| {
        ui.add_sized(
            [44.0, 44.0],
            egui::Button::new("\u{25C8}")
                .fill(indigo)
                .corner_radius(10),
        );
        ui.vertical(|ui| {
            ui.label(egui::RichText::new("Game Architect").strong().size(15.0));
            ui.label(
                egui::RichText::new("AI Assistant \u{00B7} v2.1")
                    .color(egui::Color32::from_gray(160))
                    .size(11.0),
            );
        });
    });
    ui.add_space(8.0);

    let status_text = if !chat_state.agent_state.is_empty() {
        format!("\u{25CF} {}", chat_state.agent_state)
    } else {
        match chat_state.agent_identity {
            Some(ref id) => match id.status {
                AgentStatus::Online => "\u{25CF} Online".to_string(),
                AgentStatus::Thinking => "\u{25D0} Thinking...".to_string(),
                AgentStatus::Offline => "\u{25CB} Offline".to_string(),
                AgentStatus::Error => "\u{2717} Error".to_string(),
            },
            None => "\u{25CF} Online".to_string(),
        }
    };
    let green = egui::Color32::from_rgb(16, 185, 129);
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(&status_text).color(green).size(10.0));
        if chat_state.current_step > 0 {
            ui.label(
                egui::RichText::new(format!("Step: {}", chat_state.current_step))
                    .color(egui::Color32::from_gray(160))
                    .size(10.0),
            );
        }
    });

    if let Some(progress) = chat_state.progress {
        ui.add_space(4.0);
        ui.add(egui::ProgressBar::new(progress).show_percentage().fill(indigo));
    }
    ui.add_space(8.0);

    ui.horizontal_wrapped(|ui| {
        for cap in &["Scene", "Code", "Assets", "Bevy"] {
            ui.add(
                egui::Label::new(egui::RichText::new(*cap).size(10.0).color(egui::Color32::from_gray(160)))
                    .sense(egui::Sense::hover()),
            );
            ui.add_space(4.0);
        }
    });
}

fn render_chat_messages(ui: &mut egui::Ui, chat_state: &ChatState) {
    egui::ScrollArea::vertical()
        .auto_shrink([false; 2])
        .stick_to_bottom(true)
        .show(ui, |ui| {
            ui.add_space(8.0);
            for message in &chat_state.messages {
                match message.message_type {
                    MessageType::Agent => render_agent_message(ui, message),
                    MessageType::User => render_user_message(ui, message),
                    MessageType::System => {
                        ui.label(
                            egui::RichText::new(&message.content)
                                .size(11.0)
                                .color(egui::Color32::from_gray(140)),
                        );
                    }
                    MessageType::Thought => {
                        ui.label(
                            egui::RichText::new(&format!("\u{1F4AD} {}", message.content))
                                .size(11.0)
                                .italics(),
                        );
                    }
                    _ => {
                        ui.label(
                            egui::RichText::new(&message.content)
                                .size(11.0)
                                .color(egui::Color32::from_gray(160)),
                        );
                    }
                }
                ui.add_space(12.0);
            }
        });
}

fn render_agent_message(ui: &mut egui::Ui, message: &Message) {
    ui.horizontal(|ui| {
        ui.add_sized(
            [32.0, 32.0],
            egui::Button::new("\u{25C8}")
                .fill(egui::Color32::from_rgb(99, 102, 241))
                .corner_radius(8),
        );
        ui.vertical(|ui| {
            ui.add(egui::Label::new(egui::RichText::new(&message.content).size(13.0)).wrap());
        });
    });
}

fn render_user_message(ui: &mut egui::Ui, message: &Message) {
    let blue = egui::Color32::from_rgb(37, 99, 235);
    ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
        ui.add_sized([32.0, 32.0], egui::Button::new("\u{263A}").fill(blue).corner_radius(8));
        ui.vertical(|ui| {
            let style = ui.style_mut();
            style.visuals.widgets.inactive.weak_bg_fill = blue;
            ui.add(
                egui::Label::new(egui::RichText::new(&message.content).size(13.0).color(egui::Color32::WHITE))
                    .wrap(),
            );
        });
    });
}

fn render_input_area(ui: &mut egui::Ui, chat_state: &mut ChatState) {
    ui.horizontal(|ui| {
        for tag in &["@Player", "#Physics"] {
            ui.add(
                egui::Label::new(egui::RichText::new(*tag).size(10.0).color(egui::Color32::from_gray(128)))
                    .sense(egui::Sense::hover()),
            );
        }
    });
    ui.add_space(8.0);

    ui.horizontal(|ui| {
        ui.add_sized(
            [ui.available_width() - 52.0, 44.0],
            egui::TextEdit::multiline(&mut chat_state.input_text)
                .hint_text("输入指令或查询 Agent...")
                .margin(egui::Margin::same(12)),
        );
        let send_btn = ui.add_sized(
            [44.0, 44.0],
            egui::Button::new("\u{27A4}")
                .fill(egui::Color32::from_rgb(99, 102, 241))
                .corner_radius(10),
        );
        let enter_pressed = ui.input(|i| i.key_pressed(egui::Key::Enter) && !i.modifiers.shift);
        if send_btn.clicked() || enter_pressed {
            if !chat_state.input_text.is_empty() {
                let user_msg = agent_core::Message::new_user(chat_state.input_text.clone());
                chat_state.add_message(user_msg);
                chat_state.clear_input();
            }
        }
    });
    ui.add_space(8.0);

    ui.horizontal_wrapped(|ui| {
        let actions = ["\u{4FEE}\u{590D}\u{8B66}\u{544A}", "\u{4F18}\u{5316}\u{6027}\u{80FD}", "\u{751F}\u{6210}\u{6D4B}\u{8BD5}", "\u{89E3}\u{91CA}\u{4EE3}\u{7801}"];
        for action in &actions {
            if ui.button(egui::RichText::new(*action).size(11.0)).clicked() {
                chat_state.input_text = action.to_string();
            }
            ui.add_space(4.0);
        }
    });
}

// ──────────────────────────────────────────────────────────
// Layout command queue + processing
// ──────────────────────────────────────────────────────────

#[derive(Resource, Default)]
pub struct LayoutCommandQueue {
    pub commands: Vec<LayoutCommand>,
}

impl LayoutCommandQueue {
    pub fn push(&mut self, cmd: LayoutCommand) {
        self.commands.push(cmd);
    }
}

fn process_layout_commands(
    mut queue: ResMut<LayoutCommandQueue>,
    mut layout_mgr: ResMut<LayoutManager>,
    mut chat_state: ResMut<ChatState>,
) {
    if queue.commands.is_empty() {
        return;
    }

    let cmds = std::mem::take(&mut queue.commands);
    for cmd in cmds {
        let description = cmd.execute(&mut layout_mgr);
        chat_state.add_message(Message::new_agent(format!(
            "[Layout] {}",
            description
        )));
    }
}

fn render_layout_settings(
    mut contexts: EguiContexts,
    layout_mgr: Res<LayoutManager>,
    mut queue: ResMut<LayoutCommandQueue>,
) {
    let ctx = contexts.ctx_mut();
    let Ok(ctx) = ctx else { return };

    // Gear icon button in top-right corner
    egui::Area::new("layout_settings_gear".into())
        .fixed_pos(egui::pos2(ctx.screen_rect().right() - 40.0, 4.0))
        .show(ctx, |ui| {
            if ui
                .add_sized([28.0, 28.0], egui::Button::new("\u{2699}"))
                .clicked()
            {
                ui.ctx().memory_mut(|mem| {
                    let open: &mut bool =
                        mem.data.get_temp_mut_or_default(egui::Id::new("layout_settings_open"));
                    *open = !*open;
                });
            }
        });

    let is_open: bool = ctx.memory_mut(|mem| {
        *mem.data
            .get_temp_mut_or_default(egui::Id::new("layout_settings_open"))
    });

    if !is_open {
        return;
    }

    egui::Window::new("Layout Settings")
        .id(egui::Id::new("layout_settings_window"))
        .collapsible(false)
        .resizable(true)
        .default_size([300.0, 400.0])
        .show(ctx, |ui| {
            ui.heading("Editor Layout");

            ui.separator();
            ui.label("Presets");
            ui.horizontal_wrapped(|ui| {
                let presets: Vec<String> = layout_mgr.preset_names.clone();
                for name in &presets {
                    if ui.button(name).clicked() {
                        queue.push(LayoutCommand::ApplyPreset {
                            name: name.to_lowercase(),
                        });
                    }
                }
            });

            ui.separator();
            ui.label("Panels");
            egui::ScrollArea::vertical().show(ui, |ui| {
                let panel_ids: Vec<(String, bool)> = layout_mgr
                    .layout
                    .panels
                    .iter()
                    .map(|p| (p.id.clone(), p.visible))
                    .collect();

                for (id, visible) in &panel_ids {
                    let title = layout_mgr
                        .panel_config(id)
                        .map(|c| c.title.clone())
                        .unwrap_or_else(|| id.clone());

                    let mut vis = *visible;
                    if ui.checkbox(&mut vis, &title).changed() {
                        if vis {
                            queue.push(LayoutCommand::ShowPanel {
                                panel_id: id.clone(),
                            });
                        } else {
                            queue.push(LayoutCommand::HidePanel {
                                panel_id: id.clone(),
                            });
                        }
                    }
                }
            });

            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("Reset").clicked() {
                    queue.push(LayoutCommand::ResetLayout);
                }
            });
        });
}
