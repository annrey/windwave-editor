//! System trait and built-in systems

pub mod health;
pub mod physics;

pub use health::*;
pub use physics::*;

use crate::core::scene::Scene;

/// System trait - all systems must implement this
pub trait System: Send + Sync {
    fn name(&self) -> &'static str;
    fn update(&self, scene: &mut Scene, delta_time: f32);
}
