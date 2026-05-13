//! Game Mode Types - 游戏模式核心类型定义

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 游戏模式类型
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GameModeType {
    /// 文字冒险 / 互动小说
    TextAdventure,
    /// AI对战 / 策略游戏
    AIBattle,
    /// NPC沙盒 / 开放世界
    NPCSandbox,
    /// 角色扮演 / 深度对话
    ChatRoleplay,
    /// 自定义模式
    Custom(String),
}

impl GameModeType {
    /// 获取模式显示名称
    pub fn display_name(&self) -> String {
        match self {
            GameModeType::TextAdventure => "文字冒险".to_string(),
            GameModeType::AIBattle => "AI对战".to_string(),
            GameModeType::NPCSandbox => "NPC沙盒".to_string(),
            GameModeType::ChatRoleplay => "角色扮演".to_string(),
            GameModeType::Custom(name) => format!("自定义: {}", name),
        }
    }

    /// 获取模式描述
    pub fn description(&self) -> String {
        match self {
            GameModeType::TextAdventure => {
                "互动小说式体验，AI根据玩家选择动态生成剧情".to_string()
            }
            GameModeType::AIBattle => {
                "策略对战游戏，AI担任对手或裁判".to_string()
            }
            GameModeType::NPCSandbox => {
                "自由探索的开放世界，与AI NPC自由互动".to_string()
            }
            GameModeType::ChatRoleplay => {
                "深度角色扮演，沉浸式剧情体验".to_string()
            }
            GameModeType::Custom(_) => "用户自定义的游戏模式".to_string(),
        }
    }

    /// 获取默认启用的Agent角色列表
    pub fn default_agents(&self) -> Vec<NarrativeAgentRole> {
        match self {
            GameModeType::TextAdventure => vec![
                NarrativeAgentRole::Narrator,
                NarrativeAgentRole::WorldKeeper,
                NarrativeAgentRole::DramaCurator,
            ],
            GameModeType::AIBattle => vec![
                NarrativeAgentRole::Narrator,
                NarrativeAgentRole::RuleArbiter,
            ],
            GameModeType::NPCSandbox => vec![
                NarrativeAgentRole::Narrator,
                NarrativeAgentRole::NPCDirector,
                NarrativeAgentRole::WorldKeeper,
            ],
            GameModeType::ChatRoleplay => vec![
                NarrativeAgentRole::NPCDirector,
                NarrativeAgentRole::WorldKeeper,
            ],
            GameModeType::Custom(_) => vec![
                NarrativeAgentRole::Narrator,
                NarrativeAgentRole::WorldKeeper,
                NarrativeAgentRole::NPCDirector,
                NarrativeAgentRole::RuleArbiter,
                NarrativeAgentRole::DramaCurator,
            ],
        }
    }
}

impl std::fmt::Display for GameModeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

/// 叙事Agent角色
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NarrativeAgentRole {
    /// 主控叙事：协调叙事节奏和玩家互动
    Narrator,
    /// 世界观守护：维护世界观一致性
    WorldKeeper,
    /// NPC导演：管理NPC行为和对话
    NPCDirector,
    /// 规则仲裁：执行游戏规则和判定
    RuleArbiter,
    /// 剧情策划：设计剧情走向和冲突
    DramaCurator,
}

impl NarrativeAgentRole {
    /// 获取角色显示名称
    pub fn display_name(&self) -> String {
        match self {
            NarrativeAgentRole::Narrator => "主控叙事".to_string(),
            NarrativeAgentRole::WorldKeeper => "世界观守护".to_string(),
            NarrativeAgentRole::NPCDirector => "NPC导演".to_string(),
            NarrativeAgentRole::RuleArbiter => "规则仲裁".to_string(),
            NarrativeAgentRole::DramaCurator => "剧情策划".to_string(),
        }
    }

