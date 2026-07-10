//! Few-shot examples for demonstrating tool usage patterns.

use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct FewShotExample {
    pub user_request: String,
    pub thought: String,
    pub action: String,
    pub parameters: HashMap<String, String>,
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
            self.user_request, self.thought, self.action, self.parameters, self.observation,
        )
    }

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
        params.insert(
            "step1_params".into(),
            r#"{"name":"Boss","sprite_color":"[1.0,1.0,0.0,1.0]","position":"[300.0,200.0]"}"#
                .into(),
        );
        params.insert("step2_tool".into(), "attach_runtime_agent".into());
        params.insert(
            "step2_params".into(),
            r#"{"entity_id":"<step1_result>","profile_id":"boss_ai"}"#.into(),
        );
        params.insert("step3_tool".into(), "create_prefab".into());
        params.insert(
            "step3_params".into(),
            r#"{"entity_id":"<step1_result>","name":"BossTemplate"}"#.into(),
        );

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
        params.insert(
            "code".into(),
            "fn update(&mut self) { self.x += 1; }".into(),
        );

        Self {
            user_request: "检查这段代码有没有问题".into(),
            thought: "用户请求代码审查。使用 review_code 工具对代码片段进行静态分析，检查潜在问题如边界条件、性能、命名规范等。".into(),
            action: "review_code".into(),
            parameters: params,
            observation: "Review: No critical issues found. Suggestion: consider adding bounds check if x has a maximum value.".into(),
        }
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

    /// Select the most relevant few-shot examples based on the user request.
    ///
    /// Analyzes the request for scene/code/asset/file keywords and returns
    /// up to `limit` relevant examples, prioritizing exact matches.
    pub fn select(request: &str, limit: usize) -> Vec<Self> {
        let lower = request.to_lowercase();
        let mut candidates: Vec<(f32, Self)> = Vec::new();

        for example in Self::default_examples() {
            let mut score: f32 = 0.0;

            score += Self::keyword_match_score(&lower, &example.action);
            score += Self::keyword_match_score(&lower, &example.user_request);
            score += Self::keyword_match_score(&lower, &example.observation);

            if score > 0.0 {
                candidates.push((score, example));
            }
        }

        candidates.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
        candidates.truncate(limit);
        candidates.into_iter().map(|(_, e)| e).collect()
    }

    fn keyword_match_score(request: &str, target: &str) -> f32 {
        let target_lower = target.to_lowercase();
        let keywords: Vec<&str> = target_lower.split_whitespace().collect();
        let mut score = 0.0;

        for kw in &keywords {
            if kw.len() < 3 {
                continue;
            }
            if request.contains(kw) {
                score += 1.0;
            }
        }

        if score > 0.0 {
            score / keywords.len() as f32
        } else {
            0.0
        }
    }
}
