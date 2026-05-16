//! Layered Context Builder — 自动构建完整的 L0-L3 分层上下文
//!
//! Sprint 1-C1 Enhancement:
//! - 从 SceneBridge 自动收集 L3 实体详情
//! - 从事件流自动更新 L1 会话历史
//! - 从用户请求自动提取 L2 任务信息
//! - 组装成完整的 LLM prompt 字符串
//!
//! ## Usage
//!
//! ```rust
//! # use agent_core::LayeredContextBuilder;
//! let builder = LayeredContextBuilder::new()
//!     .with_recent_actions(vec!["Created Player".into()])
//!     .with_user_request("创建一个红色敌人");
//!
//! let layered = builder.build();
//! let prompt = builder.build_prompt(&layered);
//! ```

use crate::prompt::{
    LayeredContext, L0SystemContext,
    L3EntityContext, EntityComponent, FewShotExample,
};
use crate::scene_bridge::SceneBridge;

// ===========================================================================
// LayeredContextBuilder - 自动化上下文构建器
// ===========================================================================

/// Builder pattern for constructing complete L0-L3 layered context.
///
/// Automatically collects information from runtime subsystems and
/// assembles it into a structured `LayeredContext` ready for LLM prompts.
pub struct LayeredContextBuilder<'a> {
    scene_bridge: Option<&'a dyn SceneBridge>,
    recent_actions: Vec<String>,
    user_request: Option<String>,
    selected_entity_names: Vec<String>,
    engine_name: String,
    project_name: String,
    base_context: Option<LayeredContext>,
}

impl<'a> LayeredContextBuilder<'a> {
    /// Create a new builder with defaults.
    pub fn new() -> Self {
        Self {
            scene_bridge: None,
            recent_actions: Vec::new(),
            user_request: None,
            selected_entity_names: Vec::new(),
            engine_name: "Bevy".into(),
            project_name: "AgentEdit".into(),
            base_context: None,
        }
    }

    /// Set the scene bridge for L3 entity collection.
    pub fn with_scene_bridge(mut self, bridge: &'a dyn SceneBridge) -> Self {
        self.scene_bridge = Some(bridge);
        self
    }

    /// Set optional scene bridge (for use with Option<&dyn SceneBridge>).
    pub fn with_scene_bridge_option(mut self, bridge: Option<&'a dyn SceneBridge>) -> Self {
        self.scene_bridge = bridge;
        self
    }

    /// Set recent actions for L1 session context.
    pub fn with_recent_actions(mut self, actions: Vec<String>) -> Self {
        self.recent_actions = actions;
        self
    }

    /// Set current user request for L2 task extraction.
    pub fn with_user_request(mut self, request: &str) -> Self {
        self.user_request = Some(request.to_string());
        self
    }

    /// Set currently selected entity names for L3 focus.
    pub fn with_selected_entities(mut self, names: Vec<String>) -> Self {
        self.selected_entity_names = names;
        self
    }

    /// Set engine name (default: "Bevy").
    pub fn with_engine(mut self, name: &str) -> Self {
        self.engine_name = name.to_string();
        self
    }

    /// Set project name (default: "AgentEdit").
    pub fn with_project(mut self, name: &str) -> Self {
        self.project_name = name.to_string();
        self
    }

    /// Start from an existing LayeredContext (for incremental updates).
    pub fn with_base_context(mut self, ctx: LayeredContext) -> Self {
        self.base_context = Some(ctx);
        self
    }