    /// 获取角色职责描述
    pub fn description(&self) -> String {
        match self {
            NarrativeAgentRole::Narrator => {
                "接收玩家输入，协调其他Agent，整合响应，输出最终叙事".to_string()
            }
            NarrativeAgentRole::WorldKeeper => {
                "维护设定一致性，提供世界背景细节，拒绝违背设定的内容".to_string()
            }
            NarrativeAgentRole::NPCDirector => {
                "管理NPC行为、对话和关系网，根据性格模型生成真实反应".to_string()
            }
            NarrativeAgentRole::RuleArbiter => {
                "处理所有规则判定，包括骰子机制、战斗结算、技能检定".to_string()
            }
            NarrativeAgentRole::DramaCurator => {
                "管理伏笔、高潮和情感曲线，确保叙事节奏".to_string()
            }
        }
    }

    /// 获取默认系统提示词
    pub fn default_system_prompt(&self) -> String {
        match self {
            NarrativeAgentRole::Narrator => {
                r#"你是主控叙事者（Narrator），负责协调整个叙事流程。
你的职责：
1. 接收玩家输入并理解其意图
2. 协调其他Agent获取必要信息
3. 整合所有Agent的响应
4. 输出连贯、引人入胜的叙事文本
5. 控制叙事节奏，适时推进剧情

输出要求：
- 使用生动、富有画面感的语言
- 保持第二人称视角（"你"）
- 在关键处提供玩家选择
- 控制每次输出长度在200-500字"#
                    .to_string()
            }
            NarrativeAgentRole::WorldKeeper => {
                r#"你是世界观守护者（World Keeper），负责维护游戏世界的一致性。
你的职责：
1. 审核叙事内容是否符合世界设定
2. 提供世界背景细节补充
3. 阻止违背设定的内容出现
4. 维护魔法系统、科技水平等核心设定

判断标准：
- 是否符合已建立的世界规则
- 是否与前文描述矛盾
- 是否合理运用世界元素"#
                    .to_string()
            }
            NarrativeAgentRole::NPCDirector => {
                r#"你是NPC导演（NPC Director），负责管理非玩家角色。
你的职责：
1. 根据角色性格生成真实反应
2. 维护角色间的关系网络
3. 设计角色的行为模式
4. 生成符合角色背景的对白

输出要求：
- 每个NPC应有独特的说话风格
- 反应应符合角色性格和当前关系
- 保持角色行为的一致性"#
                    .to_string()
            }
            NarrativeAgentRole::RuleArbiter => {
                r#"你是规则仲裁者（Rule Arbiter），负责执行游戏规则。
你的职责：
1. 处理战斗结算
2. 执行技能检定
3. 判定动作成功率
4. 计算伤害和效果

判定流程：
1. 确定适用的规则
2. 进行随机判定（如需要）
3. 计算结果
4. 解释判定结果"#
                    .to_string()
            }
            NarrativeAgentRole::DramaCurator => {
                r#"你是剧情策划（Drama Curator），负责设计剧情走向。
你的职责：
1. 管理伏笔和线索
2. 设计高潮和转折点
3. 控制情感曲线
4. 确保叙事结构的完整性

设计原则：
- 张弛有度，避免节奏单调
- 伏笔要有回收
- 冲突要有升级
- 结局要令人满足"#
                    .to_string()
            }
        }
    }
}

impl std::fmt::Display for NarrativeAgentRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

/// 叙事风格
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NarrativeStyle {
    /// 史诗奇幻
    EpicFantasy,
    /// 黑暗奇幻
    DarkFantasy,
    /// 科幻
    ScienceFiction,
    /// 现代都市
    ModernUrban,
    /// 悬疑推理
    Mystery,
    /// 浪漫
    Romance,
    /// 恐怖
    Horror,
    /// 轻松幽默
    Lighthearted,
    /// 自定义
    Custom(String),
}

impl NarrativeStyle {
    pub fn display_name(&self) -> String {
        match self {
            NarrativeStyle::EpicFantasy => "史诗奇幻".to_string(),
            NarrativeStyle::DarkFantasy => "黑暗奇幻".to_string(),
            NarrativeStyle::ScienceFiction => "科幻".to_string(),
            NarrativeStyle::ModernUrban => "现代都市".to_string(),
            NarrativeStyle::Mystery => "悬疑推理".to_string(),
            NarrativeStyle::Romance => "浪漫".to_string(),
            NarrativeStyle::Horror => "恐怖".to_string(),
            NarrativeStyle::Lighthearted => "轻松幽默".to_string(),
            NarrativeStyle::Custom(name) => name.clone(),
        }
    }

