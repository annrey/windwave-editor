//! TemplateSkill — Project skeleton library

use super::types::*;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

// Template files content
const PLATFORMER_MAIN: &str = include_str!("../../../../templates/platformer_main.rs");
const PLATFORMER_PLAYER: &str = include_str!("../../../../templates/platformer_player.rs");
const PLATFORMER_CARGO: &str = include_str!("../../../../templates/platformer_cargo.toml");
const RPG_MAIN: &str = include_str!("../../../../templates/rpg_main.rs");
const RPG_MAP: &str = include_str!("../../../../templates/rpg_map.rs");

pub struct TemplateSkill {
    pub(crate) templates: HashMap<String, ProjectSkeleton>,
    engine_index: HashMap<GameEngine, Vec<String>>,
    experience_db: PathBuf,
}

impl TemplateSkill {
    pub fn new(data_dir: impl AsRef<Path>) -> Self {
        let db_path = data_dir.as_ref().join("templates");
        let _ = std::fs::create_dir_all(&db_path);

        let mut skill = Self {
            templates: HashMap::new(),
            engine_index: HashMap::new(),
            experience_db: db_path,
        };

        skill.load_builtin_templates();
        skill.load_learned_templates();
        skill
    }

    /// Load built-in templates for common game types
    fn load_builtin_templates(&mut self) {
        // 2D Platformer template
        self.register_template(ProjectSkeleton {
            name: "2d_platformer".to_string(),
            engine: GameEngine::Bevy,
            description: "Side-scrolling platformer with physics".to_string(),
            files: vec![
                SkeletonFile {
                    path: "src/main.rs".to_string(),
                    template: PLATFORMER_MAIN.to_string(),
                    is_entry_point: true,
                },
                SkeletonFile {
                    path: "src/player.rs".to_string(),
                    template: PLATFORMER_PLAYER.to_string(),
                    is_entry_point: false,
                },
                SkeletonFile {
                    path: "Cargo.toml".to_string(),
                    template: PLATFORMER_CARGO.to_string(),
                    is_entry_point: false,
                },
            ],
            dependencies: vec!["bevy".to_string(), "bevy_rapier2d".to_string()],
            success_count: 0,
            failure_count: 0,
        });

        // Top-down RPG template
        self.register_template(ProjectSkeleton {
            name: "topdown_rpg".to_string(),
            engine: GameEngine::Bevy,
            description: "Top-down character movement and tilemap".to_string(),
            files: vec![
                SkeletonFile {
                    path: "src/main.rs".to_string(),
                    template: RPG_MAIN.to_string(),
                    is_entry_point: true,
                },
                SkeletonFile {
                    path: "src/map.rs".to_string(),
                    template: RPG_MAP.to_string(),
                    is_entry_point: false,
                },
            ],
            dependencies: vec!["bevy".to_string(), "bevy_ecs_tilemap".to_string()],
            success_count: 0,
            failure_count: 0,
        });
    }

    /// Load templates learned from previous successful projects
    fn load_learned_templates(&mut self) {
        let learned_path = self.experience_db.join("learned.json");
        if let Ok(content) = std::fs::read_to_string(&learned_path) {
            if let Ok(learned) = serde_json::from_str::<Vec<ProjectSkeleton>>(&content) {
                for template in learned {
                    self.register_template(template);
                }
            }
        }
    }

    /// Register a new template
    pub fn register_template(&mut self, template: ProjectSkeleton) {
        let name = template.name.clone();
        let engine = template.engine.clone();

        self.engine_index
            .entry(engine)
            .or_default()
            .push(name.clone());

        self.templates.insert(name, template);
    }

    /// Find best matching template for a request
    pub fn match_template(&self, request: &str, engine: &GameEngine) -> Option<&ProjectSkeleton> {
        let candidates = self.engine_index.get(engine)?;
        let lower_req = request.to_lowercase();

        // Score each candidate
        let mut scored: Vec<(f32, &ProjectSkeleton)> = candidates
            .iter()
            .filter_map(|name| self.templates.get(name))
            .map(|t| {
                let mut score = 0.0f32;
                let desc_lower = t.description.to_lowercase();

                // Keyword matching
                for word in lower_req.split_whitespace() {
                    if desc_lower.contains(word) {
                        score += 1.0;
                    }
                }

                // Success rate bonus
                let total = t.success_count + t.failure_count;
                if total > 0 {
                    score += (t.success_count as f32 / total as f32) * 2.0;
                }

                (score, t)
            })
            .collect();

        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
        scored.first().map(|(_, t)| *t)
    }

    /// Scaffold a project from template
    pub fn scaffold(
        &self,
        template_name: &str,
        target_dir: impl AsRef<Path>,
    ) -> Result<Vec<PathBuf>, SkillError> {
        let template = self
            .templates
            .get(template_name)
            .ok_or_else(|| SkillError::TemplateNotFound(template_name.to_string()))?;

        let target = target_dir.as_ref();
        let mut created = Vec::new();

        for file in &template.files {
            let path = target.join(&file.path);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&path, &file.template)?;
            created.push(path);
        }

        Ok(created)
    }

    /// Record success/failure for a template
    pub fn record_result(&mut self, template_name: &str, success: bool) {
        if let Some(template) = self.templates.get_mut(template_name) {
            if success {
                template.success_count += 1;
            } else {
                template.failure_count += 1;
            }
        }
    }

    /// Save learned templates to disk
    pub fn save_experience(&self) -> Result<(), SkillError> {
        let learned: Vec<&ProjectSkeleton> = self
            .templates
            .values()
            .filter(|t| t.success_count > 0 || t.failure_count > 0)
            .collect();

        let json = serde_json::to_string_pretty(&learned)?;
        std::fs::write(self.experience_db.join("learned.json"), json)?;

        Ok(())
    }

    /// Learn a new template from a successful project
    pub fn learn_from_project(
        &mut self,
        project_path: impl AsRef<Path>,
        name: &str,
    ) -> Result<(), SkillError> {
        let path = project_path.as_ref();
        let mut files = Vec::new();

        // Walk project directory and collect source files
        for entry in walkdir::WalkDir::new(path).max_depth(3) {
            let entry = entry?;
            let file_path = entry.path();

            if file_path.extension().map(|e| e == "rs").unwrap_or(false) {
                let relative = file_path.strip_prefix(path).unwrap_or(file_path);
                let content = std::fs::read_to_string(file_path)?;

                files.push(SkeletonFile {
                    path: relative.to_string_lossy().to_string(),
                    template: content,
                    is_entry_point: relative
                        .file_name()
                        .map(|n| n == "main.rs")
                        .unwrap_or(false),
                });
            }
        }

        let skeleton = ProjectSkeleton {
            name: name.to_string(),
            engine: GameEngine::Bevy,
            description: format!("Learned from project at {}", path.display()),
            files,
            dependencies: vec![],
            success_count: 1,
            failure_count: 0,
        };

        self.register_template(skeleton);
        self.save_experience()?;

        Ok(())
    }
}
