use std::collections::HashMap;

use serde::{Deserialize, Serialize};

// ========================================
// Core Message Envelope
// ========================================

/// Message is the envelope for all WebSocket messages.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    #[serde(rename = "type")]
    pub message_type: String,
    pub payload: serde_json::Value,
}

// ========================================
// Event Types
// ========================================

pub const EVENT_ISSUE_CREATED: &str = "issue:created";
pub const EVENT_ISSUE_UPDATED: &str = "issue:updated";
pub const EVENT_ISSUE_DELETED: &str = "issue:deleted";

pub const EVENT_TASK_QUEUED: &str = "task:queued";
pub const EVENT_TASK_DISPATCH: &str = "task:dispatch";
pub const EVENT_TASK_RUNNING: &str = "task:running";
pub const EVENT_TASK_PROGRESS: &str = "task:progress";
pub const EVENT_TASK_COMPLETED: &str = "task:completed";
pub const EVENT_TASK_FAILED: &str = "task:failed";
pub const EVENT_TASK_MESSAGE: &str = "task:message";
pub const EVENT_TASK_CANCELLED: &str = "task:cancelled";

pub const EVENT_DAEMON_REGISTER: &str = "daemon:register";
pub const EVENT_DAEMON_HEARTBEAT: &str = "daemon:heartbeat";
pub const EVENT_DAEMON_HEARTBEAT_ACK: &str = "daemon:heartbeat_ack";
pub const EVENT_DAEMON_TASK_AVAILABLE: &str = "daemon:task_available";

pub const EVENT_AGENT_STATUS: &str = "agent:status";
pub const EVENT_AGENT_CREATED: &str = "agent:created";

pub const EVENT_SKILL_CREATED: &str = "skill:created";
pub const EVENT_SKILL_UPDATED: &str = "skill:updated";
pub const EVENT_SKILL_DELETED: &str = "skill:deleted";

// ========================================
// Game-Specific Event Types
// ========================================

pub const EVENT_SCENE_CREATED: &str = "scene:created";
pub const EVENT_SCENE_UPDATED: &str = "scene:updated";
pub const EVENT_SCENE_DELETED: &str = "scene:deleted";

pub const EVENT_ENTITY_CREATED: &str = "entity:created";
pub const EVENT_ENTITY_UPDATED: &str = "entity:updated";
pub const EVENT_ENTITY_DELETED: &str = "entity:deleted";

pub const EVENT_ENTITY_MOVED: &str = "entity:moved";
pub const EVENT_ENTITY_COMPONENT_ADDED: &str = "entity:component_added";
pub const EVENT_ENTITY_COMPONENT_UPDATED: &str = "entity:component_updated";
pub const EVENT_ENTITY_COMPONENT_REMOVED: &str = "entity:component_removed";

pub const EVENT_RESOURCE_ADDED: &str = "resource:added";
pub const EVENT_RESOURCE_UPDATED: &str = "resource:updated";
pub const EVENT_RESOURCE_REMOVED: &str = "resource:removed";

// ========================================
// Task Payloads
// ========================================

/// TaskDispatchPayload is sent from server to daemon when a task is assigned.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskDispatchPayload {
    #[serde(rename = "task_id")]
    pub task_id: String,
    #[serde(rename = "issue_id")]
    pub issue_id: String,
    pub title: String,
    pub description: String,
}

/// TaskAvailablePayload is sent from server to daemon as a wakeup hint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskAvailablePayload {
    #[serde(rename = "runtime_id")]
    pub runtime_id: String,
    #[serde(rename = "task_id")]
    pub task_id: Option<String>,
}

/// TaskProgressPayload is sent from daemon to server during task execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskProgressPayload {
    #[serde(rename = "task_id")]
    pub task_id: String,
    pub summary: String,
    pub step: Option<i32>,
    pub total: Option<i32>,
}

/// TaskCompletedPayload is sent from daemon to server when a task finishes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskCompletedPayload {
    #[serde(rename = "task_id")]
    pub task_id: String,
    #[serde(rename = "pr_url")]
    pub pr_url: Option<String>,
    pub output: Option<String>,
}

/// TaskMessagePayload represents a single agent execution message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskMessagePayload {
    #[serde(rename = "task_id")]
    pub task_id: String,
    #[serde(rename = "issue_id")]
    pub issue_id: Option<String>,
    pub seq: i32,
    #[serde(rename = "type")]
    pub message_type: String, // "text", "tool_use", "tool_result", "error"
    pub tool: Option<String>,
    pub content: Option<String>,
    pub input: Option<serde_json::Value>,
    pub output: Option<String>,
}

