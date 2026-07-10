//! Snapshot and undo recording operations for DirectorRuntime

use crate::director::DirectorRuntime;
use crate::rollback::{Change, OperationType, SceneSnapshot, SnapshotEntity};

impl DirectorRuntime {
    /// Capture the current scene state as a snapshot for undo purposes.
    pub fn capture_scene_snapshot(&mut self) -> Option<SceneSnapshot> {
        let bridge = self.scene_bridge.as_ref()?;
        let infos = bridge.get_scene_snapshot();
        let entities: Vec<SnapshotEntity> = infos
            .iter()
            .map(|info| SnapshotEntity {
                name: info.name.clone(),
                component_names: info.components.clone(),
                serialized_state: serde_json::json!({
                    "translation": info.translation,
                    "sprite_color": info.sprite_color,
                }),
            })
            .collect();
        Some(self.rollback_manager.capture_snapshot(entities))
    }

    /// Record a create-entity operation in the undo log.
    pub fn record_create_entity_undo(&mut self) {
        if let Some(snapshot) = self.capture_scene_snapshot() {
            self.rollback_manager
                .record(None, OperationType::CreateEntity, Vec::new(), snapshot);
        }
    }

    /// Record a delete-entity operation in the undo log.
    pub fn record_delete_entity_undo(&mut self) {
        if let Some(snapshot) = self.capture_scene_snapshot() {
            self.rollback_manager
                .record(None, OperationType::DeleteEntity, Vec::new(), snapshot);
        }
    }

    /// Record a modify-component operation in the undo log.
    pub fn record_modify_component_undo(&mut self) {
        if let Some(snapshot) = self.capture_scene_snapshot() {
            self.rollback_manager.record(
                None,
                OperationType::ModifyComponent,
                Vec::new(),
                snapshot,
            );
        }
    }

    /// Generic undo recording for any operation type with explicit changes.
    pub fn record_operation_undo(&mut self, op_type: OperationType, changes: Vec<Change>) {
        if let Some(snapshot) = self.capture_scene_snapshot() {
            self.rollback_manager
                .record(None, op_type, changes, snapshot);
        }
    }
}
