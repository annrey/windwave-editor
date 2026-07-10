//! Skill Panel
//!
//! 技能管理面板组件，提供游戏技能的查看、搜索和执行界面
//! - 展示已注册技能列表
//! - 按类别筛选技能
//! - 查看技能详情和参数
//! - 在场景上下文中执行技能

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts, EguiPrimaryContextPass};
use log::info;
use std::collections::HashMap;

use crate::layout::LayoutManager;

/// 技能信息
#[derive(Debug, Clone)]
pub struct SkillInfo {
    pub name: String,
    pub description: String,
    pub category: String,
    pub parameters: Vec<SkillParamInfo>,
    pub enabled: bool,
}

/// 技能参数信息
#[derive(Debug, Clone)]
pub struct SkillParamInfo {
    pub name: String,
    pub param_type: String,
    pub required: bool,
    pub description: String,
}

/// 技能类别
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkillCategoryFilter {
    All,
    Entity,
    Scene,
    Terrain,
    Utility,
}

impl SkillCategoryFilter {
    pub fn display(&self) -> &str {
        match self {
            SkillCategoryFilter::All => "全部",
            SkillCategoryFilter::Entity => "实体",
            SkillCategoryFilter::Scene => "场景",
            SkillCategoryFilter::Terrain => "地形",
            SkillCategoryFilter::Utility => "工具",
        }
    }

    pub fn matches(&self, category: &str) -> bool {
        if self == &SkillCategoryFilter::All {
            return true;
        }
        category.to_lowercase() == format!("{:?}", self).to_lowercase()
    }
}

/// 技能面板状态资源
#[derive(Resource)]
pub struct SkillPanelState {
    pub skills: HashMap<String, SkillInfo>,
    pub selected_skill: Option<String>,
    pub filter_category: SkillCategoryFilter,
    pub search_query: String,
    pub show_skill_detail: bool,
    pub execution_log: Vec<String>,
    pub max_log_entries: usize,
}

impl Default for SkillPanelState {
    fn default() -> Self {
        Self {
            skills: HashMap::new(),
            selected_skill: None,
            filter_category: SkillCategoryFilter::All,
            search_query: String::new(),
            show_skill_detail: true,
            execution_log: Vec::new(),
            max_log_entries: 50,
        }
    }
}

impl SkillPanelState {
    pub fn register_skill(
        &mut self,
        name: String,
        description: String,
        category: String,
        parameters: Vec<SkillParamInfo>,
    ) {
        self.skills.insert(
            name.clone(),
            SkillInfo {
                name,
                description,
                category,
                parameters,
                enabled: true,
            },
        );
    }

    pub fn log_execution(&mut self, log: String) {
        self.execution_log.push(log);
        if self.execution_log.len() > self.max_log_entries {
            self.execution_log.remove(0);
        }
    }

    pub fn filtered_skills(&self) -> Vec<&SkillInfo> {
        self.skills
            .values()
            .filter(|s| self.filter_category.matches(&s.category))
            .filter(|s| {
                if self.search_query.is_empty() {
                    return true;
                }
                let q = self.search_query.to_lowercase();
                s.name.to_lowercase().contains(&q) || s.description.to_lowercase().contains(&q)
            })
            .collect()
    }
}

/// 技能面板插件
pub struct SkillPanelPlugin;

impl Plugin for SkillPanelPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SkillPanelState>()
            .add_systems(EguiPrimaryContextPass, render_skill_panel);
    }
}

fn render_skill_panel(
    mut contexts: EguiContexts,
    mut state: ResMut<SkillPanelState>,
    layout_mgr: Res<LayoutManager>,
) {
    if !layout_mgr.is_visible("skills") {
        return;
    }

    let ctx = contexts.ctx_mut();
    let Ok(ctx) = ctx else { return };

    egui::SidePanel::right("skill_panel")
        .default_width(320.0)
        .resizable(true)
        .min_width(240.0)
        .show(ctx, |ui| {
            ui.vertical(|ui| {
                render_skill_header(ui, &mut state);
                ui.separator();

                render_skill_filters(ui, &mut state);
                ui.separator();

                let total_height = ui.available_height();
                let list_height = if state.show_skill_detail {
                    total_height * 0.55
                } else {
                    total_height - 100.0
                };

                render_skill_list(ui, &mut state, list_height);

                if state.show_skill_detail {
                    ui.separator();
                    render_skill_detail(ui, &mut state);
                    ui.separator();
                    render_execution_log(ui, &state);
                }
            });
        });
}

