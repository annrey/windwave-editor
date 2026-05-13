//! State Machine Behavior for Runtime Agents
//!
//! Provides finite state machine execution for agents with StateMachine behavior spec.
//! Supports transitions based on conditions, entry/exit actions, and hierarchical states.

use agent_core::runtime_agent::{
    RuntimeBehaviorSpec, RuntimeAgentAction, RuntimeCondition, RuntimeAgentStatus,
};
use bevy::prelude::*;
use std::collections::HashMap;

// Use local RuntimeAgentComponent which is the Bevy Component wrapper
use crate::runtime_agent::RuntimeAgentComponent;

/// Component for tracking state machine execution state
#[derive(Component, Debug, Clone)]
pub struct StateMachineState {
    /// Current state name
    pub current_state: String,
    /// State entry time
    pub state_entered_at: f64,
    /// State-specific data
    pub state_data: HashMap<String, serde_json::Value>,
    /// History for hierarchical states (parent -> previous child)
    pub state_history: HashMap<String, String>,
}

impl StateMachineState {
    pub fn new(initial_state: impl Into<String>) -> Self {
        Self {
            current_state: initial_state.into(),
            state_entered_at: 0.0,
            state_data: HashMap::new(),
            state_history: HashMap::new(),
        }
    }

    pub fn transition_to(&mut self, new_state: impl Into<String>, current_time: f64) {
        let new_state = new_state.into();
        log::debug!("State transition: {} -> {}", self.current_state, new_state);
        
        // Save history for potential return
        self.state_history.insert(
            self.current_state.clone(),
            self.current_state.clone(),
        );
        
        self.current_state = new_state;
        self.state_entered_at = current_time;
    }
}

/// System: Execute state machine behavior for agents
pub fn state_machine_tick_system(
    mut query: Query<(Entity, &mut RuntimeAgentComponent, &mut StateMachineState)>,
    time: Res<Time>,
) {
    let current_time = time.elapsed_secs_f64();
    
    for (entity, mut agent, mut state_machine) in query.iter_mut() {
        // Only process agents with StateMachine behavior
        let states = match &agent.behavior {
            RuntimeBehaviorSpec::StateMachine { states, .. } => states,
            _ => continue,
        };
        
        if !agent.is_active() {
            continue;
        }
        
        agent.status = RuntimeAgentStatus::Thinking;
        
        // Find current state definition
        let current_state_def = states.iter().find(|s| s.name == state_machine.current_state);
        
        // Execute state actions
        if let Some(state) = current_state_def {
            // Check if we just entered this state
            let just_entered = current_time - state_machine.state_entered_at < 0.001;
            
            // Execute state actions
            for action in &state.actions {
                agent.pending_actions.push(action.clone());
            }
            
            // Check transitions from the current state
            for transition in &state.transitions {
                if evaluate_condition(&transition.condition, &agent, entity) {
                    // Transition
                    state_machine.transition_to(&transition.to, current_time);
                    
                    // Set status to acting if we have actions
                    if !agent.pending_actions.is_empty() {
                        agent.status = RuntimeAgentStatus::Acting;
                    }
                    break;
                }
            }
        } else {
            log::warn!(
                "Agent {:?} is in undefined state: {}",
                entity,
                state_machine.current_state
            );
        }
        
        // Set idle if no actions pending
        if agent.pending_actions.is_empty() {
            agent.status = RuntimeAgentStatus::Idle;
        }
    }
}

/// Evaluate a runtime condition
fn evaluate_condition(
    condition: &RuntimeCondition,
    agent: &RuntimeAgentComponent,
    entity: Entity,
) -> bool {
    match condition {
        RuntimeCondition::Always => true,
        RuntimeCondition::BlackboardEquals { key, value } => {
            agent.blackboard.get(key).map_or(false, |v| v == value)
        }
        RuntimeCondition::BlackboardExists { key } => {
            agent.blackboard.get(key).is_some()
        }
        RuntimeCondition::DistanceToTargetLessThan { target_key, distance } => {
            // Check blackboard for target position
            if let Some(target_value) = agent.blackboard.get(target_key) {
                // Parse target position and compare with agent position
                // This is simplified - in production you'd get actual transform
                true // Placeholder
            } else {
                false
            }
        }
        RuntimeCondition::EventReceived { event_type } => {
            // Check last observation for event
            agent.last_observation.as_ref().map_or(false, |obs| {
                obs.events.iter().any(|e| &e.event_type == event_type)
            })
        }
    }
}

/// Helper to initialize state machine for an agent
pub fn init_state_machine(
    commands: &mut Commands,
    entity: Entity,
    behavior: &RuntimeBehaviorSpec,
) {
    if let RuntimeBehaviorSpec::StateMachine { initial_state, .. } = behavior {
        commands.entity(entity).insert(StateMachineState::new(initial_state));
    }
}

/// Plugin for state machine behavior
pub struct StateMachinePlugin;

impl Plugin for StateMachinePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, state_machine_tick_system);
    }
}

