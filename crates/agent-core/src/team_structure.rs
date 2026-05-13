//! Agent team structure — formal role definitions for a multi-agent team.
//!
//! The team follows a separation-of-concerns pattern:
//!
//! - **Director** — Schedules, prioritises, and coordinates all team activity.
//!   The single source of truth for "what should happen next."
//! - **Planner** — Analyses requests and creates structured execution plans.
//!   Does NOT execute — only designs the approach.
//! - **Executor** — Carries out individual plan steps. Multiple executors
//!   (SceneExecutor, CodeExecutor, AssetExecutor) work in parallel.
//! - **Reviewer** — Validates execution results against goals. Can
//!   request plan revisions if results don't match expectations.
//! - **HR** — Manages team membership. Adds/removes agents, onboards new
//!   members with shared context, and handles agent lifecycle.
//!
//! All agents share public knowledge via `CommunicationHub::SharedContext`
//! but maintain independent per-agent conversation memories for focus.

use serde::{Deserialize, Serialize};

/// Formal team role.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TeamRole {
    /// Orchestrates the team — scheduling, prioritisation, coordination.
    Director,
    /// Analyses requests and creates execution plans.
    Planner,
    /// Carries out individual plan steps.
    Executor,
    /// Validates execution results against goals.
    Reviewer,
    /// Manages team membership (add/remove/onboard agents).
    Hr,
}

impl TeamRole {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Director => "director",
            Self::Planner => "planner",
            Self::Executor => "executor",
            Self::Reviewer => "reviewer",
            Self::Hr => "hr",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            Self::Director => "Schedules, prioritises, and coordinates all team activity",
            Self::Planner => "Analyses requests and creates structured execution plans",
            Self::Executor => "Carries out individual plan steps (scene/code/asset)",
            Self::Reviewer => "Validates execution results against goals; can request revisions",
            Self::Hr => "Manages team membership — add/remove/onboard agents",
        }
    }
}

/// A member of the agent team.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamMember {
    /// Unique agent identifier.
    pub agent_id: u64,
    /// Human-readable name.
    pub name: String,
    /// Formal role in the team.
    pub role: TeamRole,
    /// What this agent is capable of.
    pub capabilities: Vec<String>,
    /// Whether this agent is currently available.
    pub online: bool,
    /// Unix timestamp of when this agent joined.
    pub joined_at: u64,
    /// Custom configuration for this agent (e.g. model name, tool list).
    pub config: serde_json::Value,
}

/// Team roster — the current composition of the agent team.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TeamRoster {
    pub members: Vec<TeamMember>,
    /// Next available agent ID.
    pub next_agent_id: u64,
    /// Team-wide rules and constraints.
    pub rules: Vec<String>,
}

impl TeamRoster {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a new member to the team.
    pub fn add(&mut self, name: impl Into<String>, role: TeamRole, capabilities: Vec<String>) -> u64 {
        let agent_id = self.next_agent_id;
        self.next_agent_id += 1;

        self.members.push(TeamMember {
            agent_id,
            name: name.into(),
            role,
            capabilities,
            online: true,
            joined_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            config: serde_json::json!({}),
        });

        agent_id
    }

    /// Remove a member by agent ID.
    pub fn remove(&mut self, agent_id: u64) -> bool {
        let len_before = self.members.len();
        self.members.retain(|m| m.agent_id != agent_id);
        self.members.len() < len_before
    }

    /// Find members by role.
    pub fn find_by_role(&self, role: TeamRole) -> Vec<&TeamMember> {
        self.members.iter().filter(|m| m.role == role).collect()
    }

    /// Find a member by agent ID.
    pub fn find(&self, agent_id: u64) -> Option<&TeamMember> {
        self.members.iter().find(|m| m.agent_id == agent_id)
    }

    /// Set a member's online status.
    pub fn set_online(&mut self, agent_id: u64, online: bool) -> bool {
        if let Some(m) = self.members.iter_mut().find(|m| m.agent_id == agent_id) {
            m.online = online;
            true
        } else {
            false
        }
    }

    /// Count members by role.
    pub fn count_by_role(&self, role: TeamRole) -> usize {
        self.members.iter().filter(|m| m.role == role).count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_roster_add_remove() {
        let mut roster = TeamRoster::new();
        let id = roster.add("SceneBot", TeamRole::Executor, vec!["scene_read".into(), "scene_write".into()]);
        assert_eq!(roster.members.len(), 1);
        assert_eq!(roster.find(id).unwrap().name, "SceneBot");

        assert!(roster.remove(id));
        assert!(roster.members.is_empty());
    }

    #[test]
    fn test_find_by_role() {
        let mut roster = TeamRoster::new();
        roster.add("D", TeamRole::Director, vec![]);
        roster.add("P1", TeamRole::Planner, vec![]);
        roster.add("E1", TeamRole::Executor, vec![]);
        roster.add("E2", TeamRole::Executor, vec![]);

        assert_eq!(roster.find_by_role(TeamRole::Director).len(), 1);
        assert_eq!(roster.find_by_role(TeamRole::Executor).len(), 2);
        assert_eq!(roster.count_by_role(TeamRole::Reviewer), 0);
    }
}
