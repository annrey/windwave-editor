//! Rollback System — operation log with undo/redo stacks.
//!
//! Implements Design Document Section 6: every mutating operation is captured
//! as an `OperationLog` entry together with a pre-operation scene snapshot.
//! The `RollbackManager` maintains undo and redo stacks, allowing the user
//! (or the Director) to step backward and forward through the edit history.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

// ============================================================================
// Operation log entry
// ============================================================================

/// Unique identifier for an operation log entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct OperationId(pub u64);

/// A recorded editor mutation that can be undone or redone.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationLog {
    pub id: OperationId,
    pub timestamp: DateTime<Utc>,
    pub agent_id: Option<u64>,
    pub operation_type: OperationType,
    pub changes: Vec<Change>,
    pub snapshot: SceneSnapshot,
}

impl OperationLog {
    pub fn new(
        id: OperationId,
        agent_id: Option<u64>,
        operation_type: OperationType,
        changes: Vec<Change>,
        snapshot: SceneSnapshot,
    ) -> Self {
        Self {
            id,
            timestamp: Utc::now(),
            agent_id,
            operation_type,
            changes,
            snapshot,
        }
    }
}

/// Human-readable classification of the logged operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OperationType {
    CreateEntity,
    DeleteEntity,
    ModifyComponent,
    ModifyCode,
    ModifyAsset,
    BatchOperation,
    Custom(String),
}

/// A single delta produced by an operation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Change {
    SceneChange {
        entity_name: String,
        component: String,
        before: serde_json::Value,
        after: serde_json::Value,
    },
    CodeChange {
        file: PathBuf,
        before: String,
        after: String,
    },
    AssetChange {
        asset_path: PathBuf,
        action: AssetAction,
    },
}

/// What happened to an asset.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AssetAction {
    Created,
    Modified,
    Deleted,
    Renamed { old_path: PathBuf },
}

// ============================================================================
// Scene snapshot — captures full state before an operation
// ============================================================================

/// A lightweight snapshot of the scene state before a mutation.
///
/// Stored inline in the `OperationLog` so that undo can restore the exact
/// pre-mutation state without needing to reverse-engineer diffs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneSnapshot {
    /// Entities present in the scene at the time of the snapshot.
    pub entities: Vec<SnapshotEntity>,
    /// Monotonically increasing snapshot index.
    pub sequence: u64,
}

impl SceneSnapshot {
    pub fn new(sequence: u64) -> Self {
        Self {
            entities: Vec::new(),
            sequence,
        }
    }

    pub fn with_entities(sequence: u64, entities: Vec<SnapshotEntity>) -> Self {
        Self { entities, sequence }
    }
}

/// A simplified representation of one entity in a snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotEntity {
    pub name: String,
    pub component_names: Vec<String>,
    pub serialized_state: serde_json::Value,
}

// ============================================================================
// RollbackManager — undo / redo stacks
// ============================================================================

/// The central undo/redo controller.
///
/// Maintains two stacks:
/// - `undo_stack`: operations that can be undone (most recent first).
/// - `redo_stack`: operations that were undone and can be redone.
///
/// Pushing a new operation clears the redo stack (standard editor behaviour).
pub struct RollbackManager {
    undo_stack: Vec<OperationLog>,
    redo_stack: Vec<OperationLog>,
    max_undo_steps: usize,
    next_op_id: u64,
    next_snapshot_seq: u64,
}

impl RollbackManager {
    /// Create a rollback manager that retains at most `max_undo_steps`
    /// operations in the undo stack.
    pub fn new(max_undo_steps: usize) -> Self {
        Self {
            undo_stack: Vec::with_capacity(max_undo_steps),
            redo_stack: Vec::new(),
            max_undo_steps,
            next_op_id: 0,
            next_snapshot_seq: 0,
        }
    }

    // ------------------------------------------------------------------
    // Recording
    // ------------------------------------------------------------------

    /// Record a new operation and push it onto the undo stack.
    ///
    /// This automatically clears the redo stack (new action invalidates
    /// the redo history).
    pub fn record(
        &mut self,
        agent_id: Option<u64>,
        operation_type: OperationType,
        changes: Vec<Change>,
        snapshot: SceneSnapshot,
    ) -> &OperationLog {
        let id = OperationId(self.next_op_id);
        self.next_op_id += 1;

        let log = OperationLog::new(id, agent_id, operation_type, changes, snapshot);
        self.undo_stack.push(log);

        // Enforce capacity — drop oldest entry.
        if self.undo_stack.len() > self.max_undo_steps {
            self.undo_stack.remove(0);
        }

        // Clear redo stack.
        self.redo_stack.clear();

        self.undo_stack.last().unwrap()
    }

    /// Build a snapshot from the current scene representation and return its
    /// sequence number so callers can attach it to the next `record()` call.
    pub fn capture_snapshot(&mut self, entities: Vec<SnapshotEntity>) -> SceneSnapshot {
        let seq = self.next_snapshot_seq;
        self.next_snapshot_seq += 1;
        SceneSnapshot::with_entities(seq, entities)
    }

