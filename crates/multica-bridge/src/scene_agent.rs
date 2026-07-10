//! Scene-Aware Agent Framework
//!
//! Combines AgentProxy with scene context for scene-aware task execution.
//! Provides capability registry, context injection, and result tracking.

use crate::error::{BridgeError, Result};
use crate::multica_db::{EntityRecord, MulticaDb, SharedMulticaDb};
use crate::scene_context::{ComponentData, SharedSceneContext};
use crate::task_bridge::{TaskBridge, UnifiedTask};
use crate::types::AgentProvider;
use log::{info, warn};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Game editor capabilities that agents can invoke
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SceneAgentCapability {
    CreateEntity,
    ModifyEntity,
    DeleteEntity,
    QueryScene,
    GenerateTerrain,
    ApplySkill,
    ValidateScene,
    OptimizeScene,
}

impl SceneAgentCapability {
    pub fn description(&self) -> &str {
        match self {
            Self::CreateEntity => "创建新的游戏实体",
            Self::ModifyEntity => "修改现有实体的组件",
            Self::DeleteEntity => "从场景中删除实体",
            Self::QueryScene => "查询场景中的实体和状态",
            Self::GenerateTerrain => "生成地形",
            Self::ApplySkill => "应用游戏技能",
            Self::ValidateScene => "验证场景完整性",
            Self::OptimizeScene => "优化场景性能",
        }
    }
}

/// Scene-aware agent configuration
#[derive(Debug, Clone)]
pub struct SceneAgentConfig {
    pub agent_name: String,
    pub provider: AgentProvider,
    pub scene_id: Option<String>,
    pub system_prompt: Option<String>,
    pub max_turns: Option<i32>,
    pub timeout_secs: Option<u64>,
    pub capabilities: Vec<SceneAgentCapability>,
}

impl Default for SceneAgentConfig {
    fn default() -> Self {
        Self {
            agent_name: "WindWaveSceneAgent".into(),
            provider: AgentProvider::WindWave,
            scene_id: None,
            system_prompt: None,
            max_turns: Some(10),
            timeout_secs: Some(300),
            capabilities: vec![
                SceneAgentCapability::CreateEntity,
                SceneAgentCapability::ModifyEntity,
                SceneAgentCapability::DeleteEntity,
                SceneAgentCapability::QueryScene,
            ],
        }
    }
}

/// Scene context payload injected into agent prompts
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneAgentContext {
    pub scene_id: Option<String>,
    pub scene_name: Option<String>,
    pub entity_count: usize,
    pub entities: Vec<EntityRecord>,
    pub capabilities: Vec<String>,
    pub formatted_context: String,
}

impl SceneAgentContext {
    pub fn new(
        scene_id: Option<String>,
        db: &MulticaDb,
        capabilities: &[SceneAgentCapability],
    ) -> Self {
        let (scene_name, entities) = if let Some(ref sid) = scene_id {
            let name = db.get_scene(sid).map(|s| s.name.clone());
            let ents = db.get_scene_entities(sid);
            (name, ents)
        } else {
            (None, db.get_scene_entities("default"))
        };

        let entity_count = entities.len();
        let cap_list: Vec<String> = capabilities
            .iter()
            .map(|c| c.description().to_string())
            .collect();

        let mut ctx = String::new();
        ctx.push_str("## 当前场景状态\n\n");
        if let Some(ref name) = scene_name {
            ctx.push_str(&format!("场景名称: {}\n", name));
        }
        ctx.push_str(&format!("实体数量: {}\n\n", entity_count));
        ctx.push_str("### 场景实体:\n");
        for e in &entities {
            let comp_names: Vec<String> =
                e.components.iter().map(|c| c.type_name.clone()).collect();
            ctx.push_str(&format!(
                "- {} (ID: {}) 组件: {}\n",
                e.name,
                e.entity_id,
                comp_names.join(", ")
            ));
        }
        ctx.push_str("\n### 可用能力:\n");
        for c in &cap_list {
            ctx.push_str(&format!("- {}\n", c));
        }

        Self {
            scene_id,
            scene_name,
            entity_count,
            entities,
            capabilities: cap_list,
            formatted_context: ctx,
        }
    }
}

/// Execution result from a scene-aware agent
#[derive(Debug, Clone)]
pub struct SceneAgentResult {
    pub success: bool,
    pub output: Option<String>,
    pub error: Option<String>,
    pub capability_used: Option<SceneAgentCapability>,
    pub entity_ids_affected: Vec<u64>,
    pub duration_ms: u64,
}

/// Scene-aware agent combining AgentProxy with scene context
pub struct SceneAgent {
    config: SceneAgentConfig,
    db: SharedMulticaDb,
    scene_context: SharedSceneContext,
    task_bridge: Option<Arc<std::sync::Mutex<TaskBridge>>>,
}

