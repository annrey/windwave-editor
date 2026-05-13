//! Project System - Phase 4.4
//!
//! Project manifest, templates, recent projects, and configuration management.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Project manifest - defines a game project
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectManifest {
    /// Project name
    pub name: String,
    /// Project version (semver)
    pub version: String,
    /// List of scenes in the project
    pub scenes: Vec<String>,
    /// Assets directory path (relative to project root)
    pub assets_dir: String,
    /// Agent configuration for this project
    pub agent_config: AgentProjectConfig,
    /// Engine type (bevy, unity, godot, etc.)
    pub engine: String,
    /// Engine version requirement
    pub engine_version: String,
    /// Project metadata
    pub metadata: HashMap<String, serde_json::Value>,
}

impl Default for ProjectManifest {
    fn default() -> Self {
        Self {
            name: "Untitled Project".to_string(),
            version: "0.1.0".to_string(),
            scenes: vec!["main.scene".to_string()],
            assets_dir: "assets".to_string(),
            agent_config: AgentProjectConfig::default(),
            engine: "bevy".to_string(),
            engine_version: "0.17".to_string(),
            metadata: HashMap::new(),
        }
    }
}

/// Agent configuration for a project
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentProjectConfig {
    /// Default LLM provider
    pub default_llm_provider: String,
    /// Default model for this project
    pub default_model: String,
    /// Custom prompts for this project
    pub custom_prompts: HashMap<String, String>,
    /// Preferred confirmation level
    pub confirmation_level: String,
    /// Max steps for agent tasks
    pub max_steps: usize,
}

impl Default for AgentProjectConfig {
    fn default() -> Self {
        Self {
            default_llm_provider: "openai".to_string(),
            default_model: "gpt-4o".to_string(),
            custom_prompts: HashMap::new(),
            confirmation_level: "destructive".to_string(),
            max_steps: 10,
        }
    }
}

/// Project manager - handles loading/saving projects
#[derive(Debug, Default)]
pub struct ProjectManager {
    current_project: Option<ProjectManifest>,
    project_path: Option<PathBuf>,
}

impl ProjectManifest {
    /// Create a new project manifest
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Default::default()
        }
    }

    /// Set engine
    pub fn with_engine(mut self, engine: impl Into<String>, version: impl Into<String>) -> Self {
        self.engine = engine.into();
        self.engine_version = version.into();
        self
    }

    /// Add a scene
    pub fn add_scene(&mut self, scene_path: impl Into<String>) {
        let path = scene_path.into();
        if !self.scenes.contains(&path) {
            self.scenes.push(path);
        }
    }

    /// Remove a scene
    pub fn remove_scene(&mut self, scene_path: &str) {
        self.scenes.retain(|s| s != scene_path);
    }

    /// Set metadata value
    pub fn set_metadata(&mut self, key: impl Into<String>, value: impl Serialize) -> Result<(), serde_json::Error> {
        self.metadata.insert(key.into(), serde_json::to_value(value)?);
        Ok(())
    }

    /// Get metadata value
    pub fn get_metadata(&self, key: &str) -> Option<&serde_json::Value> {
        self.metadata.get(key)
    }

    /// Save manifest to file
    pub fn save(&self, path: impl AsRef<Path>) -> Result<(), ProjectError> {
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)?;
        Ok(())
    }

    /// Load manifest from file
    pub fn load(path: impl AsRef<Path>) -> Result<Self, ProjectError> {
        let json = std::fs::read_to_string(path)?;
        let manifest = serde_json::from_str(&json)?;
        Ok(manifest)
    }

    /// Create default project structure
    pub fn create_project_structure(&self, base_path: impl AsRef<Path>) -> Result<(), ProjectError> {
        let base = base_path.as_ref();

        // Create directories
        std::fs::create_dir_all(base.join(&self.assets_dir))?;
        std::fs::create_dir_all(base.join("scenes"))?;
        std::fs::create_dir_all(base.join("scripts"))?;

        // Save manifest
        self.save(base.join("project.json"))?;

        // Create default scene if none exist
        if self.scenes.is_empty() {
            let scene_path = base.join("scenes").join("main.scene");
            let default_scene = crate::scene_serializer::SceneFile::new("Main Scene");
            default_scene.save(&scene_path)
                .map_err(|e| ProjectError::Io(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("Failed to save default scene: {}", e)
                )))?;
        }

        Ok(())
    }
}

