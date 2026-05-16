//! Visual Understanding UI - Display Agent's visual perception and analysis

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use serde::{Deserialize, Serialize};

#[derive(Resource, Debug, Clone, Serialize, Deserialize)]
pub struct VisualUnderstandingState {
    pub enabled: bool,
    pub show_screenshot: bool,
    pub show_analysis: bool,
    pub show_goal_check: bool,
    pub screenshot_base64: Option<String>,
    pub screenshot_dimensions: Option<(u32, u32)>,
    pub latest_analysis: Option<VisualAnalysis>,
    pub goal_check_results: Vec<GoalCheckResult>,
    pub vgrc_cycle_history: Vec<VgrcCycleSummary>,
}

impl Default for VisualUnderstandingState {
    fn default() -> Self {
        Self {
            enabled: true,
            show_screenshot: true,
            show_analysis: true,
            show_goal_check: true,
            screenshot_base64: None,
            screenshot_dimensions: None,
            latest_analysis: None,
            goal_check_results: Vec::new(),
            vgrc_cycle_history: Vec::new(),
        }
    }
}

impl VisualUnderstandingState {
    pub fn update_screenshot(&mut self, base64: String, dimensions: (u32, u32)) {
        self.screenshot_base64 = Some(base64);
        self.screenshot_dimensions = Some(dimensions);
    }

    pub fn add_analysis(&mut self, analysis: VisualAnalysis) {
        self.latest_analysis = Some(analysis);
    }

    pub fn add_goal_check(&mut self, result: GoalCheckResult) {
        self.goal_check_results.push(result);
        if self.goal_check_results.len() > 10 {
            self.goal_check_results.remove(0);
        }
    }

    pub fn add_vgrc_cycle(&mut self, cycle: VgrcCycleSummary) {
        self.vgrc_cycle_history.push(cycle);
        if self.vgrc_cycle_history.len() > 20 {
            self.vgrc_cycle_history.remove(0);
        }
    }

