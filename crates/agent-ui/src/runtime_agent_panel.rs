//! Runtime Agent Control Panel - Editor UI for controlling runtime AI agents
//!
//! Provides a visual interface for:
//! - Viewing all active runtime agents
//! - Monitoring agent state, perception, and blackboard
//! - Controlling agent mode (Autonomous/Manual/Disabled)
//! - Setting agent goals
//! - Viewing LLM inference logs

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use bevy_adapter::{
    RuntimeAgentComponent, RuntimeAgentStatus, PerceptionCapability
};
use crate::layout::{LayoutManager, LayoutCommand, PanelPosition};
use crate::LayoutCommandQueue;

pub struct RuntimeAgentPanelPlugin;

impl Plugin for RuntimeAgentPanelPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<RuntimeAgentPanelState>()
            .add_systems(Update, render_runtime_agent_panel);
    }
}

/// State for the runtime agent panel
#[derive(Resource, Default)]
pub struct RuntimeAgentPanelState {
    /// Currently selected agent entity (if any)
    pub selected_agent: Option<Entity>,
    /// Whether the panel is visible
    pub visible: bool,
    /// Filter text for agent search
    pub filter_text: String,
    /// New goal input text
    pub goal_input: String,
    /// Blackboard key input
    pub bb_key_input: String,
    /// Blackboard value input
    pub bb_value_input: String,
    /// Show perception details
    pub show_perception: bool,
    /// Show blackboard details
    pub show_blackboard: bool,
    /// Show LLM logs
    pub show_llm_logs: bool,
}

/// Main render system for the runtime agent panel
fn render_runtime_agent_panel(
    mut contexts: EguiContexts,
    mut state: ResMut<RuntimeAgentPanelState>,
    agent_query: Query<(Entity, &RuntimeAgentComponent, Option<&Name>, Option<&PerceptionCapability>)>,
    layout_mgr: Res<LayoutManager>,
    mut layout_queue: ResMut<LayoutCommandQueue>,
) {
    let ctx = contexts.ctx_mut();
    let Ok(ctx) = ctx else { return };

    // Toggle visibility with F6 key
    if ctx.input(|i| i.key_pressed(egui::Key::F6)) {
        layout_queue.push(LayoutCommand::TogglePanel { panel_id: "runtime_agents".to_string() });
    }

    if !layout_mgr.is_visible("runtime_agents") {
        return;
    }

    let (win_w, win_h) = layout_mgr
        .panel_config("runtime_agents")
        .and_then(|c| match c.position {
            PanelPosition::Floating { width, height, .. } => Some((width, height)),
            _ => None,
        })
        .unwrap_or((500.0, 600.0));

    egui::Window::new("🤖 Runtime Agents")
        .default_size([win_w, win_h])
        .resizable(true)
        .collapsible(true)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("Active Agents");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(egui::RichText::new("Press F6 to toggle").size(10.0).color(egui::Color32::from_gray(128)));
                });
            });
            ui.separator();

            // Agent list
            render_agent_list(ui, &mut state, &agent_query);

            ui.separator();

            // Agent details panel (if selected)
            if let Some(selected) = state.selected_agent {
                if let Ok((entity, agent, name, perception)) = agent_query.get(selected) {
                    render_agent_details(ui, &mut state, entity, agent, name, perception);
                } else {
                    state.selected_agent = None;
                    ui.label("Selected agent no longer exists.");
                }
            } else {
                ui.centered_and_justified(|ui| {
                    ui.label(egui::RichText::new("Select an agent to view details").color(egui::Color32::from_gray(128)));
                });
            }
        });
}

