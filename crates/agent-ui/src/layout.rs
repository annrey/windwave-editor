//! Layout system for modular editor panels.
//!
//! Defines `LayoutManager` resource with per-panel configs, visibility toggles,
//! resize/move operations, JSON persistence, and agent-callable `LayoutCommand`s.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

// ──────────────────────────────────────────────────────────
// Position enum
// ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum PanelPosition {
    Left,
    Right,
    Bottom,
    Top,
    Floating { x: f32, y: f32, width: f32, height: f32 },
}

impl PartialEq for PanelPosition {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Left, Self::Left)
            | (Self::Right, Self::Right)
            | (Self::Bottom, Self::Bottom)
            | (Self::Top, Self::Top) => true,
            (Self::Floating { .. }, Self::Floating { .. }) => true,
            _ => false,
        }
    }
}

// ──────────────────────────────────────────────────────────
// Per-panel config
// ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PanelConfig {
    pub id: String,
    pub title: String,
    pub position: PanelPosition,
    pub order: usize,
    pub size: f32,
    pub visible: bool,
    pub tab_group: Option<String>,
}

impl PanelConfig {
    pub fn new(id: &str, title: &str, position: PanelPosition, order: usize, size: f32) -> Self {
        Self {
            id: id.to_string(),
            title: title.to_string(),
            position,
            order,
            size,
            visible: true,
            tab_group: None,
        }
    }

    pub fn hidden(id: &str, title: &str, position: PanelPosition, order: usize, size: f32) -> Self {
        Self {
            visible: false,
            ..Self::new(id, title, position, order, size)
        }
    }
}

// ──────────────────────────────────────────────────────────
// Layout definition (serializable)
// ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayoutDefinition {
    pub name: String,
    pub panels: Vec<PanelConfig>,
    pub version: u32,
}

impl Default for LayoutDefinition {
    fn default() -> Self {
        Self {
            name: "Standard Editor Layout".into(),
            version: 1,
            panels: vec![
                // ── Left panels ──
                PanelConfig::new("hierarchy", "Hierarchy", PanelPosition::Left, 0, 250.0),
                PanelConfig::new("director_desk", "Director Desk", PanelPosition::Left, 1, 280.0),
                // ── Right panels ──
                PanelConfig::new("chat", "Chat", PanelPosition::Right, 0, 380.0),
                PanelConfig::new("inspector", "Inspector", PanelPosition::Right, 1, 300.0),
                // ── Bottom panels ──
                PanelConfig::new("console", "Console", PanelPosition::Bottom, 0, 200.0),
                PanelConfig::new("director_events", "Events", PanelPosition::Bottom, 1, 180.0),
                // ── Top panels ──
                PanelConfig::hidden("game_mode_bar", "Game Mode", PanelPosition::Top, 0, 24.0),
                // ── Floating panels (hidden by default) ──
                PanelConfig::hidden("runtime_agents", "Runtime Agents", PanelPosition::Floating { x: 100.0, y: 80.0, width: 450.0, height: 400.0 }, 0, 0.0),
                PanelConfig::hidden("debug", "Debug", PanelPosition::Floating { x: 150.0, y: 120.0, width: 500.0, height: 400.0 }, 0, 0.0),
                PanelConfig::hidden("agent_config", "Agent Config", PanelPosition::Floating { x: 200.0, y: 160.0, width: 420.0, height: 380.0 }, 0, 0.0),
                PanelConfig::hidden("prefab_browser", "Prefab Browser", PanelPosition::Floating { x: 250.0, y: 200.0, width: 500.0, height: 400.0 }, 0, 0.0),
                PanelConfig::hidden("asset_browser", "Asset Browser", PanelPosition::Floating { x: 250.0, y: 200.0, width: 550.0, height: 450.0 }, 0, 0.0),
                PanelConfig::hidden("project", "Project", PanelPosition::Floating { x: 300.0, y: 100.0, width: 500.0, height: 400.0 }, 0, 0.0),
                PanelConfig::hidden("diff_preview", "Diff Preview", PanelPosition::Floating { x: 300.0, y: 150.0, width: 480.0, height: 350.0 }, 0, 0.0),
                PanelConfig::hidden("game_mode", "Game Mode", PanelPosition::Floating { x: 200.0, y: 200.0, width: 600.0, height: 500.0 }, 0, 0.0),
            ],
        }
    }
}

