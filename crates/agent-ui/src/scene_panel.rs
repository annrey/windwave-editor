//! Scene Aware Panel
//!
//! 场景感知面板组件，展示当前场景上下文信息
//! - 显示场景实体列表
//! - 展示关联任务
//! - 场景快照信息
//! - 快速跳转到场景中的实体

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts, EguiPrimaryContextPass};
use log::debug;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::layout::LayoutManager;

/// 场景实体信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneEntityInfo {
    pub id: u64,
    pub name: String,
    pub entity_type: String,
    pub position: Option<[f32; 3]>,
}

/// 场景摘要信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneSummary {
    pub scene_id: String,
    pub name: String,
    pub description: Option<String>,
    pub entity_count: usize,
    pub task_count: usize,
    pub entities: Vec<SceneEntityInfo>,
}

/// 场景面板状态资源
#[derive(Resource)]
pub struct ScenePanelState {
    pub scenes: HashMap<String, SceneSummary>,
    pub active_scene: Option<String>,
    pub selected_entity: Option<u64>,
    pub show_entity_list: bool,
    pub show_scene_tasks: bool,
    pub search_entity: String,
}

impl Default for ScenePanelState {
    fn default() -> Self {
        Self {
            scenes: HashMap::new(),
            active_scene: None,
            selected_entity: None,
            show_entity_list: true,
            show_scene_tasks: true,
            search_entity: String::new(),
        }
    }
}

impl ScenePanelState {
    pub fn set_active_scene(&mut self, scene_id: Option<String>) {
        self.active_scene = scene_id;
        self.selected_entity = None;
    }

    pub fn update_scene_summary(&mut self, summary: SceneSummary) {
        self.scenes.insert(summary.scene_id.clone(), summary);
    }

    pub fn active_scene_summary(&self) -> Option<&SceneSummary> {
        self.active_scene
            .as_ref()
            .and_then(|id| self.scenes.get(id))
    }
}

/// 场景面板插件
pub struct SceneAwarePanelPlugin;

impl Plugin for SceneAwarePanelPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ScenePanelState>()
            .add_systems(EguiPrimaryContextPass, render_scene_panel);
    }
}

fn render_scene_panel(
    mut contexts: EguiContexts,
    mut state: ResMut<ScenePanelState>,
    layout_mgr: Res<LayoutManager>,
) {
    if !layout_mgr.is_visible("scene") {
        return;
    }

    let ctx = contexts.ctx_mut();
    let Ok(ctx) = ctx else { return };

    let _panel_height = layout_mgr.panel_size("scene").unwrap_or(300.0);

    egui::SidePanel::left("scene_aware_panel")
        .default_width(280.0)
        .resizable(true)
        .min_width(200.0)
        .show(ctx, |ui| {
            ui.vertical(|ui| {
                render_scene_header(ui, &state);
                ui.separator();

                if let Some(ref active_id) = state.active_scene.clone() {
                    if let Some(summary) = state.scenes.get(active_id).cloned() {
                        render_scene_info(ui, &summary);
                        ui.separator();
                        render_entity_section(ui, &mut state, &summary);
                        ui.separator();
                        render_task_section(ui, &summary);
                    } else {
                        ui.label("场景数据不可用");
                    }
                } else {
                    ui.vertical_centered(|ui| {
                        ui.add_space(40.0);
                        ui.label(egui::RichText::new("\u{1F3DE}").size(32.0));
                        ui.add_space(8.0);
                        ui.label("未选中场景");
                        ui.label(
                            egui::RichText::new("选择一个场景查看详情")
                                .size(11.0)
                                .color(egui::Color32::from_gray(140)),
                        );
                    });
                }
            });
        });
}

fn render_scene_header(ui: &mut egui::Ui, state: &ScenePanelState) {
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new("\u{1F3DE} 场景感知")
                .strong()
                .size(14.0),
        );

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.checkbox(&mut false, "");
        });
    });

    if let Some(ref active_id) = state.active_scene {
        let count = state.scenes.len();
        ui.add_space(4.0);
        ui.label(
            egui::RichText::new(format!(
                "活动场景: {} (共 {} 个)",
                &active_id[..8.min(active_id.len())],
                count
            ))
            .size(10.0)
            .color(egui::Color32::from_gray(140)),
        );
    }
}

