//! OpenGame-Bench inspired evaluation system for AgentEdit
//!
//! Evaluates agent-generated game scenes along three dimensions:
//! - Build Health: Compilation success, warning count, dependency resolution
//! - Visual Usability: Scene renders correctly, entities visible, UI functional
//! - Intent Alignment: Generated scene matches user request

pub mod build_health;
pub mod intent_alignment;
pub mod runner;
pub mod types;
pub mod visual_usability;

pub use build_health::BuildHealthEvaluator;
pub use intent_alignment::IntentAlignmentEvaluator;
pub use runner::{BenchIntegration, BenchRunner};
pub use types::*;
pub use visual_usability::VisualUsabilityEvaluator;

#[cfg(test)]
mod tests;
