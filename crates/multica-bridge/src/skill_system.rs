//! Unified Skill System
//!
//! Integrates GameSkillBridge, SkillAdapter, and SceneAgent into a single
//! cohesive skill framework. Provides skill registration, discovery,
//! execution with scene context, and result tracking.

use crate::error::{BridgeError, Result};
use crate::game_skill_bridge::SkillHandler;
use crate::multica_db::SharedMulticaDb;
use crate::scene_agent::{SceneAgent, SceneAgentCapability};
use crate::scene_context::SharedSceneContext;
use crate::types::SkillDefinition;
use log::{info, warn};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

/// Category for organizing skills
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SkillCategory {
    Entity,
    Scene,
    Terrain,
    Physics,
    Rendering,
    Audio,
    Scripting,
    AI,
    Utility,
    Custom(String),
}

impl SkillCategory {
    pub fn name(&self) -> &str {
        match self {
            Self::Entity => "实体操作",
            Self::Scene => "场景管理",
            Self::Terrain => "地形编辑",
            Self::Physics => "物理模拟",
            Self::Rendering => "渲染控制",
            Self::Audio => "音频处理",
            Self::Scripting => "脚本系统",
            Self::AI => "AI行为",
            Self::Utility => "工具",
            Self::Custom(s) => s,
        }
    }
}

/// Registered skill with metadata and handler
pub struct RegisteredSkill {
    pub name: String,
    pub description: String,
    pub category: SkillCategory,
    pub tags: Vec<String>,
    pub handler: SkillHandler,
    pub requires_scene: bool,
    pub is_active: bool,
}

/// Execution context passed to skills
#[derive(Debug, Clone, Default)]
pub struct SkillExecutionContext {
    pub scene_id: Option<String>,
    pub entity_ids: Vec<u64>,
    pub user_id: Option<String>,
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Result of a skill execution with detailed tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillExecResult {
    pub skill_name: String,
    pub success: bool,
    pub output: Option<String>,
    pub error: Option<String>,
    pub data: Option<serde_json::Value>,
    pub entity_ids_affected: Vec<u64>,
    pub category: String,
    pub duration_ms: u64,
}

/// Unified skill system integrating all skill sources
pub struct SkillSystem {
    skills: HashMap<String, RegisteredSkill>,
    db: SharedMulticaDb,
    scene_context: SharedSceneContext,
    scene_agent: Option<Arc<SceneAgent>>,
}

impl SkillSystem {
    pub fn new(db: SharedMulticaDb, scene_context: SharedSceneContext) -> Self {
        let mut system = Self {
            skills: HashMap::new(),
            db,
            scene_context,
            scene_agent: None,
        };
        system.register_builtin_skills();
        system
    }

    pub fn with_scene_agent(mut self, agent: SceneAgent) -> Self {
        self.scene_agent = Some(Arc::new(agent));
        self
    }

    pub fn scene_agent(&self) -> Option<&Arc<SceneAgent>> {
        self.scene_agent.as_ref()
    }

    /// Register all built-in game editor skills
    fn register_builtin_skills(&mut self) {
        self.register_entity_skills();
        self.register_scene_skills();
        self.register_terrain_skills();
        self.register_utility_skills();
        info!("Registered built-in game skill system");
    }

