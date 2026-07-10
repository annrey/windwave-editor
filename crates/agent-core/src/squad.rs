//! Squad Team Collaboration System — inspired by Multica.
//!
//! A Squad is a group of agents (and optionally humans) working together under a leader.
//! The leader coordinates task assignment, manages dependencies, and reports progress.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────────┐
//! │                       @FrontendTeam (Squad)                         │
//! │  ┌─────────────────────────────────────────────────────────────────┐│
//! │  │  SquadLeader (Agent + RoutingPolicy)                            ││
//! │  │  ├── RoundRobin                                                 ││
//! │  │  ├── LoadBalance                                                ││
//! │  │  ├── SkillBased (default)                                       ││
//! │  │  └── LeaderDecides                                              ││
//! │  └─────────────────────────────────────────────────────────────────┘│
//! ├─────────────────────────────────────────────────────────────────────┤
//! │  Members:                                                           │
//! │  ├─ SceneAgent  (Capability: SceneRead, SceneWrite)                │
//! │  ├─ CodeAgent   (Capability: CodeRead, CodeWrite)                  │
//! │  └─ AssetAgent  (Capability: AssetRead, AssetWrite)                │
//! └─────────────────────────────────────────────────────────────────────┘
//! ```

use crate::registry::{AgentId, CapabilityKind};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};

// ---------------------------------------------------------------------------
// ID Types
// ---------------------------------------------------------------------------

/// Unique identifier for a Squad.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SquadId(pub u64);

impl SquadId {
    pub fn new(id: u64) -> Self {
        Self(id)
    }
}

/// Unique identifier for a Task within a Squad.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TaskId(pub u64);

impl TaskId {
    pub fn new(id: u64) -> Self {
        Self(id)
    }
}

// ---------------------------------------------------------------------------
// Routing Policy
// ---------------------------------------------------------------------------

/// Policy for routing tasks within a Squad.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum RoutingPolicy {
    /// Leader looks at the task and decides which agent to assign it to.
    LeaderDecides,
    /// Round-robin assignment in rotation.
    RoundRobin,
    /// Assign to the least busy agent with matching capabilities.
    LoadBalance,
    /// Assign to the agent whose capabilities best match the task.
    #[default]
    SkillBased,
}

impl RoutingPolicy {
    pub fn name(&self) -> &'static str {
        match self {
            Self::LeaderDecides => "leader_decides",
            Self::RoundRobin => "round_robin",
            Self::LoadBalance => "load_balance",
            Self::SkillBased => "skill_based",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            Self::LeaderDecides => "Leader looks at the task and decides assignment",
            Self::RoundRobin => "Round-robin assignment in rotation",
            Self::LoadBalance => "Assign to the least busy available agent",
            Self::SkillBased => "Assign to agent whose capabilities best match the task",
        }
    }
}

// ---------------------------------------------------------------------------
// Squad Task
// ---------------------------------------------------------------------------

/// Status of a task in a Squad.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskStatus {
    /// Waiting to be assigned.
    Pending,
    /// Assigned to an agent but not started yet.
    Assigned(AgentId),
    /// Agent is actively working on this task.
    InProgress(AgentId),
    /// Task completed successfully.
    Completed(AgentId),
    /// Task failed.
    Failed(AgentId, String),
    /// Task was cancelled.
    Cancelled,
}

/// Priority for a Squad task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Default)]
pub enum TaskPriority {
    Low = 0,
    #[default]
    Normal = 1,
    High = 2,
    Urgent = 3,
}

/// A task assigned to a Squad.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SquadTask {
    pub id: TaskId,
    pub title: String,
    pub description: String,
    pub priority: TaskPriority,
    pub status: TaskStatus,
    pub required_capabilities: Vec<CapabilityKind>,
    pub created_at: u64,
    pub started_at: Option<u64>,
    pub completed_at: Option<u64>,
    pub assigned_member: Option<AgentId>,
    pub parent_task: Option<TaskId>,
}

impl SquadTask {
    pub fn new(
        id: TaskId,
        title: String,
        description: String,
        required_capabilities: Vec<CapabilityKind>,
    ) -> Self {
        Self {
            id,
            title,
            description,
            priority: TaskPriority::Normal,
            status: TaskStatus::Pending,
            required_capabilities,
            created_at: now_secs(),
            started_at: None,
            completed_at: None,
            assigned_member: None,
            parent_task: None,
        }
    }

