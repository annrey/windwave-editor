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

// ============================================================================
// L0-L3 分层上下文 (Sprint 1)
// ============================================================================

/// L0: System-level context — agent identity, capabilities, constraints.
/// This is the most stable layer, rarely changes during a session.
#[derive(Debug, Clone, Default)]
pub struct L0SystemContext {
    /// Agent name and role
    pub agent_name: String,
    /// Engine being used
    pub engine_name: String,
    /// Programming language
    pub language: String,
    /// Agent capabilities
    pub capabilities: Vec<String>,
    /// Operating principles/constraints
    pub principles: Vec<String>,
}

impl L0SystemContext {
    pub fn default_bevy() -> Self {
        Self {
            agent_name: "AgentEdit".into(),
            engine_name: "bevy".into(),
            language: "rust".into(),
            capabilities: vec![
                "Scene manipulation".into(),
                "Code generation".into(),
                "Asset management".into(),
                "Debugging assistance".into(),
            ],
            principles: vec![
                "Always confirm destructive operations before executing".into(),
                "Provide clear explanations of actions".into(),
                "Follow Bevy ECS best practices".into(),
                "Consider performance implications".into(),
                "Maintain consistency with existing codebase".into(),
            ],
        }
    }

    pub fn describe(&self) -> String {
        format!(
            "## Agent Identity\n\
             You are {}, operating within the {} engine ({})\n\
             \n\
             ## Capabilities\n\
             {}\n\
             \n\
             ## Principles\n\
             {}",
            self.agent_name,
            self.engine_name,
            self.language,
            self.capabilities.iter().map(|c| format!("- {}", c)).collect::<Vec<_>>().join("\n"),
            self.principles.iter().map(|p| format!("- {}", p)).collect::<Vec<_>>().join("\n"),
        )
    }
}

/// L1: Session-level context — conversation history, project state, memory.
/// Persists across the entire session.
#[derive(Debug, Clone, Default)]
pub struct L1SessionContext {
    /// Project name
    pub project_name: String,
    /// Engine version
    pub engine_version: String,
    /// Recent conversation summary
    pub conversation_summary: String,
    /// Project-specific conventions
    pub conventions: Vec<String>,
    /// Recent actions (last N)
    pub recent_actions: Vec<String>,
}

impl L1SessionContext {
    pub fn describe(&self) -> String {
        format!(
            "## Project Context\n\
             Project: {} (v{})\n\
             \n\
             ## Recent Actions\n\
             {}\n\
             \n\
             ## Conventions\n\
             {}",
            self.project_name,
            self.engine_version,
            if self.recent_actions.is_empty() {
                "(none)".into()
            } else {
                self.recent_actions.iter().map(|a| format!("- {}", a)).collect::<Vec<_>>().join("\n")
            },
            if self.conventions.is_empty() {
                "(none)".into()
            } else {
                self.conventions.iter().map(|c| format!("- {}", c)).collect::<Vec<_>>().join("\n")
            },
        )
    }
}

/// L2: Task-level context — current task, selected entities, goals.
/// Changes per task.
#[derive(Debug, Clone, Default)]
pub struct L2TaskContext {
    /// Current task description
    pub current_task: String,
    /// Selected entities
    pub selected_entities: Vec<String>,
    /// Task goals
    pub goals: Vec<String>,
    /// Constraints for this task
    pub constraints: Vec<String>,
}

impl L2TaskContext {
    pub fn describe(&self) -> String {
        format!(
            "## Current Task\n\
             {}\n\
             \n\
             ## Selected Entities\n\
             {}\n\
             \n\
             ## Goals\n\
             {}\n\
             \n\
             ## Constraints\n\
             {}",
            self.current_task,
            if self.selected_entities.is_empty() {
                "(none)".into()
            } else {
                self.selected_entities.iter().map(|e| format!("- {}", e)).collect::<Vec<_>>().join("\n")
            },
            if self.goals.is_empty() {
                "(none)".into()
            } else {
                self.goals.iter().map(|g| format!("- {}", g)).collect::<Vec<_>>().join("\n")
            },
            if self.constraints.is_empty() {
                "(none)".into()
            } else {
                self.constraints.iter().map(|c| format!("- {}", c)).collect::<Vec<_>>().join("\n")
            },
        )
    }
}

