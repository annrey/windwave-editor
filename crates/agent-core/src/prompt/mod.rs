//! Prompt System — layered prompt engineering architecture
//!
//! Implements Design Document Section 9: a tiered prompt system that composes
//! base templates, engine-specific extensions, and user customisations into
//! the final LLM prompt via variable substitution.
//!
//! Sprint 1 Enhancement: L0-L3分层上下文注入
//! - L0: System-level (agent identity, capabilities)
//! - L1: Session-level (conversation history, project context)
//! - L2: Task-level (current task, selected entities)
//! - L3: Entity-level (specific entity details, component state)

use std::collections::HashMap;

pub mod budget;
pub mod context;
pub mod examples;

pub use budget::{estimate_tokens, TokenAllocation, TokenBudget};
pub use context::{
    EntityComponent, L0SystemContext, L1SessionContext, L2TaskContext, L3EntityContext,
    LayeredContext,
};
pub use examples::FewShotExample;

// ============================================================================
// PromptType — categories of prompts
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PromptType {
    SystemIdentity,
    SystemCapabilities,
    TaskPlanning,
    TaskDecomposition,
    ToolSelection,
    CodeGeneration,
    SceneManipulation,
    ResponseFormatting,
    ErrorExplanation,
    ClarificationRequest,
}

#[derive(Debug, Clone)]
pub struct PromptTemplate {
    pub name: String,
    pub template: String,
}

// ============================================================================
// Base prompt constants
// ============================================================================

pub const BASE_SYSTEM_PROMPT: &str = r#"You are {agent_name}, an AI assistant specialized in game development.
You are operating within the AgentEdit editor, working with the {engine_name} game engine.

## Your Capabilities
- Scene manipulation: Create, modify, and query game entities and components
- Code generation: Generate scripts, components, and systems in {language}
- Asset management: Import, organize, and reference game assets
- Debugging assistance: Analyze issues and suggest fixes

## Operating Principles
1. Always confirm destructive operations before executing
2. Provide clear explanations of your actions
3. When generating code, ensure it follows {engine_name} best practices
4. Consider performance implications of your suggestions
5. Maintain consistency with existing codebase style

## Response Format
For simple operations: Direct confirmation with brief explanation
For complex operations: Structured plan with step-by-step breakdown
For code generation: Full code with inline comments explaining key decisions

Current context:
- Selected entities: {selected_entities}
- Active project: {project_name}
- Engine version: {engine_version}"#;

pub const BEVY_SPECIFIC_PROMPT: &str = r#"## Bevy ECS Guidelines
- Prefer component-based design over inheritance
- Use Resources for global state, Components for entity data
- Systems should be small, focused, and composable
- Leverage Bevy's Query system for efficient data access
- Use Events for loose coupling between systems

## Code Patterns
Components:
```rust
#[derive(Component)]
pub struct YourComponent {
    // Fields with sensible defaults
}

impl Default for YourComponent {
    fn default() -> Self { ... }
}
```

Systems:
```rust
fn your_system(
    query: Query<&YourComponent, With<SomeFilter>>,
    mut commands: Commands,
) {
    // Implementation
}
```

Plugin organization:
```rust
pub struct YourPlugin;

impl Plugin for YourPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, your_system);
    }
}
```"#;

// ============================================================================
// PromptContext — the data used to fill prompt variables
// ============================================================================

#[derive(Debug, Clone)]
pub struct PromptContext {
    pub agent_name: String,
    pub engine_name: String,
    pub language: String,
    pub project_name: String,
    pub engine_version: String,
    pub selected_entities: String,
    pub extra: HashMap<String, String>,
    pub layered_context: Option<LayeredContext>,
}

impl Default for PromptContext {
    fn default() -> Self {
        Self {
            agent_name: "AgentEdit".into(),
            engine_name: "bevy".into(),
            language: "rust".into(),
            project_name: "Untitled".into(),
            engine_version: "0.17".into(),
            selected_entities: "(none)".into(),
            extra: HashMap::new(),
            layered_context: None,
        }
    }
}