impl ProjectManager {
    /// Create new project manager
    pub fn new() -> Self {
        Self::default()
    }

    /// Load a project
    pub fn load_project(&mut self, path: impl AsRef<Path>) -> Result<&ProjectManifest, ProjectError> {
        let manifest = ProjectManifest::load(&path)?;
        self.project_path = Some(path.as_ref().parent().map(|p| p.to_path_buf()).unwrap_or_default());
        self.current_project = Some(manifest);
        Ok(self.current_project.as_ref().unwrap())
    }

    /// Create a new project
    pub fn create_project(
        &mut self,
        name: impl Into<String>,
        path: impl AsRef<Path>,
    ) -> Result<&ProjectManifest, ProjectError> {
        let manifest = ProjectManifest::new(name);
        manifest.create_project_structure(&path)?;
        self.project_path = Some(path.as_ref().to_path_buf());
        self.current_project = Some(manifest);
        Ok(self.current_project.as_ref().unwrap())
    }

    /// Get current project
    pub fn current_project(&self) -> Option<&ProjectManifest> {
        self.current_project.as_ref()
    }

    /// Get current project path
    pub fn current_path(&self) -> Option<&Path> {
        self.project_path.as_deref()
    }

    /// Save current project
    pub fn save_current(&self) -> Result<(), ProjectError> {
        if let (Some(project), Some(path)) = (&self.current_project, &self.project_path) {
            project.save(path.join("project.json"))?;
            Ok(())
        } else {
            Err(ProjectError::NoProjectOpen)
        }
    }

    /// Get full path for a project-relative path
    pub fn resolve_path(&self, relative: impl AsRef<Path>) -> Option<PathBuf> {
        self.project_path.as_ref().map(|base| base.join(relative))
    }

    /// Check if a project is open
    pub fn has_project_open(&self) -> bool {
        self.current_project.is_some()
    }

    /// Get project name (or default)
    pub fn project_name(&self) -> &str {
        self.current_project
            .as_ref()
            .map(|p| p.name.as_str())
            .unwrap_or("No Project")
    }
}

/// Project-related errors
#[derive(Debug, thiserror::Error)]
pub enum ProjectError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON serialization error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("No project is currently open")]
    NoProjectOpen,
    #[error("Project not found: {0}")]
    ProjectNotFound(String),
    #[error("Invalid project structure")]
    InvalidStructure,
}

/// Default project manifest filename
pub const PROJECT_MANIFEST_FILE: &str = "project.json";

/// Check if a directory contains a valid project
pub fn is_valid_project_dir(path: impl AsRef<Path>) -> bool {
    path.as_ref().join(PROJECT_MANIFEST_FILE).exists()
}

/// Find project root starting from a path (walk up directory tree)
pub fn find_project_root(start: impl AsRef<Path>) -> Option<PathBuf> {
    let mut current = start.as_ref().to_path_buf();

    loop {
        if is_valid_project_dir(&current) {
            return Some(current);
        }

        match current.parent() {
            Some(parent) => current = parent.to_path_buf(),
            None => return None,
        }
    }
}

// ---------------------------------------------------------------------------
// Project templates
// ---------------------------------------------------------------------------

/// Predefined project templates for quick project creation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProjectTemplate {
    /// Empty Bevy project with minimal setup
    Empty,
    /// 2D platformer starter template
    Platform2D,
    /// Top-down RPG starter template
    TopDownRPG,
    /// First-person 3D template
    FirstPerson3D,
    /// UI-heavy application template
    UIApplication,
    /// Narrative / visual novel template
    Narrative,
}