    /// Build the complete LayeredContext from all available sources.
    pub fn build(&self) -> LayeredContext {
        // Start with base or default
        let mut ctx = match &self.base_context {
            Some(base) => base.clone(),
            None => LayeredContext::default(),
        };

        // === L0: System Context (stable, rarely changes) ===
        if ctx.l0_system.agent_name.is_empty() {
            ctx.l0_system = L0SystemContext::default_bevy();
            // Override engine if specified
            if !self.engine_name.is_empty() && self.engine_name.to_lowercase() != "bevy" {
                ctx.l0_system.engine_name = self.engine_name.to_lowercase();
            }
        }

        // === L1: Session Context (persists across session) ===
        self.update_l1_session(&mut ctx);

        // === L2: Task Context (changes per request) ===
        self.update_l2_task(&mut ctx);

        // === L3: Entity Context (most granular, per operation) ===
        self.update_l3_entities(&mut ctx);

        // === Few-shot Examples (always include full set) ===
        if ctx.few_shot_examples.is_empty() {
            for example in FewShotExample::default_examples() {
                ctx.add_few_shot(example);
            }
        }

        ctx
    }

    /// Build final LLM prompt string from the layered context.
    ///
    /// Assembles all layers into a single well-formatted prompt with
    /// clear section headers and token-budget awareness.
    pub fn build_prompt(&self, ctx: &LayeredContext) -> String {
        let mut parts = Vec::new();

        // L0: System identity and capabilities
        parts.push("=== SYSTEM CONTEXT (L0) ===".to_string());
        parts.push(ctx.l0_system.describe());

        // L1: Session history and project state
        parts.push("\n=== SESSION CONTEXT (L1) ===".to_string());
        parts.push(ctx.l1_session.describe());

        // L2: Current task details
        parts.push("\n=== TASK CONTEXT (L2) ===".to_string());
        parts.push(ctx.l2_task.describe());

        // L3: Entity details (if any)
        if !ctx.l3_entities.is_empty() {
            parts.push("\n=== ENTITY DETAILS (L3) ===".to_string());
            for entity in &ctx.l3_entities {
                parts.push(entity.describe());
            }
        }

        // Few-shot examples (selectively included)
        if let Some(request) = &self.user_request {
            let relevant = ctx.select_few_shot_examples(request, 2);
            if !relevant.is_empty() {
                parts.push("\n=== EXAMPLES (Few-Shot) ===".to_string());
                for example in relevant {
                    parts.push(example.describe());
                }
            }
        }

        parts.join("\n\n")
    }

    // ------------------------------------------------------------------
    // Private helpers for layer updates
    // ------------------------------------------------------------------

    /// Update L1 session context with runtime data.
    fn update_l1_session(&self, ctx: &mut LayeredContext) {
        // Project info
        if ctx.l1_session.project_name.is_empty() {
            ctx.l1_session.project_name = self.project_name.clone();
        }
        if ctx.l1_session.engine_version.is_empty() {
            ctx.l1_session.engine_version = format!("{} 0.17", self.engine_name);
        }

        // Recent actions from event stream
        if !self.recent_actions.is_empty() {
            ctx.l1_session.recent_actions = self.recent_actions.clone();
        }

        // Auto-detect conventions from project structure (future)
        // For now, use sensible defaults for Bevy projects
        if ctx.l1_session.conventions.is_empty() {
            ctx.l1_session.conventions = vec![
                "Use ECS architecture with Components and Systems".into(),
                "Entity names use PascalCase (Player, Enemy, Wall)".into(),
                "Positions are [x, y] in world coordinates".into(),
                "Colors are RGBA [r, g, b, a] with 0.0-1.0 range".into(),
            ];
        }
    }

    /// Update L2 task context by parsing user request.
    fn update_l2_task(&self, ctx: &mut LayeredContext) {
        if let Some(request) = &self.user_request {
            // Set current task
            ctx.l2_task.current_task = request.clone();

            // Extract potential entity names from request (capitalized words)
            let entities = Self::extract_entity_names_from_request(request);
            if !entities.is_empty() {
                ctx.l2_task.selected_entities = entities;
            } else {
                ctx.l2_task.selected_entities = self.selected_entity_names.clone();
            }

            // Extract goals from common patterns
            ctx.l2_task.goals = Self::extract_goals_from_request(request);

            // Extract constraints from risk-related keywords
            ctx.l2_task.constraints = Self::extract_constraints_from_request(request);
        } else if !self.selected_entity_names.is_empty() {
            ctx.l2_task.selected_entities = self.selected_entity_names.clone();
        }
    }

