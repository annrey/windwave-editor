//! Game Mode — Framework for different game types.
//!
//! Supports: TextAdventure, AIBattle, NPCSandbox, ChatRoleplay, Custom.
//! Each mode defines which agents are active, initial state, and ruleset.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GameModeType {
    TextAdventure,
    AIBattle,
    NPCSandbox,
    ChatRoleplay,
    Custom(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NarrativeAgentRole {
    Narrator,
    WorldKeeper,
    NPCDirector,
    RuleArbiter,
    DramaCurator,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CollaborationMode {
    Sequential,
    Parallel,
    Hierarchical,
    Consensus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameModeDefinition {
    pub mode_type: GameModeType,
    pub name: String,
    pub description: String,
    pub enabled_agents: Vec<NarrativeAgentRole>,
    pub collaboration: CollaborationMode,
    pub narrative_style: String,
    pub metadata: HashMap<String, serde_json::Value>,
}

impl GameModeType {
    pub fn all_presets() -> Vec<GameModeDefinition> {
        vec![
            GameModeDefinition {
                mode_type: GameModeType::TextAdventure,
                name: "Text Adventure".into(),
                description: "Classic text-based adventure with branching narrative".into(),
                enabled_agents: vec![NarrativeAgentRole::Narrator, NarrativeAgentRole::WorldKeeper],
                collaboration: CollaborationMode::Sequential,
                narrative_style: "immersive".into(),
                metadata: HashMap::new(),
            },
            GameModeDefinition {
                mode_type: GameModeType::AIBattle,
                name: "AI Battle".into(),
                description: "Multi-agent competitive storytelling with rule arbitration".into(),
                enabled_agents: vec![NarrativeAgentRole::Narrator, NarrativeAgentRole::RuleArbiter, NarrativeAgentRole::DramaCurator],
                collaboration: CollaborationMode::Consensus,
                narrative_style: "competitive".into(),
                metadata: HashMap::new(),
            },
            GameModeDefinition {
                mode_type: GameModeType::NPCSandbox,
                name: "NPC Sandbox".into(),
                description: "Open world with autonomous NPCs driven by AI agents".into(),
                enabled_agents: vec![NarrativeAgentRole::NPCDirector, NarrativeAgentRole::WorldKeeper],
                collaboration: CollaborationMode::Hierarchical,
                narrative_style: "emergent".into(),
                metadata: HashMap::new(),
            },
            GameModeDefinition {
                mode_type: GameModeType::ChatRoleplay,
                name: "Chat Roleplay".into(),
                description: "One-on-one character roleplay with a single AI persona".into(),
                enabled_agents: vec![NarrativeAgentRole::Narrator],
                collaboration: CollaborationMode::Sequential,
                narrative_style: "conversational".into(),
                metadata: HashMap::new(),
            },
        ]
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::TextAdventure => "Text Adventure",
            Self::AIBattle => "AI Battle",
            Self::NPCSandbox => "NPC Sandbox",
            Self::ChatRoleplay => "Chat Roleplay",
            Self::Custom(_) => "Custom",
        }
    }
}

/// Active game mode state
#[derive(Debug, Clone, Default)]
pub struct GameModeState {
    pub current: Option<GameModeDefinition>,
    pub is_active: bool,
    pub round: u64,
}

impl GameModeState {
    pub fn activate(&mut self, mode: GameModeDefinition) {
        self.current = Some(mode);
        self.is_active = true;
        self.round = 0;
    }

    pub fn deactivate(&mut self) {
        self.is_active = false;
    }

    pub fn next_round(&mut self) {
        self.round += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_presets_exist() {
        let presets = GameModeType::all_presets();
        assert_eq!(presets.len(), 4);
        for p in &presets {
            assert!(!p.enabled_agents.is_empty());
        }
    }

    #[test]
    fn test_mode_activation() {
        let mut state = GameModeState::default();
        let mode = GameModeType::all_presets().into_iter().next().unwrap();
        state.activate(mode);
        assert!(state.is_active);
        assert_eq!(state.round, 0);
        state.next_round();
        assert_eq!(state.round, 1);
        state.deactivate();
        assert!(!state.is_active);
    }

    #[test]
    fn test_custom_mode() {
        let custom = GameModeType::Custom("MyMode".into());
        assert_eq!(custom.label(), "Custom");
    }
}