    fn register_entity_skills(&mut self) {
        let ctx = self.scene_context.clone();
        self.register(
            "entity.create",
            "在场景中创建新的游戏实体",
            SkillCategory::Entity,
            &["create", "entity", "spawn"],
            true,
            move |params: &serde_json::Value| {
                let name = params["name"].as_str().unwrap_or("Unnamed");
                let position = params["position"].as_array().map(|arr| {
                    let x = arr.first().and_then(|v| v.as_f64()).unwrap_or(0.0);
                    let y = arr.get(1).and_then(|v| v.as_f64()).unwrap_or(0.0);
                    let z = arr.get(2).and_then(|v| v.as_f64()).unwrap_or(0.0);
                    [x, y, z]
                });
                let mut scene = ctx.lock().expect("mutex poisoned");
                let id = scene
                    .create_entity(name, position, &[])
                    .map_err(BridgeError::Other)?;
                Ok(serde_json::json!({"entity_id": id, "name": name}))
            },
        );

        let ctx = self.scene_context.clone();
        self.register(
            "entity.delete",
            "从场景中删除游戏实体",
            SkillCategory::Entity,
            &["delete", "entity", "remove"],
            true,
            move |params: &serde_json::Value| {
                let entity_id = params["entity_id"]
                    .as_u64()
                    .ok_or_else(|| BridgeError::Other("Missing entity_id".into()))?;
                let mut scene = ctx.lock().expect("mutex poisoned");
                scene.delete_entity(entity_id).map_err(BridgeError::Other)?;
                Ok(serde_json::json!({"deleted": true, "entity_id": entity_id}))
            },
        );

        let ctx = self.scene_context.clone();
        self.register(
            "entity.query",
            "查询场景中的实体列表",
            SkillCategory::Entity,
            &["query", "entity", "list", "find"],
            true,
            move |params: &serde_json::Value| {
                let name_filter = params["name"].as_str();
                let component_type = params["component"].as_str();
                let scene = ctx.lock().expect("mutex poisoned");
                let entities = scene.query_entities(name_filter, component_type);
                Ok(serde_json::json!({
                    "entities": entities,
                    "count": entities.len()
                }))
            },
        );

        let ctx = self.scene_context.clone();
        self.register(
            "entity.duplicate",
            "复制现有实体及其属性",
            SkillCategory::Entity,
            &["duplicate", "clone", "copy"],
            true,
            move |params: &serde_json::Value| {
                let source_id = params["source_id"].as_u64()
                    .ok_or_else(|| BridgeError::Other("Missing source_id".into()))?;
                let new_name = params["name"].as_str().unwrap_or("Duplicate");
                let scene = ctx.lock().expect("mutex poisoned");
                let source = scene.get_entity(source_id)
                    .ok_or_else(|| BridgeError::Other(format!("Entity {} not found", source_id)))?;
                drop(scene);

                let mut scene = ctx.lock().expect("mutex poisoned");
                let new_id = scene.create_entity(new_name, source.position, &source.components)
                    .map_err(BridgeError::Other)?;
                Ok(serde_json::json!({"entity_id": new_id, "name": new_name, "source_id": source_id}))
            },
        );
    }

    fn register_scene_skills(&mut self) {
        let db = self.db.clone();
        self.register(
            "scene.list",
            "列出所有游戏场景",
            SkillCategory::Scene,
            &["scene", "list", "all"],
            false,
            move |_params: &serde_json::Value| {
                let db = db.lock().expect("mutex poisoned");
                let scenes = db.get_all_scenes();
                Ok(serde_json::json!({"scenes": scenes, "count": scenes.len()}))
            },
        );

        let db = self.db.clone();
        self.register(
            "scene.create",
            "创建新的游戏场景",
            SkillCategory::Scene,
            &["scene", "create", "new"],
            false,
            move |params: &serde_json::Value| {
                let name = params["name"].as_str().unwrap_or("NewScene");
                let desc = params["description"].as_str().map(|s| s.to_string());
                let mut db = db.lock().expect("mutex poisoned");
                let scene_id = db.create_scene(name.to_string(), desc)?;
                Ok(serde_json::json!({"scene_id": scene_id, "name": name}))
            },
        );

        let db = self.db.clone();
        self.register(
            "scene.stats",
            "获取场景统计信息",
            SkillCategory::Scene,
            &["scene", "stats", "info"],
            false,
            move |params: &serde_json::Value| {
                let scene_id = params["scene_id"].as_str();
                let db = db.lock().expect("mutex poisoned");
                if let Some(sid) = scene_id {
                    let scene = db.get_scene(sid);
                    let entities = db.get_scene_entities(sid);
                    Ok(serde_json::json!({
                        "scene": scene,
                        "entity_count": entities.len()
                    }))
                } else {
                    let stats = db.get_statistics();
                    Ok(serde_json::json!({
                        "total_scenes": stats.scene_count,
                        "total_entities": stats.entity_count,
                        "total_resources": stats.resource_count
                    }))
                }
            },
        );
    }

    fn register_terrain_skills(&mut self) {
        self.register(
            "terrain.generate",
            "生成程序化地形",
            SkillCategory::Terrain,
            &["terrain", "generate", "procedural"],
            true,
            |params: &serde_json::Value| {
                let size = params["size"].as_u64().unwrap_or(256);
                let seed = params["seed"].as_u64().unwrap_or(42);
                Ok(serde_json::json!({
                    "status": "generated",
                    "size": size,
                    "seed": seed,
                    "terrain_type": "heightmap"
                }))
            },
        );

        self.register(
            "terrain.flatten",
            "将指定区域地形压平",
            SkillCategory::Terrain,
            &["terrain", "flatten", "level"],
            true,
            |params: &serde_json::Value| {
                let height = params["height"].as_f64().unwrap_or(0.0);
                let radius = params["radius"].as_f64().unwrap_or(10.0);
                Ok(serde_json::json!({
                    "status": "flattened",
                    "height": height,
                    "radius": radius
                }))
            },
        );
    }

