//! Runtime Agent Integration for Bevy ECS
//!
//! Bridges agent_core::runtime_agent types to Bevy Components and Systems,
//! allowing game entities to run as autonomous AI agents.

use agent_core::runtime_agent::*;
use agent_core::EntityId;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

/// Global counter for generating unique agent IDs
static AGENT_ID_COUNTER: AtomicU64 = AtomicU64::new(1);

fn generate_agent_id(prefix: &str) -> String {
    let id = AGENT_ID_COUNTER.fetch_add(1, Ordering::SeqCst);
    format!("{}_{}", prefix, id)
}

// Re-export core types for convenience
pub use agent_core::runtime_agent::{
    RuntimeAgentId, RuntimeAgentProfileId, RuntimeAgentControlMode,
    RuntimeAgentStatus, RuntimeAgentAction, RuntimeTarget,
    RuntimeGoal, RuntimeBlackboard, RuntimeObservation,
    RuntimeBehaviorSpec, EditorAgentControlCommand,
};

/// Bevy Component wrapper for runtime agent data
#[derive(Component, Clone, Debug, Serialize, Deserialize)]
pub struct RuntimeAgentComponent {
    pub id: RuntimeAgentId,
    pub profile_id: RuntimeAgentProfileId,
    pub control_mode: RuntimeAgentControlMode,
    pub status: RuntimeAgentStatus,
    pub tick_enabled: bool,
    /// Agent behavior specification (cached from profile)
    pub behavior: RuntimeBehaviorSpec,
    /// Agent's internal memory/blackboard
    pub blackboard: RuntimeBlackboard,
    /// Currently active goal (if any)
    pub active_goal: Option<RuntimeGoal>,
    /// Last observation snapshot
    pub last_observation: Option<RuntimeObservation>,
    /// Actions pending execution from last tick
    #[serde(skip)]
    pub pending_actions: Vec<RuntimeAgentAction>,
}

impl RuntimeAgentComponent {
    pub fn new(id: impl Into<String>, profile_id: impl Into<String>) -> Self {
        Self {
            id: RuntimeAgentId(id.into()),
            profile_id: RuntimeAgentProfileId(profile_id.into()),
            control_mode: RuntimeAgentControlMode::Autonomous,
            status: RuntimeAgentStatus::Idle,
            tick_enabled: true,
            behavior: RuntimeBehaviorSpec::Scripted { 
                script_name: "default".to_string() 
            },
            blackboard: RuntimeBlackboard::default(),
            active_goal: None,
            last_observation: None,
            pending_actions: Vec::new(),
        }
    }

    pub fn with_behavior(mut self, behavior: RuntimeBehaviorSpec) -> Self {
        self.behavior = behavior;
        self
    }

    pub fn with_control_mode(mut self, mode: RuntimeAgentControlMode) -> Self {
        self.control_mode = mode;
        self
    }

    pub fn is_active(&self) -> bool {
        self.tick_enabled && !matches!(self.control_mode, RuntimeAgentControlMode::Disabled)
    }

    pub fn can_act(&self) -> bool {
        self.is_active() && matches!(self.status, RuntimeAgentStatus::Idle | RuntimeAgentStatus::Thinking)
    }
}

/// Resource storing runtime agent profiles and instance mappings
#[derive(Resource, Default, Debug)]
pub struct RuntimeAgentRegistry {
    profiles: HashMap<RuntimeAgentProfileId, RuntimeAgentProfile>,
    entity_to_agent: HashMap<Entity, RuntimeAgentId>,
    agent_to_entity: HashMap<RuntimeAgentId, Entity>,
}

impl RuntimeAgentRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_profile(&mut self, profile: RuntimeAgentProfile) {
        self.profiles.insert(profile.id.clone(), profile);
    }

    pub fn get_profile(&self, id: &RuntimeAgentProfileId) -> Option<&RuntimeAgentProfile> {
        self.profiles.get(id)
    }

    pub fn bind_entity(&mut self, entity: Entity, agent_id: RuntimeAgentId) {
        self.entity_to_agent.insert(entity, agent_id.clone());
        self.agent_to_entity.insert(agent_id, entity);
    }

    pub fn unbind_entity(&mut self, entity: Entity) {
        if let Some(agent_id) = self.entity_to_agent.remove(&entity) {
            self.agent_to_entity.remove(&agent_id);
        }
    }

    pub fn get_agent_id(&self, entity: Entity) -> Option<&RuntimeAgentId> {
        self.entity_to_agent.get(&entity)
    }

    pub fn get_entity(&self, agent_id: &RuntimeAgentId) -> Option<Entity> {
        self.agent_to_entity.get(agent_id).copied()
    }

    pub fn list_active_agents(&self) -> Vec<&RuntimeAgentId> {
        self.entity_to_agent.values().collect()
    }
}

