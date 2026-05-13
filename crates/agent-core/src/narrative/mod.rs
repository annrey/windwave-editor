//! Narrative Story Graph — Branching story flow with dialogue trees.
//!
//! StoryNode: plot points with conditions, choices, effects.
//! DialogueNode: speaker, text, emotion, portrait.
//! StoryGraph: the full branching narrative structure.
//!
//! Sprint 4 扩展: NarratorAgent, NPCDirectorAgent, WorldKeeperAgent

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Story Graph
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StoryGraph {
    pub name: String,
    pub nodes: HashMap<String, StoryNode>,
    pub start_node: Option<String>,
    pub metadata: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoryNode {
    pub id: String,
    pub node_type: StoryNodeType,
    pub title: String,
    pub content: NarrativeContent,
    pub choices: Vec<Choice>,
    pub conditions: Vec<NodeCondition>,
    pub effects: Vec<StoryEffect>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum StoryNodeType {
    Narrative,
    Dialogue,
    PlayerChoice,
    Combat,
    Exploration,
    Cutscene,
    Branch,
    Ending,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NarrativeContent {
    pub text: String,
    pub dialogue: Option<DialogueNode>,
    pub media: Option<MediaRef>,
    pub tone: Option<String>,
}

// ---------------------------------------------------------------------------
// Dialogue
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DialogueNode {
    pub speaker: String,
    pub text: String,
    pub emotion: Option<String>,
    pub voice_line: Option<String>,
    pub portrait: Option<String>,
    pub speaker_side: SpeakerSide,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SpeakerSide {
    Left,
    Right,
    Center,
}

impl Default for SpeakerSide {
    fn default() -> Self { Self::Left }
}

// ---------------------------------------------------------------------------
// Choice
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Choice {
    pub id: String,
    pub text: String,
    pub description: Option<String>,
    pub enabled: bool,
    pub conditions: Vec<NodeCondition>,
    pub effects: Vec<StoryEffect>,
    pub next_node: Option<String>,
}

impl Choice {
    pub fn simple(id: impl Into<String>, text: impl Into<String>, next: impl Into<String>) -> Self {
        Self {
            id: id.into(), text: text.into(), description: None,
            enabled: true, conditions: vec![], effects: vec![],
            next_node: Some(next.into()),
        }
    }
}

// ---------------------------------------------------------------------------
// Conditions & Effects
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeCondition {
    pub condition_type: String,
    pub key: String,
    pub operator: String,  // eq, neq, gt, lt, contains
    #[serde(rename = "value")]
    pub val: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoryEffect {
    pub effect_type: String,
    pub target: String,
    pub params: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaRef {
    pub media_type: MediaType,
    pub uri: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MediaType { Image, Audio, Video }

// ---------------------------------------------------------------------------
// Graph operations
// ---------------------------------------------------------------------------

impl StoryGraph {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into(), ..Default::default() }
    }

    pub fn add_node(&mut self, node: StoryNode) {
        if self.nodes.is_empty() {
            self.start_node = Some(node.id.clone());
        }
        self.nodes.insert(node.id.clone(), node);
    }

    pub fn get_node(&self, id: &str) -> Option<&StoryNode> {
        self.nodes.get(id)
    }

    pub fn get_choices(&self, node_id: &str) -> Vec<&Choice> {
        self.nodes.get(node_id)
            .map(|n| n.choices.iter().collect())
            .unwrap_or_default()
    }

    pub fn next_node(&self, current_id: &str, choice_index: usize) -> Option<&StoryNode> {
        let node = self.nodes.get(current_id)?;
        let choice = node.choices.get(choice_index)?;
        choice.next_node.as_ref().and_then(|id| self.nodes.get(id))
    }

    /// Linearize graph into a flat sequence (for export / preview)
    pub fn linearized(&self) -> Vec<&StoryNode> {
        let mut result = Vec::new();
        let mut current = self.start_node.as_ref();
        while let Some(id) = current {
            if let Some(node) = self.nodes.get(id) {
                result.push(node);
                current = node.choices.first().and_then(|c| c.next_node.as_ref());
            } else {
                break;
            }
        }
        result
    }

    pub fn node_count(&self) -> usize { self.nodes.len() }
}

// ============================================================================
// Sprint 4: Narrative Agents
// ============================================================================

/// 叙事上下文
#[derive(Debug, Clone)]
pub struct NarrativeContext {
    pub story_graph: Option<StoryGraph>,
    pub world_state: String,
    pub player_input: String,
    pub history: Vec<String>,
    pub round: u64,
}

/// 叙事输出
#[derive(Debug, Clone)]
pub struct NarrativeOutput {
    pub text: String,
    pub choices: Vec<String>,
    pub state_changes: Vec<String>,
    pub agent_responses: HashMap<String, String>,
}

/// NarratorAgent - AI 说书人
pub struct NarratorAgent {
    llm: std::sync::Arc<dyn crate::llm::LlmClient>,
    name: String,
    style: String,
}

impl NarratorAgent {
    pub fn new(llm: std::sync::Arc<dyn crate::llm::LlmClient>, name: String, style: String) -> Self {
        Self { llm, name, style }
    }

    /// 生成叙事文本
    pub async fn narrate(&self, context: &NarrativeContext) -> Result<String, String> {
        let prompt = format!(
            "当前轮次: {}\n\n世界状态:\n{}\n\n玩家输入:\n{}\n\n对话历史:\n{}\n\n请生成下一段叙事文本，在结尾处提供 2-3 个选项。",
            context.round, context.world_state, context.player_input, context.history.join("\n")
        );

        let request = crate::llm::LlmRequest {
            model: "gpt-4o-mini".to_string(),
            messages: vec![
                crate::llm::LlmMessage {
                    role: crate::llm::Role::System,
                    content: format!(
                        "你是{}，一位AI说书人。\n叙事风格: {}\n\n你的职责：\n1. 接收玩家输入并生成连贯叙事\n2. 在关键处提供选择\n3. 控制叙事节奏\n4. 使用生动的语言",
                        self.name, self.style
                    ),
                },
                crate::llm::LlmMessage {
                    role: crate::llm::Role::User,
                    content: prompt,
                },
            ],
            tools: None,
            max_tokens: Some(1024),
            temperature: Some(0.8),
        };

        let response = self.llm.chat(request).await
            .map_err(|e| format!("LLM error: {}", e))?;
        Ok(response.content)
    }
}

/// NPC 档案
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NpcProfile {
    pub name: String,
    pub personality: String,
    pub background: String,
    pub goals: Vec<String>,
    pub relationship: HashMap<String, f32>,
}

/// NPCDirectorAgent - NPC 行为编排
pub struct NPCDirectorAgent {
    llm: std::sync::Arc<dyn crate::llm::LlmClient>,
    npc_profiles: HashMap<String, NpcProfile>,
}

impl NPCDirectorAgent {
    pub fn new(llm: std::sync::Arc<dyn crate::llm::LlmClient>) -> Self {
        Self { llm, npc_profiles: HashMap::new() }
    }

    pub fn register_npc(&mut self, profile: NpcProfile) {
        self.npc_profiles.insert(profile.name.clone(), profile);
    }

    pub async fn respond(&self, npc_name: &str, player_input: &str) -> Result<String, String> {
        let profile = self.npc_profiles.get(npc_name)
            .ok_or_else(|| format!("NPC '{}' not found", npc_name))?;

        let request = crate::llm::LlmRequest {
            model: "gpt-4o-mini".to_string(),
            messages: vec![
                crate::llm::LlmMessage {
                    role: crate::llm::Role::System,
                    content: format!(
                        "你是NPC '{}'。\n性格: {}\n背景: {}\n目标: {}\n\n以第一人称回复，保持性格一致。",
                        profile.name, profile.personality, profile.background, profile.goals.join(", ")
                    ),
                },
                crate::llm::LlmMessage {
                    role: crate::llm::Role::User,
                    content: format!("玩家对你说: {}", player_input),
                },
            ],
            tools: None,
            max_tokens: Some(512),
            temperature: Some(0.8),
        };

        let response = self.llm.chat(request).await
            .map_err(|e| format!("LLM error: {}", e))?;
        Ok(response.content)
    }
}

/// WorldKeeperAgent - 世界状态一致性维护
pub struct WorldKeeperAgent {
    world_rules: Vec<String>,
    known_facts: Vec<String>,
}

impl WorldKeeperAgent {
    pub fn new() -> Self {
        Self { world_rules: Vec::new(), known_facts: Vec::new() }
    }

    pub fn add_rule(&mut self, rule: String) {
        self.world_rules.push(rule);
    }

    pub fn add_fact(&mut self, fact: String) {
        self.known_facts.push(fact);
    }

    /// 检查一致性
    pub fn check_consistency(&self, narrative: &str) -> Vec<String> {
        let mut violations = Vec::new();
        for rule in &self.world_rules {
            let rule_lower = rule.to_lowercase();
            let narrative_lower = narrative.to_lowercase();
            if rule_lower.starts_with("no ") || rule_lower.starts_with("禁止") || rule_lower.starts_with("不能") {
                let forbidden = rule_lower.trim_start_matches("no ")
                    .trim_start_matches("禁止")
                    .trim_start_matches("不能");
                if narrative_lower.contains(forbidden) {
                    violations.push(format!("规则冲突: {} (叙事包含禁止内容: {})", rule, forbidden));
                }
            }
        }
        violations
    }

    pub fn describe_world(&self) -> String {
        let rules_str = if self.world_rules.is_empty() { "(无)".to_string() } else { self.world_rules.join(", ") };
        let facts_str = if self.known_facts.is_empty() { "(无)".to_string() } else { self.known_facts.join(", ") };
        format!("## 世界状态\n规则: {}\n已知事实: {}", rules_str, facts_str)
    }
}

#[cfg(test)]
mod narrative_agent_tests {
    use super::*;

    #[test]
    fn test_npc_profile() {
        let profile = NpcProfile {
            name: "铁匠".into(),
            personality: "豪爽直率".into(),
            background: "村里最好的铁匠".into(),
            goals: vec!["打造神兵利器".into()],
            relationship: HashMap::new(),
        };
        assert_eq!(profile.name, "铁匠");
    }

    #[test]
    fn test_world_keeper_consistency() {
        let mut keeper = WorldKeeperAgent::new();
        keeper.add_rule("禁止使用魔法".into());
        keeper.add_fact("世界是低魔设定".into());
        let violations = keeper.check_consistency("玩家使用魔法施展了一个火球术");
        assert!(!violations.is_empty());
    }

    #[test]
    fn test_world_keeper_no_violation() {
        let keeper = WorldKeeperAgent::new();
        let violations = keeper.check_consistency("玩家走进了村庄");
        assert!(violations.is_empty());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_graph() -> StoryGraph {
        let mut graph = StoryGraph::new("Test");
        graph.add_node(StoryNode {
            id: "start".into(), node_type: StoryNodeType::Dialogue, title: "Start".into(),
            content: NarrativeContent {
                text: "Hello, adventurer!".into(),
                dialogue: Some(DialogueNode {
                    speaker: "Guard".into(), text: "Welcome!".into(),
                    emotion: Some("neutral".into()), voice_line: None,
                    portrait: None, speaker_side: SpeakerSide::Left,
                }),
                media: None, tone: None,
            },
            choices: vec![Choice::simple("c1", "Enter the dungeon", "dungeon")],
            conditions: vec![], effects: vec![],
        });
        graph.add_node(StoryNode {
            id: "dungeon".into(), node_type: StoryNodeType::Narrative, title: "Dungeon".into(),
            content: NarrativeContent { text: "It's dark...".into(), dialogue: None, media: None, tone: Some("tense".into()) },
            choices: vec![], conditions: vec![], effects: vec![StoryEffect {
                effect_type: "set_flag".into(), target: "entered_dungeon".into(),
                params: std::collections::HashMap::new(),
            }],
        });
        graph
    }

    #[test]
    fn test_story_graph_traversal() {
        let graph = make_test_graph();
        assert_eq!(graph.node_count(), 2);
        let start = graph.get_node("start").unwrap();
        assert_eq!(start.node_type, StoryNodeType::Dialogue);
        let next = graph.next_node("start", 0).unwrap();
        assert_eq!(next.id, "dungeon");
    }

    #[test]
    fn test_linearization() {
        let graph = make_test_graph();
        let linear = graph.linearized();
        assert_eq!(linear.len(), 2);
        assert_eq!(linear[0].id, "start");
        assert_eq!(linear[1].id, "dungeon");
    }
}
