//! Agent team structure — formal role definitions for a multi-agent team.
//!
//! The team follows a three-layer architecture (ADR-001):
//!
//! **Layer 1 — Strategic (CEO)**
//! - **Ceo** — Receives user requests, decomposes into high-level goals,
//!   monitors multiple ProjectManagers, allocates resources, adjusts priorities.
//!
//! **Layer 2 — Orchestration (ProjectManager)**
//! - **ProjectManager** — Merged Director + Planner responsibilities.
//!   Receives goals from CEO, decomposes into EditPlans, schedules Agent
//!   execution, monitors progress, handles user approvals.
//!
//! **Layer 3 — Execution (Agent Cluster)**
//! - **Executor** — Carries out individual plan steps (scene/code/asset).
//! - **Reviewer** — Validates execution results against goals.
//! - **Hr** — Manages team membership (add/remove/onboard agents).
//!
//! All agents share public knowledge via `CommunicationHub::SharedContext`
//! but maintain independent per-agent four-layer memory spaces.

use serde::{Deserialize, Serialize};

/// Formal team role.
///
/// Three-layer architecture (ADR-001):
/// - Layer 1 (Strategic): Ceo
/// - Layer 2 (Orchestration): ProjectManager
/// - Layer 3 (Execution): Executor, Reviewer, Hr
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TeamRole {
    /// Strategic layer — monitors multiple ProjectManagers, allocates resources.
    Ceo,
    /// Orchestration layer — merged Director + Planner responsibilities.
    ProjectManager,
    /// Carries out individual plan steps (scene/code/asset).
    Executor,
    /// Validates execution results against goals.
    Reviewer,
    /// Manages team membership (add/remove/onboard agents).
    Hr,
}

impl TeamRole {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Ceo => "ceo",
            Self::ProjectManager => "project_manager",
            Self::Executor => "executor",
            Self::Reviewer => "reviewer",
            Self::Hr => "hr",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            Self::Ceo => "Strategic layer — monitors ProjectManagers, allocates resources, adjusts priorities",
            Self::ProjectManager => "Orchestration layer — decomposes goals into plans, schedules agents, monitors progress",
            Self::Executor => "Carries out individual plan steps (scene/code/asset)",
            Self::Reviewer => "Validates execution results against goals; can request revisions",
            Self::Hr => "Manages team membership — add/remove/onboard agents",
        }
    }

    /// Returns the layer this role belongs to.
    pub fn layer(&self) -> TeamLayer {
        match self {
            Self::Ceo => TeamLayer::Strategic,
            Self::ProjectManager => TeamLayer::Orchestration,
            Self::Executor | Self::Reviewer | Self::Hr => TeamLayer::Execution,
        }
    }
}

/// Architectural layer for team roles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TeamLayer {
    Strategic,
    Orchestration,
    Execution,
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
        roster.add("CEO", TeamRole::Ceo, vec![]);
        roster.add("PM1", TeamRole::ProjectManager, vec![]);
        roster.add("E1", TeamRole::Executor, vec![]);
        roster.add("E2", TeamRole::Executor, vec![]);

        assert_eq!(roster.find_by_role(TeamRole::Ceo).len(), 1);
        assert_eq!(roster.find_by_role(TeamRole::ProjectManager).len(), 1);
        assert_eq!(roster.find_by_role(TeamRole::Executor).len(), 2);
        assert_eq!(roster.count_by_role(TeamRole::Reviewer), 0);
    }

    #[test]
    fn test_role_layer() {
        assert_eq!(TeamRole::Ceo.layer(), TeamLayer::Strategic);
        assert_eq!(TeamRole::ProjectManager.layer(), TeamLayer::Orchestration);
        assert_eq!(TeamRole::Executor.layer(), TeamLayer::Execution);
        assert_eq!(TeamRole::Reviewer.layer(), TeamLayer::Execution);
        assert_eq!(TeamRole::Hr.layer(), TeamLayer::Execution);
    }
}
