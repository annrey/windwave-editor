# Multica Bridge API Reference

## Overview

The Multica Bridge module provides integration between WindWave and Multica collaboration systems. It enables task synchronization, scene-aware agents, game skill bridging, and four-tier memory system integration.

## Module Structure

```
crates/multica-bridge/src/
├── lib.rs                      # Module exports and public API
├── error.rs                    # Error types and Result alias
├── types.rs                    # Shared type definitions
├── ws_client.rs                # WebSocket client with auto-reconnect
├── task_bridge.rs              # Unified task system bridge
├── task_sync.rs                # Task synchronization
├── task_sync_module.rs         # Advanced task synchronizer
├── scene_context.rs            # Scene awareness interface
├── scene_event_bus.rs          # Scene event bus system
├── memory_scene_context.rs     # Scene memory management
├── memory_injector.rs          # Memory injection system
├── four_tier_memory_integration.rs  # Four-tier memory system
├── game_skill_bridge.rs        # Game skill bridging
├── multica_db.rs               # Unified data access layer
├── message_handler.rs          # Message processing pipeline
└── multica_daemon.rs           # Daemon process management
```

## Core APIs

### Error Handling

```rust
use multica_bridge::error::{BridgeError, Result};

/// Error types
pub enum BridgeError {
    ConnectionError(String),
    WebSocketError(String),
    SerializationError(String),
    TaskNotFoundError(String),
    SceneNotFoundError(String),
    ConnectionClosed,
    Other(String),
}
```

### Types

```rust
use multica_bridge::types::*;

/// Bridge configuration
pub struct BridgeConfig {
    pub server_url: String,
    pub daemon_id: String,
    pub agent_id: String,
    pub workspace_id: String,
    pub api_key: Option<String>,
    pub auto_reconnect: bool,
    pub reconnect_delay: u64,
}

/// Message structure
pub struct Message {
    pub message_type: String,
    pub payload: serde_json::Value,
}

/// Daemon register payload
pub struct DaemonRegisterPayload {
    pub daemon_id: String,
    pub agent_id: String,
    pub runtimes: Vec<RuntimeInfo>,
}

/// Daemon heartbeat payload
pub struct DaemonHeartbeatRequestPayload {
    pub runtime_id: String,
    pub supports_batch_import: Option<bool>,
}
```

### WebSocket Client

```rust
use multica_bridge::ws_client::*;

/// Connection status monitoring
pub enum ConnectionStatus {
    Disconnected,
    Connecting,
    Connected,
    Reconnecting,
    Error(String),
}

/// Connection monitor
pub struct ConnectionMonitor {
    // Thread-safe connection state tracking
    pub fn get_status() -> ConnectionStatus;
    pub fn is_connected() -> bool;
    pub fn get_reconnect_attempts() -> usize;
    pub fn should_reconnect() -> bool;
}

/// Message queue
pub struct MessageQueue {
    // Async message queue for offline buffering
    pub async fn enqueue(message: Message) -> Result<()>;
    pub async fn dequeue() -> Option<Message>;
    pub async fn dequeue_all() -> Vec<Message>;
    pub async fn len() -> usize;
    pub async fn is_empty() -> bool;
    pub async fn clear();
}

/// Main WebSocket client
pub struct MulticaWebSocketClient {
    pub fn new(config: BridgeConfig) -> Self;
    pub fn with_heartbeat(config: BridgeConfig, interval: Duration) -> Self;
    pub async fn connect() -> Result<()>;
    pub async fn connect_with_retry() -> Result<()>;
    pub async fn reconnect() -> Result<()>;
    pub async fn disconnect() -> Result<()>;
    pub async fn send_message(message: Message) -> Result<()>;
    pub async fn receive_message() -> Result<Option<Message>>;
    pub async fn register_daemon() -> Result<()>;
    pub async fn send_heartbeat(runtime_id: String) -> Result<()>;
    pub async fn flush_message_queue() -> Result<()>;
    pub fn get_connection_status() -> ConnectionStatus;
    pub fn is_connected() -> bool;
    pub fn connection_monitor() -> &ConnectionMonitor;
    pub fn message_queue() -> &MessageQueue;
}
```