    /// Update L3 entity context by querying SceneBridge.
    fn update_l3_entities(&self, ctx: &mut LayeredContext) {
        if let Some(bridge) = self.scene_bridge {
            // Determine which entities to collect details for
            let target_entities = if !ctx.l2_task.selected_entities.is_empty() {
                // Focus on task-relevant entities
                ctx.l2_task.selected_entities.clone()
            } else if !self.selected_entity_names.is_empty() {
                // Use explicitly selected entities
                self.selected_entity_names.clone()
            } else {
                // Collect all entities (limit to avoid token overflow)
                let all = bridge.query_entities(None, None);
                all.iter().take(5).map(|e| e.name.clone()).collect()
            };

            // Query detailed info for each target entity
            let mut l3_entities = Vec::new();
            for name in &target_entities {
                let matches = bridge.query_entities(Some(name), None);
                if let Some(entity) = matches.first() {
                    let l3_ctx = L3EntityContext {
                        entity_name: entity.name.clone(),
                        entity_id: entity.id,
                        components: Self::convert_components(&entity.components),
                        parent: None,  // EntityListItem doesn't have parent info
                        children: Vec::new(),  // EntityListItem doesn't have children info
                    };
                    l3_entities.push(l3_ctx);
                }
            }

            ctx.l3_entities = l3_entities;
        }
    }

    /// Extract potential entity names from user request text.
    ///
    /// Looks for capitalized words that might be entity names.
    fn extract_entity_names_from_request(request: &str) -> Vec<String> {
        let mut names = Vec::new();
        let mut seen = std::collections::HashSet::new();

        for word in request.split_whitespace() {
            let cleaned: String = word.chars()
                .filter(|c| c.is_alphanumeric() || *c == '_')
                .collect();

            // Check if it looks like an entity name (capitalized, >1 char)
            if cleaned.len() > 1
                && cleaned.chars().next().map(|c| c.is_uppercase()).unwrap_or(false)
                && !seen.contains(&cleaned)
            {
                // Exclude common words that aren't entity names
                if !["The", "And", "For", "With", "From", "This", "That",
                    "Create", "Delete", "Move", "Update", "Query"].contains(&cleaned.as_str())
                {
                    names.push(cleaned.clone());
                    seen.insert(cleaned);
                }
            }
        }

        names
    }

    /// Extract goals from user request using keyword matching.
    fn extract_goals_from_request(request: &str) -> Vec<String> {
        let mut goals = Vec::new();
        let lower = request.to_lowercase();

        // Common goal patterns
        if lower.contains("创建") || lower.contains("create") || lower.contains("生成") {
            goals.push("Create new entity/entities".into());
        }
        if lower.contains("删除") || lower.contains("delete") || lower.contains("移除") {
            goals.push("Remove existing entity/entities".into());
        }
        if lower.contains("移动") || lower.contains("move") || lower.contains("位置") {
            goals.push("Reposition entity/entities".into());
        }
        if lower.contains("修改") || lower.contains("更新") || lower.contains("改变") {
            goals.push("Modify entity properties".into());
        }
        if lower.contains("查询") || lower.contains("列出") || lower.contains("显示") {
            goals.push("Query and display information".into());
        }
        if lower.contains("颜色") || lower.contains("color") || lower.contains("红色")
            || lower.contains("蓝色") || lower.contains("绿色")
        {
            goals.push("Change visual appearance (color)".into());
        }

        // If no specific goals found, add a generic one
        if goals.is_empty() {
            goals.push(format!("Fulfill user request: {}", request));
        }

        goals
    }