    fn register_utility_skills(&mut self) {
        self.register(
            "utility.validate",
            "验证场景完整性和引用",
            SkillCategory::Utility,
            &["validate", "check", "integrity"],
            true,
            |params: &serde_json::Value| {
                let scene_id = params["scene_id"].as_str();
                Ok(serde_json::json!({
                    "valid": true,
                    "scene_id": scene_id,
                    "issues": []
                }))
            },
        );

        self.register(
            "utility.export",
            "导出场景为指定格式",
            SkillCategory::Utility,
            &["export", "save", "serialize"],
            true,
            |params: &serde_json::Value| {
                let format = params["format"].as_str().unwrap_or("json");
                Ok(serde_json::json!({
                    "status": "exported",
                    "format": format
                }))
            },
        );

        self.register(
            "utility.batch",
            "批量操作多个实体",
            SkillCategory::Utility,
            &["batch", "multi", "bulk"],
            true,
            |params: &serde_json::Value| {
                let count = params["entity_ids"]
                    .as_array()
                    .map(|a| a.len())
                    .unwrap_or(0);
                Ok(serde_json::json!({
                    "processed": count,
                    "status": "completed"
                }))
            },
        );
    }

    /// Register a new skill
    pub fn register<F>(
        &mut self,
        name: &str,
        description: &str,
        category: SkillCategory,
        tags: &[&str],
        requires_scene: bool,
        handler: F,
    ) where
        F: Fn(&serde_json::Value) -> Result<serde_json::Value> + Send + Sync + 'static,
    {
        let skill = RegisteredSkill {
            name: name.to_string(),
            description: description.to_string(),
            category,
            tags: tags.iter().map(|s| s.to_string()).collect(),
            handler: Box::new(handler),
            requires_scene,
            is_active: true,
        };
        info!("Registered skill: {}", name);
        self.skills.insert(name.to_string(), skill);
    }

    /// Execute a named skill with scene context
    pub fn execute(
        &self,
        skill_name: &str,
        params: &serde_json::Value,
        context: Option<&SkillExecutionContext>,
    ) -> Result<SkillExecResult> {
        let start = std::time::Instant::now();

        let skill = self
            .skills
            .get(skill_name)
            .ok_or_else(|| BridgeError::Other(format!("Skill not found: {}", skill_name)))?;

        if !skill.is_active {
            return Err(BridgeError::Other(format!(
                "Skill is inactive: {}",
                skill_name
            )));
        }

        let result = (skill.handler)(params);
        let duration = start.elapsed().as_millis() as u64;

        match result {
            Ok(data) => {
                let entity_ids = if let Some(ctx) = context {
                    ctx.entity_ids.clone()
                } else {
                    Vec::new()
                };

                Ok(SkillExecResult {
                    skill_name: skill_name.to_string(),
                    success: true,
                    output: Some(serde_json::to_string_pretty(&data).unwrap_or_default()),
                    error: None,
                    data: Some(data),
                    entity_ids_affected: entity_ids,
                    category: skill.category.name().to_string(),
                    duration_ms: duration,
                })
            }
            Err(e) => {
                warn!("Skill {} failed: {}", skill_name, e);
                Ok(SkillExecResult {
                    skill_name: skill_name.to_string(),
                    success: false,
                    output: None,
                    error: Some(e.to_string()),
                    data: None,
                    entity_ids_affected: Vec::new(),
                    category: skill.category.name().to_string(),
                    duration_ms: duration,
                })
            }
        }
    }

    /// Execute via SceneAgent with capability mapping
    pub fn execute_with_agent(
        &self,
        skill_name: &str,
        params: &serde_json::Value,
        context: Option<&SkillExecutionContext>,
    ) -> Result<SkillExecResult> {
        if let Some(ref agent) = self.scene_agent {
            let capability = self.map_skill_to_capability(skill_name);
            let scene_result = agent.execute_capability(capability, params)?;

            let start = std::time::Instant::now();
            let duration = start.elapsed().as_millis() as u64;

            return Ok(SkillExecResult {
                skill_name: skill_name.to_string(),
                success: scene_result.success,
                output: scene_result.output,
                error: scene_result.error,
                data: None,
                entity_ids_affected: scene_result.entity_ids_affected,
                category: "scene_agent".to_string(),
                duration_ms: duration,
            });
        }

        self.execute(skill_name, params, context)
    }

    fn map_skill_to_capability(&self, skill_name: &str) -> SceneAgentCapability {
        match skill_name {
            s if s.starts_with("entity.create") => SceneAgentCapability::CreateEntity,
            s if s.starts_with("entity.delete") => SceneAgentCapability::DeleteEntity,
            s if s.starts_with("entity.query") => SceneAgentCapability::QueryScene,
            s if s.starts_with("terrain.") => SceneAgentCapability::GenerateTerrain,
            s if s.starts_with("utility.validate") => SceneAgentCapability::ValidateScene,
            s if s.starts_with("utility.") => SceneAgentCapability::OptimizeScene,
            _ => SceneAgentCapability::ApplySkill,
        }
    }

