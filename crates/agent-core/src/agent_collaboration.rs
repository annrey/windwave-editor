//! Agent Collaboration System - Integrates Squad, ReasoningBank, SkillsCompound, and RuntimeRegistry
//!
//! This module provides a unified interface for all the advanced collaboration features.
//! Inspired by Ruflo, Multica, and WindWave's existing architecture.

use crate::reasoning_bank::{ReasoningBankManager, ReasoningTraceId};
use crate::registry::AgentId;
use crate::runtime_registry::RuntimeManager;
use crate::skills_compound::{PatternStep, SkillCompoundManager};
use crate::squad::{SquadId, SquadRegistry, SquadTask, TaskId, TaskPriority};

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Agent Collaboration System - Main Hub
// ---------------------------------------------------------------------------

pub struct AgentCollaborationSystem {
    pub squad_registry: SquadRegistry,
    pub reasoning_bank: ReasoningBankManager,
    pub skills_compound: SkillCompoundManager,
    pub runtime_manager: RuntimeManager,
}

impl Default for AgentCollaborationSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl AgentCollaborationSystem {
    pub fn new() -> Self {
        Self {
            squad_registry: SquadRegistry::new(),
            reasoning_bank: ReasoningBankManager::new(),
            skills_compound: SkillCompoundManager::new(),
            runtime_manager: RuntimeManager::new(),
        }
    }

    // -----------------------------------------------------------------------
    // Task Workflow - High level API
    // -----------------------------------------------------------------------

    /// Submit a task to a squad and track it end-to-end
    pub fn submit_and_track_task(
        &mut self,
        squad_id: SquadId,
        task_title: String,
        task_description: String,
        required_capabilities: Vec<crate::registry::CapabilityKind>,
        priority: TaskPriority,
    ) -> (TaskId, ReasoningTraceId) {
        let task_id = self.squad_registry.next_task_id();

        let task = SquadTask::new(
            task_id,
            task_title.clone(),
            task_description.clone(),
            required_capabilities,
        )
        .with_priority(priority);

        if let Some(squad) = self.squad_registry.get_mut(squad_id) {
            squad.submit_task(task);
        }

        let trace_id = self
            .reasoning_bank
            .start_trace(task_id, task_title, task_description);

        (task_id, trace_id)
    }

    /// Record a ReAct (Think-Act-Observe) cycle for a task
    pub fn record_react_cycle(
        &mut self,
        trace_id: ReasoningTraceId,
        think_description: String,
        think_reasoning: String,
        act_description: String,
        act_input: String,
        act_output: String,
        act_success: bool,
        observe_description: String,
        observe_output: String,
    ) {
        self.reasoning_bank
            .record_think(trace_id, think_description, think_reasoning);
        self.reasoning_bank.record_act(
            trace_id,
            act_description,
            act_input,
            act_output,
            act_success,
        );
        self.reasoning_bank
            .record_observe(trace_id, observe_description, observe_output);
    }

    /// Complete a task and trigger skill compounding
    pub fn complete_task(
        &mut self,
        task_id: TaskId,
        trace_id: ReasoningTraceId,
        success: bool,
        tags: Vec<String>,
    ) -> Option<crate::skills_compound::CompoundSkillId> {
        self.reasoning_bank
            .bank_mut()
            .mark_trace_complete(trace_id, success, tags.clone());

        if success {
            let trace = self.reasoning_bank.bank().get_trace(trace_id)?;
            let mut pattern_steps = Vec::new();
            for step in &trace.steps {
                pattern_steps.push(PatternStep {
                    order: step.id.0 as u32,
                    action: step.description.clone(),
                    parameters: Default::default(),
                    outcome: step.output.as_deref().unwrap_or("").to_string(),
                });
            }

            let _pattern = self.skills_compound.registry_mut().extract_pattern(
                &SquadTask::new(
                    task_id,
                    trace.title.clone(),
                    trace.description.clone(),
                    vec![],
                ),
                true,
                pattern_steps,
            );

            self.skills_compound.process_completed_task(
                &SquadTask::new(
                    task_id,
                    trace.title.clone(),
                    trace.description.clone(),
                    vec![],
                ),
                true,
                vec![],
            )
        } else {
            None
        }
    }

    /// Get recommendations for a new task
    pub fn get_task_recommendations(
        &self,
        task_title: String,
        task_description: String,
    ) -> TaskRecommendations {
        let keywords = vec![task_title, task_description];
        let traces = self.reasoning_bank.get_recommendations(&keywords);
        let skills = self.skills_compound.registry().list_skills();

        TaskRecommendations {
            reasoning_traces: traces.iter().map(|t| t.id).collect(),
            compound_skills: skills.iter().map(|s| s.id).collect(),
        }
    }
}

