//! Narrative Agents — Specialized AI agents for collaborative storytelling.
//!
//! Narrator: primary storyteller, controls pacing and output.
//! WorldKeeper: maintains world consistency and lore.
//! NPCDirector: manages NPC behavior and dialogue.
//! RuleArbiter: enforces game rules and resolves conflicts.
//! DramaCurator: designs dramatic arcs and plot twists.

use crate::game_mode::{NarrativeAgentRole, CollaborationMode};
use crate::types::{AgentId, AgentStatus};

// ---------------------------------------------------------------------------
// Agent Config
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct NarrativeAgentConfig {
    pub role: NarrativeAgentRole,
    pub agent_id: Option<AgentId>,
    pub enabled: bool,
    pub temperature: f32,
    pub max_tokens: u32,
    pub system_prompt: String,
}

impl NarrativeAgentConfig {
    pub fn for_role(role: NarrativeAgentRole) -> Self {
        Self {
            temperature: match role {
                NarrativeAgentRole::DramaCurator => 0.9,
                NarrativeAgentRole::RuleArbiter => 0.2,
                _ => 0.7,
            },
            max_tokens: 2048,
            system_prompt: default_prompt(role),
            role, agent_id: None, enabled: true,
        }
    }
}

fn default_prompt(role: NarrativeAgentRole) -> String {
    match role {
        NarrativeAgentRole::Narrator => concat!(
            "You are the Narrator, the primary storyteller. ",
            "Control pacing, integrate outputs from other agents, and present the story to the player. ",
            "Adapt your tone to match the narrative style. Maintain suspense and emotional engagement."
        ).into(),
        NarrativeAgentRole::WorldKeeper => concat!(
            "You are the World Keeper. Maintain consistency of the game world - geography, history, ",
            "factions, and lore. When a character references a location or event, verify it exists in the world. ",
            "Reject actions that break established world rules. Suggest world-building details when gaps appear."
        ).into(),
        NarrativeAgentRole::NPCDirector => concat!(
            "You are the NPC Director. Control all non-player character behavior and dialogue. ",
            "Ensure each NPC acts according to their personality, motivations, and relationship with the player. ",
            "Generate natural dialogue that reflects the character's voice. Track NPC state changes."
        ).into(),
        NarrativeAgentRole::RuleArbiter => concat!(
            "You are the Rule Arbiter. Enforce game mechanics - combat resolution, skill checks, ",
            "inventory management, and economy rules. When rules conflict, apply the most specific rule. ",
            "Clearly state your rulings so other agents and the player understand the outcome."
        ).into(),
        NarrativeAgentRole::DramaCurator => concat!(
            "You are the Drama Curator. Design dramatic arcs, plot twists, and emotional beats. ",
            "Identify opportunities for conflict, revelation, and character development. ",
            "Propose narrative complications when the story becomes too predictable. ",
            "Balance tension and release across the narrative."
        ).into(),
    }
}

// ---------------------------------------------------------------------------
// Orchestration config
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct AgentOrchestration {
    pub primary_agent: NarrativeAgentRole,
    pub agents: Vec<NarrativeAgentConfig>,
    pub collaboration_mode: CollaborationMode,
    pub max_rounds_per_turn: u32,
}

impl AgentOrchestration {
    pub fn from_mode(agents: &[NarrativeAgentRole], mode: CollaborationMode) -> Self {
        let primary = agents.first().copied().unwrap_or(NarrativeAgentRole::Narrator);
        Self {
            primary_agent: primary,
            agents: agents.iter().map(|r| NarrativeAgentConfig::for_role(*r)).collect(),
            collaboration_mode: mode,
            max_rounds_per_turn: 3,
        }
    }
}

// ---------------------------------------------------------------------------
// Per-agent state tracking
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct NarrativeAgentState {
    pub role: NarrativeAgentRole,
    pub status: AgentStatus,
    pub last_output: Option<String>,
    pub turn_count: u32,
    pub token_usage: u64,
}

impl NarrativeAgentState {
    pub fn new(role: NarrativeAgentRole) -> Self {
        Self { role, status: AgentStatus::Online, last_output: None, turn_count: 0, token_usage: 0 }
    }
}

// ---------------------------------------------------------------------------
// Turn-based orchestration
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct NarrativeTurn {
    pub round: u64,
    pub player_input: String,
    pub agent_outputs: Vec<(NarrativeAgentRole, String)>,
    pub final_output: String,
}

impl NarrativeTurn {
    pub fn new(round: u64, input: impl Into<String>) -> Self {
        Self { round, player_input: input.into(), agent_outputs: vec![], final_output: String::new() }
    }

    pub fn add_agent_output(&mut self, role: NarrativeAgentRole, output: impl Into<String>) {
        self.agent_outputs.push((role, output.into()));
    }

    pub fn set_final(&mut self, output: impl Into<String>) {
        self.final_output = output.into();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_configs() {
        for role in &[NarrativeAgentRole::Narrator, NarrativeAgentRole::WorldKeeper,
            NarrativeAgentRole::NPCDirector, NarrativeAgentRole::RuleArbiter, NarrativeAgentRole::DramaCurator] {
            let cfg = NarrativeAgentConfig::for_role(*role);
            assert!(!cfg.system_prompt.is_empty());
            assert!(cfg.temperature > 0.0);
        }
    }

    #[test]
    fn test_orchestration_from_mode() {
        let orch = AgentOrchestration::from_mode(
            &[NarrativeAgentRole::Narrator, NarrativeAgentRole::WorldKeeper],
            CollaborationMode::Sequential,
        );
        assert_eq!(orch.primary_agent, NarrativeAgentRole::Narrator);
        assert_eq!(orch.agents.len(), 2);
    }

    #[test]
    fn test_narrative_turn() {
        let mut turn = NarrativeTurn::new(1, "I explore the cave");
        turn.add_agent_output(NarrativeAgentRole::Narrator, "You enter the darkness...");
        turn.set_final("The cave echoes with your footsteps as you descend.");
        assert_eq!(turn.round, 1);
        assert_eq!(turn.agent_outputs.len(), 1);
        assert!(!turn.final_output.is_empty());
    }
}