    /// 获取风格描述提示词
    pub fn style_prompt(&self) -> String {
        match self {
            NarrativeStyle::EpicFantasy => {
                "宏大的世界观，英雄之旅，善恶对抗，魔法与剑".to_string()
            }
            NarrativeStyle::DarkFantasy => {
                "残酷的世界，灰色道德，生存挣扎，腐败与堕落".to_string()
            }
            NarrativeStyle::ScienceFiction => {
                "未来科技，太空探索，人工智能，社会变革".to_string()
            }
            NarrativeStyle::ModernUrban => {
                "当代城市，日常生活，人际关系，现实问题".to_string()
            }
            NarrativeStyle::Mystery => {
                "谜团重重，线索收集，逻辑推理，真相揭示".to_string()
            }
            NarrativeStyle::Romance => {
                "情感发展，关系建立，内心挣扎，幸福追求".to_string()
            }
            NarrativeStyle::Horror => {
                "恐惧氛围，未知威胁，心理压迫，生存恐惧".to_string()
            }
            NarrativeStyle::Lighthearted => {
                "轻松愉快，幽默风趣，温馨治愈，日常趣事".to_string()
            }
            NarrativeStyle::Custom(desc) => desc.clone(),
        }
    }
}

/// Agent配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    pub role: NarrativeAgentRole,
    pub enabled: bool,
    pub provider: String,           // AI Provider名称
    pub model: String,              // 模型名称
    pub temperature: f32,
    pub max_tokens: u32,
    pub system_prompt: String,
    pub priority: u8,               // 调用优先级（数字越小越优先）
}

impl AgentConfig {
    pub fn new(role: NarrativeAgentRole) -> Self {
        Self {
            role: role.clone(),
            enabled: true,
            provider: "openai".to_string(),
            model: "gpt-4o-mini".to_string(),
            temperature: 0.7,
            max_tokens: 1000,
            system_prompt: role.default_system_prompt(),
            priority: match role {
                NarrativeAgentRole::Narrator => 1,
                NarrativeAgentRole::WorldKeeper => 2,
                NarrativeAgentRole::NPCDirector => 3,
                NarrativeAgentRole::RuleArbiter => 4,
                NarrativeAgentRole::DramaCurator => 5,
            },
        }
    }

    pub fn with_provider(mut self, provider: impl Into<String>) -> Self {
        self.provider = provider.into();
        self
    }

    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = model.into();
        self
    }

    pub fn with_temperature(mut self, temperature: f32) -> Self {
        self.temperature = temperature;
        self
    }
}

/// 游戏模式定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameModeDefinition {
    pub id: String,
    pub mode_type: GameModeType,
    pub name: String,
    pub description: String,
    pub enabled_agents: Vec<AgentConfig>,
    pub narrative_style: NarrativeStyle,
    pub initial_state_template: String,     // WorldStateTemplate ID
    pub ruleset_id: Option<String>,
    pub custom_settings: HashMap<String, serde_json::Value>,
    pub created_at: i64,
    pub updated_at: i64,
}

impl GameModeDefinition {
    pub fn new(mode_type: GameModeType, name: impl Into<String>) -> Self {
        let now = chrono::Utc::now().timestamp_millis();
        let id = format!("mode_{}_{}", 
            mode_type.display_name(), 
            now
        );
        
        let enabled_agents = mode_type
            .default_agents()
            .into_iter()
            .map(AgentConfig::new)
            .collect();

        Self {
            id,
            mode_type: mode_type.clone(),
            name: name.into(),
            description: mode_type.description(),
            enabled_agents,
            narrative_style: NarrativeStyle::EpicFantasy,
            initial_state_template: "default".to_string(),
            ruleset_id: None,
            custom_settings: HashMap::new(),
            created_at: now,
            updated_at: now,
        }
    }

