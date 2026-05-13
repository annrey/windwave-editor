//! Prefab Browser Panel - Phase 4.2
//!
//! UI for browsing, creating, and instantiating prefabs.

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use agent_core::bevy_editor_model::{PrefabDefinition, PrefabId, PrefabNode, PrefabRegistry as CorePrefabRegistry, ComponentPatch};
use crate::editor_selection::EditorSelection;
use crate::layout::{LayoutManager, PanelPosition};

/// Bevy Resource wrapper for PrefabRegistry
#[derive(Resource, Default, Clone)]
pub struct PrefabRegistry { pub inner: CorePrefabRegistry }
impl PrefabRegistry {
    pub fn register(&mut self, p: PrefabDefinition) { self.inner.register(p); }
    pub fn get(&self, id: &PrefabId) -> Option<&PrefabDefinition> { self.inner.get(id) }
    pub fn list(&self) -> Vec<&PrefabDefinition> { self.inner.list() }
}

/// Event to request prefab creation from selected entity
#[derive(Message, Debug, Clone)] pub struct CreatePrefabEvent { pub name: String }
/// Event to request prefab instantiation
#[derive(Message, Debug, Clone)] pub struct InstantiatePrefabEvent { pub prefab_id: String }

pub struct PrefabBrowserPlugin;
impl Plugin for PrefabBrowserPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PrefabBrowserState>()
            .init_resource::<PrefabRegistry>()
            .add_systems(Update, render_prefab_browser)
            .add_systems(Update, handle_create_prefab)
            .add_systems(Update, handle_instantiate_prefab);
    }
}

#[derive(Resource, Default)] pub struct PrefabBrowserState {
    pub visible: bool, pub selected_prefab: Option<String>, pub search_filter: String,
    pub new_prefab_name: String, pub show_create_dialog: bool,
}
#[derive(Debug, Clone)] pub struct PrefabEntry { pub id: String, pub name: String, pub component_count: usize }

fn render_prefab_browser(
    mut contexts: EguiContexts, mut state: ResMut<PrefabBrowserState>,
    prefab_registry: Option<Res<PrefabRegistry>>,
    mut create_events: MessageWriter<CreatePrefabEvent>,
    mut instantiate_events: MessageWriter<InstantiatePrefabEvent>,
    layout_mgr: Res<LayoutManager>,
) {
    let ctx = match contexts.ctx_mut() { Ok(c) => c, Err(_) => return };
    if !layout_mgr.is_visible("prefab_browser") { return; }

    let (win_w, win_h) = layout_mgr
        .panel_config("prefab_browser")
        .and_then(|c| match c.position {
            PanelPosition::Floating { width, height, .. } => Some((width, height)),
            _ => None,
        })
        .unwrap_or((350.0, 500.0));

    egui::Window::new("Prefab Browser").default_size([win_w, win_h]).resizable(true).show(ctx, |ui| {
        ui.horizontal(|ui| {
            ui.text_edit_singleline(&mut state.search_filter);
            if ui.button("+ Create").clicked() { state.show_create_dialog = true; state.new_prefab_name.clear(); }
        });
        ui.separator();
        egui::ScrollArea::vertical().show(ui, |ui| {
            let items = get_prefab_entries(&prefab_registry, &state.search_filter);
            if items.is_empty() { ui.label("No prefabs found"); } else {
                for p in &items {
                    let sel = state.selected_prefab.as_ref() == Some(&p.id);
                    if ui.selectable_label(sel, format!("{} ({} comps)", p.name, p.component_count)).clicked() {
                        state.selected_prefab = Some(p.id.clone());
                    }
                    ui.separator();
                }
            }
        });
        ui.separator();
        if let Some(id) = &state.selected_prefab {
            if let Some(prefab) = prefab_registry.as_ref().and_then(|r| r.get(&PrefabId(id.clone()))) {
                ui.heading(&prefab.name);
                if ui.button("Instantiate in Scene").clicked() {
                    instantiate_events.write(InstantiatePrefabEvent { prefab_id: id.clone() });
                }
            }
        }
    });

    if state.show_create_dialog {
        egui::Window::new("Create Prefab").collapsible(false).resizable(false).show(ctx, |ui| {
            ui.text_edit_singleline(&mut state.new_prefab_name);
            ui.horizontal(|ui| {
                if ui.button("Create").clicked() && !state.new_prefab_name.is_empty() {
                    create_events.write(CreatePrefabEvent { name: state.new_prefab_name.clone() });
                    state.show_create_dialog = false;
                }
                if ui.button("Cancel").clicked() { state.show_create_dialog = false; }
            });
        });
    }
}