/// Plugin for runtime agent systems
pub struct RuntimeAgentPlugin;

impl Plugin for RuntimeAgentPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<RuntimeAgentRegistry>()
            .add_systems(Update, (
                runtime_agent_tick_system,
                sync_agent_entities,
                execute_pending_actions,
            ).chain());
    }
}

/// System: Tick all active runtime agents
fn runtime_agent_tick_system(
    mut query: Query<(Entity, &mut RuntimeAgentComponent)>,
    registry: Res<RuntimeAgentRegistry>,
    time: Res<Time>,
) {
    for (_entity, mut agent) in query.iter_mut() {
        if !agent.is_active() {
            continue;
        }

        // Only tick at intervals (e.g., 10Hz)
        let _delta = time.delta_secs();

        // Update status based on control mode
        match &agent.control_mode {
            RuntimeAgentControlMode::Disabled => {
                agent.status = RuntimeAgentStatus::Suspended;
                continue;
            }
            RuntimeAgentControlMode::EditorControlled { .. } => {
                // In editor control mode, agent waits for commands
                agent.status = RuntimeAgentStatus::Waiting;
                continue;
            }
            _ => {}
        }

        // Get profile for behavior evaluation
        if let Some(profile) = registry.get_profile(&agent.profile_id) {
            agent.status = RuntimeAgentStatus::Thinking;

            // Evaluate behavior based on spec
            let actions = evaluate_behavior(&agent, profile);

            if !actions.is_empty() {
                agent.status = RuntimeAgentStatus::Acting;
                agent.pending_actions = actions;
            } else {
                agent.status = RuntimeAgentStatus::Idle;
            }
        }
    }
}

/// Evaluate agent behavior based on profile spec
fn evaluate_behavior(
    agent: &RuntimeAgentComponent,
    profile: &RuntimeAgentProfile,
) -> Vec<RuntimeAgentAction> {
    match &profile.behavior {
        RuntimeBehaviorSpec::StateMachine { states, initial_state } => {
            // Simple state machine: find current state and execute its actions
            let current_state = agent.blackboard.get("current_state")
                .and_then(|v| v.as_str())
                .unwrap_or(initial_state);

            states.iter()
                .find(|s| s.name == current_state)
                .map(|s| s.actions.clone())
                .unwrap_or_default()
        }
        RuntimeBehaviorSpec::UtilityAI { considerations } => {
            // Simple utility: pick highest weight consideration
            considerations.iter()
                .max_by(|a, b| a.weight.partial_cmp(&b.weight).unwrap_or(std::cmp::Ordering::Equal))
                .map(|c| vec![c.action.clone()])
                .unwrap_or_default()
        }
        RuntimeBehaviorSpec::Scripted { script_name } => {
            vec![RuntimeAgentAction::EmitEvent {
                event_type: "script_tick".to_string(),
                payload: serde_json::json!({ "script": script_name }),
            }]
        }
        RuntimeBehaviorSpec::LlmDriven { system_prompt, tool_allowlist } => {
            vec![RuntimeAgentAction::EmitEvent {
                event_type: "llm_tick".to_string(),
                payload: serde_json::json!({
                    "prompt": system_prompt,
                    "tools": tool_allowlist,
                    "entity_id": agent.id.0,
                }),
            }]
        }
    }
}

/// System: Sync agent registry with entities
fn sync_agent_entities(
    mut registry: ResMut<RuntimeAgentRegistry>,
    query: Query<(Entity, &RuntimeAgentComponent), Changed<RuntimeAgentComponent>>,
) {
    // Add new bindings
    for (entity, agent) in query.iter() {
        if registry.get_agent_id(entity) != Some(&agent.id) {
            registry.bind_entity(entity, agent.id.clone());
        }
    }
}

