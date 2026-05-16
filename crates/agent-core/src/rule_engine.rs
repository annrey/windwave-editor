//! Rule Engine Editor - Sprint 7: 规则引擎编辑器
//!
//! 触发-条件-动作模式的规则引擎

use serde::{Deserialize, Serialize};

/// 规则
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rule {
    pub id: String,
    pub name: String,
    pub description: String,
    pub enabled: bool,
    pub trigger: Trigger,
    pub conditions: Vec<Condition>,
    pub actions: Vec<Action>,
    pub priority: u32,
}

/// 触发器
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Trigger {
    /// 实体创建
    EntityCreated { entity_type: Option<String> },
    /// 实体销毁
    EntityDestroyed { entity_type: Option<String> },
    /// 组件修改
    ComponentChanged { component: String, property: Option<String> },
    /// 定时触发
    Timer { interval_secs: f32 },
    /// 自定义事件
    CustomEvent { event_name: String },
    /// 玩家输入
    PlayerInput { input_name: String },
}

/// 条件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Condition {
    /// 实体存在
    EntityExists { name: String },
    /// 属性等于
    PropertyEquals { entity: String, component: String, property: String, value: serde_json::Value },
    /// 属性大于
    PropertyGreaterThan { entity: String, component: String, property: String, value: serde_json::Value },
    /// 标签包含
    HasTag { entity: String, tag: String },
    /// 自定义脚本
    CustomScript { script: String },
}

/// 动作
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Action {
    /// 创建实体
    CreateEntity { template: String, position: Option<[f32; 3]> },
    /// 销毁实体
    DestroyEntity { entity: String },
    /// 设置属性
    SetProperty { entity: String, component: String, property: String, value: serde_json::Value },
    /// 发送事件
    SendEvent { event_name: String, data: serde_json::Value },
    /// 运行脚本
    RunScript { script: String },
    /// 切换场景
    SwitchScene { scene_name: String },
}

/// 规则引擎
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleEngine {
    rules: Vec<Rule>,
}

impl RuleEngine {
    pub fn new() -> Self {
        Self { rules: Vec::new() }
    }

    pub fn add_rule(&mut self, rule: Rule) {
        self.rules.push(rule);
    }

    pub fn remove_rule(&mut self, id: &str) {
        self.rules.retain(|r| r.id != id);
    }

    pub fn get_rule(&self, id: &str) -> Option<&Rule> {
        self.rules.iter().find(|r| r.id == id)
    }

    pub fn get_rule_mut(&mut self, id: &str) -> Option<&mut Rule> {
        self.rules.iter_mut().find(|r| r.id == id)
    }

    pub fn all_rules(&self) -> &[Rule] {
        &self.rules
    }

    pub fn enabled_rules(&self) -> Vec<&Rule> {
        self.rules.iter().filter(|r| r.enabled).collect()
    }

    /// 查找匹配指定触发器的规则
    pub fn find_matching(&self, trigger: &Trigger) -> Vec<&Rule> {
        self.rules.iter().filter(|r| r.enabled && r.trigger_matches(trigger)).collect()
    }
}

impl Default for RuleEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl Rule {
    pub fn trigger_matches(&self, trigger: &Trigger) -> bool {
        std::mem::discriminant(&self.trigger) == std::mem::discriminant(trigger)
    }
}

// ============================================================================
// AI Provider 配置
// ============================================================================

/// AI Provider 类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AiProviderType {
    OpenAI,
    Claude,
    Gemini,
    Local,      // Ollama/LM Studio
    Custom(String),
}

/// Provider 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiProviderConfig {
    pub provider_type: AiProviderType,
    pub name: String,
    pub api_key: Option<String>,
    pub base_url: Option<String>,
    pub models: Vec<AiModelConfig>,
    pub is_default: bool,
}

/// 模型配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiModelConfig {
    pub model_id: String,
    pub display_name: String,
    pub capabilities: Vec<ModelCapability>,
    pub max_tokens: u32,
    pub temperature_range: [f32; 2],  // [min, max]
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ModelCapability {
    TextGeneration,
    Vision,
    CodeGeneration,
    FunctionCalling,
    Embedding,
}

/// AI Provider 管理器
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiProviderManager {
    pub providers: Vec<AiProviderConfig>,
}

impl AiProviderManager {
    pub fn new() -> Self {
        Self {
            providers: Self::default_providers(),
        }
    }

