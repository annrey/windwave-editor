//! CEO Agent — strategic layer for multi-ProjectManager orchestration.
//!
//! The CEO is the top-level orchestrator that:
//! 1. Receives user requests and decomposes them into high-level goals
//! 2. Monitors multiple ProjectManager instances and their execution status
//! 3. Allocates resources (LLM tokens, compute budget, time limits)
//! 4. Adjusts priorities when conflicts arise between ProjectManagers
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────┐
//! │                    CEO (Strategic)                       │
//! │  - DirectorRegistry (manages all PMs)                   │
//! │  - GoalQueue (pending goals when no PM available)       │
//! │  - ResourceBudget (global resource allocation)          │
//! └──────────────────────┬──────────────────────────────────┘
//!                        │ GoalAssignment / StatusUpdate
//! ┌──────────────────────▼──────────────────────────────────┐
//! │              ProjectManager (Orchestration)              │
//! └─────────────────────────────────────────────────────────┘
//! ```

use crate::registry::{Agent, AgentId, AgentRequest, AgentResponse, AgentResultKind, CapabilityKind, AgentError};
use crate::team_structure::TeamRole;
use std::collections::HashMap;
use serde::{Serialize, Deserialize};

// ---------------------------------------------------------------------------
// ID Types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ProjectManagerId(pub u64);

