//! Bevy Adapter - Engine adapter for Bevy ECS
//!
//! Bridges the Agent core to Bevy engine, translating Agent actions to ECS operations.
//! Provides EngineCommand DSL, SceneIndex for Agent reasoning, and rollback support.

#![allow(clippy::type_complexity)]

pub mod adapter;
pub mod ai_resident;
pub mod command_processor;
pub mod integration;
pub mod llm_runtime_agent;
pub mod open_world_components;
pub mod perception;
pub mod prefab_ops;
pub mod runtime_agent;
pub mod scene_bridge_impl;
pub mod scene_index;
pub mod scene_io;
pub mod screenshot;

pub use adapter::sync_entities_to_adapter;
pub use adapter::BevyAdapter;
pub use adapter::BevyAdapterPlugin;
pub use adapter::EngineAdapter;
pub use adapter::{AgentActionEvent, AgentEntityId, AgentTracked};
pub use adapter::{AssetReference, AssetType, ComponentPatch, EngineCommand, EngineCommandResult};
pub use adapter::{EntitySnapshot, RollbackOperation};
pub use ai_resident::{
    apply_sandbox_to_world, push_resident_scene_index_summaries,
    run_ai_resident_slice01_bevy_smoke, spawn_ai_resident_slice01, AiResidentClock,
    AiResidentPlugin, GuardPatrolPath, ResidentAgent, ResidentBehaviorState,
};
pub use command_processor::{CommandHistory, CommandProcessorPlugin, PendingCommands};
pub use llm_runtime_agent::{
    configure_llm_runtime, LlmAgentRequest, LlmAgentResponse, LlmRuntimeAgentPlugin,
    LlmRuntimeResource, PendingLlmRequest,
};
pub use open_world_components::{
    apply_open_world_replay_state_to_world, process_open_world_combat_interaction,
    process_open_world_loot_interaction, sync_open_world_replay_gameplay_state, Attack, CampMarker,
    Combatant, EnemyBrain, FollowCamera, Interactable, InteractionZone, Inventory, LootContainer,
    OpenWorldObject, OpenWorldReplayActorState, OpenWorldReplayEnemyState,
    OpenWorldReplayGameplayState, OpenWorldReplayInventoryState, OpenWorldReplayLootState,
    OpenWorldReplayPuzzleState, OpenWorldReplayQuestState, PlayerController, PuzzleAnchor,
    PuzzleSwitch, Quest, QuestObjective, TemplateObject, WorldSurface, ZoneMarker,
};
pub use perception::{
    query_agent_perception, spawn_perceivable_entity, Perceivable, PerceivedEntityInfo,
    PerceptionCapability, PerceptionConfig, PerceptionPlugin,
};
pub use runtime_agent::{
    attach_runtime_agent, detach_runtime_agent, process_editor_control_command,
    runtime_action_to_engine_command, spawn_runtime_agent_entity, RuntimeAgentAction,
    RuntimeAgentComponent, RuntimeAgentControlMode, RuntimeAgentId, RuntimeAgentPlugin,
    RuntimeAgentProfileId, RuntimeAgentRegistry, RuntimeAgentStatus, RuntimeTarget,
};
pub use screenshot::{
    ScreenshotArtifact, ScreenshotPlugin, ScreenshotQueue, ScreenshotResult, ScreenshotState,
};
