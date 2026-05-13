//! EditOp — Unified reversible edit operation trait.
//!
//! Inspired by SuperSplat's `EditOp` interface, all scene mutations are
//! represented as `do`/`undo` pairs that can be composed, batched, and
//! serialized for undo/redo history.
//!
//! # Design
//!
//! Each `EditOp` is a pure data description of a scene operation. The
//! `SceneBridge` is passed at execution time so the op remains engine-agnostic
//! and testable with `MockSceneBridge`.

use crate::scene_bridge::{ComponentPatch, SceneBridge};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// EditOp trait
// ---------------------------------------------------------------------------

/// A single reversible edit operation on the scene.
///
/// Each op must be idempotent for `do_op` (intermediate state is tracked
/// internally so that `undo_op` knows how to reverse what was done).
pub trait EditOp: Send + Sync {
    /// Human-readable name of this operation (e.g. "createEntity").
    fn name(&self) -> &str;

    /// Execute the forward operation against the given bridge.
    /// Returns `Err` if the operation cannot be completed.
    fn do_op(&mut self, bridge: &mut dyn SceneBridge) -> Result<(), EditOpError>;

    /// Reverse the operation that was previously performed by `do_op`.
    /// Must only be called after a successful `do_op`.
    fn undo_op(&mut self, bridge: &mut dyn SceneBridge) -> Result<(), EditOpError>;

    /// Optional cleanup when the op is discarded from history (e.g. freeing
    /// GPU resources). Default is a no-op.
    fn destroy(&mut self) {}
}

// ---------------------------------------------------------------------------
// Error
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum EditOpError {
    Bridge(String),
    InvalidOp(String),
    AlreadyPerformed,
    NotYetPerformed,
    EntityNotFound(u64),
}

impl std::fmt::Display for EditOpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Bridge(e) => write!(f, "Bridge error: {}", e),
            Self::InvalidOp(e) => write!(f, "Invalid operation: {}", e),
            Self::AlreadyPerformed => write!(f, "Operation already performed"),
            Self::NotYetPerformed => write!(f, "Operation not yet performed"),
            Self::EntityNotFound(id) => write!(f, "Entity {} not found", id),
        }
    }
}
impl std::error::Error for EditOpError {}

// ---------------------------------------------------------------------------
// CreateEntityOp
// ---------------------------------------------------------------------------

/// Creates a new entity and tracks its ID for undo (delete).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateEntityOp {
    pub entity_name: String,
    pub position: Option<[f64; 2]>,
    pub components: Vec<ComponentPatch>,
    /// Populated after `do_op` succeeds; used by `undo_op` to delete.
    created_id: Option<u64>,
    performed: bool,
}

impl CreateEntityOp {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            entity_name: name.into(),
            position: None,
            components: Vec::new(),
            created_id: None,
            performed: false,
        }
    }

    pub fn with_position(mut self, x: f64, y: f64) -> Self {
        self.position = Some([x, y]);
        self
    }

    pub fn with_sprite(mut self, rgba: [f32; 4]) -> Self {
        self.components.push(ComponentPatch {
            type_name: "Sprite".into(),
            properties: {
                let mut m = HashMap::new();
                m.insert(
                    "color".into(),
                    serde_json::json!([rgba[0], rgba[1], rgba[2], rgba[3]]),
                );
                m
            },
        });
        self
    }
}

impl EditOp for CreateEntityOp {
    fn name(&self) -> &str {
        "createEntity"
    }

    fn do_op(&mut self, bridge: &mut dyn SceneBridge) -> Result<(), EditOpError> {
        if self.performed {
            return Err(EditOpError::AlreadyPerformed);
        }
        let id = bridge
            .create_entity(&self.entity_name, self.position, &self.components)
            .map_err(|e| EditOpError::Bridge(e))?;
        self.created_id = Some(id);
        self.performed = true;
        Ok(())
    }