/// L3: Entity-level context — specific entity details, component state.
/// Most granular, changes per operation.
#[derive(Debug, Clone)]
pub struct L3EntityContext {
    /// Entity name
    pub entity_name: String,
    /// Entity ID
    pub entity_id: u64,
    /// Component details
    pub components: Vec<EntityComponent>,
    /// Parent entity (if any)
    pub parent: Option<String>,
    /// Children entities
    pub children: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct EntityComponent {
    pub name: String,
    pub properties: HashMap<String, String>,
}

impl L3EntityContext {
    pub fn describe(&self) -> String {
        format!(
            "## Entity: {} (ID: {})\n\
             {}\n\
             \n\
             ## Components\n\
             {}",
            self.entity_name,
            self.entity_id,
            format!(
                "Parent: {}\nChildren: {}",
                self.parent.as_deref().unwrap_or("(none)"),
                if self.children.is_empty() {
                    "(none)".into()
                } else {
                    self.children.iter().map(|c| format!("- {}", c)).collect::<Vec<_>>().join(", ")
                }
            ),
            if self.components.is_empty() {
                "(none)".into()
            } else {
                self.components.iter().map(|c| {
                    let props = c.properties.iter()
                        .map(|(k, v)| format!("{}={}", k, v))
                        .collect::<Vec<_>>().join(", ");
                    format!("- {}: {}", c.name, props)
                }).collect::<Vec<_>>().join("\n")
            },
        )
    }
}

/// Combined context for LLM prompt (all 4 layers).
#[derive(Debug, Clone, Default)]
pub struct LayeredContext {
    pub l0_system: L0SystemContext,
    pub l1_session: L1SessionContext,
    pub l2_task: L2TaskContext,
    pub l3_entities: Vec<L3EntityContext>,
    /// Sprint 1: Few-shot examples for tool usage
    pub few_shot_examples: Vec<FewShotExample>,
    /// Memory system context (Working + Episodic + Semantic + Procedural)
    pub memory_context: Option<crate::memory::MemoryContext>,
}

/// Few-shot example for demonstrating tool usage patterns.
#[derive(Debug, Clone)]
pub struct FewShotExample {
    /// User request that triggered this pattern
    pub user_request: String,
    /// Agent's thought process
    pub thought: String,
    /// Tool action taken
    pub action: String,
    /// Tool parameters
    pub parameters: HashMap<String, String>,
    /// Observation/result
    pub observation: String,
}

impl FewShotExample {
    pub fn describe(&self) -> String {
        format!(
            "User: \"{}\"\n\
             Thought: {}\n\
             Action: {} with {:?}\n\
             Observation: {}\n\
             ---",
            self.user_request,
            self.thought,
            self.action,
            self.parameters,
            self.observation,
        )
    }

    /// Create a few-shot example for creating an entity.
    pub fn create_entity_example() -> Self {
        let mut params = HashMap::new();
        params.insert("name".into(), "Enemy".into());
        params.insert("sprite_color".into(), "[1.0, 0.0, 0.0, 1.0]".into());
        params.insert("position".into(), "[100.0, 0.0]".into());

        Self {
            user_request: "创建一个红色敌人放在右侧".into(),
            thought: "用户想要创建一个敌人实体，颜色为红色，位置在右侧。需要使用 create_entity 工具，设置 sprite_color 为红色，position 为右侧坐标。".into(),
            action: "create_entity".into(),
            parameters: params,
            observation: "Entity 'Enemy' created with ID 42 at position (100, 0) with red color".into(),
        }
    }

    /// Create a few-shot example for updating a component.
    pub fn update_component_example() -> Self {
        let mut params = HashMap::new();
        params.insert("entity_id".into(), "42".into());
        params.insert("component".into(), "Sprite".into());
        params.insert("color".into(), "[0.0, 0.0, 1.0, 1.0]".into());

        Self {
            user_request: "把 Player 改成蓝色".into(),
            thought: "用户想要修改 Player 的颜色为蓝色。需要先查询 Player 的 entity_id，然后使用 update_component 工具修改 Sprite 组件的 color 属性。".into(),
            action: "update_component".into(),
            parameters: params,
            observation: "Entity 42 (Player) Sprite color updated to blue".into(),
        }
    }