    // ------------------------------------------------------------------
    // Undo / Redo
    // ------------------------------------------------------------------

    /// Undo the most recent operation.
    ///
    /// Returns `Some((OperationLog, &[OperationLog]))` where the tuple holds
    /// the undone entry and a reference to the *remaining* undo stack (so
    /// callers can re-apply the previous snapshot).
    ///
    /// Returns `None` when there is nothing to undo.
    pub fn undo(&mut self) -> Option<&OperationLog> {
        let op = self.undo_stack.pop()?;
        self.redo_stack.push(op);
        self.undo_stack.last()
    }

    /// Redo the most recently undone operation.
    ///
    /// Returns `Some(&OperationLog)` — reference to the restored log entry.
    /// Returns `None` when the redo stack is empty.
    pub fn redo(&mut self) -> Option<&OperationLog> {
        let op = self.redo_stack.pop()?;
        self.undo_stack.push(op);
        self.undo_stack.last()
    }

    // ------------------------------------------------------------------
    // Inspection
    // ------------------------------------------------------------------

    /// Whether there is anything to undo.
    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    /// Whether there is anything to redo.
    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    /// Total operations currently in the undo stack.
    pub fn undo_count(&self) -> usize {
        self.undo_stack.len()
    }

    /// Total operations currently in the redo stack.
    pub fn redo_count(&self) -> usize {
        self.redo_stack.len()
    }

    /// Peek at the top of the undo stack without removing it.
    pub fn peek_undo(&self) -> Option<&OperationLog> {
        self.undo_stack.last()
    }

    /// Peek at the top of the redo stack.
    pub fn peek_redo(&self) -> Option<&OperationLog> {
        self.redo_stack.last()
    }

    /// Clear both stacks.
    pub fn clear(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
    }
}

impl Default for RollbackManager {
    fn default() -> Self {
        Self::new(50)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn make_snapshot(seq: u64) -> SceneSnapshot {
        SceneSnapshot::new(seq)
    }

    #[test]
    fn test_record_and_undo() {
        let mut rm = RollbackManager::new(10);

        rm.record(
            None,
            OperationType::CreateEntity,
            vec![],
            make_snapshot(0),
        );
        assert!(rm.can_undo());
        assert!(!rm.can_redo());

        let _ = rm.undo();
        assert!(!rm.can_undo()); // only one entry
        assert!(rm.can_redo());
    }

    #[test]
    fn test_redo() {
        let mut rm = RollbackManager::new(10);

        rm.record(None, OperationType::CreateEntity, vec![], make_snapshot(0));
        let _ = rm.undo();
        assert!(rm.can_redo());

        let _ = rm.redo();
        assert!(rm.can_undo());
        assert!(!rm.can_redo());
    }

    #[test]
    fn test_new_action_clears_redo() {
        let mut rm = RollbackManager::new(10);

        rm.record(None, OperationType::CreateEntity, vec![], make_snapshot(0));
        let _ = rm.undo();
        assert!(rm.can_redo());

        rm.record(None, OperationType::ModifyComponent, vec![], make_snapshot(1));
        assert!(!rm.can_redo()); // redo stack cleared
        assert!(rm.can_undo());
    }

    #[test]
    fn test_max_undo_steps() {
        let mut rm = RollbackManager::new(3);

        for i in 0..5 {
            rm.record(None, OperationType::Custom(format!("op {}", i)), vec![], make_snapshot(i));
        }
        assert_eq!(rm.undo_count(), 3);
        // oldest two entries were evicted.
    }

    #[test]
    fn test_double_undo() {
        let mut rm = RollbackManager::new(10);

        rm.record(None, OperationType::CreateEntity, vec![], make_snapshot(0));
        rm.record(None, OperationType::ModifyComponent, vec![], make_snapshot(1));

        assert_eq!(rm.undo_count(), 2);
        let _ = rm.undo(); // undo modify
        assert_eq!(rm.undo_count(), 1);
        assert_eq!(rm.redo_count(), 1);
    }

    #[test]
    fn test_clear() {
        let mut rm = RollbackManager::new(10);
        rm.record(None, OperationType::CreateEntity, vec![], make_snapshot(0));
        rm.record(None, OperationType::ModifyComponent, vec![], make_snapshot(1));
        rm.clear();
        assert!(!rm.can_undo());
        assert!(!rm.can_redo());
    }

    #[test]
    fn test_peek_undo() {
        let mut rm = RollbackManager::new(10);
        rm.record(None, OperationType::CreateEntity, vec![], make_snapshot(0));

        let peeked = rm.peek_undo().unwrap();
        assert_eq!(peeked.operation_type, OperationType::CreateEntity);
        assert!(rm.can_undo()); // peek does not consume
    }

    #[test]
    fn test_capture_snapshot_increment() {
        let mut rm = RollbackManager::new(10);
        let s1 = rm.capture_snapshot(vec![]);
        let s2 = rm.capture_snapshot(vec![]);
        assert_eq!(s1.sequence, 0);
        assert_eq!(s2.sequence, 1);
    }
}
