//! Utility AI Decision System for Runtime Agents
//!
//! Provides utility-based decision making for agents with UtilityAi behavior spec.
//! Each action has utility curves evaluated based on context variables.

use agent_core::runtime_agent::{
    RuntimeAgentComponent, RuntimeBehaviorSpec, RuntimeAgentAction, RuntimeAgentStatus
};
use bevy::prelude::*;
use std::collections::HashMap;

/// Component for tracking utility AI evaluation state
#[derive(Component, Debug, Clone)]
pub struct UtilityAiState {
    /// Last evaluation time
    pub last_evaluated: f64,
    /// Evaluation cooldown (seconds)
    pub evaluation_interval: f32,
    /// Current best action and its utility score
    pub current_action: Option<(String, f32)>,
    /// All action scores from last evaluation
    pub action_scores: HashMap<String, f32>,
    /// Context variable cache
    pub context_cache: HashMap<String, f32>,
}

impl Default for UtilityAiState {
    fn default() -> Self {
        Self {
            last_evaluated: 0.0,
            evaluation_interval: 0.5, // Evaluate every 0.5 seconds
            current_action: None,
            action_scores: HashMap::new(),
            context_cache: HashMap::new(),
        }
    }
}

/// System: Execute utility AI behavior for agents
pub fn utility_ai_tick_system(
    mut query: Query<(Entity, &mut RuntimeAgentComponent, &mut UtilityAiState)>,
    time: Res<Time>,
) {
    let current_time = time.elapsed_secs_f64();
    
    for (entity, mut agent, mut utility_state) in query.iter_mut() {
        // Only process agents with UtilityAi behavior
        let actions = match &agent.behavior {
            RuntimeBehaviorSpec::UtilityAi { actions } => actions,
            _ => continue,
        };
        
        if !agent.is_active() {
            continue;
        }
        
        // Check if we should re-evaluate
        let should_evaluate = current_time - utility_state.last_evaluated > utility_state.evaluation_interval as f64;
        
        if !should_evaluate && utility_state.current_action.is_some() {
            // Continue with current action if we're still acting
            if !agent.pending_actions.is_empty() {
                agent.status = RuntimeAgentStatus::Acting;
                continue;
            }
        }
        
        agent.status = RuntimeAgentStatus::Thinking;
        utility_state.last_evaluated = current_time;
        
        // Gather context variables
        gather_context(&mut utility_state, &agent);
        
        // Evaluate all actions
        let mut best_action: Option<(String, f32)> = None;
        utility_state.action_scores.clear();
        
        for action in actions {
            let score = evaluate_utility(&action.utility, &utility_state.context_cache);
            utility_state.action_scores.insert(action.name.clone(), score);
            
            // Track best action
            match &best_action {
                None if score > 0.0 => best_action = Some((action.name.clone(), score)),
                Some((_, best_score)) if score > *best_score => {
                    best_action = Some((action.name.clone(), score));
                }
                _ => {}
            }
        }
        
        // Execute best action if score is above threshold
        if let Some((action_name, score)) = &best_action {
            if *score > 0.3 { // Minimum utility threshold
                log::debug!(
                    "Agent {:?} selected action '{}' with utility {:.2}",
                    entity,
                    action_name,
                    score
                );
                
                // Find action definition and queue it
                if let Some(action_def) = actions.iter().find(|a| &a.name == action_name) {
                    for action in &action_def.actions {
                        agent.pending_actions.push(action.clone());
                    }
                    
                    agent.status = RuntimeAgentStatus::Acting;
                    utility_state.current_action = Some((action_name.clone(), *score));
                }
            } else {
                agent.status = RuntimeAgentStatus::Idle;
                utility_state.current_action = None;
            }
        } else {
            agent.status = RuntimeAgentStatus::Idle;
            utility_state.current_action = None;
        }
    }
}

/// Gather context variables for utility evaluation
fn gather_context(state: &mut UtilityAiState, agent: &RuntimeAgentComponent) {
    state.context_cache.clear();
    
    // Distance to nearest entity
    if let Some(ref obs) = agent.last_observation {
        if let Some(nearest) = obs.visible_entities.first() {
            // Calculate distance (simplified)
            state.context_cache.insert("distance_to_target".to_string(), 10.0); // Placeholder
        } else {
            state.context_cache.insert("distance_to_target".to_string(), 999.0);
        }
        
        state.context_cache.insert(
            "visible_entity_count".to_string(),
            obs.visible_entities.len() as f32,
        );
    } else {
        state.context_cache.insert("distance_to_target".to_string(), 999.0);
        state.context_cache.insert("visible_entity_count".to_string(), 0.0);
    }
    
    // Health/energy from blackboard
    if let Some(health) = agent.blackboard.get("health").and_then(|v| v.as_f64()) {
        state.context_cache.insert("health".to_string(), health as f32);
    } else {
        state.context_cache.insert("health".to_string(), 1.0); // Assume full health
    }
    
    if let Some(energy) = agent.blackboard.get("energy").and_then(|v| v.as_f64()) {
        state.context_cache.insert("energy".to_string(), energy as f32);
    } else {
        state.context_cache.insert("energy".to_string(), 1.0);
    }
    
    // Time since last action
    state.context_cache.insert(
        "time_since_action".to_string(),
        (state.last_evaluated - state.last_evaluated) as f32, // Simplified
    );
}

/// Evaluate utility based on curves
fn evaluate_utility(utility: &agent_core::runtime_agent::UtilityCurve, context: &HashMap<String, f32>) -> f32 {
    if let Some(value) = context.get(&utility.variable) {
        let normalized = (*value - utility.min_value) / (utility.max_value - utility.min_value);
        let clamped = normalized.clamp(0.0, 1.0);
        
        let curve_value = match utility.curve_type {
            agent_core::runtime_agent::UtilityCurveType::Linear => clamped,
            agent_core::runtime_agent::UtilityCurveType::Quadratic => clamped * clamped,
            agent_core::runtime_agent::UtilityCurveType::InverseLinear => 1.0 - clamped,
            agent_core::runtime_agent::UtilityCurveType::Sigmoid => {
                // Sigmoid: 1 / (1 + e^(-10 * (x - 0.5)))
                1.0 / (1.0 + (-10.0 * (clamped - 0.5)).exp())
            }
            agent_core::runtime_agent::UtilityCurveType::Step { threshold } => {
                if clamped >= threshold { 1.0 } else { 0.0 }
            }
        };
        
        curve_value * utility.weight
    } else {
        0.0 // Variable not found, zero utility
    }
}

/// Helper to initialize utility AI for an agent
pub fn init_utility_ai(commands: &mut Commands, entity: Entity) {
    commands.entity(entity).insert(UtilityAiState::default());
}

/// Plugin for utility AI behavior
pub struct UtilityAiPlugin;

impl Plugin for UtilityAiPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, utility_ai_tick_system);
    }
}

// Action definition for utility AI
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UtilityActionDef {
    pub name: String,
    pub utility: agent_core::runtime_agent::UtilityCurve,
    pub actions: Vec<RuntimeAgentAction>,
    pub cooldown: f32,
}

use serde::{Deserialize, Serialize};
