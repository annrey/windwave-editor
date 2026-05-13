//! Snapshot and rollback support for the BevyAdapter.
//!
//! Provides point-in-time entity state capture (`capture_snapshot`)
//! and undo operations (`rollback_operation`) so that Agent actions
//! can be reversed when needed.

use agent_core::EntityId;
use bevy::prelude::*;
use bevy::sprite::Sprite;
use serde::{Deserialize, Serialize};

use super::BevyAdapter;

/// A rollback operation that can undo a previously applied command.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RollbackOperation {
    /// Remove an entity that was created.
    DeleteEntity { entity_id: u64 },
    /// Restore a Transform to a previous translation.
    RestoreTransform { entity_id: u64, translation: [f32; 3] },
    /// Restore a Sprite color to a previous RGBA value.
    RestoreSpriteColor { entity_id: u64, rgba: [f32; 4] },
}

/// A point-in-time snapshot of an entity's state for rollback purposes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntitySnapshot {
    pub entity_id: u64,
    pub name: String,
    pub translation: Option<[f32; 3]>,
    pub rotation: Option<[f32; 3]>,
    pub scale: Option<[f32; 3]>,
    pub sprite_color: Option<[f32; 4]>,
}

impl BevyAdapter {
    /// Roll back a previously applied operation.
    pub fn rollback_operation(
        &mut self,
        rollback: RollbackOperation,
        world: &mut World,
    ) -> Result<(), String> {
        match rollback {
            RollbackOperation::DeleteEntity { entity_id } => {
                let agent_eid = EntityId(entity_id);
                if let Some(bevy_entity) = self.get_bevy_entity(agent_eid) {
                    world.despawn(bevy_entity);
                    self.entity_map.remove(&agent_eid);
                    self.reverse_map.remove(&bevy_entity);
                }
                Ok(())
            }
            RollbackOperation::RestoreTransform {
                entity_id,
                translation,
            } => {
                let agent_eid = EntityId(entity_id);
                if let Some(bevy_entity) = self.get_bevy_entity(agent_eid) {
                    if let Some(mut transform) = world.get_mut::<Transform>(bevy_entity) {
                        transform.translation =
                            Vec3::new(translation[0], translation[1], translation[2]);
                    }
                }
                Ok(())
            }
            RollbackOperation::RestoreSpriteColor { entity_id, rgba } => {
                let agent_eid = EntityId(entity_id);
                if let Some(bevy_entity) = self.get_bevy_entity(agent_eid) {
                    if let Some(mut sprite) = world.get_mut::<Sprite>(bevy_entity) {
                        sprite.color = Color::linear_rgba(rgba[0], rgba[1], rgba[2], rgba[3]);
                    }
                }
                Ok(())
            }
        }
    }

    /// Capture a point-in-time snapshot of an entity's state for rollback purposes.
    pub fn capture_snapshot(
        &self,
        entity_id: u64,
        world: &World,
    ) -> Option<EntitySnapshot> {
        let agent_eid = EntityId(entity_id);
        let bevy_entity = self.get_bevy_entity(agent_eid)?;
        let entity_ref = world.get_entity(bevy_entity).ok()?;

        let name = entity_ref
            .get::<Name>()
            .map(|n| n.to_string())
            .unwrap_or_default();

        let translation = entity_ref
            .get::<Transform>()
            .map(|t| [t.translation.x, t.translation.y, t.translation.z]);

        let rotation = entity_ref
            .get::<Transform>()
            .map(|t| {
                let (roll, pitch, yaw) = t.rotation.to_euler(EulerRot::XYZ);
                [roll, pitch, yaw]
            });

        let scale = entity_ref
            .get::<Transform>()
            .map(|t| [t.scale.x, t.scale.y, t.scale.z]);

        let sprite_color = entity_ref
            .get::<Sprite>()
            .map(|s| {
                let col = s.color.to_linear();
                [col.red, col.green, col.blue, col.alpha]
            });

        Some(EntitySnapshot {
            entity_id,
            name,
            translation,
            rotation,
            scale,
            sprite_color,
        })
    }
}