impl PromptContext {
    pub fn with_layered(layered: LayeredContext) -> Self {
        Self {
            agent_name: layered.l0_system.agent_name.clone(),
            engine_name: layered.l0_system.engine_name.clone(),
            language: layered.l0_system.language.clone(),
            project_name: layered.l1_session.project_name.clone(),
            engine_version: layered.l1_session.engine_version.clone(),
            selected_entities: layered.l2_task.selected_entities.join(", "),
            extra: HashMap::new(),
            layered_context: Some(layered),
        }
    }

    pub fn with_token_budget(mut self, budget: &TokenBudget) -> Self {
        if let Some(ref mut layered) = self.layered_context {
            layered.truncate_to_budget(budget);
        }
        self
    }
}

// ============================================================================
// RuntimeContextCollector — layered context with token budget
// ============================================================================

#[derive(Debug, Clone)]
pub struct RuntimeContextCollector {
    pub layered_context: LayeredContext,
    pub token_budget: TokenBudget,
}

impl RuntimeContextCollector {
    pub fn new(layered_context: LayeredContext) -> Self {
        Self {
            layered_context,
            token_budget: TokenBudget::default(),
        }
    }

    pub fn with_budget(layered_context: LayeredContext, token_budget: TokenBudget) -> Self {
        Self {
            layered_context,
            token_budget,
        }
    }

    pub fn collect(&mut self) -> &LayeredContext {
        self.layered_context.truncate_to_budget(&self.token_budget);
        &self.layered_context
    }

    pub fn estimated_tokens(&self) -> usize {
        estimate_tokens(&self.layered_context.describe())
    }

    pub fn is_within_budget(&self) -> bool {
        self.estimated_tokens() <= self.token_budget.total
    }

    pub fn set_budget(&mut self, budget: TokenBudget) {
        self.token_budget = budget;
    }

    pub fn to_prompt_context(&mut self) -> PromptContext {
        self.collect();
        PromptContext::with_layered(self.layered_context.clone())
    }
}

// ============================================================================
// PromptSystem — tiered prompt builder
// ============================================================================

pub struct PromptSystem {
    base_prompts: HashMap<PromptType, PromptTemplate>,
    engine_specific: HashMap<String, HashMap<PromptType, PromptTemplate>>,
    user_custom: HashMap<String, PromptTemplate>,
}

impl PromptSystem {
    pub fn new() -> Self {
        Self {
            base_prompts: HashMap::new(),
            engine_specific: HashMap::new(),
            user_custom: HashMap::new(),
        }
    }

    pub fn with_defaults() -> Self {
        let mut system = Self::new();
        system.register_base(
            PromptType::SystemIdentity,
            PromptTemplate {
                name: "base-system-identity".into(),
                template: BASE_SYSTEM_PROMPT.into(),
            },
        );
        system.register_engine(
            "bevy",
            PromptType::CodeGeneration,
            PromptTemplate {
                name: "bevy-code-gen".into(),
                template: BEVY_SPECIFIC_PROMPT.into(),
            },
        );
        system
    }

    pub fn register_base(&mut self, prompt_type: PromptType, template: PromptTemplate) {
        self.base_prompts.insert(prompt_type, template);
    }

    pub fn register_engine(
        &mut self,
        engine_name: &str,
        prompt_type: PromptType,
        template: PromptTemplate,
    ) {
        self.engine_specific
            .entry(engine_name.to_string())
            .or_default()
            .insert(prompt_type, template);
    }

    pub fn register_user(&mut self, key: &str, template: PromptTemplate) {
        self.user_custom.insert(key.to_string(), template);
    }