    /// Extract constraints from user request.
    fn extract_constraints_from_request(request: &str) -> Vec<String> {
        let mut constraints = Vec::new();
        let lower = request.to_lowercase();

        // Risk-related constraints
        if lower.contains("批量") || lower.contains("多个") || lower.contains("所有") {
            constraints.push("Batch operation - apply to multiple entities".into());
        }
        if lower.contains("小心") || lower.contains("谨慎") || lower.contains("确认") {
            constraints.push("Requires user confirmation before execution".into());
        }
        if lower.contains("不可逆") || lower.contains("永久") || lower.contains("销毁") {
            constraints.push("Destructive operation - cannot be undone easily".into());
        }

        // Positional constraints
        if lower.contains("左侧") || lower.contains("左边") {
            constraints.push("Position constraint: left side of screen".into());
        }
        if lower.contains("右侧") || lower.contains("右边") {
            constraints.push("Position constraint: right side of screen".into());
        }
        if lower.contains("上方") || lower.contains("上面") {
            constraints.push("Position constraint: upper area".into());
        }
        if lower.contains("下方") || lower.contains("下面") {
            constraints.push("Position constraint: lower area".into());
        }

        constraints
    }

    /// Convert component info from SceneBridge to L3 format.
    fn convert_components(components: &[String]) -> Vec<EntityComponent> {
        components.iter().map(|comp_name| {
            EntityComponent {
                name: comp_name.clone(),
                properties: std::collections::HashMap::new(), // Details would need additional API
            }
        }).collect()
    }
}

