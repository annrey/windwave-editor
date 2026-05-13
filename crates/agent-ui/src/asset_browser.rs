//! Asset Browser Panel - Phase 4.3
//!
//! UI for browsing project assets.

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use crate::project_panel::ProjectPanelState;
use crate::layout::{LayoutManager, PanelPosition};

pub struct AssetBrowserPlugin;

impl Plugin for AssetBrowserPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AssetBrowserState>()
            .add_systems(Update, render_asset_browser);
    }
}

/// Asset browser state
#[derive(Resource, Default)]
pub struct AssetBrowserState {
    pub visible: bool,
    pub current_dir: String,
    pub search_filter: String,
    pub selected_asset: Option<String>,
}

/// Asset entry type
#[derive(Debug, Clone, PartialEq)]
pub enum AssetType {
    Scene,
    Prefab,
    Texture,
    Mesh,
    Audio,
    Script,
    Folder,
    Unknown,
}

impl AssetType {
    pub fn icon(&self) -> &'static str {
        match self {
            AssetType::Scene => "🗺️",
            AssetType::Prefab => "📦",
            AssetType::Texture => "🖼️",
            AssetType::Mesh => "🔷",
            AssetType::Audio => "🔊",
            AssetType::Script => "📜",
            AssetType::Folder => "📁",
            AssetType::Unknown => "📄",
        }
    }

    pub fn from_extension(ext: &str) -> Self {
        match ext.to_lowercase().as_str() {
            "scene" => AssetType::Scene,
            "prefab" => AssetType::Prefab,
            "png" | "jpg" | "jpeg" | "gif" | "bmp" => AssetType::Texture,
            "obj" | "fbx" | "gltf" | "glb" => AssetType::Mesh,
            "mp3" | "wav" | "ogg" => AssetType::Audio,
            "rs" | "js" | "ts" | "lua" => AssetType::Script,
            _ => AssetType::Unknown,
        }
    }
}

/// Asset entry for display
#[derive(Debug, Clone)]
pub struct AssetEntry {
    pub name: String,
    pub path: String,
    pub asset_type: AssetType,
    pub is_folder: bool,
}