fn get_prefab_entries(reg: &Option<Res<PrefabRegistry>>, filter: &str) -> Vec<PrefabEntry> {
    let Some(reg) = reg.as_ref() else { return vec![] };
    let f = filter.to_lowercase();
    reg.list().iter().filter(|p| f.is_empty() || p.name.to_lowercase().contains(&f))
        .map(|p| PrefabEntry { id: p.id.0.clone(), name: p.name.clone(), component_count: count_components(&p.root) })
        .collect()
}

fn count_components(node: &PrefabNode) -> usize {
    node.components.len() + node.children.iter().map(count_components).sum::<usize>()
}

fn handle_create_prefab(
    mut events: MessageReader<CreatePrefabEvent>, editor_selection: Res<EditorSelection>,
    _registry: ResMut<PrefabRegistry>,
) {
    for event in events.read() {
        if let Some(entity) = editor_selection.selected_entity {
            // Store prefab name for later processing - world access needed
            // For now, register a placeholder; full ECS extraction requires exclusive system
            log::info!("Prefab create requested: {} for entity {:?}", event.name, entity);
            // In full implementation: use exclusive system to extract components from world
        }
    }
}

fn handle_instantiate_prefab(
    mut events: MessageReader<InstantiatePrefabEvent>,
    registry: Option<Res<PrefabRegistry>>, mut commands: Commands,
) {
    for event in events.read() {
        if let Some(reg) = registry.as_ref() {
            if let Some(prefab) = reg.get(&PrefabId(event.prefab_id.clone())) {
                let e = spawn_prefab_cmd(&mut commands, &prefab.root);
                log::info!("Queued prefab instantiation: {} -> {:?}", event.prefab_id, e);
            }
        }
    }
}

fn spawn_prefab_cmd(commands: &mut Commands, node: &PrefabNode) -> Entity {
    let mut cmd = commands.spawn_empty();
    if !node.name.is_empty() { cmd.insert(Name::new(node.name.clone())); }
    for patch in &node.components {
        apply_patch_to_cmd(&mut cmd, patch);
    }
    let entity = cmd.id();
    for child_node in &node.children {
        let child = spawn_prefab_cmd(commands, child_node);
        commands.entity(child).set_parent_in_place(entity);
    }
    entity
}

fn apply_patch_to_cmd(cmd: &mut EntityCommands, patch: &ComponentPatch) {
    let p = &patch.properties;
    match patch.type_name.as_str() {
        "Transform" => {
            let tx = p.get("translation").and_then(|v| v.as_array()).and_then(|a| a.first()).and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
            let ty = p.get("translation").and_then(|v| v.as_array()).and_then(|a| a.get(1)).and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
            let sx = p.get("scale").and_then(|v| v.as_array()).and_then(|a| a.first()).and_then(|v| v.as_f64()).unwrap_or(1.0) as f32;
            let sy = p.get("scale").and_then(|v| v.as_array()).and_then(|a| a.get(1)).and_then(|v| v.as_f64()).unwrap_or(1.0) as f32;
            cmd.insert(Transform::from_xyz(tx, ty, 0.0).with_scale(Vec3::new(sx, sy, 1.0)));
        }
        "Sprite" => {
            let r = f32_from(p, "color_rgba", 0, 1.0); let g = f32_from(p, "color_rgba", 1, 1.0);
            let b = f32_from(p, "color_rgba", 2, 1.0); let a = f32_from(p, "color_rgba", 3, 1.0);
            let sz = p.get("custom_size").and_then(|v| v.as_array()).map(|a| {
                Vec2::new(a.first().and_then(|x| x.as_f64()).unwrap_or(50.0) as f32,
                          a.get(1).and_then(|x| x.as_f64()).unwrap_or(50.0) as f32)
            });
            cmd.insert(Sprite { color: Color::linear_rgba(r, g, b, a), custom_size: sz, ..Default::default() });
        }
        "Visibility" => {
            let v = match p.get("state").and_then(|v| v.as_str()).unwrap_or("visible") {
                "hidden" => Visibility::Hidden, _ => Visibility::Visible,
            };
            cmd.insert(v);
        }
        _ => log::warn!("Unsupported prefab component: {}", patch.type_name),
    }
}

fn f32_from(p: &std::collections::HashMap<String, serde_json::Value>, key: &str, idx: usize, def: f32) -> f32 {
    p.get(key).and_then(|v| v.as_array()).and_then(|a| a.get(idx)).and_then(|v| v.as_f64()).map(|f| f as f32).unwrap_or(def)
}

pub fn toggle_prefab_browser(state: &mut ResMut<PrefabBrowserState>) { state.visible = !state.visible; }
pub fn create_prefab_from_entity(_state: &mut ResMut<PrefabBrowserState>) {}