impl LayoutDefinition {
    pub fn compact() -> Self {
        Self {
            name: "Compact Layout".into(),
            version: 1,
            panels: vec![
                PanelConfig::new("hierarchy", "Hierarchy", PanelPosition::Left, 0, 200.0),
                PanelConfig::new("director_desk", "Director Desk", PanelPosition::Left, 1, 240.0),
                PanelConfig::new("chat", "Chat", PanelPosition::Right, 0, 320.0),
                PanelConfig::new("inspector", "Inspector", PanelPosition::Right, 1, 260.0),
                PanelConfig::new("console", "Console", PanelPosition::Bottom, 0, 150.0),
                PanelConfig::new("director_events", "Events", PanelPosition::Bottom, 1, 140.0),
                PanelConfig::hidden("game_mode_bar", "Game Mode", PanelPosition::Top, 0, 24.0),
                PanelConfig::hidden("runtime_agents", "Runtime Agents", PanelPosition::Floating { x: 100.0, y: 80.0, width: 450.0, height: 400.0 }, 0, 0.0),
                PanelConfig::hidden("debug", "Debug", PanelPosition::Floating { x: 150.0, y: 120.0, width: 500.0, height: 400.0 }, 0, 0.0),
                PanelConfig::hidden("agent_config", "Agent Config", PanelPosition::Floating { x: 200.0, y: 160.0, width: 420.0, height: 380.0 }, 0, 0.0),
                PanelConfig::hidden("prefab_browser", "Prefab Browser", PanelPosition::Floating { x: 250.0, y: 200.0, width: 500.0, height: 400.0 }, 0, 0.0),
                PanelConfig::hidden("asset_browser", "Asset Browser", PanelPosition::Floating { x: 250.0, y: 200.0, width: 550.0, height: 450.0 }, 0, 0.0),
                PanelConfig::hidden("project", "Project", PanelPosition::Floating { x: 300.0, y: 100.0, width: 500.0, height: 400.0 }, 0, 0.0),
                PanelConfig::hidden("diff_preview", "Diff Preview", PanelPosition::Floating { x: 300.0, y: 150.0, width: 480.0, height: 350.0 }, 0, 0.0),
                PanelConfig::hidden("game_mode", "Game Mode", PanelPosition::Floating { x: 200.0, y: 200.0, width: 600.0, height: 500.0 }, 0, 0.0),
            ],
        }
    }

    pub fn wide() -> Self {
        Self {
            name: "Wide Layout".into(),
            version: 1,
            panels: vec![
                PanelConfig::new("hierarchy", "Hierarchy", PanelPosition::Left, 0, 280.0),
                PanelConfig::new("director_desk", "Director Desk", PanelPosition::Left, 1, 320.0),
                PanelConfig::new("chat", "Chat", PanelPosition::Right, 0, 420.0),
                PanelConfig::new("inspector", "Inspector", PanelPosition::Right, 1, 340.0),
                PanelConfig::new("console", "Console", PanelPosition::Bottom, 0, 220.0),
                PanelConfig::new("director_events", "Events", PanelPosition::Bottom, 1, 200.0),
                PanelConfig::hidden("game_mode_bar", "Game Mode", PanelPosition::Top, 0, 24.0),
                PanelConfig::hidden("runtime_agents", "Runtime Agents", PanelPosition::Floating { x: 100.0, y: 80.0, width: 450.0, height: 400.0 }, 0, 0.0),
                PanelConfig::hidden("debug", "Debug", PanelPosition::Floating { x: 150.0, y: 120.0, width: 500.0, height: 400.0 }, 0, 0.0),
                PanelConfig::hidden("agent_config", "Agent Config", PanelPosition::Floating { x: 200.0, y: 160.0, width: 420.0, height: 380.0 }, 0, 0.0),
                PanelConfig::hidden("prefab_browser", "Prefab Browser", PanelPosition::Floating { x: 250.0, y: 200.0, width: 500.0, height: 400.0 }, 0, 0.0),
                PanelConfig::hidden("asset_browser", "Asset Browser", PanelPosition::Floating { x: 250.0, y: 200.0, width: 550.0, height: 450.0 }, 0, 0.0),
                PanelConfig::hidden("project", "Project", PanelPosition::Floating { x: 300.0, y: 100.0, width: 500.0, height: 400.0 }, 0, 0.0),
                PanelConfig::hidden("diff_preview", "Diff Preview", PanelPosition::Floating { x: 300.0, y: 150.0, width: 480.0, height: 350.0 }, 0, 0.0),
                PanelConfig::hidden("game_mode", "Game Mode", PanelPosition::Floating { x: 200.0, y: 200.0, width: 600.0, height: 500.0 }, 0, 0.0),
            ],
        }
    }