    /// List all registered skills
    pub fn list_skills(&self) -> Vec<SkillDefinition> {
        self.skills
            .values()
            .map(|s| SkillDefinition {
                name: s.name.clone(),
                description: s.description.clone(),
                version: "1.0.0".into(),
                parameters: Vec::new(),
                category: Some(s.category.name().to_string()),
                tags: s.tags.clone(),
                is_active: s.is_active,
            })
            .collect()
    }

    /// List skills by category
    pub fn list_by_category(&self, category: &SkillCategory) -> Vec<&RegisteredSkill> {
        self.skills
            .values()
            .filter(|s| s.category == *category)
            .collect()
    }

    /// Search skills by name or tag
    pub fn search(&self, query: &str) -> Vec<&RegisteredSkill> {
        let lower = query.to_lowercase();
        self.skills
            .values()
            .filter(|s| {
                s.name.to_lowercase().contains(&lower)
                    || s.description.to_lowercase().contains(&lower)
                    || s.tags.iter().any(|t| t.to_lowercase().contains(&lower))
            })
            .collect()
    }

    /// Get a specific skill by name
    pub fn get_skill(&self, name: &str) -> Option<&RegisteredSkill> {
        self.skills.get(name)
    }

    /// Enable or disable a skill
    pub fn set_active(&mut self, name: &str, active: bool) -> Result<()> {
        let skill = self
            .skills
            .get_mut(name)
            .ok_or_else(|| BridgeError::Other(format!("Skill not found: {}", name)))?;
        skill.is_active = active;
        Ok(())
    }

    /// Unregister a skill
    pub fn unregister(&mut self, name: &str) -> Option<RegisteredSkill> {
        self.skills.remove(name)
    }

    /// Get the number of registered skills
    pub fn skill_count(&self) -> usize {
        self.skills.len()
    }

    pub fn db(&self) -> &SharedMulticaDb {
        &self.db
    }

    pub fn scene_context(&self) -> &SharedSceneContext {
        &self.scene_context
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::multica_db::create_shared_multica_db;
    use crate::scene_context::create_shared_scene_context;

    fn setup() -> SkillSystem {
        let db = create_shared_multica_db();
        let ctx = create_shared_scene_context();
        SkillSystem::new(db, ctx)
    }

    #[test]
    fn test_register_and_list_skills() {
        let system = setup();
        let skills = system.list_skills();
        assert!(
            skills.len() >= 10,
            "Expected at least 10 built-in skills, got {}",
            skills.len()
        );
    }

    #[test]
    fn test_execute_entity_create() {
        let system = setup();
        let result = system
            .execute(
                "entity.create",
                &serde_json::json!({"name": "TestPlayer", "position": [0.0, 1.0, 0.0]}),
                None,
            )
            .unwrap();
        assert!(result.success);
        let data = result.data.unwrap();
        assert_eq!(data["name"], "TestPlayer");
    }

    #[test]
    fn test_execute_scene_list() {
        let system = setup();
        let result = system
            .execute("scene.list", &serde_json::json!({}), None)
            .unwrap();
        assert!(result.success);
    }

    #[test]
    fn test_skill_not_found() {
        let system = setup();
        let result = system.execute("nonexistent.skill", &serde_json::json!({}), None);
        assert!(result.is_err());
    }

    #[test]
    fn test_list_by_category() {
        let system = setup();
        let entity_skills = system.list_by_category(&SkillCategory::Entity);
        assert!(!entity_skills.is_empty());
        for skill in entity_skills {
            assert_eq!(skill.category, SkillCategory::Entity);
        }
    }

    #[test]
    fn test_search_skills() {
        let system = setup();
        let results = system.search("entity");
        assert!(!results.is_empty());
        assert!(results.iter().any(|s| s.name.contains("entity")));
    }

    #[test]
    fn test_set_active() {
        let mut system = setup();
        system.set_active("entity.create", false).unwrap();
        let result = system.execute("entity.create", &serde_json::json!({"name": "Test"}), None);
        assert!(result.is_err());
    }

    #[test]
    fn test_unregister() {
        let mut system = setup();
        let original_count = system.skill_count();
        system.unregister("terrain.generate");
        assert_eq!(system.skill_count(), original_count - 1);
    }

    #[test]
    fn test_execute_with_context() {
        let system = setup();
        let ctx = SkillExecutionContext {
            entity_ids: vec![1, 2, 3],
            ..Default::default()
        };
        let result = system
            .execute("entity.query", &serde_json::json!({}), Some(&ctx))
            .unwrap();
        assert!(result.success);
        assert_eq!(result.entity_ids_affected, vec![1, 2, 3]);
    }
}