/// Render the list of active agents
fn render_agent_list(
    ui: &mut egui::Ui,
    state: &mut RuntimeAgentPanelState,
    agent_query: &Query<(Entity, &RuntimeAgentComponent, Option<&Name>, Option<&PerceptionCapability>)>,
) {
    // Filter input
    ui.horizontal(|ui| {
        ui.label("🔍");
        ui.add(egui::TextEdit::singleline(&mut state.filter_text).hint_text("Filter agents..."));
        if ui.button("Clear").clicked() {
            state.filter_text.clear();
        }
    });
    ui.add_space(8.0);

    // Agent count
    let agent_count = agent_query.iter().count();
    ui.label(egui::RichText::new(format!("{} agents active", agent_count)).size(11.0).color(egui::Color32::from_gray(128)));
    ui.add_space(4.0);

    // Scrollable list
    egui::ScrollArea::vertical()
        .max_height(200.0)
        .show(ui, |ui| {
            for (entity, agent, name, _perception) in agent_query.iter() {
                let name_str = name.map(|n| n.as_str()).unwrap_or("Unnamed");
                let id_str = &agent.id.0;
                
                // Apply filter
                let filter = state.filter_text.to_lowercase();
                if !filter.is_empty() 
                    && !name_str.to_lowercase().contains(&filter)
                    && !id_str.to_lowercase().contains(&filter) {
                    continue;
                }

                let is_selected = state.selected_agent == Some(entity);
                let status_color = get_status_color(&agent.status);
                let mode_str = format!("{:?}", agent.control_mode);

                let response = ui.selectable_label(
                    is_selected,
                    egui::RichText::new(format!("{} [{}]", name_str, id_str))
                        .color(if is_selected { egui::Color32::WHITE } else { egui::Color32::from_gray(220) }),
                );

                ui.horizontal(|_ui| {
                    if response.clicked() {
                        state.selected_agent = Some(entity);
                    }
                    
                    _ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.colored_label(status_color, format!("{:?}", agent.status));
                        ui.label("•");
                        ui.label(mode_str);
                    });
                });
            }
        });
}