    /// Create a few-shot example for querying entities.
    pub fn query_entities_example() -> Self {
        let mut params = HashMap::new();
        params.insert("filter".into(), "Enemy".into());

        Self {
            user_request: "列出所有敌人".into(),
            thought: "用户想要查看所有敌人实体。使用 query_entities 工具，filter 参数设为 'Enemy' 来筛选。".into(),
            action: "query_entities".into(),
            parameters: params,
            observation: "Found 3 entities matching 'Enemy': Enemy_01 (id=5), Enemy_02 (id=6), Enemy_03 (id=7)".into(),
        }
    }
}

impl LayeredContext {
    /// Add a few-shot example to the context.
    pub fn add_few_shot(&mut self, example: FewShotExample) {
        self.few_shot_examples.push(example);
    }

    /// Sprint 1: Select few-shot examples most relevant to the user request.
    ///
    /// Uses simple keyword matching to score each example's relevance.
    /// Returns the top N most relevant examples.
    pub fn select_few_shot_examples(&self, user_request: &str, top_n: usize) -> Vec<&FewShotExample> {
        let request_lower = user_request.to_lowercase();
        let request_words: Vec<&str> = request_lower.split_whitespace().collect();

        let mut scored: Vec<(f32, &FewShotExample)> = self.few_shot_examples.iter()
            .map(|example| {
                let mut score = 0.0f32;

                // Score based on user_request match
                let ex_request_lower = example.user_request.to_lowercase();
                for word in &request_words {
                    if ex_request_lower.contains(word) {
                        score += 2.0;
                    }
                }

                // Score based on action match
                let action_lower = example.action.to_lowercase();
                if request_lower.contains("create") || request_lower.contains("创建") || request_lower.contains("生成") {
                    if action_lower.contains("create") { score += 3.0; }
                }
                if request_lower.contains("update") || request_lower.contains("修改") || request_lower.contains("改") || request_lower.contains("换") {
                    if action_lower.contains("update") { score += 3.0; }
                }
                if request_lower.contains("delete") || request_lower.contains("删除") || request_lower.contains("移除") {
                    if action_lower.contains("delete") { score += 3.0; }
                }
                if request_lower.contains("query") || request_lower.contains("list") || request_lower.contains("查询") || request_lower.contains("列表") {
                    if action_lower.contains("query") { score += 3.0; }
                }

                // Score based on entity type match
                if request_lower.contains("enemy") || request_lower.contains("敌人") {
                    if ex_request_lower.contains("enemy") || ex_request_lower.contains("敌人") { score += 2.0; }
                }
                if request_lower.contains("player") || request_lower.contains("玩家") {
                    if ex_request_lower.contains("player") || ex_request_lower.contains("玩家") { score += 2.0; }
                }

                // Score based on color match
                if request_lower.contains("red") || request_lower.contains("红色") {
                    if ex_request_lower.contains("red") || ex_request_lower.contains("红色") { score += 1.5; }
                }
                if request_lower.contains("blue") || request_lower.contains("蓝色") {
                    if ex_request_lower.contains("blue") || ex_request_lower.contains("蓝色") { score += 1.5; }
                }

                (score, example)
            })
            .collect();

        // Sort by score descending
        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

        // Return top N
        scored.into_iter().take(top_n).map(|(_, ex)| ex).collect()
    }

    /// Sprint 1: Build prompt with dynamically selected few-shot examples.
    ///
    /// Only includes examples relevant to the current user request,
    /// filtered by the token budget.
    pub fn build_prompt_with_selected_examples(&self, user_request: &str, top_n: usize) -> String {
        let mut parts = Vec::new();

        parts.push(self.l0_system.describe());
        parts.push(self.l1_session.describe());
        parts.push(self.l2_task.describe());

        if !self.l3_entities.is_empty() {
            parts.push("## Entity Details\n".into());
            for entity in &self.l3_entities {
                parts.push(entity.describe());
                parts.push("\n".into());
            }
        }

        // Memory system context
        if let Some(ref mem_ctx) = self.memory_context {
            let mem_text = mem_ctx.to_prompt_section();
            if !mem_text.is_empty() {
                parts.push(mem_text);
            }
        }

        // Sprint 1: Dynamically select few-shot examples
        let selected = self.select_few_shot_examples(user_request, top_n);
        if !selected.is_empty() {
            parts.push("## Few-Shot Examples (Relevant to Your Request)\n".into());
            parts.push("Here are examples similar to your request:\n".into());
            for example in selected {
                parts.push(example.describe());
            }
        }

        parts.join("\n\n")
    }

