//! Multi-Selection Tools - Sprint 5: 多选择工具 + Transform Palette
//!
//! 提供框选/套索/刷选等选择模式，以及变换预设面板。

use serde::{Deserialize, Serialize};

/// 选择模式
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SelectionMode {
    /// 单击选择
    Single,
    /// 框选 (矩形区域)
    RectSelect,
    /// 套索 (自由绘制区域)
    Lasso,
    /// 刷选 (画笔模式)
    Brush,
    /// 按类型选择
    SelectByType,
}

impl SelectionMode {
    pub fn label(&self) -> &'static str {
        match self {
            SelectionMode::Single => "单击选择",
            SelectionMode::RectSelect => "框选",
            SelectionMode::Lasso => "套索",
            SelectionMode::Brush => "刷选",
            SelectionMode::SelectByType => "按类型选择",
        }
    }
}

/// 选择上下文
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelectionContext {
    pub mode: SelectionMode,
    pub selected_ids: Vec<u64>,
    pub hovered_id: Option<u64>,
    pub selection_bounds: Option<SelectionBounds>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelectionBounds {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl SelectionContext {
    pub fn new() -> Self {
        Self {
            mode: SelectionMode::Single,
            selected_ids: Vec::new(),
            hovered_id: None,
            selection_bounds: None,
        }
    }

    pub fn select(&mut self, id: u64) {
        if self.mode == SelectionMode::Single {
            self.selected_ids.clear();
        }
        if !self.selected_ids.contains(&id) {
            self.selected_ids.push(id);
        }
    }

    pub fn deselect(&mut self, id: u64) {
        self.selected_ids.retain(|&x| x != id);
    }

    pub fn clear(&mut self) {
        self.selected_ids.clear();
        self.hovered_id = None;
        self.selection_bounds = None;
    }

    pub fn selected_count(&self) -> usize {
        self.selected_ids.len()
    }

    pub fn is_selected(&self, id: u64) -> bool {
        self.selected_ids.contains(&id)
    }
}

impl Default for SelectionContext {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Transform Palette - 变换预设
// ============================================================================

/// 变换预设
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransformPreset {
    pub name: String,
    pub description: String,
    pub transform: TransformData,
    pub category: PresetCategory,
}

/// 变换数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransformData {
    pub position: [f32; 3],
    pub rotation: [f32; 3],
    pub scale: [f32; 3],
}

impl TransformData {
    pub fn identity() -> Self {
        Self {
            position: [0.0, 0.0, 0.0],
            rotation: [0.0, 0.0, 0.0],
            scale: [1.0, 1.0, 1.0],
        }
    }
}

/// 预设分类
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PresetCategory {
    /// 网格对齐
    GridSnap,
    /// 对称放置
    Symmetry,
    /// 随机分布
    Random,
    /// 路径排列
    PathArrange,
    /// 自定义
    Custom,
}

impl PresetCategory {
    pub fn label(&self) -> &'static str {
        match self {
            PresetCategory::GridSnap => "网格对齐",
            PresetCategory::Symmetry => "对称放置",
            PresetCategory::Random => "随机分布",
            PresetCategory::PathArrange => "路径排列",
            PresetCategory::Custom => "自定义",
        }
    }
}

/// 变换预设面板
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransformPalette {
    presets: Vec<TransformPreset>,
}

impl TransformPalette {
    pub fn new() -> Self {
        let mut palette = Self { presets: Vec::new() };
        palette.register_defaults();
        palette
    }

    fn register_defaults(&mut self) {
        self.presets.push(TransformPreset {
            name: "ground_plane".into(),
            description: "地面放置 (y=0)".into(),
            transform: TransformData {
                position: [0.0, 0.0, 0.0],
                rotation: [0.0, 0.0, 0.0],
                scale: [1.0, 1.0, 1.0],
            },
            category: PresetCategory::GridSnap,
        });
        self.presets.push(TransformPreset {
            name: "wall_left".into(),
            description: "左侧墙壁".into(),
            transform: TransformData {
                position: [-5.0, 2.5, 0.0],
                rotation: [0.0, 0.0, 0.0],
                scale: [0.5, 5.0, 1.0],
            },
            category: PresetCategory::Symmetry,
        });
        self.presets.push(TransformPreset {
            name: "wall_right".into(),
            description: "右侧墙壁".into(),
            transform: TransformData {
                position: [5.0, 2.5, 0.0],
                rotation: [0.0, 0.0, 0.0],
                scale: [0.5, 5.0, 1.0],
            },
            category: PresetCategory::Symmetry,
        });
    }

    pub fn register(&mut self, preset: TransformPreset) {
        self.presets.push(preset);
    }

    pub fn get(&self, name: &str) -> Option<&TransformPreset> {
        self.presets.iter().find(|p| p.name == name)
    }

    pub fn list_by_category(&self, category: PresetCategory) -> Vec<&TransformPreset> {
        self.presets.iter().filter(|p| p.category == category).collect()
    }

    pub fn all(&self) -> &[TransformPreset] {
        &self.presets
    }

    pub fn apply_transform(&self, preset_name: &str, entity_position: &mut [f32; 3]) -> Result<(), String> {
        let preset = self.get(preset_name).ok_or_else(|| format!("预设 '{}' 不存在", preset_name))?;
        *entity_position = preset.transform.position;
        Ok(())
    }
}

impl Default for TransformPalette {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// 测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_selection_modes() {
        assert_eq!(SelectionMode::Single.label(), "单击选择");
        assert_eq!(SelectionMode::RectSelect.label(), "框选");
    }

    #[test]
    fn test_selection_context() {
        let mut ctx = SelectionContext::new();
        ctx.mode = SelectionMode::RectSelect;
        ctx.select(1);
        ctx.select(2);
        assert_eq!(ctx.selected_count(), 2);
        assert!(ctx.is_selected(1));
        
        ctx.deselect(1);
        assert!(!ctx.is_selected(1));
        assert_eq!(ctx.selected_count(), 1);
    }

    #[test]
    fn test_single_mode_replaces() {
        let mut ctx = SelectionContext::new();
        ctx.mode = SelectionMode::Single;
        ctx.select(1);
        ctx.select(2);
        assert_eq!(ctx.selected_count(), 1);  // Single mode replaces
        assert!(ctx.is_selected(2));
    }

    #[test]
    fn test_transform_palette_defaults() {
        let palette = TransformPalette::new();
        assert!(palette.all().len() >= 3);
        assert!(palette.get("ground_plane").is_some());
    }

    #[test]
    fn test_transform_palette_apply() {
        let palette = TransformPalette::new();
        let mut pos = [10.0, 10.0, 10.0];
        palette.apply_transform("ground_plane", &mut pos).unwrap();
        assert_eq!(pos, [0.0, 0.0, 0.0]);
    }

    #[test]
    fn test_presets_by_category() {
        let palette = TransformPalette::new();
        let grid = palette.list_by_category(PresetCategory::GridSnap);
        assert!(!grid.is_empty());
    }

    #[test]
    fn test_selection_clear() {
        let mut ctx = SelectionContext::new();
        ctx.select(1);
        ctx.select(2);
        ctx.clear();
        assert_eq!(ctx.selected_count(), 0);
    }
}