impl ProjectTemplate {
    /// List all available templates with descriptions
    pub fn all() -> &'static [(ProjectTemplate, &'static str, &'static str)] {
        &[
            (Self::Empty, "Empty", "Minimal Bevy project with a camera"),
            (Self::Platform2D, "2D Platformer", "Side-scrolling platformer with physics"),
            (Self::TopDownRPG, "Top-Down RPG", "Top-down character movement and tilemap"),
            (Self::FirstPerson3D, "First Person 3D", "First-person camera with basic 3D scene"),
            (Self::UIApplication, "UI Application", "UI-heavy app with panels and widgets"),
            (Self::Narrative, "Narrative Game", "AI-driven narrative game with dialogue system"),
        ]
    }

    /// Default resolution for template
    pub fn default_resolution(&self) -> (u32, u32) {
        match self {
            Self::Empty | Self::FirstPerson3D => (1280, 720),
            Self::Platform2D | Self::TopDownRPG => (960, 540),
            Self::UIApplication => (1024, 768),
            Self::Narrative => (1280, 720),
        }
    }

    /// Default scene name
    pub fn default_scene(&self) -> &'static str {
        match self {
            Self::Empty => "main.scene",
            Self::Platform2D => "level_01.scene",
            Self::TopDownRPG => "overworld.scene",
            Self::FirstPerson3D => "main_level.scene",
            Self::UIApplication => "main_ui.scene",
            Self::Narrative => "chapter_01.scene",
        }
    }

    /// Whether this template includes physics setup
    pub fn has_physics(&self) -> bool {
        matches!(self, Self::Platform2D | Self::FirstPerson3D)
    }

    /// Whether this template uses 3D rendering
    pub fn is_3d(&self) -> bool {
        matches!(self, Self::FirstPerson3D)
    }
}

impl ProjectManifest {
    /// Create a project using a template
    pub fn from_template(template: ProjectTemplate, name: impl Into<String>) -> Self {
        let name = name.into();
        let (res_w, res_h) = template.default_resolution();
        let scene = template.default_scene().to_string();

        let mut metadata = HashMap::new();
        metadata.insert("template".to_string(), serde_json::json!(format!("{:?}", template)));
        metadata.insert("resolution".to_string(), serde_json::json!([res_w, res_h]));
        if template.has_physics() {
            metadata.insert("physics".to_string(), serde_json::json!(true));
        }
        metadata.insert("render_3d".to_string(), serde_json::json!(template.is_3d()));

        Self {
            name,
            scenes: vec![scene],
            metadata,
            ..Default::default()
        }
    }
}

// ---------------------------------------------------------------------------
// Recent projects
// ---------------------------------------------------------------------------

/// A single recent project entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecentProject {
    pub name: String,
    pub path: PathBuf,
    pub template: Option<String>,
    pub last_opened: String, // ISO 8601 timestamp
}

/// List of recently opened projects, persisted to disk
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RecentProjectsList {
    pub projects: Vec<RecentProject>,
}

impl RecentProjectsList {
    /// Maximum number of recent projects to keep
    const MAX_RECENT: usize = 10;

    /// Add or update a project in the recent list
    pub fn add(&mut self, name: impl Into<String>, path: impl Into<PathBuf>, template: Option<String>) {
        let path: PathBuf = path.into();
        let name: String = name.into();

        // Remove existing entry with same path
        self.projects.retain(|p| p.path != path);

        // Insert at front
        self.projects.insert(0, RecentProject {
            name,
            path,
            template,
            last_opened: chrono::Utc::now().to_rfc3339(),
        });

        // Trim to max
        self.projects.truncate(Self::MAX_RECENT);
    }

    /// Remove a project from the list by path
    pub fn remove(&mut self, path: &Path) {
        self.projects.retain(|p| p.path != path);
    }

    /// Remove entries pointing to non-existent directories
    pub fn prune_missing(&mut self) {
        self.projects.retain(|p| p.path.exists());
    }

    /// Save recent projects to disk
    pub fn save(&self, path: impl AsRef<Path>) -> Result<(), ProjectError> {
        let json = serde_json::to_string_pretty(self)?;
        if let Some(parent) = path.as_ref().parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, json)?;
        Ok(())
    }

    /// Load recent projects from disk
    pub fn load(path: impl AsRef<Path>) -> Result<Self, ProjectError> {
        match std::fs::read_to_string(&path) {
            Ok(json) => {
                let list: Self = serde_json::from_str(&json)?;
                Ok(list)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                Ok(Self::default())
            }
            Err(e) => Err(ProjectError::Io(e)),
        }
    }

    /// Default path for recent projects file
    pub fn default_path() -> PathBuf {
        dirs_next().unwrap_or_else(|| PathBuf::from("."))
            .join("agentedit")
            .join("recent_projects.json")
    }
}

fn dirs_next() -> Option<PathBuf> {
    std::env::var("AGENTEDIT_CONFIG_DIR").ok()
        .map(PathBuf::from)
        .or_else(|| {
            #[cfg(target_os = "macos")]
            { std::env::var("HOME").ok().map(|h| PathBuf::from(h).join("Library").join("Application Support")) }
            #[cfg(not(target_os = "macos"))]
            { dirs_next_cross() }
        })
}

