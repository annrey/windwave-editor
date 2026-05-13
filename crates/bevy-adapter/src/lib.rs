//! Bevy Adapter - Engine adapter for Bevy ECS
//!
//! Bridges the Agent core to Bevy engine, translating Agent actions to ECS operations.
//! Provides EngineCommand DSL, SceneIndex for Agent reasoning, and rollback support.

pub mod adapter;
pub mod integration;
pub mod llm_runtime_agent;
pub mod perception;
pub mod runtime_agent;
pub mod scene_bridge_impl;
pub mod scene_index;
pub mod scene_io;
pub mod prefab_ops;
pub mod screenshot;

pub use adapter::BevyAdapterPlugin;
pub use adapter::BevyAdapter;
pub use adapter::EngineAdapter;
pub use adapter::{EngineCommand, AssetType, AssetReference, ComponentPatch, EngineCommandResult};
pub use adapter::{RollbackOperation, EntitySnapshot};
pub use adapter::{AgentActionEvent, AgentTracked, AgentEntityId};
pub use adapter::sync_entities_to_adapter;
pub use llm_runtime_agent::{
    LlmRuntimeAgentPlugin, LlmRuntimeResource, LlmAgentRequest, LlmAgentResponse,
    configure_llm_runtime, PendingLlmRequest,
};
pub use perception::{
    PerceptionPlugin, PerceptionCapability, Perceivable, PerceptionConfig,
    PerceivedEntityInfo, query_agent_perception, spawn_perceivable_entity,
};
pub use runtime_agent::{
    RuntimeAgentComponent, RuntimeAgentPlugin, RuntimeAgentRegistry,
    RuntimeAgentId, RuntimeAgentProfileId, RuntimeAgentControlMode,
    RuntimeAgentStatus, RuntimeAgentAction, RuntimeTarget,
    attach_runtime_agent, detach_runtime_agent, spawn_runtime_agent_entity,
    runtime_action_to_engine_command, process_editor_control_command,
};