    /// Set memory context from MemorySystem
    pub fn with_memory(mut self, memory: crate::memory::MemoryContext) -> Self {
        self.memory_context = Some(memory);
        self
    }

    /// Build full context description for LLM prompt.
    pub fn describe(&self) -> String {
        let mut parts = Vec::new();
        
        parts.push(self.l0_system.describe());
        parts.push(self.l1_session.describe());
        parts.push(self.l2_task.describe());
        
        if !self.l3_entities.is_empty() {
            parts.push("## Entity Details\n".into());
            for entity in &self.l3_entities {
                parts.push(entity.describe());
                parts.push("\n".into());
            }
        }

        // Memory system context (Working + Episodic + Semantic + Procedural)
        if let Some(ref mem_ctx) = self.memory_context {
            let mem_text = mem_ctx.to_prompt_section();
            if !mem_text.is_empty() {
                parts.push(mem_text);
            }
        }
        
        // Sprint 1: Few-shot examples
        if !self.few_shot_examples.is_empty() {
            parts.push("## Few-Shot Examples (Tool Usage Patterns)\n".into());
            parts.push("Here are examples of how to use tools effectively:\n".into());
            for example in &self.few_shot_examples {
                parts.push(example.describe());
            }
        }
        
        parts.join("\n\n")
    }

    /// Check if context is empty (no layers populated).
    pub fn is_empty(&self) -> bool {
        self.l0_system.agent_name.is_empty()
            && self.l1_session.project_name.is_empty()
            && self.l2_task.current_task.is_empty()
            && self.l3_entities.is_empty()
            && self.few_shot_examples.is_empty()
            && self.memory_context.is_none()
    }