fn render_scene_info(ui: &mut egui::Ui, summary: &SceneSummary) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(&summary.name).strong().size(13.0));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(
                egui::RichText::new(format!("{} 实体", summary.entity_count))
                    .size(10.0)
                    .color(egui::Color32::from_gray(140)),
            );
        });
    });

    if let Some(ref desc) = summary.description {
        ui.add_space(4.0);
        ui.label(
            egui::RichText::new(desc)
                .size(11.0)
                .color(egui::Color32::from_gray(160)),
        );
    }

    ui.add_space(8.0);

    let total = summary.entity_count.max(1);
    let pct = summary.entity_count as f32 / total as f32;
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("实体密度").size(10.0));
        ui.add(
            egui::ProgressBar::new(pct)
                .fill(egui::Color32::from_rgb(16, 185, 129))
                .desired_width(120.0),
        );
    });
}

fn render_entity_section(ui: &mut egui::Ui, state: &mut ScenePanelState, summary: &SceneSummary) {
    ui.horizontal(|ui| {
        let label = if state.show_entity_list {
            "\u{25BC} 实体列表"
        } else {
            "\u{25B6} 实体列表"
        };
        if ui.button(egui::RichText::new(label).size(12.0)).clicked() {
            state.show_entity_list = !state.show_entity_list;
        }
    });

    if !state.show_entity_list {
        return;
    }

    ui.add_space(4.0);

    ui.horizontal(|ui| {
        ui.add_sized(
            [ui.available_width(), 20.0],
            egui::TextEdit::singleline(&mut state.search_entity)
                .hint_text("搜索实体...")
                .font(egui::TextStyle::Body),
        );
    });

    ui.add_space(4.0);

    egui::ScrollArea::vertical()
        .max_height(200.0)
        .show(ui, |ui| {
            let entities = if state.search_entity.is_empty() {
                summary.entities.clone()
            } else {
                let q = state.search_entity.to_lowercase();
                summary
                    .entities
                    .iter()
                    .filter(|e| {
                        e.name.to_lowercase().contains(&q)
                            || e.entity_type.to_lowercase().contains(&q)
                    })
                    .cloned()
                    .collect()
            };

            if entities.is_empty() {
                ui.label(
                    egui::RichText::new("没有找到匹配的实体")
                        .size(11.0)
                        .color(egui::Color32::from_gray(140)),
                );
                return;
            }

            for entity in &entities {
                let is_selected = state.selected_entity == Some(entity.id);
                let resp = ui.selectable_label(
                    is_selected,
                    egui::RichText::new(format!("\u{1F4E6} {}", entity.name)).size(12.0),
                );
                if resp.clicked() {
                    state.selected_entity = Some(entity.id);
                }

                resp.context_menu(|ui| {
                    if ui.button("聚焦实体").clicked() {
                        ui.close();
                    }
                    if ui.button("查看属性").clicked() {
                        ui.close();
                    }
                });

                let type_color = match entity.entity_type.as_str() {
                    "PlayerSpawn" => egui::Color32::from_rgb(59, 130, 246),
                    "NPC" => egui::Color32::from_rgb(16, 185, 129),
                    "Boss" => egui::Color32::from_rgb(239, 68, 68),
                    "Terrain" => egui::Color32::from_rgb(139, 92, 46),
                    "Light" => egui::Color32::from_rgb(251, 191, 36),
                    "Pickup" => egui::Color32::from_rgb(168, 85, 247),
                    "Container" => egui::Color32::from_rgb(245, 158, 11),
                    "Particle" => egui::Color32::from_rgb(236, 72, 153),
                    _ => egui::Color32::from_gray(160),
                };

                ui.horizontal(|ui| {
                    ui.add_space(20.0);
                    ui.label(
                        egui::RichText::new(format!("[{}]", entity.entity_type))
                            .size(10.0)
                            .color(type_color),
                    );
                    if let Some(pos) = entity.position {
                        ui.label(
                            egui::RichText::new(format!(
                                "({:.0}, {:.0}, {:.0})",
                                pos[0], pos[1], pos[2]
                            ))
                            .size(10.0)
                            .color(egui::Color32::from_gray(130)),
                        );
                    }
                });

                ui.add_space(2.0);
            }
        });
}

fn render_task_section(ui: &mut egui::Ui, summary: &SceneSummary) {
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new("\u{1F4CB} 关联任务")
                .strong()
                .size(12.0),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(
                egui::RichText::new(format!("{} 个任务", summary.task_count))
                    .size(10.0)
                    .color(egui::Color32::from_gray(140)),
            );
        });
    });

    if summary.task_count == 0 {
        ui.add_space(4.0);
        ui.label(
            egui::RichText::new("当前场景没有关联任务")
                .size(11.0)
                .color(egui::Color32::from_gray(150)),
        );
        if ui
            .button(egui::RichText::new("+ 创建任务").size(11.0))
            .clicked()
        {
            debug!("Create task for scene: {}", summary.scene_id);
        }
    }
}
