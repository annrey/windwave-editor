//! EditHistory — undo/redo stack backed by `EditOp` trait objects.
//!
//! Inspired by SuperSplat's async edit history, this provides a synchronous
//! undo/redo stack where each entry is a `Box<dyn EditOp>`. Operations that
//! have been `do`ne are pushed onto the undo stack; calling `undo()` moves
//! the top entry to the redo stack after reversing it.
//!
//! # Usage
//!
//! ```ignore
//! let mut history = EditHistory::new(50);
//! let mut op = Box::new(CreateEntityOp::new("Player"));
//! op.do_op(bridge)?;
//! history.push(op);                    // undo: delete the entity
//! history.undo(bridge)?;              // entity deleted, op on redo stack
//! history.redo(bridge)?;              // entity recreated, op back on undo stack
//! ```

use crate::edit_ops::{EditOp, EditOpError};
use crate::scene_bridge::SceneBridge;

/// Manages a stack of executed `EditOp` instances for undo/redo.
pub struct EditHistory {
    /// Executed ops that can be undone (most recent first).
    undo_stack: Vec<Box<dyn EditOp>>,
    /// Reversed ops that can be redone (most recent first).
    redo_stack: Vec<Box<dyn EditOp>>,
    /// Maximum number of undo entries to retain.
    max_undo: usize,
}

impl EditHistory {
    /// Create a new edit history with a maximum undo depth.
    pub fn new(max_undo: usize) -> Self {
        Self {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            max_undo,
        }
    }

    /// Push an already-executed operation onto the undo stack.
    /// Any pending redo entries are discarded (new action invalidates redo).
    pub fn push(&mut self, op: Box<dyn EditOp>) {
        self.undo_stack.push(op);
        self.redo_stack.clear();
        // Trim oldest entry if over capacity
        if self.undo_stack.len() > self.max_undo {
            let mut old = self.undo_stack.remove(0);
            old.destroy();
        }
    }

    /// Undo the most recent operation. Moves it to the redo stack.
    /// Returns `true` if an undo was performed, `false` if stack is empty.
    pub fn undo(&mut self, bridge: &mut dyn SceneBridge) -> Result<bool, EditOpError> {
        if let Some(mut op) = self.undo_stack.pop() {
            op.undo_op(bridge)?;
            self.redo_stack.push(op);
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Redo the most recently undone operation. Moves it back to the undo stack.
    /// Returns `true` if a redo was performed, `false` if stack is empty.
    pub fn redo(&mut self, bridge: &mut dyn SceneBridge) -> Result<bool, EditOpError> {
        if let Some(mut op) = self.redo_stack.pop() {
            op.do_op(bridge)?;
            self.undo_stack.push(op);
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Returns `true` if there are entries on the undo stack.
    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    /// Returns `true` if there are entries on the redo stack.
    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    /// Number of entries on the undo stack.
    pub fn undo_count(&self) -> usize {
        self.undo_stack.len()
    }

    /// Number of entries on the redo stack.
    pub fn redo_count(&self) -> usize {
        self.redo_stack.len()
    }

    /// Clear all history, destroying every op.
    pub fn clear(&mut self) {
        for mut op in self.undo_stack.drain(..) {
            op.destroy();
        }
        for mut op in self.redo_stack.drain(..) {
            op.destroy();
        }
    }

    /// Get the name of the top operation on the undo stack.
    pub fn top_undo_name(&self) -> Option<&str> {
        self.undo_stack.last().map(|op| op.name())
    }

    /// Get the name of the top operation on the redo stack.
    pub fn top_redo_name(&self) -> Option<&str> {
        self.redo_stack.last().map(|op| op.name())
    }
}

impl Drop for EditHistory {
    fn drop(&mut self) {
        self.clear();
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edit_ops::{CreateEntityOp, SetColorOp};
    use crate::scene_bridge::MockSceneBridge;

    #[test]
    fn test_undo_redo_create_entity() {
        let mut bridge = MockSceneBridge::new();
        let mut history = EditHistory::new(10);

        // Create entity and push to history
        let mut op = Box::new(CreateEntityOp::new("Test"));
        op.do_op(&mut bridge).unwrap();
        history.push(op);

        assert!(history.can_undo());
        assert!(!history.can_redo());

        // Undo: entity should be deleted
        assert!(history.undo(&mut bridge).unwrap());
        assert!(!history.can_undo());
        assert!(history.can_redo());

        // Redo: entity should be recreated
        assert!(history.redo(&mut bridge).unwrap());
        assert!(history.can_undo());
        assert!(!history.can_redo());
    }

    #[test]
    fn test_multiple_undos() {
        let mut bridge = MockSceneBridge::new();
        let mut history = EditHistory::new(10);

        // Push 3 ops
        for name in &["A", "B", "C"] {
            let mut op = Box::new(CreateEntityOp::new(*name));
            op.do_op(&mut bridge).unwrap();
            history.push(op);
        }

        assert_eq!(history.undo_count(), 3);

        // Undo twice
        assert!(history.undo(&mut bridge).unwrap());
        assert!(history.undo(&mut bridge).unwrap());
        assert_eq!(history.undo_count(), 1);
        assert_eq!(history.redo_count(), 2);

        // Redo once
        assert!(history.redo(&mut bridge).unwrap());
        assert_eq!(history.undo_count(), 2);
        assert_eq!(history.redo_count(), 1);
    }

    #[test]
    fn test_new_action_clears_redo() {
        let mut bridge = MockSceneBridge::new();
        let mut history = EditHistory::new(10);

        let mut op = Box::new(CreateEntityOp::new("First"));
        op.do_op(&mut bridge).unwrap();
        history.push(op);

        // Undo
        assert!(history.undo(&mut bridge).unwrap());
        assert!(history.can_redo());

        // New action: redo stack should be cleared
        let mut op2 = Box::new(CreateEntityOp::new("Second"));
        op2.do_op(&mut bridge).unwrap();
        history.push(op2);

        assert!(!history.can_redo());
    }

    #[test]
    fn test_max_undo_trims_oldest() {
        let mut bridge = MockSceneBridge::new();
        let mut history = EditHistory::new(2);

        for name in &["1", "2", "3"] {
            let mut op = Box::new(CreateEntityOp::new(*name));
            op.do_op(&mut bridge).unwrap();
            history.push(op);
        }

        // Should only keep 2
        assert_eq!(history.undo_count(), 2);
    }
}