// ---------------------------------------------------------------------------
// Task Recommendations
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskRecommendations {
    pub reasoning_traces: Vec<ReasoningTraceId>,
    pub compound_skills: Vec<crate::skills_compound::CompoundSkillId>,
}

// ---------------------------------------------------------------------------
// Convenient Builder APIs
// ---------------------------------------------------------------------------

/// Builder for creating a Squad
pub struct SquadBuilder {
    name: String,
    leader: AgentId,
    policy: crate::squad::RoutingPolicy,
    members: Vec<AgentId>,
}

impl SquadBuilder {
    pub fn new(name: String, leader: AgentId) -> Self {
        Self {
            name,
            leader,
            policy: crate::squad::RoutingPolicy::SkillBased,
            members: Vec::new(),
        }
    }

    pub fn with_policy(mut self, policy: crate::squad::RoutingPolicy) -> Self {
        self.policy = policy;
        self
    }

    pub fn with_member(mut self, agent_id: AgentId) -> Self {
        self.members.push(agent_id);
        self
    }

    pub fn build(self, system: &mut AgentCollaborationSystem) -> SquadId {
        let squad_id = system
            .squad_registry
            .create_squad(self.name, self.leader, self.policy);

        if let Some(squad) = system.squad_registry.get_mut(squad_id) {
            for member in self.members {
                squad.add_member(member);
            }
        }

        squad_id
    }
}

// ---------------------------------------------------------------------------
// Example Usage Documentation Type
// ---------------------------------------------------------------------------

/// Complete example of the collaboration system in action
pub struct CollaborationExample;

impl CollaborationExample {
    /// Run a complete example workflow
    pub fn run_example() {
        println!("=== Agent Collaboration System Example ===\n");

        let mut system = AgentCollaborationSystem::new();
        println!("1. System initialized");

        let leader_id = AgentId(1);
        let member1_id = AgentId(2);
        let member2_id = AgentId(3);

        let squad_id = SquadBuilder::new("GameDev Squad".into(), leader_id)
            .with_policy(crate::squad::RoutingPolicy::SkillBased)
            .with_member(member1_id)
            .with_member(member2_id)
            .build(&mut system);

        println!("2. Squad created: GameDev Squad");

        let (task_id, trace_id) = system.submit_and_track_task(
            squad_id,
            "Create Player Entity".into(),
            "Create a player character with red material and physics".into(),
            vec![crate::registry::CapabilityKind::SceneWrite],
            TaskPriority::High,
        );

        println!("3. Task submitted: Create Player Entity");

        system.record_react_cycle(
            trace_id,
            "Planning the creation".into(),
            "I need to create an entity with mesh, material, and physics components".into(),
            "Spawning entity".into(),
            "PlayerEntity".into(),
            "Entity created successfully".into(),
            true,
            "Verifying scene".into(),
            "Player entity is present with correct components".into(),
        );

        println!("4. ReAct cycle recorded");

        let skill_id = system.complete_task(
            task_id,
            trace_id,
            true,
            vec!["entity".into(), "player".into(), "physics".into()],
        );

        if skill_id.is_some() {
            println!("5. Task completed, new compound skill created!");
        } else {
            println!("5. Task completed (needs more similar tasks for skill compounding)");
        }

        println!("\n=== Example Complete ===");
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_system_creation() {
        let system = AgentCollaborationSystem::new();
        assert!(!system.runtime_manager.registry().list_runtimes().is_empty());
    }

    #[test]
    fn test_squad_builder() {
        let mut system = AgentCollaborationSystem::new();
        let squad_id = SquadBuilder::new("Test Squad".into(), AgentId(1))
            .with_member(AgentId(2))
            .build(&mut system);

        assert!(system.squad_registry.get(squad_id).is_some());
    }

    #[test]
    fn test_task_workflow() {
        let mut system = AgentCollaborationSystem::new();
        let squad_id = SquadBuilder::new("Test".into(), AgentId(1)).build(&mut system);

        let (task_id, trace_id) = system.submit_and_track_task(
            squad_id,
            "Test Task".into(),
            "Test Description".into(),
            vec![],
            TaskPriority::Normal,
        );

        system
            .reasoning_bank
            .record_think(trace_id, "Think".into(), "Reasoning".into());

        system.complete_task(task_id, trace_id, true, vec!["test".into()]);

        let trace = system.reasoning_bank.bank().get_trace(trace_id).unwrap();
        assert!(trace.success);
    }

    #[test]
    fn test_example() {
        CollaborationExample::run_example();
    }
}