impl SceneAgent {
    pub fn new(
        config: SceneAgentConfig,
        db: SharedMulticaDb,
        scene_context: SharedSceneContext,
    ) -> Self {
        Self {
            config,
            db,
            scene_context,
            task_bridge: None,
        }
    }

    pub fn with_task_bridge(mut self, bridge: Arc<std::sync::Mutex<TaskBridge>>) -> Self {
        self.task_bridge = Some(bridge);
        self
    }

    /// Generate scene context for injection into agent prompts
    pub fn build_context(&self) -> SceneAgentContext {
        let db = self.db.lock().expect("mutex poisoned");
        SceneAgentContext::new(self.config.scene_id.clone(), &db, &self.config.capabilities)
    }

    /// Build a full system prompt with scene context
    pub fn build_system_prompt(&self) -> String {
        let ctx = self.build_context();
        let mut prompt = self.config.system_prompt.clone().unwrap_or_else(|| {
            "你是一个游戏编辑器场景助手，能够创建、修改和查询场景中的实体。使用下面的场景信息来回答用户问题。".into()
        });
        prompt.push_str("\n\n");
        prompt.push_str(&ctx.formatted_context);
        prompt
    }

    /// Execute an agent prompt with scene context injection
    pub fn execute_with_scene_context(&self, prompt: &str) -> SceneAgentResult {
        let start = std::time::Instant::now();

        let context = self.build_context();
        let _full_prompt = format!("{}\n\n{}", context.formatted_context, prompt);
        info!(
            "Executing scene agent with {} entities in context",
            context.entity_count
        );

        let affected_entities = Vec::new();
        let mut capability_detected = None;

        // Parse prompt for capability hints
        let lower = prompt.to_lowercase();
        if lower.contains("创建") || lower.contains("create") || lower.contains("新增") {
            capability_detected = Some(SceneAgentCapability::CreateEntity);
        } else if lower.contains("修改") || lower.contains("modify") || lower.contains("编辑") {
            capability_detected = Some(SceneAgentCapability::ModifyEntity);
        } else if lower.contains("删除") || lower.contains("delete") || lower.contains("移除") {
            capability_detected = Some(SceneAgentCapability::DeleteEntity);
        } else if lower.contains("查询") || lower.contains("query") || lower.contains("列出") {
            capability_detected = Some(SceneAgentCapability::QueryScene);
        } else if lower.contains("地形") || lower.contains("terrain") {
            capability_detected = Some(SceneAgentCapability::GenerateTerrain);
        } else if lower.contains("技能") || lower.contains("skill") {
            capability_detected = Some(SceneAgentCapability::ApplySkill);
        } else if lower.contains("验证") || lower.contains("validate") {
            capability_detected = Some(SceneAgentCapability::ValidateScene);
        } else if lower.contains("优化") || lower.contains("optimize") {
            capability_detected = Some(SceneAgentCapability::OptimizeScene);
        }

        // For now, we return the context analysis as output
        // In production, this would call the actual LLM agent via WebSocket
        let output = format!(
            "场景分析完成。当前场景包含 {} 个实体，可用能力: {:?}。提示词: {}",
            context.entity_count, context.capabilities, prompt
        );

        let duration = start.elapsed().as_millis() as u64;

        SceneAgentResult {
            success: true,
            output: Some(output),
            error: None,
            capability_used: capability_detected,
            entity_ids_affected: affected_entities,
            duration_ms: duration,
        }
    }

    /// Programmatically execute a named capability
    pub fn execute_capability(
        &self,
        capability: SceneAgentCapability,
        params: &serde_json::Value,
    ) -> Result<SceneAgentResult> {
        let start = std::time::Instant::now();

        match capability {
            SceneAgentCapability::QueryScene => {
                let ctx = self.build_context();
                let duration = start.elapsed().as_millis() as u64;
                Ok(SceneAgentResult {
                    success: true,
                    output: Some(serde_json::to_string_pretty(&ctx.entities).unwrap_or_default()),
                    error: None,
                    capability_used: Some(capability),
                    entity_ids_affected: ctx.entities.iter().map(|e| e.entity_id).collect(),
                    duration_ms: duration,
                })
            }
            SceneAgentCapability::CreateEntity => {
                let name = params["name"].as_str().unwrap_or("Unnamed");
                let mut ctx = self.scene_context.lock().expect("mutex poisoned");
                let components: Vec<ComponentData> = Vec::new();
                let position = params["position"].as_array().map(|arr| {
                    let x = arr.first().and_then(|v| v.as_f64()).unwrap_or(0.0);
                    let y = arr.get(1).and_then(|v| v.as_f64()).unwrap_or(0.0);
                    let z = arr.get(2).and_then(|v| v.as_f64()).unwrap_or(0.0);
                    [x, y, z]
                });

                let entity_id = ctx
                    .create_entity(name, position, &components)
                    .map_err(BridgeError::Other)?;

                let mut db = self.db.lock().expect("mutex poisoned");
                let scene_id = self
                    .config
                    .scene_id
                    .clone()
                    .unwrap_or_else(|| "default".into());
                db.create_entity(
                    scene_id,
                    name.to_string(),
                    "GameObject".to_string(),
                    components,
                    position,
                )
                .ok();

                let duration = start.elapsed().as_millis() as u64;
                Ok(SceneAgentResult {
                    success: true,
                    output: Some(format!("Created entity {} (ID: {})", name, entity_id)),
                    error: None,
                    capability_used: Some(capability),
                    entity_ids_affected: vec![entity_id],
                    duration_ms: duration,
                })
            }
            _ => {
                let duration = start.elapsed().as_millis() as u64;
                Ok(SceneAgentResult {
                    success: true,
                    output: Some(format!("Capability {:?} acknowledged", capability)),
                    error: None,
                    capability_used: Some(capability),
                    entity_ids_affected: Vec::new(),
                    duration_ms: duration,
                })
            }
        }
    }