// ========================================
// Task Lifecycle Types (Multica native task)
// ========================================

/// Task lifecycle status (Multica native task, not Issue)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskLifecycleStatus {
    Created,
    Claimed,
    Executing,
    Completed,
    Failed,
    Cancelled,
}

/// Complete task representation matching Multica's untypedTask
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MulticaTask {
    pub task_id: String,
    pub issue_id: Option<String>,
    pub title: String,
    pub description: Option<String>,
    pub status: TaskLifecycleStatus,
    pub agent_type: Option<String>,
    pub model: Option<String>,
    pub output: Option<String>,
    pub error: Option<String>,
    pub duration_ms: Option<i64>,
    pub created_at: String,
    pub updated_at: String,
}

/// TaskClaimPayload matches Go's claim endpoint response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskClaimPayload {
    #[serde(rename = "runtime_id")]
    pub runtime_id: String,
    pub tasks: Vec<String>,
    #[serde(rename = "daemon_id")]
    pub daemon_id: String,
}

// ========================================
// Daemon Payloads
// ========================================

/// DaemonRegisterPayload is sent from daemon to server on connection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonRegisterPayload {
    #[serde(rename = "daemon_id")]
    pub daemon_id: String,
    #[serde(rename = "agent_id")]
    pub agent_id: String,
    pub runtimes: Vec<RuntimeInfo>,
}

/// RuntimeInfo describes an available agent runtime on the daemon's machine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeInfo {
    #[serde(rename = "type")]
    pub runtime_type: String,
    pub version: String,
    pub status: String,
}

/// DaemonHeartbeatRequestPayload is sent from daemon to server over WebSocket.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonHeartbeatRequestPayload {
    #[serde(rename = "runtime_id")]
    pub runtime_id: String,
    #[serde(rename = "supports_batch_import")]
    pub supports_batch_import: Option<bool>,
}

/// DaemonHeartbeatAckPayload is the server's reply to heartbeat request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonHeartbeatAckPayload {
    #[serde(rename = "runtime_id")]
    pub runtime_id: String,
    pub status: String,
    #[serde(rename = "runtime_gone")]
    pub runtime_gone: Option<bool>,
    #[serde(rename = "pending_update")]
    pub pending_update: Option<PendingUpdate>,
    // ... other pending fields
}

/// Describes a CLI-update action.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingUpdate {
    pub id: String,
    #[serde(rename = "target_version")]
    pub target_version: String,
}

// ========================================
// Issue & Agent Types (for reference)
// ========================================

/// Issue status enum for easier handling
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IssueStatus {
    Backlog,
    Todo,
    InProgress,
    Review,
    Done,
    Blocked,
    Cancelled,
}

/// Agent provider type
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentProvider {
    Claude,
    Codex,
    Copilot,
    OpenClaw,
    OpenCode,
    Hermes,
    Gemini,
    Pi,
    Cursor,
    Kimi,
    Kiro,
    WindWave,
}

// ========================================
// Agent Execution Types
// ========================================

/// Agent execution options (mirrors Go's ExecOptions)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentExecOptions {
    pub cwd: Option<String>,
    pub model: Option<String>,
    #[serde(rename = "system_prompt")]
    pub system_prompt: Option<String>,
    #[serde(rename = "max_turns")]
    pub max_turns: Option<i32>,
    pub timeout_secs: Option<u64>,
    #[serde(rename = "resume_session_id")]
    pub resume_session_id: Option<String>,
    #[serde(rename = "mcp_config")]
    pub mcp_config: Option<serde_json::Value>,
    #[serde(rename = "thinking_level")]
    pub thinking_level: Option<String>,
}

/// Agent execution message type (mirrors Go's MessageType)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AgentMessageType {
    Text,
    Thinking,
    ToolUse,
    ToolResult,
    Status,
    Error,
    Log,
}

/// Agent execution message (mirrors Go's Message)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMessage {
    #[serde(rename = "type")]
    pub message_type: AgentMessageType,
    pub content: Option<String>,
    pub tool: Option<String>,
    #[serde(rename = "call_id")]
    pub call_id: Option<String>,
    pub input: Option<serde_json::Value>,
    pub output: Option<String>,
    pub status: Option<String>,
    pub level: Option<String>,
    #[serde(rename = "session_id")]
    pub session_id: Option<String>,
}

