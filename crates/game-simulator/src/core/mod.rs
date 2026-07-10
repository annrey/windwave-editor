//! Core simulation types: Entity, Component, Scene, Simulator

pub mod component;
pub mod error;
pub mod scene;
pub mod simulator;

pub use component::*;
pub use error::{EntityId, SimError, SimResult};
pub use scene::Scene;
pub use simulator::GameSimulator;