impl Default for LayeredContextBuilder<'_> {
    fn default() -> Self {
        Self::new()
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builder_default_initialization() {
        let builder = LayeredContextBuilder::new();
        let ctx = builder.build();

        // Should have basic L0 system context
        assert!(!ctx.l0_system.agent_name.is_empty());
        assert_eq!(ctx.l0_system.engine_name, "bevy");

        // Should have few-shot examples
        assert!(!ctx.few_shot_examples.is_empty());
        assert!(ctx.few_shot_examples.len() >= 3);
    }

    #[test]
    fn test_builder_with_custom_engine() {
        let builder = LayeredContextBuilder::new()
            .with_engine("Unity");
        let ctx = builder.build();

        assert_eq!(ctx.l0_system.engine_name, "unity");
    }

    #[test]
    fn test_builder_with_recent_actions() {
        let builder = LayeredContextBuilder::new()
            .with_recent_actions(vec![
                "Created Player".into(),
                "Moved Player to (100, 200)".into(),
            ]);
        let ctx = builder.build();

        assert_eq!(ctx.l1_session.recent_actions.len(), 2);
        assert!(ctx.l1_session.recent_actions[0].contains("Created"));
    }

    #[test]
    fn test_builder_extract_entity_names() {
        let builder = LayeredContextBuilder::new()
            .with_user_request("把 Player 移到 Enemy 旁边");
        let ctx = builder.build();

        // Should extract Player and Enemy as entities
        assert!(ctx.l2_task.selected_entities.contains(&"Player".to_string()));
        assert!(ctx.l2_task.selected_entities.contains(&"Enemy".to_string()));
    }

    #[test]
    fn test_builder_extract_goals() {
        let builder = LayeredContextBuilder::new()
            .with_user_request("创建一个红色敌人放在右侧");
        let ctx = builder.build();

        // Should extract creation goal and position constraint
        let has_create_goal = ctx.l2_task.goals.iter().any(|g| g.contains("Create"));
        assert!(has_create_goal, "Should detect creation goal");

        let has_position_constraint = ctx.l2_task.constraints.iter()
            .any(|c| c.contains("right") || c.contains("右侧"));
        assert!(has_position_constraint, "Should detect position constraint");
    }

    #[test]
    fn test_build_prompt_includes_all_layers() {
        let builder = LayeredContextBuilder::new()
            .with_user_request("查询场景中的Player")
            .with_recent_actions(vec!["Created Player".into()]);
        let ctx = builder.build();
        let prompt = builder.build_prompt(&ctx);

        // Verify all layers are present
        assert!(prompt.contains("SYSTEM CONTEXT (L0)"));
        assert!(prompt.contains("SESSION CONTEXT (L1)"));
        assert!(prompt.contains("TASK CONTEXT (L2)"));
        assert!(prompt.contains("Player"), "Should mention extracted entity");

        // Verify L0 has agent identity
        assert!(prompt.contains("AgentEdit"));

        // Verify L1 has recent actions
        assert!(prompt.contains("Created Player"));
    }

    #[test]
    fn test_few_shot_selection_relevance() {
        let ctx = LayeredContextBuilder::new().build();

        // Test create request → should prefer create example
        let create_examples = ctx.select_few_shot_examples("创建一个蓝色玩家", 1);
        assert_eq!(create_examples.len(), 1);
        assert!(create_examples[0].action.contains("create"),
            "Expected action containing 'create', got: '{}'", create_examples[0].action);

        // Test update request → should prefer update example
        let update_examples = ctx.select_few_shot_examples("把Enemy改成红色", 1);
        assert_eq!(update_examples.len(), 1);
        assert!(update_examples[0].action.contains("update"));
    }

    #[test]
    fn test_incremental_update_with_base_context() {
        let base = LayeredContextBuilder::new()
            .with_project("MyGame")
            .build();

        let updated = LayeredContextBuilder::new()
            .with_base_context(base)
            .with_user_request("创建Boss")
            .with_recent_actions(vec!["Previous action".into()])
            .build();

        // Should preserve project name from base
        assert_eq!(updated.l1_session.project_name, "MyGame");

        // Should add new task info
        assert!(updated.l2_task.current_task.contains("Boss"));

        // Should update recent actions
        assert_eq!(updated.l1_session.recent_actions.len(), 1);
    }

    #[test]
    fn test_full_few_shot_set_included() {
        let ctx = LayeredContextBuilder::new().build();
        use crate::prompt::FewShotExample;
        let defaults = FewShotExample::default_examples();
        assert!(ctx.few_shot_examples.len() >= defaults.len(),
            "Builder should include all default examples, got {} expected at least {}",
            ctx.few_shot_examples.len(), defaults.len());
    }

    #[test]
    fn test_delete_request_selects_delete_example() {
        let ctx = LayeredContextBuilder::new().build();
        let results = ctx.select_few_shot_examples("删除这个敌人", 1);
        assert_eq!(results.len(), 1);
        assert!(results[0].action.contains("delete"),
            "Delete request should select delete example, got: {}", results[0].action);
    }

    #[test]
    fn test_code_gen_request_selects_generate_example() {
        let ctx = LayeredContextBuilder::new().build();
        let results = ctx.select_few_shot_examples("写一个移动组件", 1);
        assert_eq!(results.len(), 1);
        assert!(results[0].action.contains("generate"),
            "Code gen request should select generate example, got: {}", results[0].action);
    }

    #[test]
    fn test_prefab_request_selects_prefab_example() {
        let ctx = LayeredContextBuilder::new().build();
        let results = ctx.select_few_shot_examples("存为预制体模板", 1);
        assert_eq!(results.len(), 1);
        assert!(results[0].action.contains("prefab"),
            "Prefab request should select prefab example, got: {}", results[0].action);
    }

    #[test]
    fn test_agent_request_selects_agent_example() {
        let ctx = LayeredContextBuilder::new().build();
        let results = ctx.select_few_shot_examples("给Boss挂上AI", 1);
        assert_eq!(results.len(), 1);
        assert!(results[0].action.contains("agent") || results[0].action.contains("attach"),
            "Agent request should select agent example, got: {}", results[0].action);
    }

    #[test]
    fn test_review_request_selects_review_example() {
        let ctx = LayeredContextBuilder::new().build();
        let results = ctx.select_few_shot_examples("检查这段代码有没有问题", 1);
        assert_eq!(results.len(), 1);
        assert!(results[0].action.contains("review"),
            "Review request should select review example, got: {}", results[0].action);
    }

    #[test]
    fn test_category_helpers_return_correct_counts() {
        use crate::prompt::FewShotExample;
        assert_eq!(FewShotExample::scene_tool_examples().len(), 4);
        assert_eq!(FewShotExample::code_tool_examples().len(), 3);
        assert_eq!(FewShotExample::asset_tool_examples().len(), 2);
        assert_eq!(FewShotExample::agent_tool_examples().len(), 2);
    }
}