fn render_skill_header(ui: &mut egui::Ui, state: &mut SkillPanelState) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("\u{2699} 技能管理").strong().size(14.0));

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.checkbox(&mut state.show_skill_detail, "详情");
            if ui
                .button(egui::RichText::new("\u{1F4BE}").size(12.0))
                .on_hover_text("导出技能")
                .clicked()
            {
                info!("Export skills requested");
            }
        });
    });

    let total = state.skills.len();
    let visible = state.filtered_skills().len();
    ui.add_space(2.0);
    ui.label(
        egui::RichText::new(format!("{}/{} 技能可用", visible, total))
            .size(10.0)
            .color(egui::Color32::from_gray(140)),
    );
}

fn render_skill_filters(ui: &mut egui::Ui, state: &mut SkillPanelState) {
    ui.horizontal(|ui| {
        ui.add_sized(
            [ui.available_width() * 0.5, 22.0],
            egui::TextEdit::singleline(&mut state.search_query)
                .hint_text("\u{1F50D} 搜索技能...")
                .font(egui::TextStyle::Body),
        );

        let cats = [
            SkillCategoryFilter::All,
            SkillCategoryFilter::Entity,
            SkillCategoryFilter::Scene,
            SkillCategoryFilter::Terrain,
            SkillCategoryFilter::Utility,
        ];

        egui::ComboBox::from_id_salt("skill_category_filter")
            .selected_text(state.filter_category.display())
            .width(ui.available_width())
            .show_ui(ui, |ui| {
                for cat in &cats {
                    if ui
                        .selectable_label(state.filter_category == *cat, cat.display())
                        .clicked()
                    {
                        state.filter_category = cat.clone();
                    }
                }
            });
    });
}

fn render_skill_list(ui: &mut egui::Ui, state: &mut SkillPanelState, max_height: f32) {
    let skills: Vec<SkillInfo> = state.filtered_skills().into_iter().cloned().collect();
    let mut skills_sorted = skills;
    skills_sorted.sort_by(|a, b| a.name.cmp(&b.name));

    egui::ScrollArea::vertical()
        .max_height(max_height)
        .show(ui, |ui| {
            if skills_sorted.is_empty() {
                ui.vertical_centered(|ui| {
                    ui.add_space(30.0);
                    ui.label(egui::RichText::new("\u{2699}").size(24.0));
                    ui.add_space(4.0);
                    ui.label("没有找到技能");
                    ui.label(
                        egui::RichText::new("请检查筛选条件或搜索关键词")
                            .size(10.0)
                            .color(egui::Color32::from_gray(140)),
                    );
                });
                return;
            }

            for skill in &skills_sorted {
                let is_selected = state.selected_skill.as_deref() == Some(&skill.name);
                let resp =
                    ui.selectable_label(is_selected, egui::RichText::new(&skill.name).size(12.0));
                if resp.clicked() {
                    state.selected_skill = Some(skill.name.clone());
                    state.show_skill_detail = true;
                }

                let _skill_name = skill.name.clone();
                resp.context_menu(|ui| {
                    if ui.button("\u{25B6} 执行技能").clicked() {
                        ui.close();
                    }
                    if ui.button("\u{2139} 查看详情").clicked() {
                        ui.close();
                    }
                });

                let category_color = match skill.category.as_str() {
                    "entity" => egui::Color32::from_rgb(59, 130, 246),
                    "scene" => egui::Color32::from_rgb(16, 185, 129),
                    "terrain" => egui::Color32::from_rgb(139, 92, 46),
                    "utility" => egui::Color32::from_rgb(251, 191, 36),
                    _ => egui::Color32::from_gray(140),
                };

                ui.horizontal(|ui| {
                    ui.add_space(16.0);
                    ui.label(
                        egui::RichText::new(format!("[{}]", skill.category))
                            .size(10.0)
                            .color(category_color),
                    );
                    ui.label(
                        egui::RichText::new(&skill.description)
                            .size(10.0)
                            .color(egui::Color32::from_gray(140)),
                    );
                });
            }
        });
}