    /// Truncate context layers to fit within token budget.
    ///
    /// Priority order (highest to lowest):
    /// 1. L0 System context (always preserved)
    /// 2. L2 Task context (always preserved, truncated last)
    /// 3. L1 Session context (truncated if needed)
    /// 4. L3 Entity details (truncated before L1)
    /// 5. Few-shot examples (truncated first)
    ///
    /// If still over budget after truncating L3 and few-shot, L2 task description
    /// will be proportionally reduced.
    pub fn truncate_to_budget(&mut self, budget: &TokenBudget) {
        let allocation = budget.allocate();

        // Map layers to budget allocations
        let _l0_budget = allocation.system_prompt;
        let l1_budget = allocation.conversation;
        let l2_budget = allocation.scene_context;
        let l3_budget = allocation.tool_description;

        // Step 1: Truncate few-shot examples first (lowest priority)
        if !self.few_shot_examples.is_empty() {
            let few_shot_text = self.few_shot_examples.iter()
                .map(|e| e.describe())
                .collect::<Vec<_>>()
                .join("\n");
            let few_shot_tokens = estimate_tokens(&few_shot_text);
            let few_shot_budget = l3_budget / 2; // Use part of tool budget for few-shot

            if few_shot_tokens > few_shot_budget {
                // Keep examples that fit within budget
                let mut kept = Vec::new();
                let mut current_tokens = 0;
                for example in &self.few_shot_examples {
                    let ex_tokens = estimate_tokens(&example.describe());
                    if current_tokens + ex_tokens <= few_shot_budget {
                        kept.push(example.clone());
                        current_tokens += ex_tokens;
                    } else {
                        break;
                    }
                }
                self.few_shot_examples = kept;
            }
        }

        // Step 2: Truncate L3 entity details
        if !self.l3_entities.is_empty() {
            let l3_text = self.l3_entities.iter()
                .map(|e| e.describe())
                .collect::<Vec<_>>()
                .join("\n");
            let l3_tokens = estimate_tokens(&l3_text);

            if l3_tokens > l3_budget {
                let mut kept = Vec::new();
                let mut current_tokens = 0;
                for entity in &self.l3_entities {
                    let entity_tokens = estimate_tokens(&entity.describe());
                    if current_tokens + entity_tokens <= l3_budget {
                        kept.push(entity.clone());
                        current_tokens += entity_tokens;
                    } else {
                        break;
                    }
                }
                self.l3_entities = kept;
            }
        }

        // Step 3: Truncate L1 session context
        let l1_text = self.l1_session.describe();
        let l1_tokens = estimate_tokens(&l1_text);
        if l1_tokens > l1_budget {
            // Truncate recent_actions first
            let max_actions = (l1_budget / 20).max(1); // Rough estimate: ~20 tokens per action
            if self.l1_session.recent_actions.len() > max_actions {
                let start = self.l1_session.recent_actions.len() - max_actions;
                self.l1_session.recent_actions = self.l1_session.recent_actions[start..].to_vec();
            }

            // If still over budget, truncate conversation_summary
            let summary_budget = l1_budget / 4;
            let summary_tokens = estimate_tokens(&self.l1_session.conversation_summary);
            if summary_tokens > summary_budget {
                let chars_per_token = 4;
                let max_chars = summary_budget * chars_per_token;
                if self.l1_session.conversation_summary.len() > max_chars {
                    self.l1_session.conversation_summary = format!(
                        "{}...[truncated]",
                        &self.l1_session.conversation_summary[..max_chars.saturating_sub(15)]
                    );
                }
            }
        }

        // Step 4: If still over total budget, truncate L2 task description
        let total_after = estimate_tokens(&self.describe());
        if total_after > budget.total {
            let l2_text = self.l2_task.describe();
            let l2_tokens = estimate_tokens(&l2_text);
            if l2_tokens > l2_budget {
                let chars_per_token = 4;
                let max_chars = l2_budget * chars_per_token;
                if self.l2_task.current_task.len() > max_chars {
                    self.l2_task.current_task = format!(
                        "{}...[truncated]",
                        &self.l2_task.current_task[..max_chars.saturating_sub(15)]
                    );
                }

                // Also truncate goals and constraints if needed
                let max_goals = (l2_budget / 30).max(1);
                if self.l2_task.goals.len() > max_goals {
                    let start = self.l2_task.goals.len() - max_goals;
                    self.l2_task.goals = self.l2_task.goals[start..].to_vec();
                }
                if self.l2_task.constraints.len() > max_goals {
                    let start = self.l2_task.constraints.len() - max_goals;
                    self.l2_task.constraints = self.l2_task.constraints[start..].to_vec();
                }
            }
        }
    }
}

/// Categories of prompts used throughout the Agent lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PromptType {
    /// Agent identity and role definition.
    SystemIdentity,
    /// Description of available capabilities.
    SystemCapabilities,
    /// Task planning instructions.
    TaskPlanning,
    /// Task decomposition strategies.
    TaskDecomposition,
    /// Tool selection heuristics.
    ToolSelection,
    /// Code generation guidelines.
    CodeGeneration,
    /// Scene manipulation instructions.
    SceneManipulation,
    /// Response formatting rules.
    ResponseFormatting,
    /// Error explanation template.
    ErrorExplanation,
    /// Clarification request template.
    ClarificationRequest,
}

/// A named, parameterised prompt template.
#[derive(Debug, Clone)]
pub struct PromptTemplate {
    /// Human-readable label (e.g. "bevy-system-base").
    pub name: String,
    /// The template text containing `{variable}` placeholders.
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
// Context bundle — the data used to fill prompt variables
// ============================================================================

/// Values substituted into prompt templates for `{variable}` placeholders.
/// Sprint 1: Now includes layered context (L0-L3) for richer LLM prompts.
#[derive(Debug, Clone)]
pub struct PromptContext {
    pub agent_name: String,
    pub engine_name: String,
    pub language: String,
    pub project_name: String,
    pub engine_version: String,
    pub selected_entities: String,
    /// Arbitrary extra key-value pairs.
    pub extra: HashMap<String, String>,
    /// Sprint 1: Layered context for L0-L3 injection
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
    /// Create a context with layered context for Sprint 1 LLM闭环.
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

    /// Create a context with a token budget applied to the layered context.
    ///
    /// If layered context exists, it will be truncated to fit within the budget.
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

/// Collects runtime context with token budget management.
///
/// Combines LayeredContext (L0-L3) with TokenBudget for automatic
/// context truncation when building prompts.
#[derive(Debug, Clone)]
pub struct RuntimeContextCollector {
    pub layered_context: LayeredContext,
    pub token_budget: TokenBudget,
}

impl RuntimeContextCollector {
    /// Create a new collector with default budget (8192 tokens).
    pub fn new(layered_context: LayeredContext) -> Self {
        Self {
            layered_context,
            token_budget: TokenBudget::default(),
        }
    }