    pub fn minimal() -> Self {
        Self {
            name: "Minimal Layout".into(),
            version: 1,
            panels: vec![
                PanelConfig::new("chat", "Chat", PanelPosition::Right, 0, 380.0),
                PanelConfig::hidden("hierarchy", "Hierarchy", PanelPosition::Left, 0, 250.0),
                PanelConfig::hidden("director_desk", "Director Desk", PanelPosition::Left, 1, 280.0),
                PanelConfig::hidden("inspector", "Inspector", PanelPosition::Right, 1, 300.0),
                PanelConfig::hidden("console", "Console", PanelPosition::Bottom, 0, 200.0),
                PanelConfig::hidden("director_events", "Events", PanelPosition::Bottom, 1, 180.0),
                PanelConfig::hidden("game_mode_bar", "Game Mode", PanelPosition::Top, 0, 24.0),
                PanelConfig::hidden("runtime_agents", "Runtime Agents", PanelPosition::Floating { x: 100.0, y: 80.0, width: 450.0, height: 400.0 }, 0, 0.0),
                PanelConfig::hidden("debug", "Debug", PanelPosition::Floating { x: 150.0, y: 120.0, width: 500.0, height: 400.0 }, 0, 0.0),
                PanelConfig::hidden("agent_config", "Agent Config", PanelPosition::Floating { x: 200.0, y: 160.0, width: 420.0, height: 380.0 }, 0, 0.0),
                PanelConfig::hidden("prefab_browser", "Prefab Browser", PanelPosition::Floating { x: 250.0, y: 200.0, width: 500.0, height: 400.0 }, 0, 0.0),
                PanelConfig::hidden("asset_browser", "Asset Browser", PanelPosition::Floating { x: 250.0, y: 200.0, width: 550.0, height: 450.0 }, 0, 0.0),
                PanelConfig::hidden("project", "Project", PanelPosition::Floating { x: 300.0, y: 100.0, width: 500.0, height: 400.0 }, 0, 0.0),
                PanelConfig::hidden("diff_preview", "Diff Preview", PanelPosition::Floating { x: 300.0, y: 150.0, width: 480.0, height: 350.0 }, 0, 0.0),
                PanelConfig::hidden("game_mode", "Game Mode", PanelPosition::Floating { x: 200.0, y: 200.0, width: 600.0, height: 500.0 }, 0, 0.0),
            ],
        }
    }

    pub fn focus_right() -> Self {
        let mut d = Self::default();
        d.name = "Focus Right".into();
        // Chat panel takes full right side, hide inspector
        for p in d.panels.iter_mut() {
            match p.id.as_str() {
                "chat" => p.size = 500.0,
                "inspector" => p.visible = false,
                "hierarchy" => p.visible = false,
                "director_desk" => p.visible = false,
                "console" => p.visible = false,
                "director_events" => p.visible = false,
                _ => {}
            }
        }
        d
    }
}

// ──────────────────────────────────────────────────────────
// Layout manager resource
// ──────────────────────────────────────────────────────────

#[derive(Resource)]
pub struct LayoutManager {
    pub layout: LayoutDefinition,
    pub preset_names: Vec<String>,
    pub dirty: bool,
    #[cfg(feature = "persistence")]
    pub persist_path: std::path::PathBuf,
}

impl LayoutManager {
    pub fn new(layout: LayoutDefinition) -> Self {
        Self {
            layout,
            preset_names: vec![
                "Standard".into(),
                "Compact".into(),
                "Wide".into(),
                "Minimal".into(),
                "FocusRight".into(),
            ],
            dirty: false,
            #[cfg(feature = "persistence")]
            persist_path: std::path::PathBuf::from("layout.json"),
        }
    }