/// Render details for the selected agent
fn render_agent_details(
    ui: &mut egui::Ui,
    state: &mut RuntimeAgentPanelState,
    _entity: Entity,
    agent: &RuntimeAgentComponent,
    name: Option<&Name>,
    perception: Option<&PerceptionCapability>,
) {
    let name_str = name.map(|n| n.as_str()).unwrap_or("Unnamed");
    
    // Header
    ui.horizontal(|ui| {
        ui.heading(name_str);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let status_color = get_status_color(&agent.status);
            ui.colored_label(status_color, format!("{:?}", agent.status));
        });
    });
    ui.label(format!("ID: {}", agent.id.0));
    ui.label(format!("Profile: {}", agent.profile_id.0));
    ui.add_space(8.0);

    // Control Mode display (read-only for now - needs events to modify)
    ui.horizontal(|ui| {
        ui.label("Control Mode:");
        ui.label(format!("{:?}", agent.control_mode));
    });
    ui.add_space(8.0);

    // Goal section
    ui.collapsing("🎯 Goal", |ui| {
        if let Some(ref goal) = agent.active_goal {
            ui.label(format!("Description: {}", goal.description));
            ui.label(format!("Priority: {:?}", goal.priority));
            if ui.button("Clear Goal").clicked() {
                // Note: This would require mutable access
                // In production, use events or commands
            }
        } else {
            ui.label("No active goal");
        }
        
        ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(&mut state.goal_input).hint_text("New goal..."));
            if ui.button("Set Goal").clicked() && !state.goal_input.is_empty() {
                // In production, send command to set goal
                state.goal_input.clear();
            }
        });
    });
    ui.add_space(4.0);

    // Perception section
    ui.checkbox(&mut state.show_perception, "👁 Show Perception");
    if state.show_perception {
        egui::Frame::group(ui.style()).show(ui, |ui| {
            if let Some(ref obs) = agent.last_observation {
                ui.label(format!("Visible entities: {}", obs.visible_entities.len()));
                ui.label(format!("Recent events: {}", obs.events.len()));
                
                // Perception capability details
                if let Some(perc) = perception {
                    ui.separator();
                    ui.label("Perception Capability:");
                    ui.label(format!("  Vision range: {:.1}", perc.vision_range));
                    ui.label(format!("  Vision angle: {:.0}°", perc.vision_angle));
                    ui.label(format!("  Hearing range: {:.1}", perc.hearing_range));
                }
                
                // Observation details
                if !obs.visible_entities.is_empty() {
                    ui.separator();
                    ui.label("Visible entities:");
                    for (i, entity_id) in obs.visible_entities.iter().enumerate().take(5) {
                        ui.label(format!("  {}: {:?}", i + 1, entity_id.0));
                    }
                }
            } else {
                ui.label("No observation data");
            }
        });
    }
    ui.add_space(4.0);

    // Blackboard section
    ui.checkbox(&mut state.show_blackboard, "📝 Show Blackboard");
    if state.show_blackboard {
        egui::Frame::group(ui.style()).show(ui, |ui| {
            let snapshot = agent.blackboard.snapshot();
            if snapshot.is_empty() {
                ui.label("Blackboard is empty");
            } else {
                egui::ScrollArea::vertical().max_height(150.0).show(ui, |ui| {
                    for (key, value) in snapshot.iter() {
                ui.horizontal(|ui| {
                            ui.label(format!("{}:", key));
                            ui.monospace(format!("{:?}", value));
                        });
                    }
                });
            }
            
            // Add new entry
            ui.separator();
            ui.horizontal(|ui| {
                ui.add(egui::TextEdit::singleline(&mut state.bb_key_input).hint_text("Key"));
                ui.add(egui::TextEdit::singleline(&mut state.bb_value_input).hint_text("Value"));
                if ui.button("Set").clicked() && !state.bb_key_input.is_empty() {
                    // In production, send command to set blackboard
                    state.bb_key_input.clear();
                    state.bb_value_input.clear();
                }
            });
        });
    }
    ui.add_space(4.0);

    // LLM Logs section
    ui.checkbox(&mut state.show_llm_logs, "🧠 Show LLM Logs");
    if state.show_llm_logs {
        egui::Frame::group(ui.style()).show(ui, |ui| {
            if let Some(thought) = agent.blackboard.get("last_llm_thought") {
                if let Some(thought_str) = thought.as_str() {
                    ui.label("Last LLM thought:");
                    ui.add(egui::Label::new(egui::RichText::new(thought_str).size(11.0)).wrap());
                }
            } else {
                ui.label("No LLM inference history");
            }
            
            if let Some(count) = agent.blackboard.get("pending_action_count") {
                if let Some(count_num) = count.as_u64() {
                    ui.label(format!("Pending actions: {}", count_num));
                }
            }
            
            ui.separator();
            ui.label(format!("Behavior: {:?}", agent.behavior));
        });
    }

    // Actions
    ui.separator();
    ui.horizontal(|ui| {
        if ui.button("🔄 Force Tick").clicked() {
            // In production, trigger immediate tick
        }
        if ui.button("🗑 Detach Agent").clicked() {
            state.selected_agent = None;
            // In production, send detach command
        }
    });
}

/// Get color for agent status
fn get_status_color(status: &RuntimeAgentStatus) -> egui::Color32 {
    match status {
        RuntimeAgentStatus::Idle => egui::Color32::from_rgb(128, 128, 128),
        RuntimeAgentStatus::Thinking => egui::Color32::from_rgb(255, 193, 7),
        RuntimeAgentStatus::Acting => egui::Color32::from_rgb(16, 185, 129),
        RuntimeAgentStatus::Waiting => egui::Color32::from_rgb(59, 130, 246),
        RuntimeAgentStatus::Suspended => egui::Color32::from_rgb(139, 92, 246),
        RuntimeAgentStatus::Error { .. } => egui::Color32::from_rgb(239, 68, 68),
    }
}

/// Helper to toggle panel visibility via layout command
pub fn toggle_runtime_agent_panel(queue: &mut LayoutCommandQueue) {
    queue.push(LayoutCommand::TogglePanel { panel_id: "runtime_agents".to_string() });
}