    /// Create a new collector with a custom token budget.
    pub fn with_budget(layered_context: LayeredContext, token_budget: TokenBudget) -> Self {
        Self {
            layered_context,
            token_budget,
        }
    }

    /// Apply token budget and return a truncated layered context.
    pub fn collect(&mut self) -> &LayeredContext {
        self.layered_context.truncate_to_budget(&self.token_budget);
        &self.layered_context
    }

    /// Get current token usage estimate.
    pub fn estimated_tokens(&self) -> usize {
        estimate_tokens(&self.layered_context.describe())
    }

    /// Check if current context is within budget.
    pub fn is_within_budget(&self) -> bool {
        self.estimated_tokens() <= self.token_budget.total
    }

    /// Update token budget.
    pub fn set_budget(&mut self, budget: TokenBudget) {
        self.token_budget = budget;
    }

    /// Build a PromptContext from this collector (applying budget truncation).
    pub fn to_prompt_context(&mut self) -> PromptContext {
        self.collect();
        PromptContext::with_layered(self.layered_context.clone())
    }
}

// ============================================================================
// PromptSystem — tiered prompt builder
// ============================================================================

/// The central prompt factory.
///
/// Holds three tiers of templates:
/// 1. **Base** — shared across all engines / users.
/// 2. **Engine-specific** — overrides or extensions for a particular engine.
/// 3. **User-custom** — per-user overrides (highest priority).
pub struct PromptSystem {
    base_prompts: HashMap<PromptType, PromptTemplate>,
    engine_specific: HashMap<String, HashMap<PromptType, PromptTemplate>>,
    user_custom: HashMap<String, PromptTemplate>,
}

impl PromptSystem {
    // ------------------------------------------------------------------
    // Construction
    // ------------------------------------------------------------------

    /// Create an empty prompt system.
    pub fn new() -> Self {
        Self {
            base_prompts: HashMap::new(),
            engine_specific: HashMap::new(),
            user_custom: HashMap::new(),
        }
    }

    /// Create a system pre-loaded with the default base templates.
    pub fn with_defaults() -> Self {
        let mut system = Self::new();
        system.register_base(PromptType::SystemIdentity, PromptTemplate {
            name: "base-system-identity".into(),
            template: BASE_SYSTEM_PROMPT.into(),
        });
        system.register_engine("bevy", PromptType::CodeGeneration, PromptTemplate {
            name: "bevy-code-gen".into(),
            template: BEVY_SPECIFIC_PROMPT.into(),
        });
        system
    }

    // ------------------------------------------------------------------
    // Registration
    // ------------------------------------------------------------------

    /// Register a base (engine-independent) template.
    pub fn register_base(&mut self, prompt_type: PromptType, template: PromptTemplate) {
        self.base_prompts.insert(prompt_type, template);
    }

    /// Register an engine-specific template.
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

    /// Register a user-custom override (keyed by a custom name).
    pub fn register_user(&mut self, key: &str, template: PromptTemplate) {
        self.user_custom.insert(key.to_string(), template);
    }

    // ------------------------------------------------------------------
    // Prompt building
    // ------------------------------------------------------------------

    /// Build the final prompt string for a given type and context.
    ///
    /// Sprint 1: If layered_context is provided, injects L0-L3 context before
    /// the standard template. The merging order is:
    /// layered_context (L0→L1→L2→L3) → base → engine-specific → variable substitution.
    pub fn build_prompt(
        &self,
        prompt_type: PromptType,
        context: &PromptContext,
    ) -> String {
        // Sprint 1: Start with layered context if available
        let mut merged = if let Some(ref layered) = context.layered_context {
            layered.describe()
        } else {
            String::new()
        };

        // 1. Base template
        let base = self
            .base_prompts
            .get(&prompt_type)
            .map(|t| t.template.clone())
            .unwrap_or_default();

        if !merged.is_empty() && !base.is_empty() {
            merged.push_str("\n\n");
        }
        merged.push_str(&base);

        // 2. Engine-specific extension (appended)
        if let Some(engine_map) = self.engine_specific.get(&context.engine_name) {
            if let Some(engine_tpl) = engine_map.get(&prompt_type) {
                if !merged.is_empty() {
                    merged.push_str("\n\n");
                }
                merged.push_str(&engine_tpl.template);
            }
        }

        // 3. Variable substitution
        self.replace_variables(&merged, context)
    }

