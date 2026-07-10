//! Health and damage system

use crate::core::scene::Scene;
use crate::systems::System;

/// Health regeneration system
pub struct HealthRegenSystem {
    pub regen_rate: f32,
}

impl HealthRegenSystem {
    pub fn new(regen_rate: f32) -> Self {
        Self { regen_rate }
    }
}

impl Default for HealthRegenSystem {
    fn default() -> Self {
        Self::new(1.0)
    }
}

impl System for HealthRegenSystem {
    fn name(&self) -> &'static str {
        "HealthRegenSystem"
    }

    fn update(&self, scene: &mut Scene, delta_time: f32) {
        for mut entity in scene.entities() {
            if let Some(health) = &mut entity.health {
                health.current = (health.current + self.regen_rate * delta_time).min(health.max);
                scene.update_entity(entity);
            }
        }
    }
}