/// Token usage per model (mirrors Go's TokenUsage)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUsage {
    #[serde(rename = "input_tokens")]
    pub input_tokens: i64,
    #[serde(rename = "output_tokens")]
    pub output_tokens: i64,
    #[serde(rename = "cache_read_tokens")]
    pub cache_read_tokens: Option<i64>,
    #[serde(rename = "cache_write_tokens")]
    pub cache_write_tokens: Option<i64>,
}

/// Agent execution result (mirrors Go's Result)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentExecutionResult {
    pub status: String,
    pub output: Option<String>,
    pub error: Option<String>,
    #[serde(rename = "duration_ms")]
    pub duration_ms: i64,
    #[serde(rename = "session_id")]
    pub session_id: Option<String>,
    pub usage: Option<HashMap<String, TokenUsage>>,
}

// ========================================
// Bridge Configuration
// ========================================

/// Bridge configuration
#[derive(Debug, Clone)]
pub struct BridgeConfig {
    pub server_url: String,
    pub workspace_id: String,
    pub api_key: Option<String>,
    pub auto_reconnect: bool,
    pub reconnect_delay: u64,
    pub agent_id: String,
    pub daemon_id: String,
}

impl Default for BridgeConfig {
    fn default() -> Self {
        Self {
            server_url: "ws://localhost:8080".to_string(),
            workspace_id: "default".to_string(),
            api_key: None,
            auto_reconnect: true,
            reconnect_delay: 5000,
            agent_id: uuid::Uuid::new_v4().to_string(),
            daemon_id: uuid::Uuid::new_v4().to_string(),
        }
    }
}

// ========================================
// WindWave Game Types
// ========================================

/// Game scene type - represents a game level/scene
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameScene {
    #[serde(rename = "id")]
    pub id: String,

    #[serde(rename = "workspace_id")]
    pub workspace_id: String,

    #[serde(rename = "name")]
    pub name: String,

    #[serde(rename = "description")]
    pub description: Option<String>,

    #[serde(rename = "parent_scene_id")]
    pub parent_scene_id: Option<String>,

    #[serde(rename = "scene_data")]
    pub scene_data: serde_json::Value,

    #[serde(rename = "scene_template_id")]
    pub scene_template_id: Option<String>,

    #[serde(rename = "tags")]
    pub tags: Vec<String>,

    #[serde(rename = "version")]
    pub version: i32,

    #[serde(rename = "created_at")]
    pub created_at: String,

    #[serde(rename = "updated_at")]
    pub updated_at: String,
}

/// Game entity type - represents a Bevy ECS entity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameEntity {
    #[serde(rename = "id")]
    pub id: String,

    #[serde(rename = "workspace_id")]
    pub workspace_id: String,

    #[serde(rename = "scene_id")]
    pub scene_id: String,

    #[serde(rename = "name")]
    pub name: String,

    #[serde(rename = "parent_entity_id")]
    pub parent_entity_id: Option<String>,

    #[serde(rename = "components")]
    pub components: serde_json::Value,

    #[serde(rename = "prefab_id")]
    pub prefab_id: Option<String>,

    #[serde(rename = "tags")]
    pub tags: Vec<String>,

    #[serde(rename = "created_at")]
    pub created_at: String,

    #[serde(rename = "updated_at")]
    pub updated_at: String,
}

/// Game resource type - models, textures, scripts, etc.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameResource {
    #[serde(rename = "id")]
    pub id: String,

    #[serde(rename = "workspace_id")]
    pub workspace_id: String,

    #[serde(rename = "name")]
    pub name: String,

    #[serde(rename = "resource_type")]
    pub resource_type: String, // "model", "texture", "material", "sound", "script", "prefab"

    #[serde(rename = "path")]
    pub path: String,

    #[serde(rename = "mime_type")]
    pub mime_type: Option<String>,

    #[serde(rename = "file_size")]
    pub file_size: Option<i64>,

    #[serde(rename = "metadata")]
    pub metadata: serde_json::Value,

    #[serde(rename = "tags")]
    pub tags: Vec<String>,

    #[serde(rename = "created_at")]
    pub created_at: String,

    #[serde(rename = "updated_at")]
    pub updated_at: String,
}