    pub fn with_priority(mut self, priority: TaskPriority) -> Self {
        self.priority = priority;
        self
    }
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

// ---------------------------------------------------------------------------
// Squad Definition
// ---------------------------------------------------------------------------

/// A Squad is a group of agents working together under a leader.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Squad {
    pub id: SquadId,
    pub name: String,
    pub description: String,
    pub leader: AgentId,
    pub members: Vec<AgentId>,
    pub policy: RoutingPolicy,
    pub task_queue: VecDeque<SquadTask>,
    pub active_tasks: HashMap<TaskId, SquadTask>,
    pub max_parallel_tasks: usize,
}

impl Squad {
    pub fn new(id: SquadId, name: String, leader: AgentId, policy: RoutingPolicy) -> Self {
        Self {
            id,
            name,
            description: String::new(),
            leader,
            members: Vec::new(),
            policy,
            task_queue: VecDeque::new(),
            active_tasks: HashMap::new(),
            max_parallel_tasks: 4,
        }
    }

    pub fn add_member(&mut self, agent_id: AgentId) {
        if !self.members.contains(&agent_id) {
            self.members.push(agent_id);
        }
    }

    pub fn remove_member(&mut self, agent_id: AgentId) -> bool {
        let before_len = self.members.len();
        self.members.retain(|&a| a != agent_id);
        self.members.len() < before_len
    }

    pub fn is_member(&self, agent_id: AgentId) -> bool {
        self.members.contains(&agent_id)
    }

    pub fn is_leader(&self, agent_id: AgentId) -> bool {
        self.leader == agent_id
    }

    /// Submit a new task to the Squad.
    pub fn submit_task(&mut self, task: SquadTask) {
        // Insert at the correct position based on priority
        let insert_pos = self
            .task_queue
            .iter()
            .position(|t| task.priority > t.priority)
            .unwrap_or(self.task_queue.len());
        self.task_queue.insert(insert_pos, task);
    }

    /// Get number of currently active tasks.
    pub fn active_task_count(&self) -> usize {
        self.active_tasks.len()
    }

    /// Check if more tasks can be accepted.
    pub fn can_accept_new_task(&self) -> bool {
        self.active_task_count() < self.max_parallel_tasks
    }

    /// Cancel a task.
    pub fn cancel_task(&mut self, task_id: TaskId) {
        // Remove from queue if present
        self.task_queue.retain(|t| t.id != task_id);
        // Mark as cancelled if active
        if let Some(task) = self.active_tasks.get_mut(&task_id) {
            task.status = TaskStatus::Cancelled;
        }
    }

    /// Get task by ID.
    pub fn get_task(&self, task_id: TaskId) -> Option<&SquadTask> {
        self.active_tasks
            .get(&task_id)
            .or_else(|| self.task_queue.iter().find(|t| t.id == task_id))
    }

    /// Get task by ID (mutable).
    pub fn get_task_mut(&mut self, task_id: TaskId) -> Option<&mut SquadTask> {
        self.active_tasks
            .get_mut(&task_id)
            .or_else(|| self.task_queue.iter_mut().find(|t| t.id == task_id))
    }
}

// ---------------------------------------------------------------------------
// Squad Registry
// ---------------------------------------------------------------------------

/// Registry of all Squads in the system.
pub struct SquadRegistry {
    squads: HashMap<SquadId, Squad>,
    next_squad_id: u64,
    next_task_id: u64,
}

impl Default for SquadRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl SquadRegistry {
    pub fn new() -> Self {
        Self {
            squads: HashMap::new(),
            next_squad_id: 0,
            next_task_id: 0,
        }
    }

    /// Create a new Squad.
    pub fn create_squad(
        &mut self,
        name: String,
        leader: AgentId,
        policy: RoutingPolicy,
    ) -> SquadId {
        let id = SquadId(self.next_squad_id);
        self.next_squad_id += 1;

        let squad = Squad::new(id, name, leader, policy);
        self.squads.insert(id, squad);
        id
    }

    /// Delete a Squad.
    pub fn delete_squad(&mut self, squad_id: SquadId) -> bool {
        self.squads.remove(&squad_id).is_some()
    }

    /// Get a Squad reference.
    pub fn get(&self, squad_id: SquadId) -> Option<&Squad> {
        self.squads.get(&squad_id)
    }

    /// Get a mutable Squad reference.
    pub fn get_mut(&mut self, squad_id: SquadId) -> Option<&mut Squad> {
        self.squads.get_mut(&squad_id)
    }

    /// Get mutable Squad with a fresh TaskId (avoids double-borrow).
    pub fn get_mut_with_task_id(&mut self, squad_id: SquadId) -> Option<(&mut Squad, TaskId)> {
        let tid = self.next_task_id();
        self.squads.get_mut(&squad_id).map(|squad| (squad, tid))
    }

    /// List all Squads.
    pub fn list(&self) -> Vec<(&SquadId, &Squad)> {
        self.squads.iter().collect()
    }