#[cfg(not(target_os = "macos"))]
fn dirs_next_cross() -> Option<PathBuf> {
    std::env::var("XDG_CONFIG_HOME").ok()
        .map(PathBuf::from)
        .or_else(|| std::env::var("HOME").ok().map(|h| PathBuf::from(h).join(".config")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_project_manifest_defaults() {
        let manifest = ProjectManifest::new("Test Project");
        assert_eq!(manifest.name, "Test Project");
        assert_eq!(manifest.engine, "bevy");
        assert!(!manifest.scenes.is_empty());
    }

    #[test]
    fn test_project_save_load() {
        let dir = std::env::temp_dir().join("test_project");
        let _ = std::fs::remove_dir_all(&dir);

        let manifest = ProjectManifest::new("Test");
        manifest.create_project_structure(&dir).unwrap();

        assert!(dir.join("project.json").exists());
        assert!(dir.join("assets").exists());
        assert!(dir.join("scenes").exists());

        let loaded = ProjectManifest::load(dir.join("project.json")).unwrap();
        assert_eq!(loaded.name, "Test");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_find_project_root() {
        let dir = std::env::temp_dir().join("test_project_find");
        let _ = std::fs::remove_dir_all(&dir);

        // Create project
        let manifest = ProjectManifest::new("Test");
        manifest.create_project_structure(&dir).unwrap();

        // Create subdirectory
        let subdir = dir.join("assets").join("textures");
        std::fs::create_dir_all(&subdir).unwrap();

        // Find from subdirectory
        let found = find_project_root(&subdir);
        assert_eq!(found, Some(dir.clone()));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_project_from_template() {
        let manifest = ProjectManifest::from_template(ProjectTemplate::Platform2D, "MyGame");
        assert_eq!(manifest.name, "MyGame");
        assert_eq!(manifest.scenes, vec!["level_01.scene"]);
        assert!(manifest.metadata.contains_key("template"));
        assert!(manifest.metadata.contains_key("physics"));
    }

    #[test]
    fn test_all_templates_have_metadata() {
        for (template, name, _desc) in ProjectTemplate::all() {
            let manifest = ProjectManifest::from_template(*template, *name);
            assert!(!manifest.name.is_empty());
            assert!(!manifest.scenes.is_empty());
            assert!(manifest.metadata.contains_key("resolution"));
        }
    }

    #[test]
    fn test_recent_projects_add_and_trim() {
        let mut list = RecentProjectsList::default();
        for i in 0..15 {
            let dir = std::env::temp_dir().join(format!("proj_{}", i));
            list.add(format!("Project {}", i), &dir, None);
        }
        assert_eq!(list.projects.len(), RecentProjectsList::MAX_RECENT);
        // Most recent should be first
        assert_eq!(list.projects[0].name, "Project 14");
    }

    #[test]
    fn test_recent_projects_dedup() {
        let mut list = RecentProjectsList::default();
        let dir = std::env::temp_dir().join("dedup_test");
        list.add("First", &dir, None);
        list.add("Second", &dir, None);
        assert_eq!(list.projects.len(), 1);
        assert_eq!(list.projects[0].name, "Second");
    }

    #[test]
    fn test_recent_projects_remove() {
        let mut list = RecentProjectsList::default();
        let dir = std::env::temp_dir().join("remove_test");
        list.add("Test", &dir, None);
        assert_eq!(list.projects.len(), 1);
        list.remove(&dir);
        assert_eq!(list.projects.len(), 0);
    }

    #[test]
    fn test_recent_projects_save_load() {
        let file = std::env::temp_dir().join("recent_projects_test.json");
        let _ = std::fs::remove_file(&file);

        let mut list = RecentProjectsList::default();
        let dir = std::env::temp_dir().join("save_load_test");
        list.add("SavedProject", &dir, Some("Empty".into()));

        list.save(&file).unwrap();
        let loaded = RecentProjectsList::load(&file).unwrap();
        assert_eq!(loaded.projects.len(), 1);
        assert_eq!(loaded.projects[0].name, "SavedProject");

        let _ = std::fs::remove_file(&file);
    }

    #[test]
    fn test_recent_projects_load_missing_file() {
        let list = RecentProjectsList::load("/nonexistent/path/recent.json").unwrap();
        assert!(list.projects.is_empty());
    }
}
