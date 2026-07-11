//! Bevy Integration System — periodic SceneIndex rebuild, visual observation,
//! and full pipeline wiring between agent-core and Bevy ECS.
//!
//! Design reference: Section 12.0.4 + Section 12.7 of
//! gpt-agent-team-task-event-skill-architecture.md
//!
//! Provides three plugins:
//! - SceneIndexRebuildPlugin — periodic SceneIndex snapshot from ECS World
//! - VisionPlugin           — screenshot capture + visual analysis pipeline
//! - IntegrationPlugin      — wires DirectorRuntime ↔ ECS ↔ UI ↔ EventBus

use crate::scene_index::{ComponentSummary, SceneEntityNode, SceneIndex};
use crate::BevyAdapter;
use crate::EngineCommand;
use agent_core::goal_checker::SceneEntityInfo as CoreSceneEntityInfo;
use agent_core::ports::scene::{ComponentPatch, EntityListItem, SceneBridge};
use bevy::ecs::hierarchy::{ChildOf, Children};
use bevy::prelude::*;
use bevy::sprite::Sprite;
use log::info;
use std::collections::HashMap;

mod incremental;
mod integration_core;
mod providers;
mod scene_bridge;
mod scene_index;
mod screenshot;
#[cfg(test)]
mod tests;

pub use incremental::{
    compute_scene_hash, SceneIndexGenerationTracker, SceneIndexIncrementalPlugin,
};
pub use integration_core::{IntegrationPlugin, IntegrationState};
pub use providers::{BevyScreenshotProvider, SceneIndexVisionProvider};
pub use scene_bridge::SceneIndexSceneBridge;
pub use scene_index::{SceneIndexCache, SceneIndexRebuildPlugin};
pub use screenshot::{
    MockScreenshotProvider, MockVisionProvider, ScreenshotArtifact, ScreenshotProvider,
    VisionError, VisionPlugin, VisionProvider, VisionState, VisualObservation,
};