### Task Bridge

```rust
use multica_bridge::task_bridge::*;

/// Bridged task identifier
pub struct BridgedTaskId {
    pub multica_id: Option<u64>,
    pub bridge_id: u64,
}

/// Unified task status
pub enum UnifiedTaskStatus {
    Pending,
    Running,
    Done,
    Failed,
    Cancelled,
}

/// Unified task structure
pub struct UnifiedTask {
    pub id: BridgedTaskId,
    pub status: UnifiedTaskStatus,
    pub title: String,
    pub description: String,
    pub scene_id: Option<String>,
    pub entity_ids: Vec<u64>,
    pub resource_ids: Vec<u64>,
    pub scene_snapshot: Option<SceneSnapshot>,
    pub multica_task: Option<MulticaTask>,
    pub created_at: String,
    pub updated_at: String,
}
```

### Task Synchronization

```rust
use multica_bridge::task_sync_module::*;

/// Sync direction
pub enum SyncDirection {
    WindWaveToMultica,
    MulticaToWindWave,
    Bidirectional,
}

/// Conflict resolution strategy
pub enum ConflictResolution {
    WindWaveWins,
    MulticaWins,
    LastWriteWins,
    Manual,
}

/// Sync configuration
pub struct TaskSyncConfig {
    pub direction: SyncDirection,
    pub conflict_resolution: ConflictResolution,
    pub sync_interval: Duration,
    pub auto_sync: bool,
    pub max_retry_count: usize,
}

/// Sync status
pub enum SyncStatus {
    Idle,
    Syncing,
    Success,
    Conflict(Vec<SyncConflict>),
    Failed(String),
}

/// Sync conflict
pub struct SyncConflict {
    pub task_id: BridgedTaskId,
    pub windwave_status: UnifiedTaskStatus,
    pub multica_status: UnifiedTaskStatus,
    pub windwave_updated_at: String,
    pub multica_updated_at: String,
    pub resolution: Option<ConflictResolution>,
}

/// Sync statistics
pub struct SyncStats {
    pub total_syncs: usize,
    pub successful_syncs: usize,
    pub failed_syncs: usize,
    pub conflict_count: usize,
    pub resolved_conflicts: usize,
    pub last_sync_at: Option<u64>,
    pub last_sync_duration_ms: Option<u128>,
    pub synced_tasks_count: usize,
}

/// Task synchronizer
pub struct TaskSynchronizer {
    pub fn new(config: TaskSyncConfig) -> Self;
    pub fn add_local_task(task: UnifiedTask);
    pub fn add_remote_task(task: UnifiedTask);
    pub fn update_local_task_status(bridge_id: u64, status: UnifiedTaskStatus) -> Result<()>;
    pub fn update_remote_task_status(multica_id: u64, status: UnifiedTaskStatus) -> Result<()>;
    pub fn sync() -> Result<SyncStats>;
    pub fn get_status() -> SyncStatus;
    pub fn get_stats() -> SyncStats;
    pub fn pending_sync_count() -> usize;
    pub fn add_pending_sync(task_id: BridgedTaskId);
    pub fn clear_pending_sync();
    pub fn resolve_conflict(task_id: BridgedTaskId, resolution: ConflictResolution) -> Result<()>;
    pub fn local_task_count() -> usize;
    pub fn remote_task_count() -> usize;
}

// Factory functions
pub fn create_shared_task_synchronizer(config: TaskSyncConfig) -> Arc<TaskSynchronizer>;
```

### Scene Context

```rust
use multica_bridge::scene_context::*;

/// Scene entity
pub struct SceneEntity {
    pub id: u64,
    pub name: String,
    pub components: Vec<ComponentData>,
    pub position: Option<[f32; 3]>,
}

/// Component data
pub struct ComponentData {
    pub type_name: String,
    pub properties: HashMap<String, serde_json::Value>,
}

/// Scene snapshot
pub struct SceneSnapshot {
    pub version: u64,
    pub timestamp: String,
    pub entities: Vec<SceneEntity>,
}

/// Scene diff
pub struct SceneDiff {
    pub added: Vec<SceneEntity>,
    pub removed: Vec<SceneEntity>,
    pub modified: Vec<SceneEntity>,
}
```

