//! Multica Bridge
//!
//! Bridge between WindWave and Multica agent platform.
//!
//! ## Phase 2 Modules (Deep Integration)
//!
//! **Memory System Integration:**
//! - [memory_injector]: Scene-to-4-tier memory injection bridge (L0 Procedural,
//!   L1 Semantic, L2 Episodic, L3 Working). Subscribes to scene events and
//!   automatically triggers memory injection on entity CRUD.
//! - [four_tier_memory_integration]: Dedicated injectors for each memory tier
//!   with `FourTierSceneInjector` orchestrator and `SceneMemoryAutoUpdater` for
//!   background scene-to-memory synchronization.
//! - [memory_scene_context]: Scene memory data structures (`SceneMemoryEntry`,
//!   `SceneContextMemory`) bridging multica-db scene data into agent-core's
//!   4-tier memory system, with snapshot save/restore and diff recording.
//!
//! **Network Connection:**
//! - [ws_client]: WebSocket client with auto-reconnect, message queue for
//!   offline buffering, and connection status monitoring.
//! - [multica_daemon]: Background daemon with heartbeat, status reporting,
//!   automatic reconnection, and message handling pipeline.
//! - [message_handler]: Handles incoming Multica server messages including
//!   task create/update/delete and heartbeat responses.
//!
//! **Task Synchronization:**
//! - [task_sync], [task_sync_module]: Bidirectional task sync with configurable
//!   conflict resolution strategies.
//! - [task_bridge]: Unified task view bridging WindWave and Multica task models.

#![allow(clippy::field_reassign_with_default)]
#![allow(clippy::should_implement_trait)]
#![allow(clippy::new_without_default)]

pub mod agent_orchestrator;
pub mod agent_proxy;
pub mod compatibility;
pub mod db_layer;
pub mod error;
pub mod four_tier_memory_integration;
pub mod game_skill_bridge;
pub mod memory_injector;
pub mod memory_scene_context;
pub mod message_handler;
pub mod multica_daemon;
pub mod multica_db;
pub mod realtime_bridge;
pub mod scene_agent;
pub mod scene_context;
pub mod scene_event_bus;
pub mod skill_adapter;
pub mod skill_system;
pub mod task_bridge;
pub mod task_sync;
pub mod task_sync_module;
pub mod test_server;
pub mod types;
pub mod ws_client;

pub use agent_orchestrator::{
    AgentExecutor, AgentOrchestrator, ExecutionMode, PipelineStats, PipelineStatus, PipelineTask,
    PipelineTaskStatus, SkillProvider,
};
pub use agent_proxy::{AgentInfo, AgentProxy};
pub use compatibility::{
    create_daemon_register_payload, create_task_claim_payload, create_task_completed_payload,
    create_task_progress_payload, export_local_task, export_unified_task, import_to_local_task,
    import_to_unified_task, windwave_event_to_multica_message, CompatMode, ExportedTask,
    ImportResult, MulticaCompatLayer, WindWaveTaskMetadata,
};
pub use db_layer::{
    BatchOp, BatchOpResult, BatchResult, BatchTransaction, DbLayer, DbSnapshot, EntityFilter,
    EntityIndex, EntityQuery, PaginatedResult, Pagination, SceneFilter, SceneQuery, SortField,
    SortOrder, SortSpec,
};
pub use error::{BridgeError, Result};
pub use four_tier_memory_integration::{
    CrossLayerQueryResult, FourTierSceneInjector, InjectionResult, SceneEpisodicMemoryInjector,
    SceneMemoryAutoUpdater, SceneProceduralMemoryInjector, SceneSemanticMemoryInjector,
    SceneWorkingMemoryInjector,
};
pub use game_skill_bridge::{
    CreateEntityParams, DeleteEntityParams, GameSkillBridge, QueryEntitiesParams,
    SkillExecutionResult, UpdateComponentParams,
};
pub use memory_injector::{
    create_shared_memory_injector, create_shared_memory_injector_with_four_tier,
    MemoryInjectionStats, MemoryInjectorConfig, MulticaMemoryInjector, SceneMemoryEventHandler,
};
pub use memory_scene_context::{
    create_shared_scene_context_memory, SceneChangeEvent, SceneChangeListener, SceneContextMemory,
    SceneMemoryEntry, SceneMemoryInjector, SceneMemoryStats, SceneNodeType,
    SharedSceneContextMemory,
};
pub use multica_daemon::{
    create_shared_daemon, DaemonStats, DaemonStatus, MulticaDaemon, SharedMulticaDaemon,
};
pub use multica_db::{
    create_shared_multica_db, DbStatistics, EntityRecord, MulticaDb, ResourceRecord, SceneRecord,
    SharedMulticaDb, TaskRecord, TaskSceneEntityQueryResult, TaskSceneRelation,
    TaskSceneRelationType,
};
pub use realtime_bridge::{
    EntityEventPayload, RealtimeBridge, RealtimeStats, RealtimeStatus, SceneEventPayload,
    SceneEventStreamer, SceneSyncRequestPayload, SceneSyncResponsePayload, SkillDispatchHandler,
    SkillDispatchPayload, SkillResultPayload, EVENT_ENTITY_EVENT, EVENT_REALTIME_ERROR,
    EVENT_REALTIME_STATUS, EVENT_SCENE_EVENT, EVENT_SCENE_SYNC_REQUEST, EVENT_SCENE_SYNC_RESPONSE,
    EVENT_SKILL_DISPATCH, EVENT_SKILL_LIST_REQUEST, EVENT_SKILL_LIST_RESPONSE, EVENT_SKILL_RESULT,
};
pub use scene_agent::{
    SceneAgent, SceneAgentCapability, SceneAgentConfig, SceneAgentContext, SceneAgentResult,
};
pub use scene_context::{
    create_shared_scene_context, create_shared_scene_context_with, ComponentData, EntityChange,
    InMemorySceneContext, SceneContext, SceneDiff, SceneEntity, SceneSnapshot, SharedSceneContext,
};
pub use scene_event_bus::{
    create_shared_event_bus, SceneEvent, SceneEventBus, SceneEventSubscriber, SceneEventType,
    SharedSceneEventBus, SubscriberId,
};
pub use skill_adapter::{LocalSkill, SkillAdapter};
pub use skill_system::{
    RegisteredSkill, SkillCategory, SkillExecResult, SkillExecutionContext, SkillSystem,
};
pub use task_bridge::{BridgeStats, BridgedTaskId, TaskBridge, UnifiedTask, UnifiedTaskStatus};
pub use task_sync::{LocalTask, TaskStatus, TaskSync};
pub use task_sync_module::{
    create_shared_task_synchronizer, ConflictResolution, SyncConflict, SyncDirection, SyncStats,
    SyncStatus, TaskSyncConfig, TaskSynchronizer,
};
pub use test_server::*;
pub use types::*;
pub use ws_client::MulticaWebSocketClient;