    /// Find Squads that contain a specific agent as a member.
    pub fn find_by_member(&self, agent_id: AgentId) -> Vec<(&SquadId, &Squad)> {
        self.squads
            .iter()
            .filter(|(_, s)| s.is_member(agent_id))
            .collect()
    }

    /// Find Squads where the agent is leader.
    pub fn find_by_leader(&self, agent_id: AgentId) -> Vec<(&SquadId, &Squad)> {
        self.squads
            .iter()
            .filter(|(_, s)| s.is_leader(agent_id))
            .collect()
    }

    /// Generate a new Task ID.
    pub fn next_task_id(&mut self) -> TaskId {
        let id = self.next_task_id;
        self.next_task_id += 1;
        TaskId(id)
    }
}

// ---------------------------------------------------------------------------
// Squad Task Router
// ---------------------------------------------------------------------------

/// Handles routing and assignment of tasks within a Squad.
pub struct TaskRouter<'a> {
    squad: &'a mut Squad,
}

impl<'a> TaskRouter<'a> {
    pub fn new(squad: &'a mut Squad) -> Self {
        Self { squad }
    }

    /// Try to assign as many pending tasks as possible.
    pub fn process_queue(&mut self) -> Vec<(TaskId, AgentId)> {
        let mut assigned = Vec::new();

        while self.squad.can_accept_new_task() {
            let next_task = self.squad.task_queue.pop_front();
            if let Some(task) = next_task {
                let member = match self.squad.policy {
                    RoutingPolicy::LeaderDecides => self.select_by_leader_decision(&task),
                    RoutingPolicy::RoundRobin => self.select_round_robin(),
                    RoutingPolicy::LoadBalance => self.select_by_load_balance(),
                    RoutingPolicy::SkillBased => self.select_by_skill_match(&task),
                };

                if let Some(agent_id) = member {
                    let mut task = task;
                    task.status = TaskStatus::Assigned(agent_id);
                    task.assigned_member = Some(agent_id);
                    assigned.push((task.id, agent_id));
                    self.squad.active_tasks.insert(task.id, task);
                } else {
                    // No suitable member found — put it back at the end of queue
                    self.squad.task_queue.push_back(task);
                    break;
                }
            } else {
                break;
            }
        }

        assigned
    }

    fn select_by_leader_decision(&self, _task: &SquadTask) -> Option<AgentId> {
        // For now, leader just picks first available member
        self.squad.members.first().copied()
    }

    fn select_round_robin(&self) -> Option<AgentId> {
        // Simplified: pick first member
        self.squad.members.first().copied()
    }

    fn select_by_load_balance(&self) -> Option<AgentId> {
        // For now: pick first member
        self.squad.members.first().copied()
    }

    fn select_by_skill_match(&self, task: &SquadTask) -> Option<AgentId> {
        // Find member whose capabilities best match task requirements
        if task.required_capabilities.is_empty() {
            return self.squad.members.first().copied();
        }

        // For now: just pick first member
        self.squad.members.first().copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::AgentId;

    #[test]
    fn test_create_squad() {
        let mut registry = SquadRegistry::new();
        let leader_id = AgentId(1);
        let squad_id =
            registry.create_squad("TestTeam".into(), leader_id, RoutingPolicy::default());

        assert_eq!(registry.list().len(), 1);

        let squad = registry.get(squad_id).unwrap();
        assert_eq!(squad.name, "TestTeam");
        assert_eq!(squad.leader, leader_id);
    }

    #[test]
    fn test_add_remove_member() {
        let mut registry = SquadRegistry::new();
        let leader_id = AgentId(1);
        let squad_id =
            registry.create_squad("TestTeam".into(), leader_id, RoutingPolicy::default());

        let squad = registry.get_mut(squad_id).unwrap();
        squad.add_member(AgentId(2));
        assert_eq!(squad.members.len(), 1);
        assert!(squad.is_member(AgentId(2)));

        squad.add_member(AgentId(3));
        assert_eq!(squad.members.len(), 2);

        squad.remove_member(AgentId(2));
        assert_eq!(squad.members.len(), 1);
    }

    #[test]
    fn test_submit_and_queue() {
        let mut registry = SquadRegistry::new();
        let leader_id = AgentId(1);
        let squad_id =
            registry.create_squad("TestTeam".into(), leader_id, RoutingPolicy::default());

        let (squad, task_id) = registry.get_mut_with_task_id(squad_id).unwrap();
        squad.add_member(AgentId(2));

        let task = SquadTask::new(
            task_id,
            "Create player".into(),
            "Create a player entity".into(),
            vec![CapabilityKind::SceneRead, CapabilityKind::SceneWrite],
        );

        squad.submit_task(task);
        assert_eq!(squad.task_queue.len(), 1);
    }
}