impl ProjectManagerId {
    pub fn new(id: u64) -> Self {
        Self(id)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GoalId(pub u64);

// ---------------------------------------------------------------------------
// Goal Types
// ---------------------------------------------------------------------------

/// A high-level goal assigned by the CEO to a ProjectManager.
#[derive(Debug, Clone)]
pub struct HighLevelGoal {
    pub id: GoalId,
    pub description: String,
    pub priority: u8,           // 0-255, higher = more important
    pub assigned_pm: Option<ProjectManagerId>,
    pub status: GoalStatus,
    pub constraints: GoalConstraints,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GoalStatus {
    Pending,
    Assigned(ProjectManagerId),
    InProgress(ProjectManagerId),
    Completed(ProjectManagerId),
    Failed(ProjectManagerId, String),
}

#[derive(Debug, Clone, Default)]
pub struct GoalConstraints {
    pub max_tokens: Option<u64>,
    pub max_time_seconds: Option<u64>,
    pub required_capabilities: Vec<String>,
    pub forbidden_capabilities: Vec<String>,
}

// ---------------------------------------------------------------------------
// Resource Budget
// ---------------------------------------------------------------------------

/// Resource budget for a ProjectManager.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceBudget {
    pub token_budget: u64,
    pub time_budget_seconds: u64,
    pub max_parallel_agents: usize,
}

impl Default for ResourceBudget {
    fn default() -> Self {
        Self {
            token_budget: 100_000,
            time_budget_seconds: 300,
            max_parallel_agents: 4,
        }
    }
}

// ---------------------------------------------------------------------------
// Director Registry (manages all ProjectManagers)
// ---------------------------------------------------------------------------

/// Registry of all ProjectManager instances managed by the CEO.
pub struct DirectorRegistry {
    directors: HashMap<ProjectManagerId, DirectorHandle>,
    next_director_id: u64,
    next_goal_id: u64,
}

pub struct DirectorHandle {
    pub id: ProjectManagerId,
    pub name: String,
    pub status: DirectorStatus,
    pub current_goal: Option<HighLevelGoal>,
    pub metrics: DirectorMetrics,
    pub budget: ResourceBudget,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DirectorStatus {
    Idle,
    Executing,
    WaitingApproval,
    Paused,
    Error(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DirectorMetrics {
    pub total_tasks_completed: u64,
    pub total_tasks_failed: u64,
    pub avg_completion_time_seconds: f64,
    pub token_usage_total: u64,
}

/// Status report for a single ProjectManager.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectorStatusReport {
    pub pm_id: ProjectManagerId,
    pub name: String,
    pub status: DirectorStatus,
    pub current_goal: Option<GoalId>,
    pub metrics: DirectorMetrics,
    pub budget: ResourceBudget,
}

impl DirectorRegistry {
    pub fn new() -> Self {
        Self {
            directors: HashMap::new(),
            next_director_id: 0,
            next_goal_id: 0,
        }
    }

    pub fn next_goal_id(&mut self) -> GoalId {
        let id = self.next_goal_id;
        self.next_goal_id += 1;
        GoalId(id)
    }

    /// Find an idle ProjectManager suitable for a goal.
    pub fn find_idle_for_goal(&self, _goal: &HighLevelGoal) -> Option<ProjectManagerId> {
        self.directors
            .iter()
            .find(|(_, h)| h.status == DirectorStatus::Idle)
            .map(|(id, _)| *id)
    }

    /// Assign a goal to a ProjectManager.
    pub fn assign_goal(&mut self, pm_id: ProjectManagerId, goal: HighLevelGoal) {
        if let Some(handle) = self.directors.get_mut(&pm_id) {
            handle.current_goal = Some(goal.clone());
            handle.status = DirectorStatus::Executing;
        }
    }

    /// Mark a goal as completed.
    pub fn mark_goal_completed(&mut self, pm_id: ProjectManagerId, goal_id: GoalId) {
        if let Some(handle) = self.directors.get_mut(&pm_id) {
            if let Some(ref mut goal) = handle.current_goal {
                if goal.id == goal_id {
                    goal.status = GoalStatus::Completed(pm_id);
                }
            }
            handle.status = DirectorStatus::Idle;
            handle.metrics.total_tasks_completed += 1;
        }
    }

    /// Mark a goal as failed.
    pub fn mark_goal_failed(&mut self, pm_id: ProjectManagerId, goal_id: GoalId, reason: String) {
        if let Some(handle) = self.directors.get_mut(&pm_id) {
            if let Some(ref mut goal) = handle.current_goal {
                if goal.id == goal_id {
                    goal.status = GoalStatus::Failed(pm_id, reason);
                }
            }
            handle.status = DirectorStatus::Idle;
            handle.metrics.total_tasks_failed += 1;
        }
    }

    /// Update budget for a ProjectManager.
    pub fn update_budget(&mut self, pm_id: ProjectManagerId, new_budget: ResourceBudget) {
        if let Some(handle) = self.directors.get_mut(&pm_id) {
            handle.budget = new_budget;
        }
    }

    /// Get status report for all ProjectManagers.
    pub fn status_report(&self) -> Vec<DirectorStatusReport> {
        self.directors
            .iter()
            .map(|(id, h)| DirectorStatusReport {
                pm_id: *id,
                name: h.name.clone(),
                status: h.status.clone(),
                current_goal: h.current_goal.as_ref().map(|g| g.id),
                metrics: h.metrics.clone(),
                budget: h.budget.clone(),
            })
            .collect()
    }

    /// Spawn a new ProjectManager instance.
    pub fn spawn(&mut self, name: &str) -> ProjectManagerId {
        let id = ProjectManagerId(self.next_director_id);
        self.next_director_id += 1;

        self.directors.insert(id, DirectorHandle {
            id,
            name: name.to_string(),
            status: DirectorStatus::Idle,
            current_goal: None,
            metrics: DirectorMetrics::default(),
            budget: ResourceBudget::default(),
        });

        id
    }

    /// Terminate a ProjectManager instance.
    pub fn terminate(&mut self, pm_id: ProjectManagerId) {
        self.directors.remove(&pm_id);
    }

    /// Get a ProjectManager by ID.
    pub fn get(&self, pm_id: ProjectManagerId) -> Option<&DirectorHandle> {
        self.directors.get(&pm_id)
    }

    /// Get mutable reference to a ProjectManager.
    pub fn get_mut(&mut self, pm_id: ProjectManagerId) -> Option<&mut DirectorHandle> {
        self.directors.get_mut(&pm_id)
    }

    /// List all ProjectManager IDs.
    pub fn list_ids(&self) -> Vec<ProjectManagerId> {
        self.directors.keys().copied().collect()
    }

    /// Count of ProjectManagers.
    pub fn count(&self) -> usize {
        self.directors.len()
    }
}

impl Default for DirectorRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// CeoAgent
// ---------------------------------------------------------------------------

pub struct CeoAgent {
    id: AgentId,
    name: String,
    director_registry: DirectorRegistry,
    goal_queue: Vec<HighLevelGoal>,
    resource_budget: ResourceBudget,
}

impl CeoAgent {
    pub fn new(id: AgentId) -> Self {
        Self {
            id,
            name: "CEO".into(),
            director_registry: DirectorRegistry::new(),
            goal_queue: Vec::new(),
            resource_budget: ResourceBudget::default(),
        }
    }

    /// Assign a high-level goal to a ProjectManager.
    pub fn assign_goal(&mut self, goal: HighLevelGoal) -> ProjectManagerId {
        let pm_id = self.director_registry.find_idle_for_goal(&goal);
        if let Some(id) = pm_id {
            self.director_registry.assign_goal(id, goal.clone());
            id
        } else {
            // No idle PM available — queue the goal
            self.goal_queue.push(goal);
            // Return a placeholder ID; the goal will be assigned when a PM becomes idle
            ProjectManagerId(0)
        }
    }

    /// Monitor all ProjectManagers and report status.
    pub fn monitor(&self) -> Vec<DirectorStatusReport> {
        self.director_registry.status_report()
    }

    /// Adjust resource allocation for a ProjectManager.
    pub fn adjust_budget(&mut self, pm_id: ProjectManagerId, new_budget: ResourceBudget) {
        self.director_registry.update_budget(pm_id, new_budget);
    }

    /// Re-prioritize goals when conflicts arise.
    pub fn reassign_priorities(&mut self, new_priorities: Vec<(GoalId, u8)>) {
        for (goal_id, priority) in new_priorities {
            // Find the goal in assigned PMs or queue and update priority
            for handle in self.director_registry.directors.values_mut() {
                if let Some(ref mut goal) = handle.current_goal {
                    if goal.id == goal_id {
                        goal.priority = priority;
                    }
                }
            }
            for goal in &mut self.goal_queue {
                if goal.id == goal_id {
                    goal.priority = priority;
                }
            }
        }
    }

    /// Spawn a new ProjectManager instance.
    pub fn spawn_director(&mut self, name: &str) -> ProjectManagerId {
        self.director_registry.spawn(name)
    }

    /// Terminate a ProjectManager instance.
    pub fn terminate_director(&mut self, pm_id: ProjectManagerId) {
        self.director_registry.terminate(pm_id);
    }

    /// Try to assign queued goals to idle ProjectManagers.
    pub fn process_goal_queue(&mut self) -> Vec<(GoalId, ProjectManagerId)> {
        let mut assigned = Vec::new();
        while let Some(goal) = self.goal_queue.pop() {
            if let Some(pm_id) = self.director_registry.find_idle_for_goal(&goal) {
                self.director_registry.assign_goal(pm_id, goal.clone());
                assigned.push((goal.id, pm_id));
            } else {
                // Put it back at the front
                self.goal_queue.insert(0, goal);
                break;
            }
        }
        assigned
    }
}

#[async_trait::async_trait]
impl Agent for CeoAgent {
    fn id(&self) -> AgentId {
        self.id
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn role(&self) -> &str {
        "ceo"
    }

    fn capabilities(&self) -> &[CapabilityKind] {
        &[
            CapabilityKind::Orchestrate,
            CapabilityKind::EngineControl, // CEO can control engine-level resources
        ]
    }

    async fn handle(&mut self, request: AgentRequest) -> Result<AgentResponse, AgentError> {
        let ins = request.instruction.to_lowercase();

        if ins.contains("create") || ins.contains("spawn") || ins.contains("创建") {
            let name = request
                .context
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("PM1")
                .to_string();
            let pm_id = self.spawn_director(&name);
            Ok(AgentResponse {
                agent_id: self.id,
                agent_name: self.name.clone(),
                result: AgentResultKind::Success {
                    summary: format!("Created ProjectManager '{}'", name),
                    output: serde_json::json!({ "pm_id": pm_id.0, "name": name }),
                },
                events: vec![],
            })
        } else if ins.contains("monitor") || ins.contains("监控") {
            let reports = self.monitor();
            Ok(AgentResponse {
                agent_id: self.id,
                agent_name: self.name.clone(),
                result: AgentResultKind::Success {
                    summary: format!("Monitoring {} ProjectManagers", reports.len()),
                    output: serde_json::json!({ "directors": reports }),
                },
                events: vec![],
            })
        } else if ins.contains("assign") || ins.contains("分配") {
            let description = request
                .context
                .get("goal")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let priority = request
                .context
                .get("priority")
                .and_then(|v| v.as_u64())
                .unwrap_or(50) as u8;
            let goal = HighLevelGoal {
                id: self.director_registry.next_goal_id(),
                description,
                priority,
                assigned_pm: None,
                status: GoalStatus::Pending,
                constraints: GoalConstraints::default(),
            };
            let pm_id = self.assign_goal(goal);
            Ok(AgentResponse {
                agent_id: self.id,
                agent_name: self.name.clone(),
                result: AgentResultKind::Success {
                    summary: format!("Assigned goal to ProjectManager {}", pm_id.0),
                    output: serde_json::json!({ "pm_id": pm_id.0 }),
                },
                events: vec![],
            })
        } else if ins.contains("adjust") || ins.contains("调整") {
            let pm_id_val = request
                .context
                .get("pm_id")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            let token_budget = request
                .context
                .get("tokens")
                .and_then(|v| v.as_u64());
            let new_budget = ResourceBudget {
                token_budget: token_budget.unwrap_or(100_000),
                time_budget_seconds: 300,
                max_parallel_agents: 4,
            };
            self.adjust_budget(ProjectManagerId(pm_id_val), new_budget);
            Ok(AgentResponse {
                agent_id: self.id,
                agent_name: self.name.clone(),
                result: AgentResultKind::Success {
                    summary: "Budget adjusted".into(),
                    output: serde_json::json!({}),
                },
                events: vec![],
            })
        } else if ins.contains("terminate") || ins.contains("终止") {
            let pm_id_val = request
                .context
                .get("pm_id")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            self.terminate_director(ProjectManagerId(pm_id_val));
            Ok(AgentResponse {
                agent_id: self.id,
                agent_name: self.name.clone(),
                result: AgentResultKind::Success {
                    summary: format!("Terminated ProjectManager {}", pm_id_val),
                    output: serde_json::json!({}),
                },
                events: vec![],
            })
        } else if ins.contains("queue") || ins.contains("队列") {
            Ok(AgentResponse {
                agent_id: self.id,
                agent_name: self.name.clone(),
                result: AgentResultKind::Success {
                    summary: format!("{} goals in queue", self.goal_queue.len()),
                    output: serde_json::json!({ "queued_goals": self.goal_queue.len() }),
                },
                events: vec![],
            })
        } else {
            Ok(AgentResponse {
                agent_id: self.id,
                agent_name: self.name.clone(),
                result: AgentResultKind::Failed {
                    reason: "Unknown CEO command. Try: create <name>, monitor, assign <goal>, adjust <pm_id> <budget>, terminate <pm_id>, queue".into(),
                },
                events: vec![],
            })
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_director_registry_spawn_and_assign() {
        let mut registry = DirectorRegistry::new();
        let pm_id = registry.spawn("PM1");
        assert_eq!(registry.count(), 1);

        let goal = HighLevelGoal {
            id: registry.next_goal_id(),
            description: "Test goal".into(),
            priority: 50,
            assigned_pm: None,
            status: GoalStatus::Pending,
            constraints: GoalConstraints::default(),
        };
        registry.assign_goal(pm_id, goal.clone());

        let handle = registry.get(pm_id).unwrap();
        assert_eq!(handle.status, DirectorStatus::Executing);
        assert!(handle.current_goal.is_some());
    }

    #[test]
    fn test_ceo_spawn_and_monitor() {
        let mut ceo = CeoAgent::new(AgentId(1));
        let pm_id = ceo.spawn_director("PM1");
        assert_eq!(ceo.director_registry.count(), 1);

        let reports = ceo.monitor();
        assert_eq!(reports.len(), 1);
        assert_eq!(reports[0].name, "PM1");
    }

    #[test]
    fn test_goal_queue_processing() {
        let mut ceo = CeoAgent::new(AgentId(1));
        ceo.spawn_director("PM1");
        ceo.spawn_director("PM2");

        // Assign goals to both PMs
        let goal1 = HighLevelGoal {
            id: ceo.director_registry.next_goal_id(),
            description: "Goal 1".into(),
            priority: 50,
            assigned_pm: None,
            status: GoalStatus::Pending,
            constraints: GoalConstraints::default(),
        };
        let goal2 = HighLevelGoal {
            id: ceo.director_registry.next_goal_id(),
            description: "Goal 2".into(),
            priority: 60,
            assigned_pm: None,
            status: GoalStatus::Pending,
            constraints: GoalConstraints::default(),
        };
        ceo.assign_goal(goal1);
        ceo.assign_goal(goal2);

        // Both PMs are now busy, queue a third goal
        let goal3 = HighLevelGoal {
            id: ceo.director_registry.next_goal_id(),
            description: "Goal 3".into(),
            priority: 70,
            assigned_pm: None,
            status: GoalStatus::Pending,
            constraints: GoalConstraints::default(),
        };
        ceo.assign_goal(goal3);
        assert_eq!(ceo.goal_queue.len(), 1);

        // Complete one goal to free a PM
        let pm_ids = ceo.director_registry.list_ids();
        ceo.director_registry.mark_goal_completed(pm_ids[0], GoalId(0));

        // Process queue — should assign goal3 to the freed PM
        let assigned = ceo.process_goal_queue();
        assert_eq!(assigned.len(), 1);
        assert_eq!(ceo.goal_queue.len(), 0);
    }
}