    /// Build a named user-custom prompt.
    pub fn build_user_prompt(
        &self,
        key: &str,
        context: &PromptContext,
    ) -> Option<String> {
        self.user_custom
            .get(key)
            .map(|tpl| self.replace_variables(&tpl.template, context))
    }

    /// Build prompt with token budget enforcement.
    ///
    /// 1. Builds the full prompt with layered context
    /// 2. Estimates token count
    /// 3. If over budget, truncates layered context layers proportionally
    /// 4. Priority: L0 System > L2 Task > L1 Session > L3 Entity > Few-shot
    pub fn build_prompt_with_budget(
        &self,
        prompt_type: PromptType,
        context: &PromptContext,
        budget: &TokenBudget,
    ) -> String {
        // Clone context so we can mutate layered context for truncation
        let mut ctx = context.clone();

        // Apply token budget truncation if layered context exists
        if ctx.layered_context.is_some() {
            ctx = ctx.with_token_budget(budget);
        }

        // Build prompt with potentially truncated context
        let prompt = self.build_prompt(prompt_type, &ctx);

        // Final safety check: if still over budget, do a hard truncate on the final string
        let tokens = estimate_tokens(&prompt);
        if tokens > budget.total {
            // Hard truncate: keep first budget.total * 4 chars (rough estimate)
            let safe_chars = budget.total.saturating_mul(4);
            if prompt.len() > safe_chars {
                return format!("{}\n\n[...truncated to fit token budget]", &prompt[..safe_chars]);
            }
        }

        prompt
    }

    // ------------------------------------------------------------------
    // Variable substitution
    // ------------------------------------------------------------------

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

// ============================================================================
// TokenBudget — 上下文窗口预算分配
// ============================================================================

/// Token budget allocator for LLM context windows.
///
/// Divides the total token budget across prompt sections to prevent
/// context overflow. Ratios are configurable.
#[derive(Debug, Clone)]
pub struct TokenBudget {
    /// Total token budget (e.g. 8192 for gpt-4o-mini).
    pub total: usize,
    /// Fraction for system prompt / agent identity (default 0.30).
    pub system_prompt_ratio: f32,
    /// Fraction for scene context (default 0.40).
    pub scene_context_ratio: f32,
    /// Fraction for conversation history (default 0.20).
    pub conversation_ratio: f32,
    /// Fraction for tool/skill descriptions (default 0.10).
    pub tool_description_ratio: f32,
}

impl Default for TokenBudget {
    fn default() -> Self {
        Self {
            total: 8192,
            system_prompt_ratio: 0.30,
            scene_context_ratio: 0.40,
            conversation_ratio: 0.20,
            tool_description_ratio: 0.10,
        }
    }
}

impl TokenBudget {
    pub fn new(total: usize) -> Self {
        Self { total, ..Default::default() }
    }

    /// Allocate the budget across sections.
    pub fn allocate(&self) -> TokenAllocation {
        TokenAllocation {
            system_prompt: (self.total as f32 * self.system_prompt_ratio) as usize,
            scene_context: (self.total as f32 * self.scene_context_ratio) as usize,
            conversation: (self.total as f32 * self.conversation_ratio) as usize,
            tool_description: (self.total as f32 * self.tool_description_ratio) as usize,
        }
    }
}

/// Result of TokenBudget::allocate().
#[derive(Debug, Clone)]
pub struct TokenAllocation {
    pub system_prompt: usize,
    pub scene_context: usize,
    pub conversation: usize,
    pub tool_description: usize,
}

/// Rough token estimator: 1 token ≈ 4 characters for CJK, 1 token ≈ 4 chars for English.
/// Coarse estimate for budget enforcement — not exact.
pub fn estimate_tokens(text: &str) -> usize {
    // Count CJK characters (≈1.5 tokens each) + ASCII (≈0.25 tokens each)
    let cjk_count = text.chars().filter(|c| c >= &'\u{4E00}' && c <= &'\u{9FFF}').count();
    let ascii_count = text.len() - cjk_count;
    (cjk_count * 3 / 2) + (ascii_count / 4)
}

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
        ctx.extra
            .insert("execution_mode".into(), "Direct".into());
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
}