/// System: Execute pending actions from agents
fn execute_pending_actions(
    mut query: Query<(Entity, &mut RuntimeAgentComponent, &mut Transform)>,
    mut commands: Commands,
) {
    for (entity, mut agent, mut transform) in query.iter_mut() {
        if agent.pending_actions.is_empty() {
            continue;
        }

        let actions: Vec<RuntimeAgentAction> = agent.pending_actions.drain(..).collect();
        for action in actions {
            execute_action(action, entity, &mut agent, &mut transform, &mut commands);
        }

        // Reset status after actions executed
        if agent.pending_actions.is_empty() {
            agent.status = RuntimeAgentStatus::Idle;
        }
    }
}

/// Execute a single runtime agent action
fn execute_action(
    action: RuntimeAgentAction,
    _entity: Entity,
    agent: &mut RuntimeAgentComponent,
    transform: &mut Transform,
    _commands: &mut Commands,
) {
    match action {
        RuntimeAgentAction::Noop => {}
        RuntimeAgentAction::MoveTo { target } => {
            let target_pos = resolve_target(target, transform);
            // Simple move: update position towards target
            let direction = target_pos - transform.translation;
            if direction.length() > 0.1 {
                transform.translation += direction.normalize() * 0.5;
                agent.blackboard.set("last_move_direction", serde_json::json!([direction.x, direction.y, direction.z]));
            }
        }
        RuntimeAgentAction::LookAt { target } => {
            let target_pos = resolve_target(target, transform);
            let direction = target_pos - transform.translation;
            if direction.length() > 0.001 {
                let angle = direction.y.atan2(direction.x);
                transform.rotation = Quat::from_rotation_z(angle);
            }
        }
        RuntimeAgentAction::SetVelocity { velocity } => {
            agent.blackboard.set("velocity", serde_json::json!(velocity));
        }
        RuntimeAgentAction::PlayAnimation { name } => {
            agent.blackboard.set("animation", serde_json::json!(name));
        }
        RuntimeAgentAction::SpawnPrefab { prefab_id, at } => {
            let spawn_pos = resolve_target(at, transform);
            agent.blackboard.set("spawned_prefab", serde_json::json!({
                "prefab_id": prefab_id,
                "position": [spawn_pos.x, spawn_pos.y, spawn_pos.z],
            }));
            // Note: Actual prefab spawning would require asset server access
        }
        RuntimeAgentAction::ModifyOwnComponent { component_type, property, value } => {
            agent.blackboard.set(
                &format!("component_override_{}_{}", component_type, property),
                value,
            );
        }
        RuntimeAgentAction::EmitEvent { event_type, payload } => {
            // Store event in blackboard for now
            agent.blackboard.set(&format!("event_{}", event_type), payload);
        }
        RuntimeAgentAction::RequestEditorCommand { command } => {
            // Queue command for editor processing
            agent.blackboard.set(
                "pending_editor_command",
                serde_json::to_value(command).unwrap_or_default(),
            );
        }
    }
}

/// Resolve a RuntimeTarget to a world position
fn resolve_target(target: RuntimeTarget, current_transform: &Transform) -> Vec3 {
    match target {
        RuntimeTarget::SelfEntity => current_transform.translation,
        RuntimeTarget::Position(pos) => Vec3::new(pos[0], pos[1], pos[2]),
        RuntimeTarget::BlackboardKey(_key) => {
            // Would need access to blackboard to resolve
            // For now, return current position
            current_transform.translation
        }
        _ => current_transform.translation,
    }
}

/// Convert RuntimeAgentAction to EngineCommand for editor-side execution
pub fn runtime_action_to_engine_command(
    action: &RuntimeAgentAction,
    entity_id: EntityId,
) -> Option<crate::EngineCommand> {
    match action {
        RuntimeAgentAction::MoveTo { target } => {
            if let RuntimeTarget::Position(pos) = target {
                Some(crate::EngineCommand::SetTransform {
                    entity_id: entity_id.0,
                    translation: Some([pos[0], pos[1], pos[2]]),
                    rotation: None,
                    scale: None,
                })
            } else {
                None
            }
        }
        RuntimeAgentAction::ModifyOwnComponent { component_type, property, value } => {
            Some(crate::EngineCommand::ModifyComponent {
                entity_id: entity_id.0,
                component_type: component_type.clone(),
                property: property.clone(),
                value: value.clone(),
            })
        }
        _ => None,
    }
}

/// Helper to attach a runtime agent to an entity
pub fn attach_runtime_agent(
    commands: &mut Commands,
    entity: Entity,
    agent_component: RuntimeAgentComponent,
) {
    commands.entity(entity).insert(agent_component);
}