    /// Register a task in the task bridge for this scene operation
    pub fn register_scene_task(&self, title: String, description: String) -> Option<UnifiedTask> {
        if let Some(ref bridge) = self.task_bridge {
            let bridge = bridge.lock().expect("mutex poisoned");
            let scene_id = self
                .config
                .scene_id
                .clone()
                .unwrap_or_else(|| "default".into());
            Some(bridge.register_scene_task(title, description, scene_id, Vec::new()))
        } else {
            warn!("No task bridge configured for scene agent");
            None
        }
    }

    pub fn config(&self) -> &SceneAgentConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::multica_db::create_shared_multica_db;
    use crate::scene_context::create_shared_scene_context;

    fn setup_test_agent() -> (SceneAgent, SharedMulticaDb, SharedSceneContext) {
        let db = create_shared_multica_db();
        let ctx = create_shared_scene_context();
        let config = SceneAgentConfig {
            scene_id: Some("test_scene".into()),
            ..Default::default()
        };
        let agent = SceneAgent::new(config, db.clone(), ctx.clone());
        (agent, db, ctx)
    }

    #[test]
    fn test_build_context_empty_scene() {
        let (agent, _db, _ctx) = setup_test_agent();
        let ctx = agent.build_context();
        assert_eq!(ctx.entity_count, 0);
        assert_eq!(ctx.scene_id.as_deref(), Some("test_scene"));
    }

    #[test]
    fn test_build_system_prompt() {
        let (agent, _db, _ctx) = setup_test_agent();
        let prompt = agent.build_system_prompt();
        assert!(prompt.contains("场景"));
        assert!(prompt.contains("实体数量: 0"));
    }

    #[test]
    fn test_execute_with_context() {
        let (agent, _db, _ctx) = setup_test_agent();
        let result = agent.execute_with_scene_context("创建");
        assert!(result.success);
        assert!(result.output.is_some());
    }

    #[test]
    fn test_execute_capability_query() {
        let (agent, _db, _ctx) = setup_test_agent();
        let result = agent
            .execute_capability(SceneAgentCapability::QueryScene, &serde_json::json!({}))
            .unwrap();
        assert!(result.success);
        assert_eq!(
            result.capability_used,
            Some(SceneAgentCapability::QueryScene)
        );
    }

    #[test]
    fn test_execute_capability_create_entity() {
        let (agent, _db, ctx) = setup_test_agent();
        let result = agent
            .execute_capability(
                SceneAgentCapability::CreateEntity,
                &serde_json::json!({"name": "TestEntity", "position": [1.0, 2.0, 3.0]}),
            )
            .unwrap();
        assert!(result.success);
        assert_eq!(result.entity_ids_affected.len(), 1);

        let scene_ctx = ctx.lock().unwrap();
        let entity = scene_ctx.get_entity(result.entity_ids_affected[0]);
        assert!(entity.is_some());
    }

    #[test]
    fn test_capability_detection() {
        let (agent, _db, _ctx) = setup_test_agent();

        let result = agent.execute_with_scene_context("创建一个角色");
        assert_eq!(
            result.capability_used,
            Some(SceneAgentCapability::CreateEntity)
        );

        let result = agent.execute_with_scene_context("修改物体的位置");
        assert_eq!(
            result.capability_used,
            Some(SceneAgentCapability::ModifyEntity)
        );

        let result = agent.execute_with_scene_context("删除选中的实体");
        assert_eq!(
            result.capability_used,
            Some(SceneAgentCapability::DeleteEntity)
        );

        let result = agent.execute_with_scene_context("查询所有玩家");
        assert_eq!(
            result.capability_used,
            Some(SceneAgentCapability::QueryScene)
        );
    }

    #[test]
    fn test_register_scene_task_without_bridge() {
        let (agent, _db, _ctx) = setup_test_agent();
        let task = agent.register_scene_task("test".into(), "desc".into());
        assert!(task.is_none());
    }
}