### Scene Event Bus

```rust
use multica_bridge::scene_event_bus::*;

/// Event types
pub enum SceneEventType {
    EntityCreated,
    EntityUpdated,
    EntityDeleted,
    ComponentAdded,
    ComponentUpdated,
    ComponentRemoved,
    Custom(String),
}

/// Scene event
pub struct SceneEvent {
    pub scene_id: String,
    pub event_type: SceneEventType,
    pub entity_id: u64,
    pub entity_before: Option<SceneEntity>,
    pub entity_after: Option<SceneEntity>,
    pub component: Option<ComponentData>,
    pub timestamp: String,
}

/// Event subscriber trait
pub trait SceneEventSubscriber: Send + Sync {
    fn on_event(&self, event: &SceneEvent);
}

/// Scene event bus
pub struct SceneEventBus {
    pub fn new() -> Self;
    pub fn subscribe(subscriber: Arc<dyn SceneEventSubscriber>) -> String;
    pub fn unsubscribe(subscription_id: &str);
    pub fn publish(event: SceneEvent);
    pub fn publish_entity_created(scene_id: String, entity: SceneEntity);
    pub fn publish_entity_updated(scene_id: String, before: SceneEntity, after: SceneEntity);
    pub fn publish_entity_deleted(scene_id: String, entity: SceneEntity);
    pub fn publish_component_added(scene_id: String, entity_id: u64, component: ComponentData);
    pub fn publish_component_updated(scene_id: String, entity_id: u64, component: ComponentData);
    pub fn get_stats() -> EventBusStats;
    pub fn clear_history();
    pub fn clear_subscribers();
}

pub fn create_shared_scene_event_bus() -> Arc<Mutex<SceneEventBus>>;
```

### Memory System

```rust
use multica_bridge::memory_scene_context::*;

/// Scene memory entry
pub struct SceneMemoryEntry {
    pub scene_id: String,
    pub scene_name: String,
    pub snapshot: SceneSnapshot,
    pub related_task_ids: Vec<String>,
    pub context_tags: Vec<String>,
    pub created_at: String,
}

/// Scene change event
pub struct SceneChangeEvent {
    pub scene_id: String,
    pub diff: SceneDiff,
    pub timestamp: String,
}

/// Scene context memory
pub struct SceneContextMemory {
    pub active_scene: Option<SceneMemoryEntry>,
    pub scene_history: HashMap<String, Vec<SceneMemoryEntry>>,
    pub change_events: Vec<SceneChangeEvent>,
    pub relation_nodes: HashMap<String, SceneRelationNode>,
    pub relation_edges: Vec<SceneRelationEdge>,
    
    pub fn save_scene_snapshot(scene_id: String, scene_name: String, snapshot: SceneSnapshot) -> Result<()>;
    pub fn record_scene_change(diff: SceneDiff, scene_id: String) -> Result<()>;
    pub fn add_relation_node(node_type: SceneNodeType, name: String, properties: HashMap<String, String>) -> String;
    pub fn add_relation_edge(from: String, to: String, relation: String);
    pub fn get_latest_scene(scene_id: &str) -> Option<&SceneMemoryEntry>;
    pub fn get_change_history(scene_id: &str) -> Vec<&SceneChangeEvent>;
    pub fn get_scene_related_nodes(scene_id: &str) -> (Vec<&SceneRelationNode>, Vec<&SceneRelationEdge>);
    pub fn get_stats() -> SceneMemoryStats;
}

pub fn create_shared_scene_context_memory() -> SharedSceneContextMemory;
```

### Memory Injector

