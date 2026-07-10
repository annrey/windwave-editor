//! # Game Simulator - Headless Game Logic Testing
//!
//! This crate provides a headless game logic simulator that allows testing game systems
//! without launching the full game engine.
//!
//! ## Example Usage
//!
//! ```rust
//! use game_simulator::prelude::*;
//!
//! let mut sim = GameSimulator::new();
//! sim.add_system(PhysicsSystem::new());
//!
//! let player_id = sim.spawn_entity("player");
//! let mut player = sim.scene().get_entity(player_id).unwrap();
//! player.position = Some(Position::new(0.0, 0.0, 0.0));
//! player.velocity = Some(Velocity::new(1.0, 0.0, 0.0));
//! sim.scene_mut().update_entity(player);
//!
//! sim.run_for(60);
//!
//! let player = sim.scene().get_entity(player_id).unwrap();
//! assert!((player.position.unwrap().x - 1.0).abs() < 0.01);
//! ```

pub mod core;
pub mod progress;
pub mod systems;
pub mod test_framework;

/// Prelude for easy importing of common types
pub mod prelude {
    pub use crate::core::*;
    pub use crate::progress::*;
    pub use crate::systems::*;
    pub use crate::test_framework::*;
}

pub use prelude::*;
