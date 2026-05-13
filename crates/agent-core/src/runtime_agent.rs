use crate::bevy_editor_model::{BevyEditorCommand, PrefabInstanceId};
use crate::registry::{AgentId, CapabilityKind};
use crate::types::EntityId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RuntimeAgentId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RuntimeAgentProfileId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RuntimeAgentControlMode {
    Disabled,
    Manual,
    Assisted,
    Autonomous,
    EditorControlled { controller: AgentId },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RuntimeAgentStatus {
    Idle,
    Thinking,
    Acting,
    Waiting,
    Suspended,
    Error { message: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeAgentComponent {
    pub id: RuntimeAgentId,
    pub profile_id: RuntimeAgentProfileId,
    pub control_mode: RuntimeAgentControlMode,
    pub status: RuntimeAgentStatus,
    pub tick_enabled: bool,
}

impl RuntimeAgentComponent {
    pub fn new(id: impl Into<String>, profile_id: impl Into<String>) -> Self {
        Self {
            id: RuntimeAgentId(id.into()),
            profile_id: RuntimeAgentProfileId(profile_id.into()),
            control_mode: RuntimeAgentControlMode::Autonomous,
            status: RuntimeAgentStatus::Idle,
            tick_enabled: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeAgentProfile {
    pub id: RuntimeAgentProfileId,
    pub name: String,
    pub description: String,
    pub capabilities: Vec<CapabilityKind>,
    pub default_goal: Option<RuntimeGoal>,
    pub behavior: RuntimeBehaviorSpec,
    pub memory_policy: RuntimeMemoryPolicy,
}

impl RuntimeAgentProfile {
    pub fn new(id: impl Into<String>, name: impl Into<String>, behavior: RuntimeBehaviorSpec) -> Self {
        Self {
            id: RuntimeAgentProfileId(id.into()),
            name: name.into(),
            description: String::new(),
            capabilities: vec![CapabilityKind::SceneRead, CapabilityKind::SceneWrite],
            default_goal: None,
            behavior,
            memory_policy: RuntimeMemoryPolicy::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RuntimeBehaviorSpec {
    StateMachine { states: Vec<RuntimeBehaviorState>, initial_state: String },
    UtilityAI { considerations: Vec<RuntimeConsideration> },
    Scripted { script_name: String },
    LlmDriven { system_prompt: String, tool_allowlist: Vec<String> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeBehaviorState {
    pub name: String,
    pub actions: Vec<RuntimeAgentAction>,
    pub transitions: Vec<RuntimeBehaviorTransition>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeBehaviorTransition {
    pub to: String,
    pub condition: RuntimeCondition,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeConsideration {
    pub name: String,
    pub weight: f32,
    pub action: RuntimeAgentAction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RuntimeCondition {
    Always,
    BlackboardEquals { key: String, value: serde_json::Value },
    BlackboardExists { key: String },
    DistanceToTargetLessThan { target_key: String, distance: f32 },
    EventReceived { event_type: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RuntimeAgentAction {
    Noop,
    MoveTo { target: RuntimeTarget },
    LookAt { target: RuntimeTarget },
    SetVelocity { velocity: [f32; 3] },
    PlayAnimation { name: String },
    SpawnPrefab { prefab_id: String, at: RuntimeTarget },
    ModifyOwnComponent { component_type: String, property: String, value: serde_json::Value },
    EmitEvent { event_type: String, payload: serde_json::Value },
    RequestEditorCommand { command: BevyEditorCommand },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RuntimeTarget {
    SelfEntity,
    Entity(EntityId),
    PrefabInstance(PrefabInstanceId),
    Position([f32; 3]),
    BlackboardKey(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeGoal {
    pub description: String,
    pub success_conditions: Vec<RuntimeCondition>,
    pub priority: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeMemoryPolicy {
    pub remember_events: bool,
    pub max_blackboard_entries: usize,
    pub persist_between_sessions: bool,
}

impl Default for RuntimeMemoryPolicy {
    fn default() -> Self {
        Self {
            remember_events: true,
            max_blackboard_entries: 128,
            persist_between_sessions: false,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RuntimeBlackboard {
    values: HashMap<String, serde_json::Value>,
}

impl RuntimeBlackboard {
    pub fn set(&mut self, key: impl Into<String>, value: serde_json::Value) {
        self.values.insert(key.into(), value);
    }

    pub fn get(&self, key: &str) -> Option<&serde_json::Value> {
        self.values.get(key)
    }

    pub fn remove(&mut self, key: &str) -> Option<serde_json::Value> {
        self.values.remove(key)
    }

    pub fn snapshot(&self) -> &HashMap<String, serde_json::Value> {
        &self.values
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeAgentInstance {
    pub entity_id: EntityId,
    pub component: RuntimeAgentComponent,
    pub blackboard: RuntimeBlackboard,
    pub active_goal: Option<RuntimeGoal>,
    pub last_observation: Option<RuntimeObservation>,
}

impl RuntimeAgentInstance {
    pub fn new(entity_id: EntityId, component: RuntimeAgentComponent) -> Self {
        Self {
            entity_id,
            component,
            blackboard: RuntimeBlackboard::default(),
            active_goal: None,
            last_observation: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeObservation {
    pub entity_id: EntityId,
    pub visible_entities: Vec<EntityId>,
    pub nearby_prefab_instances: Vec<PrefabInstanceId>,
    pub events: Vec<RuntimeAgentEvent>,
    pub facts: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeAgentEvent {
    pub event_type: String,
    pub source_entity: Option<EntityId>,
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EditorAgentControlCommand {
    AttachRuntimeAgent {
        entity_id: EntityId,
        component: RuntimeAgentComponent,
    },
    DetachRuntimeAgent {
        entity_id: EntityId,
    },
    SetControlMode {
        entity_id: EntityId,
        mode: RuntimeAgentControlMode,
    },
    SetRuntimeGoal {
        entity_id: EntityId,
        goal: RuntimeGoal,
    },
    SetBlackboardValue {
        entity_id: EntityId,
        key: String,
        value: serde_json::Value,
    },
    SendRuntimeEvent {
        entity_id: EntityId,
        event: RuntimeAgentEvent,
    },
    ExecuteRuntimeAction {
        entity_id: EntityId,
        action: RuntimeAgentAction,
    },
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RuntimeAgentRegistry {
    profiles: HashMap<RuntimeAgentProfileId, RuntimeAgentProfile>,
    instances: HashMap<EntityId, RuntimeAgentInstance>,
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

    pub fn attach_instance(&mut self, instance: RuntimeAgentInstance) {
        self.instances.insert(instance.entity_id, instance);
    }

    pub fn detach_instance(&mut self, entity_id: EntityId) -> Option<RuntimeAgentInstance> {
        self.instances.remove(&entity_id)
    }

    pub fn get_instance(&self, entity_id: EntityId) -> Option<&RuntimeAgentInstance> {
        self.instances.get(&entity_id)
    }

    pub fn get_instance_mut(&mut self, entity_id: EntityId) -> Option<&mut RuntimeAgentInstance> {
        self.instances.get_mut(&entity_id)
    }

    pub fn list_instances(&self) -> Vec<&RuntimeAgentInstance> {
        self.instances.values().collect()
    }

    pub fn apply_editor_control(&mut self, command: EditorAgentControlCommand) -> Result<(), String> {
        match command {
            EditorAgentControlCommand::AttachRuntimeAgent { entity_id, component } => {
                self.attach_instance(RuntimeAgentInstance::new(entity_id, component));
                Ok(())
            }
            EditorAgentControlCommand::DetachRuntimeAgent { entity_id } => {
                self.detach_instance(entity_id);
                Ok(())
            }
            EditorAgentControlCommand::SetControlMode { entity_id, mode } => {
                let instance = self.instances.get_mut(&entity_id).ok_or_else(|| format!("runtime agent not found for entity {:?}", entity_id))?;
                instance.component.control_mode = mode;
                Ok(())
            }
            EditorAgentControlCommand::SetRuntimeGoal { entity_id, goal } => {
                let instance = self.instances.get_mut(&entity_id).ok_or_else(|| format!("runtime agent not found for entity {:?}", entity_id))?;
                instance.active_goal = Some(goal);
                Ok(())
            }
            EditorAgentControlCommand::SetBlackboardValue { entity_id, key, value } => {
                let instance = self.instances.get_mut(&entity_id).ok_or_else(|| format!("runtime agent not found for entity {:?}", entity_id))?;
                instance.blackboard.set(key, value);
                Ok(())
            }
            EditorAgentControlCommand::SendRuntimeEvent { entity_id, event } => {
                let instance = self.instances.get_mut(&entity_id).ok_or_else(|| format!("runtime agent not found for entity {:?}", entity_id))?;
                let mut observation = instance.last_observation.clone().unwrap_or(RuntimeObservation {
                    entity_id,
                    visible_entities: Vec::new(),
                    nearby_prefab_instances: Vec::new(),
                    events: Vec::new(),
                    facts: HashMap::new(),
                });
                observation.events.push(event);
                instance.last_observation = Some(observation);
                Ok(())
            }
            EditorAgentControlCommand::ExecuteRuntimeAction { entity_id, action } => {
                let instance = self.instances.get_mut(&entity_id).ok_or_else(|| format!("runtime agent not found for entity {:?}", entity_id))?;
                instance.component.status = RuntimeAgentStatus::Acting;
                instance.blackboard.set("last_editor_action", serde_json::to_value(action).unwrap_or(serde_json::Value::Null));
                Ok(())
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeAgentTickResult {
    pub entity_id: EntityId,
    pub status: RuntimeAgentStatus,
    pub requested_actions: Vec<RuntimeAgentAction>,
}

pub fn evaluate_runtime_agent_tick(
    instance: &RuntimeAgentInstance,
    profile: &RuntimeAgentProfile,
) -> RuntimeAgentTickResult {
    if !instance.component.tick_enabled || matches!(instance.component.control_mode, RuntimeAgentControlMode::Disabled) {
        return RuntimeAgentTickResult {
            entity_id: instance.entity_id,
            status: RuntimeAgentStatus::Suspended,
            requested_actions: Vec::new(),
        };
    }

    let requested_actions = match &profile.behavior {
        RuntimeBehaviorSpec::StateMachine { states, initial_state } => states
            .iter()
            .find(|state| &state.name == initial_state)
            .map(|state| state.actions.clone())
            .unwrap_or_default(),
        RuntimeBehaviorSpec::UtilityAI { considerations } => considerations
            .iter()
            .max_by(|a, b| a.weight.partial_cmp(&b.weight).unwrap_or(std::cmp::Ordering::Equal))
            .map(|consideration| vec![consideration.action.clone()])
            .unwrap_or_default(),
        RuntimeBehaviorSpec::Scripted { script_name } => vec![RuntimeAgentAction::EmitEvent {
            event_type: "script_tick".to_string(),
            payload: serde_json::json!({ "script": script_name }),
        }],
        RuntimeBehaviorSpec::LlmDriven { .. } => vec![RuntimeAgentAction::EmitEvent {
            event_type: "llm_tick_requested".to_string(),
            payload: serde_json::json!({ "entity_id": instance.entity_id.0 }),
        }],
    };

    RuntimeAgentTickResult {
        entity_id: instance.entity_id,
        status: RuntimeAgentStatus::Acting,
        requested_actions,
    }
}