    // ── Queries ──

    pub fn is_visible(&self, panel_id: &str) -> bool {
        self.layout
            .panels
            .iter()
            .any(|p| p.id == panel_id && p.visible)
    }

    pub fn panel_config(&self, panel_id: &str) -> Option<&PanelConfig> {
        self.layout.panels.iter().find(|p| p.id == panel_id)
    }

    pub fn config_mut(&mut self, panel_id: &str) -> Option<&mut PanelConfig> {
        self.layout.panels.iter_mut().find(|p| p.id == panel_id)
    }

    pub fn panel_size(&self, panel_id: &str) -> Option<f32> {
        self.panel_config(panel_id).map(|c| c.size)
    }

    pub fn visible_panels_in(&self, position: PanelPosition) -> Vec<&PanelConfig> {
        let mut panels: Vec<_> = self
            .layout
            .panels
            .iter()
            .filter(|p| p.visible && p.position == position)
            .collect();
        panels.sort_by_key(|p| p.order);
        panels
    }

    pub fn all_panel_ids(&self) -> Vec<&str> {
        self.layout.panels.iter().map(|p| p.id.as_str()).collect()
    }

    pub fn visible_panel_ids(&self) -> Vec<&str> {
        self.layout
            .panels
            .iter()
            .filter(|p| p.visible)
            .map(|p| p.id.as_str())
            .collect()
    }

    pub fn floating_panels(&self) -> Vec<&PanelConfig> {
        self.layout
            .panels
            .iter()
            .filter(|p| p.visible && matches!(p.position, PanelPosition::Floating { .. }))
            .collect()
    }

    // ── Mutations ──

    pub fn show_panel(&mut self, panel_id: &str) {
        if let Some(cfg) = self.config_mut(panel_id) {
            if !cfg.visible {
                cfg.visible = true;
                self.dirty = true;
            }
        }
    }

    pub fn hide_panel(&mut self, panel_id: &str) {
        if let Some(cfg) = self.config_mut(panel_id) {
            if cfg.visible {
                cfg.visible = false;
                self.dirty = true;
            }
        }
    }

    pub fn toggle_panel(&mut self, panel_id: &str) -> bool {
        if let Some(cfg) = self.layout.panels.iter_mut().find(|p| p.id == panel_id) {
            cfg.visible = !cfg.visible;
            let visible = cfg.visible;
            self.dirty = true;
            visible
        } else {
            false
        }
    }

    pub fn move_panel(&mut self, panel_id: &str, position: PanelPosition) -> bool {
        if let Some(cfg) = self.config_mut(panel_id) {
            cfg.position = position;
            self.dirty = true;
            true
        } else {
            false
        }
    }

    pub fn resize_panel(&mut self, panel_id: &str, size: f32) -> bool {
        if let Some(cfg) = self.config_mut(panel_id) {
            cfg.size = size.clamp(50.0, 2000.0);
            self.dirty = true;
            true
        } else {
            false
        }
    }

    pub fn set_order(&mut self, panel_id: &str, order: usize) -> bool {
        if let Some(cfg) = self.config_mut(panel_id) {
            cfg.order = order;
            self.dirty = true;
            true
        } else {
            false
        }
    }

    pub fn focus_panel(&mut self, panel_id: &str) {
        // Show and bring to top of its position group
        self.show_panel(panel_id);
        // Maximize within its position
        if let Some(cfg) = self.config_mut(panel_id) {
            let max_size = match cfg.position {
                PanelPosition::Left | PanelPosition::Right => 800.0,
                PanelPosition::Bottom | PanelPosition::Top => 600.0,
                PanelPosition::Floating { .. } => 1200.0,
            };
            cfg.size = max_size;
            self.dirty = true;
        }
    }

    pub fn apply_preset(&mut self, name: &str) -> bool {
        let preset = match name.to_lowercase().as_str() {
            "standard" | "default" => LayoutDefinition::default(),
            "compact" => LayoutDefinition::compact(),
            "wide" => LayoutDefinition::wide(),
            "minimal" => LayoutDefinition::minimal(),
            "focusright" | "focus_right" => LayoutDefinition::focus_right(),
            _ => return false,
        };
        self.layout = preset;
        self.dirty = true;
        true
    }

