//! Hierarchy Panel - Scene entity tree view
//!
//! Provides a left-side panel displaying the scene's entity hierarchy.
//! Features:
//! - Tree view of entities with parent-child relationships
//! - Click to select entity
//! - Expand/collapse tree nodes
//! - Search/filter entities by name

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use bevy_adapter::{RuntimeAgentComponent};
use crate::editor_selection::EditorSelection;
use crate::layout::LayoutManager;
use std::collections::HashSet;
use std::collections::HashMap;

pub struct HierarchyPanelPlugin;

impl Plugin for HierarchyPanelPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<HierarchyState>()
            .add_systems(Update, render_hierarchy_panel);
    }
}

#[derive(Resource, Default)]
pub struct HierarchyState {
    pub selected_entity: Option<Entity>,
    pub expanded_nodes: HashSet<Entity>,
    pub search_query: String,
    pub filter_by_agent: bool,
}

/// Component to mark entities that should appear in hierarchy
#[derive(Component)]
pub struct HierarchyNode {
    pub name: String,
    pub depth: usize,
}

fn render_hierarchy_panel(
    mut contexts: EguiContexts,
    mut state: ResMut<HierarchyState>,
    mut editor_selection: ResMut<EditorSelection>,
    query: Query<(Entity, Option<&Name>, Option<&Children>)>,
    agent_query: Query<(Entity, &RuntimeAgentComponent)>,
    layout_mgr: Res<LayoutManager>,
) {
    if !layout_mgr.is_visible("hierarchy") { return; }

    let ctx = contexts.ctx_mut();
    let Ok(ctx) = ctx else { return };

    egui::SidePanel::left("hierarchy_panel")
        .default_width(250.0)
        .resizable(true)
        .show(ctx, |ui| {
            ui.heading("Hierarchy");
            ui.separator();

            // Search bar
            ui.horizontal(|ui| {
                ui.label("🔍");
                ui.text_edit_singleline(&mut state.search_query);
            });

            // Filter options
            ui.checkbox(&mut state.filter_by_agent, "Show only Agents");

            ui.separator();

            // Entity tree
            egui::ScrollArea::vertical()
                .auto_shrink([false; 2])
                .show(ui, |ui| {
                    // Build parent-child relationships
                    let mut root_entities: Vec<Entity> = Vec::new();
                    let mut entity_children: HashMap<Entity, Vec<Entity>> = HashMap::new();

                    // Collect all entities with their children
                    for (entity, _name, children) in query.iter() {
                        // For now, treat all entities as roots (simplified)
                        root_entities.push(entity);
                        if let Some(children) = children {
                            let child_list: Vec<Entity> = children.to_vec();
                            entity_children.insert(entity, child_list);
                        }
                    }

                    // Show tree starting from roots
                    for root in root_entities {
                        render_entity_node(
                            ui,
                            root,
                            &mut state,
                            &mut editor_selection,
                            &query,
                            &agent_query,
                            &entity_children,
                            0,
                        );
                    }
                });

            // Sync hierarchy selection to editor selection
            editor_selection.selected_entity = state.selected_entity;

            // Selection info footer
            if let Some(selected) = state.selected_entity {
                ui.separator();
                ui.label(format!("Selected: {:?}", selected));
            }
        });
}

fn render_entity_node(
    ui: &mut egui::Ui,
    entity: Entity,
    state: &mut HierarchyState,
    editor_selection: &mut EditorSelection,
    query: &Query<(Entity, Option<&Name>, Option<&Children>)>,
    agent_query: &Query<(Entity, &RuntimeAgentComponent)>,
    children_map: &HashMap<Entity, Vec<Entity>>,
    depth: usize,
) {
    let Ok((_, name_opt, _)) = query.get(entity) else { return };

    let name = name_opt.map(|n| n.as_str().to_string()).unwrap_or_else(|| "Unnamed".to_string());

    // Filter by search query
    if !state.search_query.is_empty() {
        let query_lower = state.search_query.to_lowercase();
        if !name.to_lowercase().contains(&query_lower) {
            return;
        }
    }

    // Filter by agent
    if state.filter_by_agent && agent_query.get(entity).is_err() {
        return;
    }

    let is_selected = editor_selection.selected_entity == Some(entity);
    let has_children = children_map.contains_key(&entity);
    let is_expanded = state.expanded_nodes.contains(&entity);

    let indent = depth * 20;

    ui.horizontal(|ui| {
        // Indent
        ui.add_space(indent as f32);

        // Expand/collapse button
        if has_children {
            let icon = if is_expanded { "▼" } else { "▶" };
            if ui.button(icon).clicked() {
                if is_expanded {
                    state.expanded_nodes.remove(&entity);
                } else {
                    state.expanded_nodes.insert(entity);
                }
            }
        } else {
            ui.add_space(24.0); // Spacer for alignment
        }

        // Entity label
        let label_text = if agent_query.get(entity).is_ok() {
            format!("🤖 {}", name)
        } else {
            format!("📦 {}", name)
        };

        let response = ui.selectable_label(is_selected, label_text);

        if response.clicked() {
            state.selected_entity = Some(entity);
            editor_selection.select(entity);
            info!("Selected entity: {:?} ({})", entity, name);
        }
    });

    // Render children if expanded
    if is_expanded && has_children {
        if let Some(children) = children_map.get(&entity) {
            for child in children {
                render_entity_node(
                    ui,
                    *child,
                    state,
                    editor_selection,
                    query,
                    agent_query,
                    children_map,
                    depth + 1,
                );
            }
        }
    }
}

