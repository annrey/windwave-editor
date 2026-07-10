//! BevySceneBridge — real Bevy ECS implementation of agent_core::scene_bridge::SceneBridge.
//!
//! Provides complete scene operations including:
//! - Entity query and manipulation
//! - Prefab instantiation
//! - Scene save/load (serialization)
//! - Component patching
//! - **Hierarchy preservation**: Parent-child relationships are maintained during save/load
//!
//! Uses a command queue pattern to bridge between the SceneBridge trait (no World access)
//! and actual Bevy ECS operations (requires World access).
//!
//! ## Hierarchy Implementation Details
//!
//! The scene serialization now supports entity hierarchy:
//! - **Serialization**: Extracts children from Bevy's `Children` component and maps them to scene IDs
//! - **Deserialization**: Uses two-pass approach:
//!   1. First pass: Create all entities and register in the bridge
//!   2. Second pass: Rebuild parent-child relationships using `set_parent()`
//!
//! ### Current Limitations
//! - Only direct children are serialized (full tree structure is implicit)
//! - Entities must be registered in the bridge before hierarchy can be resolved
//! - Circular references are not validated (Bevy will handle them at runtime)
//! - Transform inheritance is automatically handled by Bevy after hierarchy is established

pub mod bridge;
pub mod commands;
pub mod helpers;
pub mod ops;
pub mod serialization;
pub mod systems;

pub use bridge::BevySceneBridge;
pub use commands::{SceneCommand, SceneCommandQueue, SceneCommandResult};
pub use helpers::{
    request_create_entity, request_instantiate_prefab, request_load_scene, request_save_scene,
};
pub use ops::BevySceneOps;
pub use serialization::{SerializableEntity, SerializableScene};
pub use systems::{process_deferred_updates, process_scene_commands, BevySceneBridgePlugin};
