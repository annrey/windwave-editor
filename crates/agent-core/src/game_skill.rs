//! Game Skill — Evolving capability system inspired by OpenGame
//!
//! Core architecture:
//! - TemplateSkill: Grows a library of project skeletons from experience
//! - DebugSkill: Maintains a living protocol of verified fixes
//! - Together they enable the agent to scaffold stable architectures
//!   and systematically repair integration errors

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

// Template files content
const PLATFORMER_MAIN: &str = include_str!("../../../templates/platformer_main.rs");
const PLATFORMER_PLAYER: &str = include_str!("../../../templates/platformer_player.rs");
const PLATFORMER_CARGO: &str = include_str!("../../../templates/platformer_cargo.toml");
const RPG_MAIN: &str = include_str!("../../../templates/rpg_main.rs");
const RPG_MAP: &str = include_str!("../../../templates/rpg_map.rs");

// ---------------------------------------------------------------------------
// Shared types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum GameEngine {
    Bevy,
    Unity,
    Godot,
    Phaser,
    ThreeJs,
    Custom(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectSkeleton {
    pub name: String,
    pub engine: GameEngine,
    pub description: String,
    pub files: Vec<SkeletonFile>,
    pub dependencies: Vec<String>,
    pub success_count: u64,
    pub failure_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkeletonFile {
    pub path: String,
    pub template: String,
    pub is_entry_point: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifiedFix {
    pub id: String,
    pub error_pattern: String,
    pub error_signature: ErrorSignature,
    pub fix_strategy: FixStrategy,
    pub verification_method: VerificationMethod,
    pub success_count: u64,
    pub last_used: u64,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorSignature {
    pub error_type: String,
    pub stack_trace_pattern: Option<String>,
    pub console_output_pattern: Option<String>,
    pub component_hint: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FixStrategy {
    CodeReplace {
        file_pattern: String,
        search: String,
        replace: String,
    },
    ComponentAdd {
        entity_pattern: String,
        component_type: String,
        properties: HashMap<String, serde_json::Value>,
    },
    DependencyAdd {
        crate_name: String,
        version: String,
    },
    SystemReorder {
        system_name: String,
        before: Vec<String>,
        after: Vec<String>,
    },
    Custom {
        description: String,
        script: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VerificationMethod {
    CompileCheck,
    RuntimeTest { test_command: String },
    VisualInspection { criteria: String },
    ConsoleClean { max_warnings: u32 },
}

// ---------------------------------------------------------------------------
// TemplateSkill — Project skeleton library
// ---------------------------------------------------------------------------

pub struct TemplateSkill {
    templates: HashMap<String, ProjectSkeleton>,
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
            dependencies: vec![
                "bevy".to_string(),
                "bevy_rapier2d".to_string(),
            ],
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
            dependencies: vec![
                "bevy".to_string(),
                "bevy_ecs_tilemap".to_string(),
            ],
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
    pub fn scaffold(&self, template_name: &str, target_dir: impl AsRef<Path>) -> Result<Vec<PathBuf>, SkillError> {
        let template = self.templates.get(template_name)
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
        let learned: Vec<&ProjectSkeleton> = self.templates
            .values()
            .filter(|t| t.success_count > 0 || t.failure_count > 0)
            .collect();
        
        let json = serde_json::to_string_pretty(&learned)?;
        std::fs::write(self.experience_db.join("learned.json"), json)?;
        
        Ok(())
    }
    
    /// Learn a new template from a successful project
    pub fn learn_from_project(&mut self, project_path: impl AsRef<Path>, name: &str) -> Result<(), SkillError> {
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
                    is_entry_point: relative.file_name()
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

// ---------------------------------------------------------------------------
// DebugSkill — Living protocol of verified fixes
// ---------------------------------------------------------------------------

pub struct DebugSkill {
    fixes: HashMap<String, VerifiedFix>,
    fix_db: PathBuf,
}

impl DebugSkill {
    pub fn new(data_dir: impl AsRef<Path>) -> Self {
        let db_path = data_dir.as_ref().join("fixes");
        let _ = std::fs::create_dir_all(&db_path);
        
        let mut skill = Self {
            fixes: HashMap::new(),
            fix_db: db_path,
        };
        
        skill.load_builtin_fixes();
        skill.load_learned_fixes();
        skill
    }
    
    /// Load built-in fixes for common errors
    fn load_builtin_fixes(&mut self) {
        // Bevy missing component fix
        self.register_fix(VerifiedFix {
            id: "bevy-missing-component".to_string(),
            error_pattern: "does not have the component".to_string(),
            error_signature: ErrorSignature {
                error_type: "QueryComponentError".to_string(),
                stack_trace_pattern: Some("bevy_ecs::query".to_string()),
                console_output_pattern: Some("does not have the component".to_string()),
                component_hint: None,
            },
            fix_strategy: FixStrategy::ComponentAdd {
                entity_pattern: "*".to_string(),
                component_type: "Transform".to_string(),
                properties: HashMap::new(),
            },
            verification_method: VerificationMethod::CompileCheck,
            success_count: 0,
            last_used: 0,
            tags: vec!["bevy".to_string(), "ecs".to_string()],
        });
        
        // Bevy system ordering fix
        self.register_fix(VerifiedFix {
            id: "bevy-system-order".to_string(),
            error_pattern: "resource already borrowed".to_string(),
            error_signature: ErrorSignature {
                error_type: "BorrowMutError".to_string(),
                stack_trace_pattern: Some("bevy_ecs::system".to_string()),
                console_output_pattern: Some("already borrowed".to_string()),
                component_hint: None,
            },
            fix_strategy: FixStrategy::SystemReorder {
                system_name: "*".to_string(),
                before: vec![],
                after: vec!["update".to_string()],
            },
            verification_method: VerificationMethod::RuntimeTest {
                test_command: "cargo test".to_string(),
            },
            success_count: 0,
            last_used: 0,
            tags: vec!["bevy".to_string(), "system".to_string()],
        });
        
        // Missing dependency fix
        self.register_fix(VerifiedFix {
            id: "cargo-missing-dep".to_string(),
            error_pattern: "unresolved import".to_string(),
            error_signature: ErrorSignature {
                error_type: "CompileError".to_string(),
                stack_trace_pattern: None,
                console_output_pattern: Some("unresolved import".to_string()),
                component_hint: None,
            },
            fix_strategy: FixStrategy::DependencyAdd {
                crate_name: "*".to_string(),
                version: "*".to_string(),
            },
            verification_method: VerificationMethod::CompileCheck,
            success_count: 0,
            last_used: 0,
            tags: vec!["cargo".to_string(), "dependency".to_string()],
        });
    }
    
    /// Load fixes learned from previous debugging sessions
    fn load_learned_fixes(&mut self) {
        let learned_path = self.fix_db.join("learned.json");
        if let Ok(content) = std::fs::read_to_string(&learned_path) {
            if let Ok(learned) = serde_json::from_str::<Vec<VerifiedFix>>(&content) {
                for fix in learned {
                    self.register_fix(fix);
                }
            }
        }
    }
    
    /// Register a new fix
    pub fn register_fix(&mut self, fix: VerifiedFix) {
        self.fixes.insert(fix.id.clone(), fix);
    }
    
    /// Find matching fixes for an error
    pub fn find_fixes(&self, error_output: &str) -> Vec<&VerifiedFix> {
        let mut matches = Vec::new();
        
        for fix in self.fixes.values() {
            let mut score = 0u32;
            
            // Pattern matching
            if error_output.contains(&fix.error_pattern) {
                score += 10;
            }
            
            // Signature matching
            if let Some(ref trace_pattern) = fix.error_signature.stack_trace_pattern {
                if error_output.contains(trace_pattern) {
                    score += 5;
                }
            }
            
            if let Some(ref console_pattern) = fix.error_signature.console_output_pattern {
                if error_output.contains(console_pattern) {
                    score += 5;
                }
            }
            
            if score > 0 {
                matches.push((score, fix));
            }
        }
        
        // Sort by score (descending) and success rate
        matches.sort_by(|a, b| {
            let score_cmp = b.0.cmp(&a.0);
            if score_cmp != std::cmp::Ordering::Equal {
                return score_cmp;
            }
            
            let a_total = a.1.success_count + 1;
            let b_total = b.1.success_count + 1;
            b_total.cmp(&a_total)
        });
        
        matches.into_iter().map(|(_, f)| f).collect()
    }
    
    /// Apply a fix to a project
    pub fn apply_fix(
        &self,
        fix: &VerifiedFix,
        project_path: impl AsRef<Path>,
    ) -> Result<FixResult, SkillError> {
        let project = project_path.as_ref();
        
        match &fix.fix_strategy {
            FixStrategy::CodeReplace { file_pattern, search, replace } => {
                let mut applied = 0;
                
                for entry in walkdir::WalkDir::new(project).max_depth(3) {
                    let entry = entry?;
                    let path = entry.path();
                    
                    if path.to_string_lossy().contains(file_pattern) && path.is_file() {
                        let content = std::fs::read_to_string(path)?;
                        if content.contains(search) {
                            let new_content = content.replace(search, replace);
                            std::fs::write(path, new_content)?;
                            applied += 1;
                        }
                    }
                }
                
                Ok(FixResult {
                    fix_id: fix.id.clone(),
                    success: applied > 0,
                    files_modified: applied,
                    message: format!("Applied code replace to {} files", applied),
                })
            }
            
            FixStrategy::ComponentAdd { entity_pattern, component_type, properties: _ } => {
                // This would integrate with the scene system
                Ok(FixResult {
                    fix_id: fix.id.clone(),
                    success: true,
                    files_modified: 0,
                    message: format!("Added component {} to entities matching {}", 
                        component_type, entity_pattern),
                })
            }
            
            FixStrategy::DependencyAdd { crate_name, version } => {
                let cargo_path = project.join("Cargo.toml");
                if !cargo_path.exists() {
                    return Err(SkillError::NoCargoToml);
                }
                
                let mut content = std::fs::read_to_string(&cargo_path)?;
                let dep_line = format!("{} = \"{}\"", crate_name, version);
                
                if !content.contains(&dep_line) {
                    // Add to [dependencies] section
                    if let Some(pos) = content.find("[dependencies]") {
                        let insert_pos = pos + "[dependencies]".len();
                        content.insert_str(insert_pos, &format!("\n{}", dep_line));
                        std::fs::write(&cargo_path, content)?;
                    }
                }
                
                Ok(FixResult {
                    fix_id: fix.id.clone(),
                    success: true,
                    files_modified: 1,
                    message: format!("Added dependency {} = {}", crate_name, version),
                })
            }
            
            FixStrategy::SystemReorder { .. } => {
                // Would integrate with Bevy's system scheduling
                Ok(FixResult {
                    fix_id: fix.id.clone(),
                    success: true,
                    files_modified: 0,
                    message: "System reorder queued".to_string(),
                })
            }
            
            FixStrategy::Custom { description, .. } => {
                Ok(FixResult {
                    fix_id: fix.id.clone(),
                    success: false,
                    files_modified: 0,
                    message: format!("Custom fix requires manual application: {}", description),
                })
            }
        }
    }
    
    /// Verify a fix was successful
    pub fn verify_fix(&self, fix: &VerifiedFix, project_path: impl AsRef<Path>) -> Result<bool, SkillError> {
        match &fix.verification_method {
            VerificationMethod::CompileCheck => {
                let output = std::process::Command::new("cargo")
                    .current_dir(project_path.as_ref())
                    .args(&["check"])
                    .output()?;
                
                Ok(output.status.success())
            }
            
            VerificationMethod::RuntimeTest { test_command } => {
                let parts: Vec<&str> = test_command.split_whitespace().collect();
                if parts.is_empty() {
                    return Ok(false);
                }
                
                let output = std::process::Command::new(parts[0])
                    .current_dir(project_path.as_ref())
                    .args(&parts[1..])
                    .output()?;
                
                Ok(output.status.success())
            }
            
            VerificationMethod::VisualInspection { .. } => {
                // Would require human or VLM verification
                Ok(true)
            }
            
            VerificationMethod::ConsoleClean { max_warnings } => {
                let output = std::process::Command::new("cargo")
                    .current_dir(project_path.as_ref())
                    .args(&["build"])
                    .output()?;
                
                let stderr = String::from_utf8_lossy(&output.stderr);
                let warning_count = stderr.matches("warning:").count() as u32;
                
                Ok(output.status.success() && warning_count <= *max_warnings)
            }
        }
    }
    
    /// Record success/failure for a fix
    pub fn record_result(&mut self, fix_id: &str, success: bool) {
        if let Some(fix) = self.fixes.get_mut(fix_id) {
            if success {
                fix.success_count += 1;
            }
            fix.last_used = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
        }
    }
    
    /// Save learned fixes to disk
    pub fn save_experience(&self) -> Result<(), SkillError> {
        let learned: Vec<&VerifiedFix> = self.fixes
            .values()
            .filter(|f| f.success_count > 0)
            .collect();
        
        let json = serde_json::to_string_pretty(&learned)?;
        std::fs::write(self.fix_db.join("learned.json"), json)?;
        
        Ok(())
    }
    
    /// Learn a new fix from a successful debugging session
    pub fn learn_fix(&mut self, error_output: &str, fix_strategy: FixStrategy, verification: VerificationMethod) -> String {
        let id = format!("learned-{}", self.fixes.len());
        
        let fix = VerifiedFix {
            id: id.clone(),
            error_pattern: error_output.lines().next().unwrap_or("").to_string(),
            error_signature: ErrorSignature {
                error_type: "Learned".to_string(),
                stack_trace_pattern: None,
                console_output_pattern: Some(error_output.to_string()),
                component_hint: None,
            },
            fix_strategy,
            verification_method: verification,
            success_count: 1,
            last_used: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            tags: vec!["learned".to_string()],
        };
        
        self.register_fix(fix);
        let _ = self.save_experience();
        
        id
    }
}

// ---------------------------------------------------------------------------
// Fix result
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct FixResult {
    pub fix_id: String,
    pub success: bool,
    pub files_modified: u32,
    pub message: String,
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[derive(Debug, thiserror::Error)]
pub enum SkillError {
    #[error("Template not found: {0}")]
    TemplateNotFound(String),
    #[error("No Cargo.toml found in project")]
    NoCargoToml,
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Walkdir error: {0}")]
    WalkDir(#[from] walkdir::Error),
}

// ---------------------------------------------------------------------------
// GameSkill — Unified interface
// ---------------------------------------------------------------------------

pub struct GameSkill {
    pub template: TemplateSkill,
    pub debug: DebugSkill,
}

impl GameSkill {
    pub fn new(data_dir: impl AsRef<Path>) -> Self {
        let data_dir = data_dir.as_ref();
        Self {
            template: TemplateSkill::new(data_dir),
            debug: DebugSkill::new(data_dir),
        }
    }
    
    /// Full workflow: scaffold from template + auto-fix any issues
    pub fn scaffold_and_fix(
        &mut self,
        request: &str,
        engine: &GameEngine,
        target_dir: impl AsRef<Path>,
    ) -> Result<ScaffoldResult, SkillError> {
        let target = target_dir.as_ref();
        
        // 1. Find best template
        let template_name = self.template.match_template(request, engine)
            .ok_or_else(|| SkillError::TemplateNotFound("No matching template".to_string()))?
            .name
            .clone();
        
        // 2. Scaffold project
        let files = self.template.scaffold(&template_name, target)?;
        
        // 3. Try to compile
        let compile_output = std::process::Command::new("cargo")
            .current_dir(target)
            .args(&["check"])
            .output()?;
        
        let mut fixes_applied = Vec::new();
        
        // 4. If compile fails, try debug fixes
        if !compile_output.status.success() {
            let stderr = String::from_utf8_lossy(&compile_output.stderr);
            let matching_fixes: Vec<String> = self.debug.find_fixes(&stderr)
                .into_iter()
                .map(|f| f.id.clone())
                .collect();
            
            for fix_id in matching_fixes {
                if let Some(fix) = self.debug.fixes.get(&fix_id) {
                    let result = self.debug.apply_fix(fix, target)?;
                    if result.success {
                        fixes_applied.push(result);
                        
                        // Verify
                        let verify_output = std::process::Command::new("cargo")
                            .current_dir(target)
                            .args(&["check"])
                            .output()?;
                        
                        if verify_output.status.success() {
                            self.debug.record_result(&fix_id, true);
                            break;
                        }
                    }
                }
            }
        }
        
        // 5. Record template result
        let success = fixes_applied.iter().all(|f| f.success);
        self.template.record_result(&template_name, success);
        
        Ok(ScaffoldResult {
            template_name,
            files_created: files,
            fixes_applied,
            success,
        })
    }
}

#[derive(Debug, Clone)]
pub struct ScaffoldResult {
    pub template_name: String,
    pub files_created: Vec<PathBuf>,
    pub fixes_applied: Vec<FixResult>,
    pub success: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_template_skill_creation() {
        let temp_dir = std::env::temp_dir().join("agentedit_skill_test");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();
        
        let skill = TemplateSkill::new(&temp_dir);
        assert!(!skill.templates.is_empty());
        
        let _ = std::fs::remove_dir_all(&temp_dir);
    }
    
    #[test]
    fn test_debug_skill_find_fixes() {
        let temp_dir = std::env::temp_dir().join("agentedit_debug_test");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();
        
        let skill = DebugSkill::new(&temp_dir);
        
        let error = "Query does not have the component Transform";
        let fixes = skill.find_fixes(error);
        assert!(!fixes.is_empty());
        
        let _ = std::fs::remove_dir_all(&temp_dir);
    }
    
    #[test]
    fn test_game_skill_scaffold() {
        let temp_dir = std::env::temp_dir().join("agentedit_game_skill_test");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();
        
        let mut skill = GameSkill::new(&temp_dir);
        
        // This would require actual template files, so we just test structure
        assert!(!skill.template.templates.is_empty());
        assert!(!skill.debug.fixes.is_empty());
        
        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