/// Task-scene relation - links Multica tasks to game content
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskSceneRelation {
    #[serde(rename = "id")]
    pub id: String,

    #[serde(rename = "workspace_id")]
    pub workspace_id: String,

    #[serde(rename = "issue_id")]
    pub issue_id: String,

    #[serde(rename = "scene_id")]
    pub scene_id: Option<String>,

    #[serde(rename = "entity_id")]
    pub entity_id: Option<String>,

    #[serde(rename = "resource_id")]
    pub resource_id: Option<String>,

    #[serde(rename = "relation_type")]
    pub relation_type: String, // "targets", "references", "modifies", "creates", "deletes"

    #[serde(rename = "snapshot_data")]
    pub snapshot_data: Option<serde_json::Value>,

    #[serde(rename = "created_at")]
    pub created_at: String,
}

/// Agent-scene permissions - fine-grained access control
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentScenePermission {
    #[serde(rename = "id")]
    pub id: String,

    #[serde(rename = "workspace_id")]
    pub workspace_id: String,

    #[serde(rename = "agent_id")]
    pub agent_id: String,

    #[serde(rename = "scene_id")]
    pub scene_id: String,

    #[serde(rename = "permissions")]
    pub permissions: Vec<String>, // "read", "write", "execute", "admin"

    #[serde(rename = "created_at")]
    pub created_at: String,
}

// ========================================
// Task Dispatch with Game Context
// ========================================

/// Extended task dispatch with game context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameTaskDispatchPayload {
    #[serde(rename = "task_id")]
    pub task_id: String,

    #[serde(rename = "issue_id")]
    pub issue_id: String,

    pub title: String,
    pub description: String,

    #[serde(rename = "target_scenes")]
    pub target_scenes: Vec<GameScene>,

    #[serde(rename = "target_entities")]
    pub target_entities: Vec<GameEntity>,

    #[serde(rename = "snapshot")]
    pub snapshot: Option<serde_json::Value>,
}

// ========================================
// Component Operation Types
// ========================================

/// Component added payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityComponentAddedPayload {
    #[serde(rename = "entity_id")]
    pub entity_id: String,

    #[serde(rename = "scene_id")]
    pub scene_id: String,

    #[serde(rename = "component_type")]
    pub component_type: String,

    #[serde(rename = "component_data")]
    pub component_data: serde_json::Value,
}

/// Component updated payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityComponentUpdatedPayload {
    #[serde(rename = "entity_id")]
    pub entity_id: String,

    #[serde(rename = "scene_id")]
    pub scene_id: String,

    #[serde(rename = "component_type")]
    pub component_type: String,

    #[serde(rename = "old_data")]
    pub old_data: serde_json::Value,

    #[serde(rename = "new_data")]
    pub new_data: serde_json::Value,
}

/// Component removed payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityComponentRemovedPayload {
    #[serde(rename = "entity_id")]
    pub entity_id: String,

    #[serde(rename = "scene_id")]
    pub scene_id: String,

    #[serde(rename = "component_type")]
    pub component_type: String,
}

// ========================================
// Skill System Types
// ========================================

/// Skill definition matching Multica's skill registry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillDefinition {
    pub name: String,
    pub description: String,
    pub version: String,
    pub parameters: Vec<SkillParameter>,
    pub category: Option<String>,
    pub tags: Vec<String>,
    #[serde(rename = "is_active")]
    pub is_active: bool,
}

/// Skill parameter types
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkillParameterType {
    String,
    Number,
    Boolean,
    File,
    Json,
    Enum,
}

/// Skill parameter definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillParameter {
    pub name: String,
    #[serde(rename = "type")]
    pub param_type: SkillParameterType,
    pub description: String,
    pub required: bool,
    pub default: Option<serde_json::Value>,
    pub options: Option<Vec<String>>,
}

/// Skill execution request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillExecutionRequest {
    #[serde(rename = "skill_name")]
    pub skill_name: String,
    pub parameters: HashMap<String, serde_json::Value>,
    #[serde(rename = "scene_id")]
    pub scene_id: Option<String>,
}

/// Skill execution result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillExecutionResult {
    #[serde(rename = "skill_name")]
    pub skill_name: String,
    pub success: bool,
    pub output: Option<String>,
    pub error: Option<String>,
    pub changes: Vec<EntityChangePayload>,
    #[serde(rename = "duration_ms")]
    pub duration_ms: i64,
}

/// Entity change from skill execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityChangePayload {
    pub action: String, // "create", "update", "delete"
    #[serde(rename = "entity_id")]
    pub entity_id: Option<u64>,
    #[serde(rename = "entity_name")]
    pub entity_name: String,
    #[serde(rename = "entity_type")]
    pub entity_type: Option<String>,
    pub components: Option<serde_json::Value>,
}