    pub fn reset_defaults(&mut self) {
        self.layout = LayoutDefinition::default();
        self.dirty = true;
    }

    // ── Persistence ──

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(&self.layout).unwrap_or_default()
    }

    pub fn from_json(json: &str) -> Result<LayoutDefinition, String> {
        serde_json::from_str(json).map_err(|e| e.to_string())
    }

    pub fn apply_json(&mut self, json: &str) -> Result<(), String> {
        let def = Self::from_json(json)?;
        self.layout = def;
        self.dirty = true;
        Ok(())
    }

    pub fn snapshot(&self) -> LayoutDefinition {
        self.layout.clone()
    }

    pub fn restore_snapshot(&mut self, snapshot: LayoutDefinition) {
        self.layout = snapshot;
        self.dirty = true;
    }
}

// ──────────────────────────────────────────────────────────
// Agent-controllable layout commands
// ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LayoutCommand {
    ShowPanel { panel_id: String },
    HidePanel { panel_id: String },
    TogglePanel { panel_id: String },
    MovePanel { panel_id: String, position: PanelPosition },
    ResizePanel { panel_id: String, size: f32 },
    ApplyPreset { name: String },
    ResetLayout,
    SwitchToCompact,
    SwitchToWide,
    SwitchToMinimal,
    FocusPanel { panel_id: String },
    ShowOnly { panel_ids: Vec<String> },
    /// Apply a full layout from JSON
    ApplyLayoutFromJson { json: String },
}

impl LayoutCommand {
    /// Execute this command against a LayoutManager, returning a description of what was done.
    pub fn execute(&self, mgr: &mut LayoutManager) -> String {
        match self {
            Self::ShowPanel { panel_id } => {
                mgr.show_panel(panel_id);
                format!("shown panel '{}'", panel_id)
            }
            Self::HidePanel { panel_id } => {
                mgr.hide_panel(panel_id);
                format!("hidden panel '{}'", panel_id)
            }
            Self::TogglePanel { panel_id } => {
                let visible = mgr.toggle_panel(panel_id);
                format!(
                    "panel '{}' now {}",
                    panel_id,
                    if visible { "visible" } else { "hidden" }
                )
            }
            Self::MovePanel {
                panel_id,
                position,
            } => {
                let ok = mgr.move_panel(panel_id, *position);
                format!(
                    "move panel '{}': {}",
                    panel_id,
                    if ok { "ok" } else { "panel not found" }
                )
            }
            Self::ResizePanel { panel_id, size } => {
                let ok = mgr.resize_panel(panel_id, *size);
                format!(
                    "resize panel '{}': {}",
                    panel_id,
                    if ok { "ok" } else { "panel not found" }
                )
            }
            Self::ApplyPreset { name } => {
                let ok = mgr.apply_preset(name);
                format!(
                    "apply preset '{}': {}",
                    name,
                    if ok { "ok" } else { "unknown preset" }
                )
            }
            Self::ResetLayout => {
                mgr.reset_defaults();
                "layout reset to defaults".into()
            }
            Self::SwitchToCompact => {
                mgr.apply_preset("compact");
                "switched to compact layout".into()
            }
            Self::SwitchToWide => {
                mgr.apply_preset("wide");
                "switched to wide layout".into()
            }
            Self::SwitchToMinimal => {
                mgr.apply_preset("minimal");
                "switched to minimal layout".into()
            }
            Self::FocusPanel { panel_id } => {
                mgr.focus_panel(panel_id);
                format!("focused panel '{}'", panel_id)
            }
            Self::ShowOnly { panel_ids } => {
                // Hide all, then show only the specified ones
                for p in mgr.layout.panels.iter_mut() {
                    p.visible = panel_ids.contains(&p.id);
                }
                mgr.dirty = true;
                format!("showing only panels: {:?}", panel_ids)
            }
            Self::ApplyLayoutFromJson { json } => {
                match mgr.apply_json(json) {
                    Ok(()) => "layout applied from JSON".into(),
                    Err(e) => format!("failed to apply layout: {}", e),
                }
            }
        }
    }
}