```rust
use multica_bridge::memory_injector::*;

/// Memory injector configuration
pub struct MemoryInjectorConfig {
    pub auto_inject: bool,
    pub auto_trigger_on_change: bool,
    pub task_associated_memory: bool,
    pub update_interval_secs: u64,
    pub max_concurrent_injections: usize,
}

/// Memory injection statistics
pub struct MemoryInjectionStats {
    pub total_injections: usize,
    pub successful_injections: usize,
    pub failed_injections: usize,
    pub auto_triggered: usize,
    pub manually_triggered: usize,
    pub last_injection_timestamp: Option<u64>,
    pub injections_per_scene: HashMap<String, usize>,
}

/// Memory injector
pub struct MulticaMemoryInjector {
    pub fn new(scene_memory: SharedSceneContextMemory, config: MemoryInjectorConfig) -> Self;
    pub fn with_four_tier_memory(
        scene_memory: SharedSceneContextMemory,
        four_tier_injector: Arc<Mutex<FourTierSceneInjector>>,
        config: MemoryInjectorConfig,
    ) -> Self;
    pub fn inject_scene(scene_id: &str) -> Result<InjectionResult>;
    pub fn inject_scenes(scene_ids: &[String]) -> Result<Vec<InjectionResult>>;
    pub fn update_scene_memory(scene_id: &str) -> Result<()>;
    pub fn associate_task_with_scene(task_id: &str, scene_id: &str);
    pub fn get_tasks_for_scene(scene_id: &str) -> Vec<String>;
    pub fn get_scenes_for_task(task_id: &str) -> Vec<String>;
    pub fn on_task_start(task_id: &str, scene_id: &str) -> Result<InjectionResult>;
    pub fn on_task_complete(task_id: &str, scene_id: &str) -> Result<()>;
    pub fn on_task_failed(task_id: &str, scene_id: &str, error_msg: &str) -> Result<()>;
    pub fn get_stats() -> MemoryInjectionStats;
    pub fn get_injected_scenes() -> Vec<String>;
}

/// Scene memory event handler
pub struct SceneMemoryEventHandler {
    pub fn new(injector: Arc<MulticaMemoryInjector>) -> Self;
    pub fn handle_scene_change(scene_id: &str, diff: SceneDiff) -> Result<()>;
    pub fn pending_changes_count() -> usize;
}

// Factory functions
pub fn create_shared_memory_injector(
    scene_memory: SharedSceneContextMemory,
    config: MemoryInjectorConfig,
) -> Arc<MulticaMemoryInjector>;

pub fn create_shared_memory_injector_with_four_tier(
    scene_memory: SharedSceneContextMemory,
    four_tier_injector: Arc<Mutex<FourTierSceneInjector>>,
    config: MemoryInjectorConfig,
) -> Arc<MulticaMemoryInjector>;
```

### Four-Tier Memory Integration