fn render_skill_detail(ui: &mut egui::Ui, state: &mut SkillPanelState) {
    let Some(ref skill_name) = state.selected_skill else {
        ui.vertical_centered(|ui| {
            ui.label(
                egui::RichText::new("选择一个技能查看详情")
                    .size(11.0)
                    .color(egui::Color32::from_gray(140)),
            );
        });
        return;
    };

    let Some(skill) = state.skills.get(skill_name).cloned() else {
        ui.label("技能数据不可用");
        return;
    };

    ui.heading(egui::RichText::new(&skill.name).size(13.0));
    ui.add_space(4.0);
    ui.label(
        egui::RichText::new(&skill.description)
            .size(11.0)
            .color(egui::Color32::from_gray(160)),
    );
    ui.add_space(8.0);

    let status_color = if skill.enabled {
        egui::Color32::from_rgb(16, 185, 129)
    } else {
        egui::Color32::from_rgb(239, 68, 68)
    };
    ui.label(
        egui::RichText::new(format!(
            "\u{25CF} {}",
            if skill.enabled {
                "已启用"
            } else {
                "已禁用"
            }
        ))
        .size(11.0)
        .color(status_color),
    );

    if !skill.parameters.is_empty() {
        ui.add_space(8.0);
        ui.label(egui::RichText::new("\u{1F4DD} 参数").strong().size(12.0));
        ui.add_space(4.0);

        for param in &skill.parameters {
            ui.horizontal(|ui| {
                let required_mark = if param.required { "*" } else { "" };
                ui.label(
                    egui::RichText::new(format!("{}{}", param.name, required_mark)).size(11.0),
                );
                ui.label(
                    egui::RichText::new(format!("<{}>", param.param_type))
                        .size(10.0)
                        .color(egui::Color32::from_rgb(139, 92, 46)),
                );
            });
            ui.label(
                egui::RichText::new(format!("  {}", param.description))
                    .size(10.0)
                    .color(egui::Color32::from_gray(140)),
            );
        }
    } else {
        ui.add_space(4.0);
        ui.label(
            egui::RichText::new("此技能无需参数")
                .size(10.0)
                .color(egui::Color32::from_gray(140)),
        );
    }

    ui.add_space(12.0);

    ui.horizontal(|ui| {
        let execute_btn = egui::Button::new(egui::RichText::new("\u{25B6} 执行").size(13.0))
            .fill(egui::Color32::from_rgb(16, 185, 129))
            .min_size(egui::vec2(80.0, 28.0));
        if ui.add(execute_btn).clicked() {
            state.log_execution(format!("执行技能: {}", skill.name));
        }

        if ui
            .button(egui::RichText::new("\u{1F504} 重置").size(13.0))
            .clicked()
        {
            state.log_execution(format!("重置技能: {}", skill.name));
        }
    });
}

fn render_execution_log(ui: &mut egui::Ui, state: &SkillPanelState) {
    ui.label(
        egui::RichText::new("\u{1F4DC} 执行日志")
            .strong()
            .size(11.0),
    );

    egui::ScrollArea::vertical()
        .max_height(100.0)
        .stick_to_bottom(true)
        .show(ui, |ui| {
            if state.execution_log.is_empty() {
                ui.label(
                    egui::RichText::new("还没有执行记录")
                        .size(10.0)
                        .color(egui::Color32::from_gray(140)),
                );
                return;
            }

            for entry in state.execution_log.iter().rev().take(10) {
                ui.label(
                    egui::RichText::new(entry)
                        .size(10.0)
                        .color(egui::Color32::from_gray(160)),
                );
            }
        });
}