    pub fn clear(&mut self) {
        self.screenshot_base64 = None;
        self.screenshot_dimensions = None;
        self.latest_analysis = None;
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisualAnalysis {
    pub timestamp: f64,
    pub goal: String,
    pub observation_summary: String,
    pub detected_entities: Vec<String>,
    pub confidence: f32,
    pub suggestions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalCheckResult {
    pub timestamp: f64,
    pub goal: String,
    pub passed: bool,
    pub details: String,
    pub matches: Vec<MatchInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchInfo {
    pub entity_name: String,
    pub match_type: String,
    pub confidence: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VgrcCycleSummary {
    pub cycle_id: u32,
    pub goal: String,
    pub vision_count: usize,
    pub realize_attempts: usize,
    pub check_passed: bool,
    pub total_duration_ms: u64,
}

pub struct VisualUnderstandingPlugin;

impl Plugin for VisualUnderstandingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<VisualUnderstandingState>()
            .add_systems(Update, render_visual_understanding_panel);
    }
}

fn render_visual_understanding_panel(
    mut contexts: EguiContexts,
    mut state: ResMut<VisualUnderstandingState>,
) {
    if !state.enabled {
        return;
    }

    let ctx = contexts.ctx_mut();
    let Ok(ctx) = ctx else { return };

    egui::Window::new("Visual Understanding")
        .id(egui::Id::new("visual_understanding_panel"))
        .default_size([400.0, 500.0])
        .vscroll(true)
        .show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.checkbox(&mut state.show_screenshot, "Screenshot");
                ui.checkbox(&mut state.show_analysis, "Analysis");
                ui.checkbox(&mut state.show_goal_check, "Goal Check");
                ui.separator();
                if ui.button("Clear").clicked() {
                    state.clear();
                }
            });

            ui.separator();

            if state.show_screenshot {
                ui.heading("Screenshot");
                if let Some(dimensions) = state.screenshot_dimensions {
                    ui.label(format!("Dimensions: {}x{}", dimensions.0, dimensions.1));
                    ui.add_space(4.0);
                    let available_width = ui.available_width().min(360.0);
                    let aspect_ratio = if dimensions.1 > 0 {
                        dimensions.0 as f32 / dimensions.1 as f32
                    } else {
                        16.0 / 9.0
                    };
                    let display_height = available_width / aspect_ratio;
                    let (rect, _) = ui.allocate_at_least(egui::vec2(available_width, display_height), egui::Sense::hover());
                    ui.painter().rect_filled(rect, 4.0, egui::Color32::from_gray(30));
                    let text_pos = rect.center() - egui::vec2(60.0, 10.0);
                    ui.painter().text(text_pos, egui::Align2::LEFT_CENTER, "Screenshot Preview", egui::FontId::proportional(14.0), egui::Color32::from_gray(140));
                } else {
                    ui.label("No screenshot captured");
                    ui.label("Agent will capture when VGRC cycle runs");
                }
                ui.separator();
            }

            if state.show_analysis {
                ui.heading("Vision Analysis");
                if let Some(analysis) = &state.latest_analysis {
                    ui.label(egui::RichText::new("Goal:").strong());
                    ui.label(&analysis.goal);
                    ui.add_space(4.0);
                    ui.label(egui::RichText::new("Observation:").strong());
                    ui.label(&analysis.observation_summary);
                    if !analysis.detected_entities.is_empty() {
                        ui.add_space(4.0);
                        ui.label(egui::RichText::new("Detected:").strong());
                        for entity in &analysis.detected_entities {
                            ui.label(format!("  - {}", entity));
                        }
                    }
                    let confidence_color = if analysis.confidence > 0.8 {
                        egui::Color32::from_rgb(16, 185, 129)
                    } else if analysis.confidence > 0.5 {
                        egui::Color32::from_rgb(245, 158, 11)
                    } else {
                        egui::Color32::from_rgb(239, 68, 68)
                    };
                    ui.label(egui::RichText::new(format!("Confidence: {:.0}%", analysis.confidence * 100.0)).color(confidence_color));
                } else {
                    ui.label("No analysis available");
                }
                ui.separator();
            }

            if state.show_goal_check {
                ui.heading("Goal Check Results");
                if state.goal_check_results.is_empty() {
                    ui.label("No goal checks performed");
                } else {
                    for result in state.goal_check_results.iter().rev().take(5) {
                        let status_color = if result.passed {
                            egui::Color32::from_rgb(16, 185, 129)
                        } else {
                            egui::Color32::from_rgb(239, 68, 68)
                        };
                        let status_icon = if result.passed { "[OK]" } else { "[FAIL]" };
                        ui.label(egui::RichText::new(format!("{} {}", status_icon, result.goal)).color(status_color));
                        ui.label(format!("  {}", result.details));
                        ui.add_space(4.0);
                    }
                }
                ui.separator();
            }

            ui.heading("VGRC Cycle History");
            if state.vgrc_cycle_history.is_empty() {
                ui.label("No VGRC cycles recorded");
            } else {
                for cycle in state.vgrc_cycle_history.iter().rev().take(5) {
                    let status_icon = if cycle.check_passed { "[OK]" } else { "[...]" };
                    ui.label(format!("#{} {} - {}", cycle.cycle_id, status_icon, cycle.goal));
                    ui.label(format!("  Vision:{} Realize:{} {}ms", cycle.vision_count, cycle.realize_attempts, cycle.total_duration_ms));
                    ui.add_space(4.0);
                }
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_state_creation() {
        let state = VisualUnderstandingState::default();
        assert!(state.enabled);
        assert!(state.screenshot_base64.is_none());
    }

    #[test]
    fn test_add_goal_check() {
        let mut state = VisualUnderstandingState::default();
        state.add_goal_check(GoalCheckResult {
            timestamp: 0.0,
            goal: "Test".to_string(),
            passed: true,
            details: "OK".to_string(),
            matches: vec![],
        });
        assert_eq!(state.goal_check_results.len(), 1);
    }

    #[test]
    fn test_vgrc_cycle_limit() {
        let mut state = VisualUnderstandingState::default();
        for i in 0..25 {
            state.add_vgrc_cycle(VgrcCycleSummary {
                cycle_id: i,
                goal: format!("Goal {}", i),
                vision_count: 1,
                realize_attempts: 1,
                check_passed: true,
                total_duration_ms: 100,
            });
        }
        assert_eq!(state.vgrc_cycle_history.len(), 20);
    }
}