    pub fn with_narrative_style(mut self, style: NarrativeStyle) -> Self {
        self.narrative_style = style;
        self
    }

    pub fn with_agents(mut self, agents: Vec<AgentConfig>) -> Self {
        self.enabled_agents = agents;
        self
    }

    pub fn get_enabled_agents(&self) -> Vec<&AgentConfig> {
        self.enabled_agents.iter().filter(|a| a.enabled).collect()
    }

    pub fn get_agent_config(&self, role: &NarrativeAgentRole) -> Option<&AgentConfig> {
        self.enabled_agents.iter().find(|a| a.role == *role)
    }

    pub fn update_agent_config(&mut self, config: AgentConfig) {
        if let Some(idx) = self.enabled_agents.iter().position(|a| a.role == config.role) {
            self.enabled_agents[idx] = config;
            self.updated_at = chrono::Utc::now().timestamp_millis();
        }
    }
}

/// 协作模式
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CollaborationMode {
    /// 顺序调用：按优先级依次调用Agent
    Sequential,
    /// 并行调用：同时调用多个Agent
    Parallel,
    /// 层级协调：主控Agent协调其他Agent
    Hierarchical,
    /// 共识决策：多个Agent达成共识
    Consensus,
}

impl CollaborationMode {
    pub fn display_name(&self) -> String {
        match self {
            CollaborationMode::Sequential => "顺序调用".to_string(),
            CollaborationMode::Parallel => "并行调用".to_string(),
            CollaborationMode::Hierarchical => "层级协调".to_string(),
            CollaborationMode::Consensus => "共识决策".to_string(),
        }
    }
}

/// Agent协作配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentOrchestration {
    pub primary_agent: NarrativeAgentRole,
    pub collaboration_mode: CollaborationMode,
    pub fallback_strategy: FallbackStrategy,
    pub timeout_seconds: u64,
}

impl Default for AgentOrchestration {
    fn default() -> Self {
        Self {
            primary_agent: NarrativeAgentRole::Narrator,
            collaboration_mode: CollaborationMode::Hierarchical,
            fallback_strategy: FallbackStrategy::UsePrimaryOnly,
            timeout_seconds: 30,
        }
    }
}

/// 降级策略
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum FallbackStrategy {
    /// 仅使用主控Agent
    UsePrimaryOnly,
    /// 降级到规则引擎
    FallbackToRules,
    /// 请求用户介入
    AskUser,
    /// 使用备用模型
    UseBackupModel,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_game_mode_type_display() {
        assert_eq!(GameModeType::TextAdventure.display_name(), "文字冒险");
        assert_eq!(GameModeType::AIBattle.display_name(), "AI对战");
    }

    #[test]
    fn test_narrative_agent_role_display() {
        assert_eq!(NarrativeAgentRole::Narrator.display_name(), "主控叙事");
        assert_eq!(NarrativeAgentRole::WorldKeeper.display_name(), "世界观守护");
    }

    #[test]
    fn test_default_agents() {
        let agents = GameModeType::TextAdventure.default_agents();
        assert!(agents.contains(&NarrativeAgentRole::Narrator));
        assert!(agents.contains(&NarrativeAgentRole::WorldKeeper));
    }

    #[test]
    fn test_game_mode_definition_creation() {
        let mode = GameModeDefinition::new(GameModeType::TextAdventure, "测试模式");
        assert_eq!(mode.name, "测试模式");
        assert!(!mode.enabled_agents.is_empty());
    }

    #[test]
    fn test_agent_config_builder() {
        let config = AgentConfig::new(NarrativeAgentRole::Narrator)
            .with_provider("anthropic")
            .with_model("claude-3-sonnet")
            .with_temperature(0.5);
        
        assert_eq!(config.provider, "anthropic");
        assert_eq!(config.model, "claude-3-sonnet");
        assert_eq!(config.temperature, 0.5);
    }

    #[test]
    fn test_narrative_style_prompt() {
        assert!(NarrativeStyle::EpicFantasy.style_prompt().contains("魔法"));
        assert!(NarrativeStyle::ScienceFiction.style_prompt().contains("科技"));
    }
}