fn render_asset_browser(
    mut contexts: EguiContexts,
    mut state: ResMut<AssetBrowserState>,
    project_state: Res<ProjectPanelState>,
    layout_mgr: Res<LayoutManager>,
) {
    let ctx = match contexts.ctx_mut() {
        Ok(ctx) => ctx,
        Err(_) => return,
    };

    if !layout_mgr.is_visible("asset_browser") {
        return;
    }

    // Determine scan root from project or current dir
    let scan_root = get_assets_dir(&project_state);

    let (win_w, win_h) = layout_mgr
        .panel_config("asset_browser")
        .and_then(|c| match c.position {
            PanelPosition::Floating { width, height, .. } => Some((width, height)),
            _ => None,
        })
        .unwrap_or((400.0, 500.0));

    egui::Window::new("📂 Asset Browser")
        .default_size([win_w, win_h])
        .resizable(true)
        .collapsible(true)
        .show(ctx, |ui| {
            // Breadcrumb path
            ui.horizontal(|ui| {
                if ui.button("⬆️").clicked() && !state.current_dir.is_empty() {
                    // Go up one directory
                    let path = std::path::Path::new(&state.current_dir);
                    if let Some(parent) = path.parent() {
                        state.current_dir = parent.to_string_lossy().to_string();
                    }
                }
                ui.label(format!("📁 {}", if state.current_dir.is_empty() { "Assets" } else { &state.current_dir }));
            });

            ui.separator();

            // Search bar
            ui.horizontal(|ui| {
                ui.label("🔍");
                ui.text_edit_singleline(&mut state.search_filter);
            });

            ui.separator();

            // Asset grid/list
            egui::ScrollArea::vertical()
                .show(ui, |ui| {
                    let assets = scan_assets(&state.current_dir, &state.search_filter, &scan_root);

                    if assets.is_empty() {
                        ui.label("No assets found");
                    } else {
                        // Grid layout - 4 columns
                        let columns = 4;
                        let rows = (assets.len() + columns - 1) / columns;

                        for row in 0..rows {
                            ui.horizontal(|ui| {
                                for col in 0..columns {
                                    let idx = row * columns + col;
                                    if idx >= assets.len() {
                                        break;
                                    }

                                    let asset = &assets[idx];
                                    let is_selected = state.selected_asset.as_ref() == Some(&asset.path);

                                    // Asset card
                                    let response = ui.vertical(|ui| {
                                        ui.horizontal(|ui| {
                                            ui.label(asset.asset_type.icon());
                                            if asset.is_folder {
                                                ui.label("📁");
                                            }
                                        });

                                        let text = if is_selected {
                                            egui::RichText::new(&asset.name)
                                                .strong()
                                                .color(egui::Color32::WHITE)
                                        } else {
                                            egui::RichText::new(&asset.name)
                                                .color(egui::Color32::LIGHT_GRAY)
                                        };
                                        ui.label(text);
                                    });

                                    if response.response.clicked() {
                                        if asset.is_folder {
                                            state.current_dir = asset.path.clone();
                                        } else {
                                            state.selected_asset = Some(asset.path.clone());
                                        }
                                    }

                                    // Double click to import
                                    if response.response.double_clicked() && !asset.is_folder {
                                        // Signal import
                                    }
                                }
                            });
                            ui.separator();
                        }
                    }
                });

            // Selected asset details
            if let Some(ref path) = state.selected_asset {
                ui.separator();
                ui.heading("Selected");
                ui.label(format!("Path: {}", path));

                if ui.button("Import to Scene").clicked() {
                    // Import asset
                }
            }
        });
}

/// Get the assets directory from the project or fall back to current dir
fn get_assets_dir(project_state: &ProjectPanelState) -> String {
    if let (Some(project), Some(path)) = (
        project_state.project_manager.current_project(),
        project_state.project_manager.current_path(),
    ) {
        return path.join(&project.assets_dir).to_string_lossy().to_string();
    }
    std::env::current_dir()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string()
}

/// Scan directory for assets
fn scan_assets(dir: &str, filter: &str, default_root: &str) -> Vec<AssetEntry> {
    let mut entries = Vec::new();
    let filter_lower = filter.to_lowercase();

    let base = if dir.is_empty() {
        std::path::PathBuf::from(default_root)
    } else {
        std::path::PathBuf::from(dir)
    };

    if let Ok(read_dir) = std::fs::read_dir(&base) {
        for entry in read_dir.flatten() {
            let metadata = entry.metadata();
            let is_folder = metadata.as_ref().map(|m| m.is_dir()).unwrap_or(false);
            let name = entry.file_name().to_string_lossy().to_string();

            // Apply filter
            if !filter.is_empty() && !name.to_lowercase().contains(&filter_lower) {
                continue;
            }

            let asset_type = if is_folder {
                AssetType::Folder
            } else {
                entry
                    .path()
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(AssetType::from_extension)
                    .unwrap_or(AssetType::Unknown)
            };

            entries.push(AssetEntry {
                path: entry.path().to_string_lossy().to_string(),
                name,
                asset_type,
                is_folder,
            });
        }
    }

    // Sort: folders first, then by name
    entries.sort_by(|a, b| {
        match (a.is_folder, b.is_folder) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.name.cmp(&b.name),
        }
    });

    entries
}

/// Toggle asset browser visibility
pub fn toggle_asset_browser(state: &mut ResMut<AssetBrowserState>) {
    state.visible = !state.visible;
}

/// Refresh asset list
pub fn refresh_assets(_state: &mut ResMut<AssetBrowserState>) {
    // Rescan will happen automatically on next render
}
