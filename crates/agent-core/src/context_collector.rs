//! RuntimeContextCollector — 自动从运行时的各子系统收集上下文信息。
//!
//! 收集来源：
//! - SceneBridge: 场景实体列表 + 实体详情
//! - SessionMemory: 最近的操作序列
//! - WorkingMemory: 当前关注的变量

use crate::prompt::PromptContext;
use crate::scene_bridge::SceneBridge;

/// Collects runtime context from all available subsystems
/// and returns a `PromptContext` ready for prompt assembly.
pub struct RuntimeContextCollector<'a> {
    pub scene_bridge: Option<&'a dyn SceneBridge>,
    pub recent_actions: Vec<String>,
    pub working_variables: Vec<(String, String)>,
    pub engine_name: String,
    pub project_name: String,
}

impl<'a> RuntimeContextCollector<'a> {
    pub fn new() -> Self {
        Self {
            scene_bridge: None,
            recent_actions: Vec::new(),
            working_variables: Vec::new(),
            engine_name: "Bevy".into(),
            project_name: "AgentEdit".into(),
        }
    }

    pub fn with_bridge(mut self, bridge: &'a dyn SceneBridge) -> Self {
        self.scene_bridge = Some(bridge);
        self
    }

    pub fn with_recent_actions(mut self, actions: Vec<String>) -> Self {
        self.recent_actions = actions;
        self
    }

    pub fn with_engine(mut self, engine: impl Into<String>) -> Self {
        self.engine_name = engine.into();
        self
    }

    pub fn with_project(mut self, project: impl Into<String>) -> Self {
        self.project_name = project.into();
        self
    }

    /// Collect all available context into a single `PromptContext`.
    pub fn collect(&self) -> PromptContext {
        let mut ctx = PromptContext {
            engine_name: self.engine_name.clone(),
            project_name: self.project_name.clone(),
            ..PromptContext::default()
        };

        // Scene context: entity names
        if let Some(bridge) = self.scene_bridge {
            let entities = bridge.query_entities(None, None);
            let names: Vec<String> = entities.iter().map(|e| e.name.clone()).collect();
            let entity_count = names.len();

            ctx.selected_entities = names.join(", ");
            ctx.extra.insert("entity_count".into(), entity_count.to_string());

            // Scene summary for LLM (cap at 20 entities to avoid overflow)
            let scene_summary = if entity_count <= 20 {
                format!("Scene contains {} entities: {}", entity_count, ctx.selected_entities)
            } else {
                format!(
                    "Scene contains {} entities (showing 20): {}...",
                    entity_count,
                    names.iter().take(20).cloned().collect::<Vec<_>>().join(", ")
                )
            };
            ctx.extra.insert("scene_summary".into(), scene_summary);
        }

        // Recent action history
        if !self.recent_actions.is_empty() {
            let action_summary = self.recent_actions.iter()
                .take(5)
                .cloned()
                .collect::<Vec<_>>()
                .join(" → ");
            ctx.extra.insert("recent_actions".into(), action_summary);
        }

        // Working variables
        for (key, val) in &self.working_variables {
            ctx.extra.insert(key.clone(), val.clone());
        }

        ctx
    }
}

impl<'a> Default for RuntimeContextCollector<'a> {
    fn default() -> Self { Self::new() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene_bridge::MockSceneBridge;

    #[test]
    fn test_collect_empty_returns_basic_context() {
        let collector = RuntimeContextCollector::new();
        let ctx = collector.collect();
        assert_eq!(ctx.engine_name, "Bevy");
        assert_eq!(ctx.project_name, "AgentEdit");
    }

    #[test]
    fn test_collect_with_bridge_gathers_entities() {
        let mut bridge = MockSceneBridge::new();
        bridge.create_entity("Player", None, &[]).unwrap();
        bridge.create_entity("Enemy", None, &[]).unwrap();
        bridge.create_entity("Boss", None, &[]).unwrap();

        let collector = RuntimeContextCollector::new().with_bridge(&bridge);
        let ctx = collector.collect();

        assert!(ctx.selected_entities.contains("Player"));
        assert!(ctx.selected_entities.contains("Enemy"));
        assert!(ctx.extra.get("entity_count").unwrap() == "3");
    }

    #[test]
    fn test_collect_recent_actions() {
        let collector = RuntimeContextCollector::new()
            .with_recent_actions(vec![
                "create_entity(Player)".into(),
                "set_color(Player, red)".into(),
                "move_entity(Player, x=5)".into(),
            ]);
        let ctx = collector.collect();

        let actions = ctx.extra.get("recent_actions").unwrap();
        assert!(actions.contains("create_entity"));
    }
}
