//! Project Management System - Sprint 6: 项目管理系统
//!
//! 项目创建向导、模板管理、最近项目列表

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// 项目模板
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectTemplate {
    pub id: String,
    pub name: String,
    pub description: String,
    pub engine: String,
    pub engine_version: String,
    pub category: TemplateCategory,
    pub default_settings: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TemplateCategory {
    Empty,          // 空项目
    Platformer2D,   // 2D平台跳跃
    RPG,            // RPG
    Puzzle,         // 解谜
    Narrative,      // 叙事/视觉小说
    Strategy,       // 策略
}

impl TemplateCategory {
    pub fn label(&self) -> &'static str {
        match self {
            TemplateCategory::Empty => "空项目",
            TemplateCategory::Platformer2D => "2D平台跳跃",
            TemplateCategory::RPG => "RPG",
            TemplateCategory::Puzzle => "解谜",
            TemplateCategory::Narrative => "叙事/视觉小说",
            TemplateCategory::Strategy => "策略",
        }
    }
}

/// 项目元数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectMeta {
    pub id: String,
    pub name: String,
    pub path: PathBuf,
    pub engine: String,
    pub engine_version: String,
    pub template_id: Option<String>,
    pub created_at: u64,
    pub last_opened: u64,
    pub tags: Vec<String>,
}

/// 项目管理器
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectManager {
    pub recent_projects: Vec<ProjectMeta>,
    pub templates: Vec<ProjectTemplate>,
    max_recent: usize,
}

impl ProjectManager {
    pub fn new() -> Self {
        Self {
            recent_projects: Vec::new(),
            templates: Self::default_templates(),
            max_recent: 10,
        }
    }

    fn default_templates() -> Vec<ProjectTemplate> {
        vec![
            ProjectTemplate {
                id: "empty".into(),
                name: "空项目".into(),
                description: "从零开始的空白项目".into(),
                engine: "bevy".into(),
                engine_version: "0.17".into(),
                category: TemplateCategory::Empty,
                default_settings: HashMap::new(),
            },
            ProjectTemplate {
                id: "platformer_2d".into(),
                name: "2D平台跳跃".into(),
                description: "包含玩家控制器、物理、摄像机的2D平台跳跃游戏模板".into(),
                engine: "bevy".into(),
                engine_version: "0.17".into(),
                category: TemplateCategory::Platformer2D,
                default_settings: HashMap::new(),
            },
            ProjectTemplate {
                id: "rpg_basic".into(),
                name: "RPG基础".into(),
                description: "包含角色系统、背包、对话系统的RPG模板".into(),
                engine: "bevy".into(),
                engine_version: "0.17".into(),
                category: TemplateCategory::RPG,
                default_settings: HashMap::new(),
            },
            ProjectTemplate {
                id: "narrative".into(),
                name: "叙事/视觉小说".into(),
                description: "包含故事图、对话树、角色系统的叙事游戏模板".into(),
                engine: "bevy".into(),
                engine_version: "0.17".into(),
                category: TemplateCategory::Narrative,
                default_settings: HashMap::new(),
            },
        ]
    }

    /// 记录打开的项目
    pub fn record_open(&mut self, project: ProjectMeta) {
        self.recent_projects.retain(|p| p.id != project.id);
        self.recent_projects.insert(0, project);
        if self.recent_projects.len() > self.max_recent {
            self.recent_projects.pop();
        }
    }

    /// 获取最近项目
    pub fn recent(&self) -> &[ProjectMeta] {
        &self.recent_projects
    }

    /// 按模板创建项目元数据
    pub fn create_from_template(&self, name: &str, path: PathBuf, template_id: &str) -> Option<ProjectMeta> {
        let template = self.templates.iter().find(|t| t.id == template_id)?;
        Some(ProjectMeta {
            id: format!("proj_{}", chrono::Utc::now().timestamp_millis()),
            name: name.into(),
            path,
            engine: template.engine.clone(),
            engine_version: template.engine_version.clone(),
            template_id: Some(template.id.clone()),
            created_at: chrono::Utc::now().timestamp_millis() as u64,
            last_opened: chrono::Utc::now().timestamp_millis() as u64,
            tags: Vec::new(),
        })
    }

    /// 搜索项目
    pub fn search(&self, query: &str) -> Vec<&ProjectMeta> {
        let q = query.to_lowercase();
        self.recent_projects.iter().filter(|p| {
            p.name.to_lowercase().contains(&q) || p.tags.iter().any(|t| t.to_lowercase().contains(&q))
        }).collect()
    }

    /// 获取所有模板
    pub fn templates(&self) -> &[ProjectTemplate] {
        &self.templates
    }
}

impl Default for ProjectManager {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// 资源管理系统增强
// ============================================================================

/// 资源类型
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AssetType {
    Texture,    // 纹理/图片
    Model,      // 3D模型
    Audio,      // 音频
    Font,       // 字体
    Script,     // 脚本
    Prefab,     // 预制体
    Scene,      // 场景
    Shader,     // Shader
    Animation,  // 动画
    Other,      // 其他
}

impl AssetType {
    pub fn label(&self) -> &'static str {
        match self {
            AssetType::Texture => "纹理",
            AssetType::Model => "模型",
            AssetType::Audio => "音频",
            AssetType::Font => "字体",
            AssetType::Script => "脚本",
            AssetType::Prefab => "预制体",
            AssetType::Scene => "场景",
            AssetType::Shader => "Shader",
            AssetType::Animation => "动画",
            AssetType::Other => "其他",
        }
    }
}

/// 资源条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetEntry {
    pub id: String,
    pub name: String,
    pub path: PathBuf,
    pub asset_type: AssetType,
    pub tags: Vec<String>,
    pub size_bytes: u64,
    pub referenced_by: Vec<String>,  // 被哪些资源引用
    pub references: Vec<String>,     // 引用了哪些资源
    pub last_modified: u64,
}