    pub fn build_prompt(&self, prompt_type: PromptType, context: &PromptContext) -> String {
        let mut merged = if let Some(ref layered) = context.layered_context {
            layered.describe()
        } else {
            String::new()
        };

        let base = self
            .base_prompts
            .get(&prompt_type)
            .map(|t| t.template.clone())
            .unwrap_or_default();

        if !merged.is_empty() && !base.is_empty() {
            merged.push_str("\n\n");
        }
        merged.push_str(&base);

        if let Some(engine_map) = self.engine_specific.get(&context.engine_name) {
            if let Some(engine_tpl) = engine_map.get(&prompt_type) {
                if !merged.is_empty() {
                    merged.push_str("\n\n");
                }
                merged.push_str(&engine_tpl.template);
            }
        }

        self.replace_variables(&merged, context)
    }

    pub fn build_user_prompt(&self, key: &str, context: &PromptContext) -> Option<String> {
        self.user_custom
            .get(key)
            .map(|tpl| self.replace_variables(&tpl.template, context))
    }

    pub fn build_prompt_with_budget(
        &self,
        prompt_type: PromptType,
        context: &PromptContext,
        budget: &TokenBudget,
    ) -> String {
        let mut ctx = context.clone();

        if ctx.layered_context.is_some() {
            ctx = ctx.with_token_budget(budget);
        }

        let prompt = self.build_prompt(prompt_type, &ctx);

        let tokens = estimate_tokens(&prompt);
        if tokens > budget.total {
            let safe_chars = budget.total.saturating_mul(4);
            if prompt.len() > safe_chars {
                return format!(
                    "{}\n\n[...truncated to fit token budget]",
                    &prompt[..safe_chars]
                );
            }
        }

        prompt
    }

    fn replace_variables(&self, template: &str, context: &PromptContext) -> String {
        let mut result = template.to_string();

        result = result.replace("{agent_name}", &context.agent_name);
        result = result.replace("{engine_name}", &context.engine_name);
        result = result.replace("{language}", &context.language);
        result = result.replace("{project_name}", &context.project_name);
        result = result.replace("{engine_version}", &context.engine_version);
        result = result.replace("{selected_entities}", &context.selected_entities);

        for (key, value) in &context.extra {
            result = result.replace(&format!("{{{}}}", key), value);
        }

        result
    }
}

