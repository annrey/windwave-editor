//! Physics simulation system

use crate::core::scene::Scene;
use crate::systems::System;

/// Physics system that updates positions based on velocity
pub struct PhysicsSystem;

impl PhysicsSystem {
    pub fn new() -> Self {
        Self
    }
}

impl Default for PhysicsSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl System for PhysicsSystem {
    fn name(&self) -> &'static str {
        "PhysicsSystem"
    }

    fn update(&self, scene: &mut Scene, delta_time: f32) {
        for mut entity in scene.entities() {
            if let (Some(pos), Some(vel)) = (&mut entity.position, &entity.velocity) {
                pos.x += vel.x * delta_time;
                pos.y += vel.y * delta_time;
                pos.z += vel.z * delta_time;
                scene.update_entity(entity);
            }
        }
    }
}