```rust
use multica_bridge::four_tier_memory_integration::*;

/// Memory entry
pub struct MemoryEntry {
    pub id: String,
    pub content: String,
    pub tags: Vec<String>,
    pub created_at: u64,
}

/// Working memory interface
pub trait WorkingMemoryInterface: Send + Sync {
    fn add(&mut self, content: String);
    fn clear_by_prefix(&mut self, prefix: &str);
    fn get_all(&self) -> Vec<String>;
}

/// Episodic memory interface
pub trait EpisodicMemoryInterface: Send + Sync {
    fn add_event(&mut self, content: String, tags: Vec<String>);
    fn search(&self, query: &str, tags: Option<&Vec<String>>, limit: usize) -> Vec<MemoryEntry>;
}

/// Semantic memory interface
pub trait SemanticMemoryInterface: Send + Sync {
    fn add_knowledge(&mut self, id: String, content: String, tags: Vec<String>);
    fn search(&self, query: &str, tags: Option<&Vec<String>>, limit: usize) -> Vec<MemoryEntry>;
}

/// Procedural memory interface
pub trait ProceduralMemoryInterface: Send + Sync {
    fn add_workflow(&mut self, id: String, content: String, tags: Vec<String>);
    fn match_workflow(&self, query: &str, tags: Option<&Vec<String>>) -> Vec<MemoryEntry>;
}

/// Injection result
pub struct InjectionResult {
    pub l3_working_success: bool,
    pub l2_episodic_success: bool,
    pub l1_semantic_success: bool,
    pub l0_procedural_success: bool,
    pub l3_working_error: Option<String>,
    pub l2_episodic_error: Option<String>,
    pub l1_semantic_error: Option<String>,
    pub l0_procedural_error: Option<String>,
    pub l1_semantic_relations: usize,
}

/// Cross-layer query result
pub struct CrossLayerQueryResult {
    pub working_memory: Option<Vec<String>>,
    pub episodic_memory: Vec<MemoryEntry>,
    pub semantic_memory: Vec<MemoryEntry>,
    pub procedural_memory: Vec<MemoryEntry>,
}

/// Four-tier scene injector
pub struct FourTierSceneInjector {
    pub fn new(
        scene_memory: SharedSceneContextMemory,
        working_memory: Arc<Mutex<dyn WorkingMemoryInterface>>,
        episodic_memory: Arc<Mutex<dyn EpisodicMemoryInterface>>,
        semantic_memory: Arc<Mutex<dyn SemanticMemoryInterface>>,
        procedural_memory: Arc<Mutex<dyn ProceduralMemoryInterface>>,
    ) -> Self;
    pub fn inject_all_layers(scene_id: &str) -> Result<InjectionResult>;
    pub fn update_all_layers(scene_id: &str) -> Result<()>;
    pub fn query_all_layers(scene_id: &str, query: &str) -> Result<CrossLayerQueryResult>;
}

/// Scene memory auto updater
pub struct SceneMemoryAutoUpdater {
    pub fn new(scene_memory: SharedSceneContextMemory) -> Self;
}
```

### Message Handler

```rust
use multica_bridge::message_handler::*;

/// Message handle result
pub struct MessageHandleResult {
    pub success: bool,
    pub message_type: String,
    pub error: Option<String>,
    pub processed_at: String,
    pub metadata: HashMap<String, String>,
}

/// Message handler statistics
pub struct MessageHandlerStats {
    pub total_processed: usize,
    pub successful: usize,
    pub failed: usize,
    pub ignored: usize,
    pub messages_by_type: HashMap<String, usize>,
    pub last_processed_at: Option<u64>,
    pub avg_processing_time_ms: f64,
    pub total_processing_time_ms: u128,
}

/// Message handler trait
pub trait MessageHandler: Send + Sync {
    fn handle_message(&self, message: &Message) -> Result<MessageHandleResult>;
    fn handler_name(&self) -> &str;
}

/// Default message handler
pub struct DefaultMessageHandler {
    pub fn new() -> Self;
    pub fn with_task_synchronizer(task_synchronizer: Arc<TaskSynchronizer>) -> Self;
    pub fn register_handler(message_type: String, handler: Box<dyn MessageHandler>);
    pub fn process_message(message: &Message) -> MessageHandleResult;
    pub fn get_stats() -> MessageHandlerStats;
}

/// Message pipeline
pub struct MessagePipeline {
    pub fn new() -> Self;
    pub fn add_handler(handler: Box<dyn MessageHandler>);
    pub fn process_message(message: &Message) -> Vec<MessageHandleResult>;
    pub fn get_stats() -> MessageHandlerStats;
}

// Factory functions
pub fn create_shared_message_handler() -> Arc<DefaultMessageHandler>;
pub fn create_shared_message_handler_with_sync(
    task_synchronizer: Arc<TaskSynchronizer>,
) -> Arc<DefaultMessageHandler>;
```

### Multica Daemon