// ──────────────────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_layout_has_core_panels() {
        let layout = LayoutDefinition::default();
        let ids: Vec<_> = layout.panels.iter().map(|p| &p.id).collect();
        assert!(ids.contains(&&"chat".to_string()));
        assert!(ids.contains(&&"inspector".to_string()));
        assert!(ids.contains(&&"hierarchy".to_string()));
        assert!(ids.contains(&&"console".to_string()));
    }

    #[test]
    fn test_layout_manager_visibility() {
        let mut mgr = LayoutManager::new(LayoutDefinition::default());

        assert!(mgr.is_visible("chat"));
        assert!(mgr.is_visible("hierarchy"));
        assert!(!mgr.is_visible("runtime_agents"));
        assert!(!mgr.is_visible("debug"));
    }

    #[test]
    fn test_toggle_panel() {
        let mut mgr = LayoutManager::new(LayoutDefinition::default());

        // Toggle off -> false
        let visible = mgr.toggle_panel("chat");
        assert!(!visible);
        assert!(!mgr.is_visible("chat"));

        // Toggle on -> true
        let visible = mgr.toggle_panel("chat");
        assert!(visible);
        assert!(mgr.is_visible("chat"));
    }

    #[test]
    fn test_move_panel() {
        let mut mgr = LayoutManager::new(LayoutDefinition::default());

        mgr.move_panel("chat", PanelPosition::Bottom);
        let cfg = mgr.panel_config("chat").unwrap();
        assert_eq!(cfg.position, PanelPosition::Bottom);
    }

    #[test]
    fn test_resize_panel() {
        let mut mgr = LayoutManager::new(LayoutDefinition::default());

        mgr.resize_panel("chat", 500.0);
        assert_eq!(mgr.panel_size("chat"), Some(500.0));

        // Clamp
        mgr.resize_panel("chat", 10.0);
        assert_eq!(mgr.panel_size("chat"), Some(50.0));
    }

    #[test]
    fn test_show_only() {
        let mut mgr = LayoutManager::new(LayoutDefinition::default());

        LayoutCommand::ShowOnly {
            panel_ids: vec!["chat".into()],
        }
        .execute(&mut mgr);

        assert!(mgr.is_visible("chat"));
        assert!(!mgr.is_visible("hierarchy"));
        assert!(!mgr.is_visible("inspector"));
        assert!(!mgr.is_visible("console"));
    }

    #[test]
    fn test_preset_switching() {
        let mut mgr = LayoutManager::new(LayoutDefinition::default());
        assert!(mgr.apply_preset("compact"));
        assert_eq!(mgr.layout.name, "Compact Layout");

        assert!(mgr.apply_preset("wide"));
        assert_eq!(mgr.layout.name, "Wide Layout");

        assert!(mgr.apply_preset("minimal"));
        // minimal only shows chat
        assert!(mgr.is_visible("chat"));
        assert!(!mgr.is_visible("hierarchy"));

        assert!(!mgr.apply_preset("nonexistent"));
    }

    #[test]
    fn test_visible_panels_in() {
        let mgr = LayoutManager::new(LayoutDefinition::default());

        let left_panels = mgr.visible_panels_in(PanelPosition::Left);
        assert!(!left_panels.is_empty());

        let right_panels = mgr.visible_panels_in(PanelPosition::Right);
        assert!(right_panels.iter().any(|p| p.id == "chat"));
    }

    #[test]
    fn test_json_roundtrip() {
        let mgr = LayoutManager::new(LayoutDefinition::default());
        let json = mgr.to_json();

        let mut mgr2 = LayoutManager::new(LayoutDefinition::compact());
        mgr2.apply_json(&json).unwrap();

        assert_eq!(mgr2.layout.name, "Standard Editor Layout");
        assert!(mgr2.is_visible("chat"));
    }

    #[test]
    fn test_layout_command_execute() {
        let mut mgr = LayoutManager::new(LayoutDefinition::default());

        let msg = LayoutCommand::HidePanel {
            panel_id: "chat".into(),
        }
        .execute(&mut mgr);
        assert!(msg.contains("hidden"));
        assert!(!mgr.is_visible("chat"));

        let msg = LayoutCommand::ShowPanel {
            panel_id: "chat".into(),
        }
        .execute(&mut mgr);
        assert!(!msg.contains("hidden"));
        assert!(mgr.is_visible("chat"));

        let msg = LayoutCommand::SwitchToCompact.execute(&mut mgr);
        assert!(msg.contains("compact"));
    }
}