    fn default_providers() -> Vec<AiProviderConfig> {
        vec![
            AiProviderConfig {
                provider_type: AiProviderType::OpenAI,
                name: "OpenAI".into(),
                api_key: None,
                base_url: None,
                models: vec![
                    AiModelConfig {
                        model_id: "gpt-4o".into(),
                        display_name: "GPT-4o".into(),
                        capabilities: vec![ModelCapability::TextGeneration, ModelCapability::Vision, ModelCapability::FunctionCalling],
                        max_tokens: 16384,
                        temperature_range: [0.0, 2.0],
                    },
                    AiModelConfig {
                        model_id: "gpt-4o-mini".into(),
                        display_name: "GPT-4o Mini".into(),
                        capabilities: vec![ModelCapability::TextGeneration, ModelCapability::FunctionCalling],
                        max_tokens: 16384,
                        temperature_range: [0.0, 2.0],
                    },
                ],
                is_default: true,
            },
            AiProviderConfig {
                provider_type: AiProviderType::Claude,
                name: "Claude".into(),
                api_key: None,
                base_url: None,
                models: vec![
                    AiModelConfig {
                        model_id: "claude-3-opus".into(),
                        display_name: "Claude 3 Opus".into(),
                        capabilities: vec![ModelCapability::TextGeneration, ModelCapability::Vision, ModelCapability::CodeGeneration],
                        max_tokens: 4096,
                        temperature_range: [0.0, 1.0],
                    },
                ],
                is_default: false,
            },
        ]
    }

    pub fn add_provider(&mut self, config: AiProviderConfig) {
        self.providers.push(config);
    }

    pub fn get_provider(&self, name: &str) -> Option<&AiProviderConfig> {
        self.providers.iter().find(|p| p.name == name)
    }

    pub fn get_default(&self) -> Option<&AiProviderConfig> {
        self.providers.iter().find(|p| p.is_default)
    }

    pub fn all_providers(&self) -> &[AiProviderConfig] {
        &self.providers
    }
}

impl Default for AiProviderManager {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// 测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rule_engine_add() {
        let mut engine = RuleEngine::new();
        engine.add_rule(Rule {
            id: "r1".into(),
            name: "Player Spawn".into(),
            description: "When player spawns, set up camera".into(),
            enabled: true,
            trigger: Trigger::EntityCreated { entity_type: Some("Player".into()) },
            conditions: vec![Condition::EntityExists { name: "Camera".into() }],
            actions: vec![Action::SetProperty {
                entity: "Camera".into(),
                component: "Transform".into(),
                property: "position".into(),
                value: serde_json::json!([0.0, 0.0, 10.0]),
            }],
            priority: 1,
        });
        assert_eq!(engine.all_rules().len(), 1);
    }

    #[test]
    fn test_rule_trigger_matching() {
        let trigger = Trigger::EntityCreated { entity_type: Some("Enemy".into()) };
        let rule = Rule {
            id: "r2".into(), name: "test".into(), description: "".into(),
            enabled: true, trigger: trigger.clone(), conditions: vec![], actions: vec![], priority: 0,
        };
        assert!(rule.trigger_matches(&trigger));
        assert!(!rule.trigger_matches(&Trigger::Timer { interval_secs: 1.0 }));
    }

    #[test]
    fn test_ai_provider_defaults() {
        let mgr = AiProviderManager::new();
        assert_eq!(mgr.all_providers().len(), 2);
        assert!(mgr.get_default().is_some());
    }

    #[test]
    fn test_ai_provider_add() {
        let mut mgr = AiProviderManager::new();
        mgr.add_provider(AiProviderConfig {
            provider_type: AiProviderType::Local,
            name: "Local".into(),
            api_key: None,
            base_url: Some("http://localhost:11434".into()),
            models: vec![],
            is_default: false,
        });
        assert_eq!(mgr.all_providers().len(), 3);
    }

    #[test]
    fn test_find_matching_rules() {
        let mut engine = RuleEngine::new();
        engine.add_rule(Rule {
            id: "r1".into(), name: "Spawn".into(), description: "".into(),
            enabled: true, trigger: Trigger::EntityCreated { entity_type: None },
            conditions: vec![], actions: vec![], priority: 0,
        });
        let matches = engine.find_matching(&Trigger::EntityCreated { entity_type: None });
        assert_eq!(matches.len(), 1);
    }

    #[test]
    fn test_enabled_rules_only() {
        let mut engine = RuleEngine::new();
        engine.add_rule(Rule {
            id: "r1".into(), name: "Disabled".into(), description: "".into(),
            enabled: false, trigger: Trigger::Timer { interval_secs: 1.0 },
            conditions: vec![], actions: vec![], priority: 0,
        });
        engine.add_rule(Rule {
            id: "r2".into(), name: "Enabled".into(), description: "".into(),
            enabled: true, trigger: Trigger::Timer { interval_secs: 1.0 },
            conditions: vec![], actions: vec![], priority: 0,
        });
        assert_eq!(engine.enabled_rules().len(), 1);
    }
}