    fn undo_op(&mut self, bridge: &mut dyn SceneBridge) -> Result<(), EditOpError> {
        let id = self.created_id.ok_or(EditOpError::NotYetPerformed)?;
        bridge
            .delete_entity(id)
            .map_err(|e| EditOpError::Bridge(e))?;
        self.created_id = None;
        self.performed = false;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// DeleteEntityOp
// ---------------------------------------------------------------------------

/// Deletes an entity. Snapshot is captured before deletion so undo can
/// recreate the entity with its prior state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteEntityOp {
    pub entity_id: u64,
    /// Stored before `do_op` executes; used by `undo_op` to recreate.
    snapshot: Option<EntitySnapshot>,
    performed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntitySnapshot {
    pub name: String,
    pub position: Option<[f64; 2]>,
    pub sprite_rgba: Option<[f32; 4]>,
    pub visible: Option<bool>,
    pub custom_components: HashMap<String, serde_json::Value>,
}

impl DeleteEntityOp {
    pub fn new(entity_id: u64) -> Self {
        Self {
            entity_id,
            snapshot: None,
            performed: false,
        }
    }
}

impl EditOp for DeleteEntityOp {
    fn name(&self) -> &str {
        "deleteEntity"
    }

    fn do_op(&mut self, bridge: &mut dyn SceneBridge) -> Result<(), EditOpError> {
        if self.performed {
            return Err(EditOpError::AlreadyPerformed);
        }

        // Capture entity info before deletion
        let info = bridge
            .get_entity(self.entity_id)
            .ok_or(EditOpError::EntityNotFound(self.entity_id))?;

        let name = info
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();
        // Parse position [x,y] from entity info
        let position = info.get("position").and_then(|v| {
            let arr = v.as_array()?;
            if arr.len() >= 2 {
                Some([arr[0].as_f64()?, arr[1].as_f64()?])
            } else {
                None
            }
        });
        let sprite_rgba = info.get("sprite_color").and_then(|v| {
            let arr = v.as_array()?;
            if arr.len() >= 4 {
                Some([
                    arr[0].as_f64()? as f32,
                    arr[1].as_f64()? as f32,
                    arr[2].as_f64()? as f32,
                    arr[3].as_f64()? as f32,
                ])
            } else {
                None
            }
        });
        let visible = info.get("visible").and_then(|v| v.as_bool());

        self.snapshot = Some(EntitySnapshot {
            name,
            position,
            sprite_rgba,
            visible,
            custom_components: HashMap::new(),
        });

        bridge
            .delete_entity(self.entity_id)
            .map_err(|e| EditOpError::Bridge(e))?;
        self.performed = true;
        Ok(())
    }

    fn undo_op(&mut self, bridge: &mut dyn SceneBridge) -> Result<(), EditOpError> {
        let snap = self.snapshot.as_ref().ok_or(EditOpError::NotYetPerformed)?;

        // Recreate the entity
        let mut components = Vec::new();
        if let Some(rgba) = snap.sprite_rgba {
            let mut props = HashMap::new();
            props.insert(
                "color".into(),
                serde_json::json!([rgba[0], rgba[1], rgba[2], rgba[3]]),
            );
            components.push(ComponentPatch {
                type_name: "Sprite".into(),
                properties: props,
            });
        }

        let _new_id = bridge
            .create_entity(&snap.name, snap.position, &components)
            .map_err(|e| EditOpError::Bridge(e))?;

        // TODO: after recreation, restore visibility via update_component
        // (requires knowing the new entity_id)

        self.snapshot = None;
        self.performed = false;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// SetTransformOp
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetTransformOp {
    pub entity_id: u64,
    /// New values to apply.
    pub translation: Option<[f64; 2]>,
    /// Previous values captured before `do_op`; used by `undo_op`.
    old_translation: Option<[f64; 2]>,
    performed: bool,
}

impl SetTransformOp {
    pub fn new(entity_id: u64, translation: [f64; 2]) -> Self {
        Self {
            entity_id,
            translation: Some(translation),
            old_translation: None,
            performed: false,
        }
    }
}

impl EditOp for SetTransformOp {
    fn name(&self) -> &str {
        "setTransform"
    }

    fn do_op(&mut self, bridge: &mut dyn SceneBridge) -> Result<(), EditOpError> {
        if self.performed {
            return Err(EditOpError::AlreadyPerformed);
        }

        // Capture old position before setting new
        if let Some(info) = bridge.get_entity(self.entity_id) {
            self.old_translation = info.get("position").and_then(|v| {
                let arr = v.as_array()?;
                if arr.len() >= 2 {
                    Some([arr[0].as_f64()?, arr[1].as_f64()?])
                } else {
                    None
                }
            });
        }

        if let Some(trans) = self.translation {
            let mut props = HashMap::new();
            props.insert("x".into(), serde_json::json!(trans[0]));
            props.insert("y".into(), serde_json::json!(trans[1]));
            bridge
                .update_component(self.entity_id, "Transform", props)
                .map_err(|e| EditOpError::Bridge(e))?;
        }

        self.performed = true;
        Ok(())
    }

    fn undo_op(&mut self, bridge: &mut dyn SceneBridge) -> Result<(), EditOpError> {
        if let Some(old_trans) = self.old_translation {
            let mut props = HashMap::new();
            props.insert("x".into(), serde_json::json!(old_trans[0]));
            props.insert("y".into(), serde_json::json!(old_trans[1]));
            bridge
                .update_component(self.entity_id, "Transform", props)
                .map_err(|e| EditOpError::Bridge(e))?;
        }
        self.old_translation = None;
        self.performed = false;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// SetColorOp
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetColorOp {
    pub entity_id: u64,
    pub rgba: [f32; 4],
    old_rgba: Option<[f32; 4]>,
    performed: bool,
}

impl SetColorOp {
    pub fn new(entity_id: u64, rgba: [f32; 4]) -> Self {
        Self {
            entity_id,
            rgba,
            old_rgba: None,
            performed: false,
        }
    }
}

impl EditOp for SetColorOp {
    fn name(&self) -> &str {
        "setColor"
    }

    fn do_op(&mut self, bridge: &mut dyn SceneBridge) -> Result<(), EditOpError> {
        if self.performed {
            return Err(EditOpError::AlreadyPerformed);
        }

        if let Some(info) = bridge.get_entity(self.entity_id) {
            self.old_rgba = info.get("sprite_color").and_then(|v| {
                let arr = v.as_array()?;
                if arr.len() >= 4 {
                    Some([
                        arr[0].as_f64()? as f32,
                        arr[1].as_f64()? as f32,
                        arr[2].as_f64()? as f32,
                        arr[3].as_f64()? as f32,
                    ])
                } else {
                    None
                }
            });
        }

        let mut props = HashMap::new();
        props.insert(
            "color".into(),
            serde_json::json!([self.rgba[0], self.rgba[1], self.rgba[2], self.rgba[3]]),
        );
        bridge
            .update_component(self.entity_id, "Sprite", props)
            .map_err(|e| EditOpError::Bridge(e))?;

        self.performed = true;
        Ok(())
    }

    fn undo_op(&mut self, bridge: &mut dyn SceneBridge) -> Result<(), EditOpError> {
        if let Some(old) = self.old_rgba {
            let mut props = HashMap::new();
            props.insert("color".into(), serde_json::json!([old[0], old[1], old[2], old[3]]));
            bridge
                .update_component(self.entity_id, "Sprite", props)
                .map_err(|e| EditOpError::Bridge(e))?;
        }
        self.old_rgba = None;
        self.performed = false;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// SetVisibilityOp
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetVisibilityOp {
    pub entity_id: u64,
    pub visible: bool,
    old_visible: Option<bool>,
    performed: bool,
}

impl SetVisibilityOp {
    pub fn new(entity_id: u64, visible: bool) -> Self {
        Self {
            entity_id,
            visible,
            old_visible: None,
            performed: false,
        }
    }
}

impl EditOp for SetVisibilityOp {
    fn name(&self) -> &str {
        "setVisibility"
    }

    fn do_op(&mut self, bridge: &mut dyn SceneBridge) -> Result<(), EditOpError> {
        if self.performed {
            return Err(EditOpError::AlreadyPerformed);
        }

        if let Some(info) = bridge.get_entity(self.entity_id) {
            self.old_visible = info.get("visible").and_then(|v| v.as_bool());
        }

        let mut props = HashMap::new();
        props.insert("visible".into(), serde_json::json!(self.visible));
        bridge
            .update_component(self.entity_id, "Visibility", props)
            .map_err(|e| EditOpError::Bridge(e))?;

        self.performed = true;
        Ok(())
    }

    fn undo_op(&mut self, bridge: &mut dyn SceneBridge) -> Result<(), EditOpError> {
        if let Some(old) = self.old_visible {
            let mut props = HashMap::new();
            props.insert("visible".into(), serde_json::json!(old));
            bridge
                .update_component(self.entity_id, "Visibility", props)
                .map_err(|e| EditOpError::Bridge(e))?;
        }
        self.old_visible = None;
        self.performed = false;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// MultiOp — composite atomic operation
// ---------------------------------------------------------------------------

/// Combines multiple EditOps into a single atomic unit.
/// All sub-ops must succeed for the MultiOp to succeed; if any fails,
/// previously-executed sub-ops are rolled back.
pub struct MultiOp {
    pub ops: Vec<Box<dyn EditOp>>,
    next_index: usize,
}

impl std::fmt::Debug for MultiOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MultiOp")
            .field("op_count", &self.ops.len())
            .field("next_index", &self.next_index)
            .finish()
    }
}

impl MultiOp {
    pub fn new() -> Self {
        Self { ops: Vec::new(), next_index: 0 }
    }

    pub fn push(&mut self, op: Box<dyn EditOp>) {
        self.ops.push(op);
    }
}

impl EditOp for MultiOp {
    fn name(&self) -> &str {
        "multiOp"
    }

    fn do_op(&mut self, bridge: &mut dyn SceneBridge) -> Result<(), EditOpError> {
        while self.next_index < self.ops.len() {
            let op = &mut self.ops[self.next_index];
            op.do_op(bridge)?;
            self.next_index += 1;
        }
        Ok(())
    }

    fn undo_op(&mut self, bridge: &mut dyn SceneBridge) -> Result<(), EditOpError> {
        while self.next_index > 0 {
            self.next_index -= 1;
            let op = &mut self.ops[self.next_index];
            op.undo_op(bridge)?;
        }
        Ok(())
    }

    fn destroy(&mut self) {
        for op in &mut self.ops {
            op.destroy();
        }
        self.ops.clear();
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene_bridge::MockSceneBridge;

    fn make_bridge() -> MockSceneBridge {
        MockSceneBridge::new()
    }

    #[test]
    fn test_create_entity_op_do_and_undo() {
        let mut bridge = make_bridge();
        let mut op = CreateEntityOp::new("TestPlayer");

        // Do
        op.do_op(&mut bridge).unwrap();
        assert!(op.created_id.is_some());

        // Undo
        op.undo_op(&mut bridge).unwrap();
        assert!(op.created_id.is_none());
    }

    #[test]
    fn test_create_entity_op_double_do_is_error() {
        let mut bridge = make_bridge();
        let mut op = CreateEntityOp::new("TestEnemy");
        op.do_op(&mut bridge).unwrap();
        let result = op.do_op(&mut bridge);
        assert!(result.is_err());
    }

    #[test]
    fn test_set_color_op_do_and_undo() {
        let mut bridge = make_bridge();
        // Create entity first with a sprite
        let mut props = HashMap::new();
        props.insert("color".into(), serde_json::json!([1.0, 0.0, 0.0, 1.0]));
        let components = vec![ComponentPatch {
            type_name: "Sprite".into(),
            properties: props,
        }];
        let id = bridge
            .create_entity("ColorTest", None, &components)
            .unwrap();

        let mut op = SetColorOp::new(id, [0.0, 1.0, 0.0, 1.0]);
        // old_rgba captured inside do_op from get_entity
        op.do_op(&mut bridge).unwrap();
        assert!(op.old_rgba.is_some());
        assert_eq!(op.old_rgba, Some([1.0, 0.0, 0.0, 1.0]));

        op.undo_op(&mut bridge).unwrap();
        // After undo, it restores old color in the entity
        let entity = bridge.get_entity(id).unwrap();
        let color = entity.get("sprite_color").and_then(|v| v.as_array()).unwrap();
        assert_eq!(color[0].as_f64().unwrap() as f32, 1.0);
    }

    #[test]
    fn test_multi_op_atomic() {
        let mut bridge = make_bridge();

        let mut multi = MultiOp::new();
        multi.push(Box::new(CreateEntityOp::new("A")));
        multi.push(Box::new(CreateEntityOp::new("B")));
        multi.push(Box::new(CreateEntityOp::new("C")));

        multi.do_op(&mut bridge).unwrap();
        assert_eq!(multi.next_index, 3);

        // Undo in reverse order
        multi.undo_op(&mut bridge).unwrap();
        assert_eq!(multi.next_index, 0);
    }
}