```rust
use multica_bridge::multica_daemon::*;

/// Daemon status
pub enum DaemonStatus {
    Initializing,
    Connecting,
    Connected,
    Syncing,
    Idle,
    Error(String),
}

/// Daemon statistics
pub struct DaemonStats {
    pub start_time: String,
    pub message_count: usize,
    pub heartbeat_count: usize,
    pub last_heartbeat: Option<String>,
    pub reconnect_count: usize,
    pub last_error: Option<String>,
}

/// Multica daemon
pub struct MulticaDaemon {
    pub config: BridgeConfig,
    pub ws_client: MulticaWebSocketClient,
    pub task_sync: TaskSync,
    pub message_handler: Arc<DefaultMessageHandler>,
    pub task_synchronizer: Arc<TaskSynchronizer>,
    pub status: DaemonStatus,
    pub stats: DaemonStats,
    pub is_running: bool,
    pub heartbeat_interval: Duration,
    pub running_flag: Arc<AtomicBool>,
    
    pub fn new(config: BridgeConfig) -> Self;
    pub async fn start() -> Result<()>;
    pub async fn stop() -> Result<()>;
    pub async fn register_daemon() -> Result<()>;
    pub async fn send_heartbeat() -> Result<()>;
    pub async fn handle_reconnect() -> Result<()>;
    pub fn get_status() -> DaemonStatus;
    pub fn get_stats() -> DaemonStats;
}

// Factory function
pub fn create_shared_daemon(config: BridgeConfig) -> Arc<Mutex<MulticaDaemon>>;
```

## Usage Examples

### Basic Setup

```rust
use multica_bridge::*;
use multica_bridge::types::BridgeConfig;

// Configure bridge
let config = BridgeConfig {
    server_url: "ws://localhost:8080".to_string(),
    daemon_id: "my-daemon".to_string(),
    agent_id: "my-agent".to_string(),
    workspace_id: "workspace-1".to_string(),
    api_key: None,
    auto_reconnect: true,
    reconnect_delay: 5000,
};
```

### Task Synchronization

```rust
use multica_bridge::task_sync_module::*;

let sync_config = TaskSyncConfig {
    direction: SyncDirection::Bidirectional,
    conflict_resolution: ConflictResolution::LastWriteWins,
    sync_interval: Duration::from_secs(30),
    auto_sync: true,
    max_retry_count: 3,
};

let synchronizer = create_shared_task_synchronizer(sync_config);

// Add tasks
synchronizer.add_local_task(local_task);
synchronizer.add_remote_task(remote_task);

// Sync
let stats = synchronizer.sync()?;
println!("Synced {} tasks", stats.synced_tasks_count);
```

### Memory Injection

```rust
use multica_bridge::memory_injector::*;
use multica_bridge::memory_scene_context::create_shared_scene_context_memory;

let scene_memory = create_shared_scene_context_memory();
let config = MemoryInjectorConfig::default();
let injector = create_shared_memory_injector(scene_memory, config);

// Inject scene
injector.inject_scene("scene-1")?;

// Task lifecycle
injector.on_task_start("task-1", "scene-1")?;
injector.on_task_complete("task-1", "scene-1")?;
```

### Message Handling

```rust
use multica_bridge::message_handler::*;

let handler = create_shared_message_handler();

let message = Message {
    message_type: "task:created".to_string(),
    payload: serde_json::json!({...}),
};

let result = handler.process_message(&message);
assert!(result.success);
```

## Testing

Run all tests:

```bash
cargo test --package multica-bridge --lib --tests
```

Run specific module tests:

```bash
cargo test --package multica-bridge ws_client
cargo test --package multica-bridge memory_injector
cargo test --package multica-bridge task_sync_module
cargo test --package multica-bridge e2e_integration
```

## Module Dependencies

```
multica-bridge
├── agent-core (for memory interfaces)
├── serde + serde_json (serialization)
├── tokio (async runtime)
├── tokio-tungstenite (WebSocket)
├── futures-util (stream processing)
├── log (logging)
└── chrono (time handling)
```

## Thread Safety

All public APIs are thread-safe and can be used from multiple threads:
- Shared state uses `Arc<Mutex<T>>` or `Arc<std::sync::Mutex<T>>`
- Async operations use `tokio::sync::Mutex` where appropriate
- Atomic operations for simple flags use `AtomicBool` with `Ordering::SeqCst`