/// Helper to detach a runtime agent from an entity
pub fn detach_runtime_agent(
    commands: &mut Commands,
    entity: Entity,
) {
    commands.entity(entity).remove::<RuntimeAgentComponent>();
}

/// Process editor control commands
pub fn process_editor_control_command(
    command: EditorAgentControlCommand,
    _registry: &mut RuntimeAgentRegistry,
    world: &mut World,
) -> Result<(), String> {
    match command {
        EditorAgentControlCommand::AttachRuntimeAgent { entity_id, component: _ } => {
            // Note: entity_id here is agent_core::EntityId, need to find or create Bevy Entity
            // For now, this is a placeholder - actual implementation needs entity mapping
            log::info!("Attach runtime agent requested for entity {:?}", entity_id);
            Ok(())
        }
        EditorAgentControlCommand::DetachRuntimeAgent { entity_id } => {
            log::info!("Detach runtime agent requested for entity {:?}", entity_id);
            Ok(())
        }
        EditorAgentControlCommand::SetControlMode { entity_id, mode } => {
            if let Some(entity) = find_entity_by_agent_id(world, &RuntimeAgentId(entity_id.0.to_string())) {
                if let Some(mut agent) = world.get_mut::<RuntimeAgentComponent>(entity) {
                    agent.control_mode = mode;
                }
            }
            Ok(())
        }
        EditorAgentControlCommand::SetRuntimeGoal { entity_id, goal } => {
            if let Some(entity) = find_entity_by_agent_id(world, &RuntimeAgentId(entity_id.0.to_string())) {
                if let Some(mut agent) = world.get_mut::<RuntimeAgentComponent>(entity) {
                    agent.active_goal = Some(goal);
                }
            }
            Ok(())
        }
        EditorAgentControlCommand::SetBlackboardValue { entity_id, key, value } => {
            if let Some(entity) = find_entity_by_agent_id(world, &RuntimeAgentId(entity_id.0.to_string())) {
                if let Some(mut agent) = world.get_mut::<RuntimeAgentComponent>(entity) {
                    agent.blackboard.set(key, value);
                }
            }
            Ok(())
        }
        EditorAgentControlCommand::SendRuntimeEvent { entity_id, event } => {
            if let Some(entity) = find_entity_by_agent_id(world, &RuntimeAgentId(entity_id.0.to_string())) {
                if let Some(mut agent) = world.get_mut::<RuntimeAgentComponent>(entity) {
                    let mut observation = agent.last_observation.clone().unwrap_or(RuntimeObservation {
                        entity_id,
                        visible_entities: Vec::new(),
                        nearby_prefab_instances: Vec::new(),
                        events: Vec::new(),
                        facts: std::collections::HashMap::new(),
                    });
                    observation.events.push(event);
                    agent.last_observation = Some(observation);
                }
            }
            Ok(())
        }
        EditorAgentControlCommand::ExecuteRuntimeAction { entity_id, action } => {
            if let Some(entity) = find_entity_by_agent_id(world, &RuntimeAgentId(entity_id.0.to_string())) {
                if let Some(mut agent) = world.get_mut::<RuntimeAgentComponent>(entity) {
                    agent.pending_actions.push(action);
                    agent.status = RuntimeAgentStatus::Acting;
                }
            }
            Ok(())
        }
    }
}

/// Find Bevy Entity by RuntimeAgentId
fn find_entity_by_agent_id(world: &mut World, agent_id: &RuntimeAgentId) -> Option<Entity> {
    let mut query = world.query::<(Entity, &RuntimeAgentComponent)>();
    for (entity, agent) in query.iter(world) {
        if &agent.id == agent_id {
            return Some(entity);
        }
    }
    None
}

/// Spawn a new entity with a runtime agent attached
pub fn spawn_runtime_agent_entity(
    commands: &mut Commands,
    name: &str,
    position: Vec3,
    profile_id: &str,
    control_mode: RuntimeAgentControlMode,
) -> Entity {
    commands.spawn((
        Name::new(name.to_string()),
        Transform::from_translation(position),
        GlobalTransform::default(),
        Visibility::default(),
        crate::AgentTracked,
        Sprite::default(),
        RuntimeAgentComponent::new(
            generate_agent_id(name),
            profile_id,
        ).with_control_mode(control_mode),
    )).id()
}
