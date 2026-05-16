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

    pub fn delete_entity_example() -> Self {
        let mut params = HashMap::new();
        params.insert("entity_id".into(), "15".into());
        params.insert("confirm".into(), "true".into());

        Self {
            user_request: "删除id为15的实体".into(),
            thought: "用户要求删除特定实体。需要使用 delete_entity 工具，必须提供 entity_id 和 confirm=true 安全确认参数来执行删除操作。".into(),
            action: "delete_entity".into(),
            parameters: params,
            observation: "Entity 15 (Old_Enemy) deleted successfully".into(),
        }
    }

    pub fn generate_component_example() -> Self {
        let mut params = HashMap::new();
        params.insert("name".into(), "Health".into());
        params.insert("properties".into(), "current_hp: f64, max_hp: f64".into());
        params.insert("derives".into(), "Component, Debug, Clone, Reflect".into());

        Self {
            user_request: "创建一个血量组件".into(),
            thought: "用户需要一个表示生命值的组件。使用 generate_component 工具生成 Bevy Component 结构体，包含 current_hp 和 max_hp 字段，并派生常用的 trait。".into(),
            action: "generate_component".into(),
            parameters: params,
            observation: "Generated component 'Health' with fields: current_hp (f64), max_hp (f64), derives: [Component, Debug, Clone, Reflect]".into(),
        }
    }

    pub fn generate_system_example() -> Self {
        let mut params = HashMap::new();
        params.insert("name".into(), "health_regen".into());
        params.insert("query".into(), "Query<&mut Health>".into());
        params.insert("logic".into(), "for mut health in &mut query { health.current_hp = (health.current_hp + 1.0).min(health.max_hp); }".into());

        Self {
            user_request: "写一个自动回血的逻辑".into(),
            thought: "用户需要实现一个每帧恢复生命值的系统逻辑。使用 generate_system 工具生成 Bevy System，查询 Health 组件并逐步恢复到最大值。".into(),
            action: "generate_system".into(),
            parameters: params,
            observation: "Generated system 'health_regen' with query: Query<&mut Health>, logic applied".into(),
        }
    }

    pub fn create_prefab_example() -> Self {
        let mut params = HashMap::new();
        params.insert("entity_id".into(), "42".into());
        params.insert("name".into(), "PlayerPrefab".into());

        Self {
            user_request: "把当前玩家存为预制体".into(),
            thought: "用户想把一个已配置好的实体保存为可复用的模板。使用 create_prefab 工具从指定实体创建预制体，之后可以通过 instantiate_prefab 复用。".into(),
            action: "create_prefab".into(),
            parameters: params,
            observation: "Prefab 'PlayerPrefab' created from entity 42, containing [Sprite, Transform, PlayerController]".into(),
        }
    }

    pub fn instantiate_prefab_example() -> Self {
        let mut params = HashMap::new();
        params.insert("prefab_id".into(), "PlayerPrefab".into());
        params.insert("position".into(), "[200.0, 100.0]".into());

        Self {
            user_request: "在(200,100)位置放一个玩家预制体".into(),
            thought: "用户需要在指定位置实例化一个已有的预制体。使用 instantiate_prefab 工具，传入 prefab_id 和目标位置坐标。".into(),
            action: "instantiate_prefab".into(),
            parameters: params,
            observation: "Instantiated 'PlayerPrefab' at position (200, 100) as entity ID 88".into(),
        }
    }

    pub fn attach_agent_example() -> Self {
        let mut params = HashMap::new();
        params.insert("entity_id".into(), "42".into());
        params.insert("profile_id".into(), "patrol_ai".into());
        params.insert("control_mode".into(), "Autonomous".into());

        Self {
            user_request: "给这个敌人挂上巡逻AI".into(),
            thought: "用户想为实体附加AI行为。使用 attach_runtime_agent 工具将 patrol_ai 配置文件绑定到实体上，设为自主控制模式让AI自动运行。".into(),
            action: "attach_runtime_agent".into(),
            parameters: params,
            observation: "Runtime agent 'patrol_ai' attached to entity 42 in Autonomous mode".into(),
        }
    }

    pub fn multi_step_workflow_example() -> Self {
        let mut params = HashMap::new();
        params.insert("step1_tool".into(), "create_entity".into());
        params.insert("step1_params".into(), r#"{"name":"Boss","sprite_color":"[1.0,1.0,0.0,1.0]","position":"[300.0,200.0]"}"#.into());
        params.insert("step2_tool".into(), "attach_runtime_agent".into());
        params.insert("step2_params".into(), r#"{"entity_id":"<step1_result>","profile_id":"boss_ai"}"#.into());
        params.insert("step3_tool".into(), "create_prefab".into());
        params.insert("step3_params".into(), r#"{"entity_id":"<step1_result>","name":"BossTemplate"}"#.into());

        Self {
            user_request: "创建一个黄色Boss并挂上AI然后存为模板".into(),
            thought: "这是一个多步骤复杂请求：1)先用 create_entity 创建Boss实体设置颜色和位置；2)再用 attach_runtime_agent 附加AI行为；3)最后用 create_prefab 保存为可复用模板。步骤间需传递 entity_id。".into(),
            action: "multi_step_plan".into(),
            parameters: params,
            observation: "Workflow completed: Boss(entity#99) created → boss_ai attached → BossTemplate prefab saved".into(),
        }
    }

    pub fn code_review_example() -> Self {
        let mut params = HashMap::new();
        params.insert("code".into(), "fn update(&mut self) { self.x += 1; }".into());

        Self {
            user_request: "检查这段代码有没有问题".into(),
            thought: "用户请求代码审查。使用 review_code 工具对代码片段进行静态分析，检查潜在问题如边界条件、性能、命名规范等。".into(),
            action: "review_code".into(),
            parameters: params,
            observation: "Review: No critical issues found. Suggestion: consider adding bounds check if x has a maximum value.".into(),
        }
    }

    pub fn default_examples() -> Vec<Self> {
        vec![
            Self::create_entity_example(),
            Self::update_component_example(),
            Self::query_entities_example(),
            Self::delete_entity_example(),
            Self::generate_component_example(),
            Self::generate_system_example(),
            Self::create_prefab_example(),
            Self::instantiate_prefab_example(),
            Self::attach_agent_example(),
            Self::multi_step_workflow_example(),
            Self::code_review_example(),
            Self::read_file_example(),
            Self::write_file_example(),
            Self::grep_example(),
            Self::edit_file_example(),
        ]
    }

    pub fn scene_tool_examples() -> Vec<Self> {
        vec![
            Self::create_entity_example(),
            Self::update_component_example(),
            Self::delete_entity_example(),
            Self::query_entities_example(),
        ]
    }

    pub fn code_tool_examples() -> Vec<Self> {
        vec![
            Self::generate_component_example(),
            Self::generate_system_example(),
            Self::code_review_example(),
        ]
    }

    pub fn asset_tool_examples() -> Vec<Self> {
        vec![
            Self::create_prefab_example(),
            Self::instantiate_prefab_example(),
        ]
    }

    pub fn agent_tool_examples() -> Vec<Self> {
        vec![
            Self::attach_agent_example(),
            Self::multi_step_workflow_example(),
        ]
    }

    pub fn file_tool_examples() -> Vec<Self> {
        vec![
            Self::read_file_example(),
            Self::write_file_example(),
            Self::grep_example(),
            Self::edit_file_example(),
        ]
    }

    pub fn read_file_example() -> Self {
        let mut params = HashMap::new();
        params.insert("path".into(), "src/main.rs".into());
        params.insert("offset".into(), "1".into());
        params.insert("limit".into(), "50".into());

        Self {
            user_request: "看看main.rs的内容".into(),
            thought: "用户想查看源代码文件内容。使用 read_file 工具读取文件，设置 offset=1 从头开始，limit=50 限制输出行数。".into(),
            action: "read_file".into(),
            parameters: params,
            observation: "Read 50 lines (1-50 of 120) from 'src/main.rs'".into(),
        }
    }

    pub fn write_file_example() -> Self {
        let mut params = HashMap::new();
        params.insert("path".into(), "src/components/player.rs".into());
        params.insert("content".into(), "pub struct Player { pub name: String, pub hp: f64 }\nimpl Player { pub fn new(name: &str) -> Self { Self { name: name.into(), hp: 100.0 } } }".into());

        Self {
            user_request: "创建一个Player组件文件".into(),
            thought: "用户需要创建新的Rust源文件。使用 write_file 工具写入完整代码内容到指定路径，create_dirs=true 自动创建父目录。".into(),
            action: "write_file".into(),
            parameters: params,
            observation: "Wrote 142 bytes to 'src/components/player.rs'".into(),
        }
    }

    pub fn grep_example() -> Self {
        let mut params = HashMap::new();
        params.insert("pattern".into(), "fn update".into());
        params.insert("path".into(), "src".into());
        params.insert("file_pattern".into(), "*.rs".into());

        Self {
            user_request: "在src目录下找所有包含update函数的文件".into(),
            thought: "用户需要在代码库中搜索特定模式。使用 grep 工具在 src 目录下搜索 'fn update' 模式，过滤 .rs 文件类型。".into(),
            action: "grep".into(),
            parameters: params,
            observation: "Found 5 match(es) in 3 file(s), searched 12 file(s)".into(),
        }
    }

    pub fn edit_file_example() -> Self {
        let mut params = HashMap::new();
        params.insert("path".into(), "src/config.toml".into());
        params.insert("old_text".into(), "window_width = 800".into());
        params.insert("new_text".into(), "window_width = 1920".into());

        Self {
            user_request: "把配置文件的窗口宽度改成1920".into(),
            thought: "用户要修改配置文件中的某个值。使用 edit_file 工具做精确的文本替换，将旧值替换为新值。".into(),
            action: "edit_file".into(),
            parameters: params,
            observation: "Replaced 1 occurrence(s) of 'window_width = 800' in 'src/config.toml'".into(),
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

                // Score based on action match (intent signal - highest priority)
                let action_lower = example.action.to_lowercase();
                if request_lower.contains("create") || request_lower.contains("创建") || request_lower.contains("生成") {
                    if action_lower.contains("create") { score += 5.0; }
                }
                if request_lower.contains("update") || request_lower.contains("修改") || request_lower.contains("改") || request_lower.contains("换") {
                    if action_lower.contains("update") { score += 5.0; }
                }
                if request_lower.contains("delete") || request_lower.contains("删除") || request_lower.contains("移除") {
                    if action_lower.contains("delete") { score += 5.0; }
                }
                if request_lower.contains("query") || request_lower.contains("list") || request_lower.contains("查询") || request_lower.contains("列表") {
                    if action_lower.contains("query") { score += 5.0; }
                }
                if request_lower.contains("generate") || request_lower.contains("写") || request_lower.contains("生成代码")
                    || request_lower.contains("组件") || request_lower.contains("系统") {
                    if action_lower.contains("generate") { score += 5.0; }
                }
                if request_lower.contains("prefab") || request_lower.contains("预制体") || request_lower.contains("模板") {
                    if action_lower.contains("prefab") { score += 5.0; }
                }
                if request_lower.contains("agent") || request_lower.contains("ai") || request_lower.contains("智能")
                    || request_lower.contains("挂上") || request_lower.contains("附加") {
                    if action_lower.contains("agent") || action_lower.contains("attach") { score += 5.0; }
                }
                if request_lower.contains("review") || request_lower.contains("审查") || request_lower.contains("检查")
                    || request_lower.contains("有没有问题") {
                    if action_lower.contains("review") { score += 5.0; }
                }
                if request_lower.contains("read") || request_lower.contains("读取") || request_lower.contains("看看")
                    || request_lower.contains("查看") || request_lower.contains("打开") {
                    if action_lower.contains("read_file") { score += 5.0; }
                }
                if request_lower.contains("write") || request_lower.contains("写入") || request_lower.contains("保存")
                    || request_lower.contains("创建文件") || request_lower.contains("新建文件") {
                    if action_lower.contains("write_file") { score += 5.0; }
                }
                if request_lower.contains("search") || request_lower.contains("搜索") || request_lower.contains("找")
                    || request_lower.contains("查找") || request_lower.contains("grep") {
                    if action_lower.contains("grep") { score += 5.0; }
                }
                if request_lower.contains("edit") || request_lower.contains("编辑") || request_lower.contains("改文件")
                    || request_lower.contains("替换") || request_lower.contains("修改配置") {
                    if action_lower.contains("edit_file") { score += 5.0; }
                }

                // Score based on entity type match
                if request_lower.contains("enemy") || request_lower.contains("敌人") {
                    if ex_request_lower.contains("enemy") || ex_request_lower.contains("敌人") { score += 2.0; }
                }
                if request_lower.contains("player") || request_lower.contains("玩家") {
                    if ex_request_lower.contains("player") || ex_request_lower.contains("玩家") { score += 2.0; }
                }
                if request_lower.contains("boss") || request_lower.contains("首领") {
                    if ex_request_lower.contains("boss") || ex_request_lower.contains("首领") { score += 2.0; }
                }

                // Score based on color match
                if request_lower.contains("red") || request_lower.contains("红色") {
                    if ex_request_lower.contains("red") || ex_request_lower.contains("红色") { score += 1.5; }
                }
                if request_lower.contains("blue") || request_lower.contains("蓝色") {
                    if ex_request_lower.contains("blue") || ex_request_lower.contains("蓝色") { score += 1.5; }
                }
                if request_lower.contains("yellow") || request_lower.contains("黄色") || request_lower.contains("金色") {
                    if ex_request_lower.contains("yellow") || ex_request_lower.contains("黄色") || ex_request_lower.contains("金色") { score += 1.5; }
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
