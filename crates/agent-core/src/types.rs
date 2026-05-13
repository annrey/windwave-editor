//! Core types shared across the Agent system

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Unique identifier for entities in the editor
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct EntityId(pub u64);

/// Unique identifier for Agents
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub struct AgentId(pub u64);

/// Types of messages in the Agent conversation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageType {
    User,
    Agent,
    System,
    Action,
    Thought,
    Observation,
}

/// A single message in the Agent conversation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    pub message_type: MessageType,
    pub content: String,
    pub timestamp: u64,
    pub metadata: HashMap<String, String>,
}

impl Message {
    pub fn new_agent(content: impl Into<String>) -> Self {
        Self {
            id: generate_id(),
            message_type: MessageType::Agent,
            content: content.into(),
            timestamp: current_timestamp(),
            metadata: HashMap::new(),
        }
    }

    pub fn new_user(content: impl Into<String>) -> Self {
        Self {
            id: generate_id(),
            message_type: MessageType::User,
            content: content.into(),
            timestamp: current_timestamp(),
            metadata: HashMap::new(),
        }
    }

    pub fn thought(content: impl Into<String>) -> Self {
        Self {
            id: generate_id(),
            message_type: MessageType::Thought,
            content: content.into(),
            timestamp: current_timestamp(),
            metadata: HashMap::new(),
        }
    }

    pub fn action(content: impl Into<String>) -> Self {
        Self {
            id: generate_id(),
            message_type: MessageType::Action,
            content: content.into(),
            timestamp: current_timestamp(),
            metadata: HashMap::new(),
        }
    }

    pub fn observation(content: impl Into<String>) -> Self {
        Self {
            id: generate_id(),
            message_type: MessageType::Observation,
            content: content.into(),
            timestamp: current_timestamp(),
            metadata: HashMap::new(),
        }
    }
}

/// Context tag for Agent understanding (e.g., @Player, #Physics)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextTag {
    pub tag_type: TagType,
    pub value: String,
    pub active: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TagType {
    Entity,    // @Player
    Topic,     // #Physics
    Command,   // /save
    Urgent,    // !urgent
}

/// Agent capability descriptor
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentCapability {
    pub name: String,
    pub description: String,
    pub supported_engines: Vec<String>,
}

/// Core Agent identity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentIdentity {
    pub name: String,
    pub version: String,
    pub role: String,
    pub status: AgentStatus,
    pub capabilities: Vec<AgentCapability>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AgentStatus {
    Online,
    Thinking,
    Offline,
    Error,
}

/// Entity information returned from engine query
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EntityInfo {
    pub id: EntityId,
    pub name: String,
    pub entity_type: String,
    pub components: Vec<ComponentInfo>,
    pub children: Vec<EntityId>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentInfo {
    pub name: String,
    pub properties: HashMap<String, PropertyValue>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum PropertyValue {
    Float(f64),
    Int(i64),
    Bool(bool),
    String(String),
    Vec2 { x: f32, y: f32 },
    Vec3 { x: f32, y: f32, z: f32 },
    Color { r: f32, g: f32, b: f32, a: f32 },
}

/// Action the Agent wants to perform
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AgentAction {
    CreateComponent {
        entity_id: EntityId,
        component_type: String,
        properties: HashMap<String, PropertyValue>,
    },
    UpdateComponent {
        entity_id: EntityId,
        component_name: String,
        property: String,
        value: PropertyValue,
    },
    DeleteComponent {
        entity_id: EntityId,
        component_name: String,
    },
    GenerateCode {
        template: String,
        context: HashMap<String, String>,
    },
    ExecuteCommand {
        command: String,
        args: Vec<String>,
    },
}

/// Result of an action execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionResult {
    pub success: bool,
    pub message: String,
    pub data: Option<serde_json::Value>,
}

/// Adapter error types
#[derive(Debug, thiserror::Error)]
pub enum AdapterError {
    #[error("Entity not found: {0:?}")]
    EntityNotFound(EntityId),
    #[error("Component not found: {0}")]
    ComponentNotFound(String),
    #[error("Engine not connected")]
    EngineNotConnected,
    #[error("Action not supported: {0}")]
    ActionNotSupported(String),
    #[error("Invalid property value: {0}")]
    InvalidProperty(String),
}

/// Agent error types
#[derive(Debug, thiserror::Error)]
pub enum AgentError {
    #[error("Max steps reached")]
    MaxStepsReached,
    #[error("Agent is stuck: {0}")]
    Stuck(String),
    #[error("LLM unavailable")]
    LLMUnavailable,
    #[error("Tool error: {0}")]
    ToolError(String),
    #[error("Execution timeout")]
    Timeout,
    #[error("User cancelled")]
    UserCancelled,
}

/// User request structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserRequest {
    pub content: String,
    pub entity_refs: Vec<String>,
    pub estimated_steps: usize,
}

/// Execution result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    pub success: bool,
    pub message: String,
    pub actions: Vec<AgentAction>,
}

// Helper functions
use std::sync::atomic::{AtomicU64, Ordering};

static ID_COUNTER: AtomicU64 = AtomicU64::new(0);

fn generate_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let seq = ID_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("msg_{}_{}", ts, seq)
}

pub fn current_timestamp() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

pub fn now_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