impl Default for PromptSystem {
    fn default() -> Self {
        Self::with_defaults()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_variable_substitution() {
        let system = PromptSystem::default();
        let tmpl = "Hello {agent_name}, using {engine_name} v{engine_version}";
        let ctx = PromptContext {
            agent_name: "TestBot".into(),
            engine_name: "bevy".into(),
            engine_version: "0.17".into(),
            ..Default::default()
        };
        let result = system.replace_variables(tmpl, &ctx);
        assert!(result.contains("TestBot"));
        assert!(result.contains("bevy"));
        assert!(result.contains("0.17"));
        assert!(!result.contains("{agent_name}"));
    }

    #[test]
    fn test_extra_variables() {
        let system = PromptSystem::default();
        let tmpl = "Mode: {execution_mode}";
        let mut ctx = PromptContext::default();
        ctx.extra.insert("execution_mode".into(), "Direct".into());
        let result = system.replace_variables(tmpl, &ctx);
        assert_eq!(result, "Mode: Direct");
    }

    #[test]
    fn test_system_identity_prompt() {
        let system = PromptSystem::with_defaults();
        let ctx = PromptContext {
            agent_name: "EditorAgent".into(),
            engine_name: "bevy".into(),
            engine_version: "0.17".into(),
            language: "rust".into(),
            project_name: "MyGame".into(),
            selected_entities: "Player, Enemy".into(),
            ..Default::default()
        };
        let prompt = system.build_prompt(PromptType::SystemIdentity, &ctx);
        assert!(prompt.contains("EditorAgent"));
        assert!(prompt.contains("bevy"));
        assert!(prompt.contains("MyGame"));
        assert!(prompt.contains("Player, Enemy"));
    }

    #[test]
    fn test_engine_specific_merge() {
        let mut system = PromptSystem::new();
        system.register_base(
            PromptType::CodeGeneration,
            PromptTemplate {
                name: "base-code".into(),
                template: "Base: {engine_name}".into(),
            },
        );
        system.register_engine(
            "bevy",
            PromptType::CodeGeneration,
            PromptTemplate {
                name: "bevy-code".into(),
                template: "Bevy extensions here".into(),
            },
        );
        let ctx = PromptContext {
            engine_name: "bevy".into(),
            ..Default::default()
        };
        let prompt = system.build_prompt(PromptType::CodeGeneration, &ctx);
        assert!(prompt.contains("Base: bevy"));
        assert!(prompt.contains("Bevy extensions"));
    }

    #[test]
    fn test_user_prompt() {
        let mut system = PromptSystem::default();
        system.register_user(
            "greeting",
            PromptTemplate {
                name: "greet".into(),
                template: "Welcome {agent_name}!".into(),
            },
        );
        let ctx = PromptContext {
            agent_name: "User".into(),
            ..Default::default()
        };
        let result = system.build_user_prompt("greeting", &ctx);
        assert_eq!(result, Some("Welcome User!".into()));
    }

    #[test]
    fn test_missing_user_prompt() {
        let system = PromptSystem::default();
        let result = system.build_user_prompt("nonexistent", &PromptContext::default());
        assert!(result.is_none());
    }

    #[test]
    fn test_empty_base() {
        let system = PromptSystem::new();
        let ctx = PromptContext::default();
        let prompt = system.build_prompt(PromptType::TaskPlanning, &ctx);
        assert_eq!(prompt, "");
    }

    #[test]
    fn test_delete_example_has_confirm_param() {
        let ex = FewShotExample::delete_entity_example();
        assert_eq!(ex.action, "delete_entity");
        assert!(ex.parameters.contains_key("confirm"));
        assert_eq!(ex.parameters.get("confirm").unwrap(), "true");
        assert!(ex.thought.contains("安全确认"));
    }

    #[test]
    fn test_generate_component_has_derives() {
        let ex = FewShotExample::generate_component_example();
        assert_eq!(ex.action, "generate_component");
        assert!(ex.parameters.contains_key("derives"));
        assert!(ex.parameters.get("derives").unwrap().contains("Component"));
    }

    #[test]
    fn test_prefab_examples_chain() {
        let create_ex = FewShotExample::create_prefab_example();
        let inst_ex = FewShotExample::instantiate_prefab_example();
        assert_eq!(create_ex.action, "create_prefab");
        assert_eq!(inst_ex.action, "instantiate_prefab");
        assert!(create_ex.observation.contains("PlayerPrefab"));
        assert!(inst_ex.parameters.get("prefab_id").unwrap() == "PlayerPrefab");
    }

    #[test]
    fn test_multi_step_example_has_three_steps() {
        let ex = FewShotExample::multi_step_workflow_example();
        assert_eq!(ex.action, "multi_step_plan");
        assert!(ex.parameters.contains_key("step1_tool"));
        assert!(ex.parameters.contains_key("step2_tool"));
        assert!(ex.parameters.contains_key("step3_tool"));
        assert!(ex.thought.contains("步骤间需传递"));
    }

    #[test]
    fn test_default_examples_count() {
        let defaults = FewShotExample::default_examples();
        assert_eq!(defaults.len(), 15);
        let actions: Vec<&str> = defaults.iter().map(|e| e.action.as_str()).collect();
        assert!(actions.contains(&"create_entity"));
        assert!(actions.contains(&"delete_entity"));
        assert!(actions.contains(&"generate_component"));
        assert!(actions.contains(&"create_prefab"));
        assert!(actions.contains(&"attach_runtime_agent"));
        assert!(actions.contains(&"review_code"));
        assert!(actions.contains(&"read_file"));
        assert!(actions.contains(&"write_file"));
        assert!(actions.contains(&"grep"));
        assert!(actions.contains(&"edit_file"));
    }

    #[test]
    fn test_describe_output_format() {
        let ex = FewShotExample::create_entity_example();
        let desc = ex.describe();
        assert!(desc.contains("User:"));
        assert!(desc.contains("Thought:"));
        assert!(desc.contains("Action: create_entity"));
        assert!(desc.contains("Observation:"));
        assert!(desc.contains("---"));
    }
}