/// 资源管理器
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetManager {
    assets: HashMap<String, AssetEntry>,
    tags: HashMap<String, Vec<String>>,  // tag → asset_ids
}

impl AssetManager {
    pub fn new() -> Self {
        Self { assets: HashMap::new(), tags: HashMap::new() }
    }

    pub fn register(&mut self, asset: AssetEntry) {
        let id = asset.id.clone();
        for tag in &asset.tags {
            self.tags.entry(tag.clone()).or_default().push(id.clone());
        }
        self.assets.insert(id, asset);
    }

    pub fn get(&self, id: &str) -> Option<&AssetEntry> {
        self.assets.get(id)
    }

    pub fn search_by_tag(&self, tag: &str) -> Vec<&AssetEntry> {
        self.tags.get(tag)
            .map(|ids| ids.iter().filter_map(|id| self.assets.get(id)).collect())
            .unwrap_or_default()
    }

    pub fn search_by_name(&self, query: &str) -> Vec<&AssetEntry> {
        let q = query.to_lowercase();
        self.assets.values().filter(|a| a.name.to_lowercase().contains(&q)).collect()
    }

    pub fn search_by_type(&self, asset_type: AssetType) -> Vec<&AssetEntry> {
        self.assets.values().filter(|a| a.asset_type == asset_type).collect()
    }

    pub fn all(&self) -> Vec<&AssetEntry> {
        self.assets.values().collect()
    }

    pub fn total_count(&self) -> usize {
        self.assets.len()
    }
}

impl Default for AssetManager {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// 运行时调试面板
// ============================================================================

/// 性能度量
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    pub fps: f32,
    pub frame_time_ms: f32,
    pub entity_count: usize,
    pub system_count: usize,
    pub memory_mb: f32,
}

/// 调试面板
#[derive(Debug, Clone)]
pub struct DebugPanel {
    pub entity_inspector: EntityInspector,
    pub performance_monitor: PerformanceMonitor,
    pub log_viewer: LogViewer,
}

#[derive(Debug, Clone)]
pub struct EntityInspector {
    pub selected_entity: Option<u64>,
    pub components: Vec<String>,
    pub properties: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct PerformanceMonitor {
    pub history: Vec<PerformanceMetrics>,
    pub max_history: usize,
}

#[derive(Debug, Clone)]
pub struct LogViewer {
    pub logs: Vec<LogEntry>,
    pub max_logs: usize,
    pub filter: Option<String>,
}

#[derive(Debug, Clone)]
pub struct LogEntry {
    pub timestamp: u64,
    pub level: LogLevel,
    pub message: String,
    pub source: String,
}

#[derive(Debug, Clone)]
pub enum LogLevel {
    Debug, Info, Warn, Error,
}

impl PerformanceMonitor {
    pub fn new(max_history: usize) -> Self {
        Self { history: Vec::new(), max_history }
    }

    pub fn record(&mut self, metrics: PerformanceMetrics) {
        self.history.push(metrics);
        if self.history.len() > self.max_history {
            self.history.remove(0);
        }
    }

    pub fn avg_fps(&self) -> f32 {
        if self.history.is_empty() { return 0.0; }
        self.history.iter().map(|m| m.fps).sum::<f32>() / self.history.len() as f32
    }

    pub fn min_fps(&self) -> f32 {
        self.history.iter().map(|m| m.fps).fold(f32::MAX, |a, b| a.min(b))
    }
}

// ============================================================================
// 测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_project_manager_templates() {
        let pm = ProjectManager::new();
        assert_eq!(pm.templates().len(), 4);
    }

    #[test]
    fn test_project_manager_recent() {
        let mut pm = ProjectManager::new();
        let meta = ProjectMeta {
            id: "test_1".into(),
            name: "TestProject".into(),
            path: PathBuf::from("/test"),
            engine: "bevy".into(),
            engine_version: "0.17".into(),
            template_id: None,
            created_at: 0,
            last_opened: 0,
            tags: Vec::new(),
        };
        pm.record_open(meta);
        assert_eq!(pm.recent().len(), 1);
    }

    #[test]
    fn test_asset_manager_registration() {
        let mut am = AssetManager::new();
        am.register(AssetEntry {
            id: "tex_1".into(),
            name: "player_sprite.png".into(),
            path: PathBuf::from("assets/player.png"),
            asset_type: AssetType::Texture,
            tags: vec!["player".into(), "sprite".into()],
            size_bytes: 1024,
            referenced_by: Vec::new(),
            references: Vec::new(),
            last_modified: 0,
        });
        assert_eq!(am.total_count(), 1);
        assert_eq!(am.search_by_tag("player").len(), 1);
    }

    #[test]
    fn test_performance_monitor() {
        let mut pm = PerformanceMonitor::new(100);
        pm.record(PerformanceMetrics {
            fps: 60.0, frame_time_ms: 16.6, entity_count: 100,
            system_count: 20, memory_mb: 256.0,
        });
        assert!((pm.avg_fps() - 60.0).abs() < 0.1);
    }

    #[test]
    fn test_search_by_type() {
        let mut am = AssetManager::new();
        am.register(AssetEntry {
            id: "s_1".into(), name: "test.scene".into(), path: PathBuf::from("test.scene"),
            asset_type: AssetType::Scene, tags: vec![], size_bytes: 0,
            referenced_by: vec![], references: vec![], last_modified: 0,
        });
        assert_eq!(am.search_by_type(AssetType::Scene).len(), 1);
        assert_eq!(am.search_by_type(AssetType::Audio).len(), 0);
    }
}