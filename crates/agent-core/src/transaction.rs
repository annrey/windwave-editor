//! Transaction types — atomic, undoable sequences of engine mutations.
//! Every plan step that touches engine state is wrapped in a transaction so
//! that failures can be rolled back deterministically.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A single atomic edit transaction attached to a plan step.
///
/// Transactions record every operation performed so that they can be replayed
/// or reversed. A transaction is either committed (all operations succeed),
/// rolled back, or partially rolled back (some operations succeeded before a
/// failure).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EditTransaction {
    /// Unique transaction identifier (e.g. `"txn_007"`).
    pub id: String,

    /// The task this transaction belongs to.
    pub task_id: u64,

    /// The plan step that triggered this transaction.
    pub step_id: String,

    /// The agent that opened this transaction, if any.
    pub agent_id: Option<u64>,

    /// Ordered list of forward-operations performed.
    pub operations: Vec<EditOperation>,

    /// Reverse-operations that can undo the forward-operations.
    pub rollback_plan: Vec<RollbackOperation>,

    /// Current lifecycle status.
    pub status: TransactionStatus,

    /// Wall-clock timestamp (ms since UNIX epoch) when the transaction was opened.
    pub started_at_ms: u64,

    /// Wall-clock timestamp (ms since UNIX epoch) when the transaction was
    /// committed or rolled back.
    pub finished_at_ms: Option<u64>,
}

impl EditTransaction {
    /// Open a new transaction in `Open` status.
    pub fn new(
        id: impl Into<String>,
        task_id: u64,
        step_id: impl Into<String>,
        started_at_ms: u64,
    ) -> Self {
        Self {
            id: id.into(),
            task_id,
            step_id: step_id.into(),
            agent_id: None,
            operations: Vec::new(),
            rollback_plan: Vec::new(),
            status: TransactionStatus::Open,
            started_at_ms,
            finished_at_ms: None,
        }
    }

    /// Record a forward-operation and its corresponding rollback operation.
    pub fn record(&mut self, operation: EditOperation, rollback: RollbackOperation) {
        self.operations.push(operation);
        self.rollback_plan.push(rollback);
    }

    /// Mark the transaction as committed.
    pub fn commit(&mut self, finished_at_ms: u64) {
        self.status = TransactionStatus::Committed;
        self.finished_at_ms = Some(finished_at_ms);
    }

    /// Mark the transaction as rolled back.
    pub fn rollback(&mut self, finished_at_ms: u64) {
        self.status = TransactionStatus::RolledBack;
        self.finished_at_ms = Some(finished_at_ms);
    }

    /// Mark the transaction as failed.
    pub fn fail(&mut self, finished_at_ms: u64) {
        self.status = TransactionStatus::Failed;
        self.finished_at_ms = Some(finished_at_ms);
    }

    /// Returns `true` when the transaction is still open.
    pub fn is_open(&self) -> bool {
        self.status == TransactionStatus::Open
    }

    /// Returns `true` when the transaction reached a terminal state.
    pub fn is_finished(&self) -> bool {
        matches!(
            self.status,
            TransactionStatus::Committed
                | TransactionStatus::RolledBack
                | TransactionStatus::PartiallyRolledBack
                | TransactionStatus::Failed
        )
    }
}

/// Terminal states for a transaction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransactionStatus {
    /// The transaction is still accepting operations.
    Open,

    /// All operations succeeded.
    Committed,

    /// The transaction was fully rolled back.
    RolledBack,

    /// Only some operations were rolled back.
    PartiallyRolledBack,

    /// An unrecoverable error occurred during commit or rollback.
    Failed,
}

/// A single forward-mutation performed on the engine or project.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum EditOperation {
    /// Spawn a new entity. The `components_json` holds the initial component
    /// values for the entity.
    CreateEntity {
        entity_name: String,
        components_json: serde_json::Value,
    },

    /// Despawn an entity. `snapshot_before` optionally captures the entity's
    /// state before deletion so that it can be restored.
    DeleteEntity {
        entity_name: String,
        snapshot_before: Option<serde_json::Value>,
    },

    /// Modify a single component on an entity. Both `before` and `after` are
    /// recorded so the change can be reversed precisely.
    UpdateComponent {
        entity_name: String,
        component: String,
        before: serde_json::Value,
        after: serde_json::Value,
    },

    /// Create a new asset file (binary or text).
    CreateAsset { path: String },

    /// Modify a text file. Both the content before and after the edit are
    /// stored so the change can be undone.
    ModifyFile {
        path: String,
        before_content: Option<String>,
        after_content: Option<String>,
    },

    /// Set the Transform translation on an entity.
    SetTransform {
        entity_name: String,
        translation_before: Option<[f32; 3]>,
        translation_after: [f32; 3],
    },

    /// Set the Sprite colour on an entity.
    SetSpriteColor {
        entity_name: String,
        rgba_before: Option<[f32; 4]>,
        rgba_after: [f32; 4],
    },
}

/// A single reverse-operation that undoes a corresponding `EditOperation`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum RollbackOperation {
    /// Delete the entity that was created.
    DeleteEntity { entity_name: String },

    /// Restore a component to its previous value.
    RestoreComponent {
        entity_name: String,
        component: String,
        value: serde_json::Value,
    },

    /// Delete the file that was created.
    DeleteFile { path: String },

    /// Restore the Transform translation to its previous value.
    RestoreTransform {
        entity_name: String,
        translation: [f32; 3],
    },

    /// Restore the Sprite colour to its previous value.
    RestoreSpriteColor {
        entity_name: String,
        rgba: [f32; 4],
    },
}

// ---------------------------------------------------------------------------
// TransactionStore — in-memory registry of all transactions
// ---------------------------------------------------------------------------

/// A simple in-memory store that owns all `EditTransaction` instances and
/// provides lookup by ID, task, and status.
#[derive(Debug, Clone, PartialEq)]
pub struct TransactionStore {
    /// Transaction ID -> transaction.
    transactions: HashMap<String, EditTransaction>,
}

impl TransactionStore {
    /// Create an empty store.
    pub fn new() -> Self {
        Self {
            transactions: HashMap::new(),
        }
    }

    /// Register a new transaction. Returns a clone of the transaction for
    /// convenience (callers typically hold the ID).
    pub fn start(&mut self, transaction: EditTransaction) -> EditTransaction {
        let clone = transaction.clone();
        self.transactions.insert(transaction.id.clone(), transaction);
        clone
    }

    /// Record an operation + rollback pair on an open transaction.
    pub fn record_operation(
        &mut self,
        transaction_id: &str,
        operation: EditOperation,
        rollback: RollbackOperation,
    ) -> Result<(), String> {
        let txn = self
            .transactions
            .get_mut(transaction_id)
            .ok_or_else(|| format!("Transaction {} not found", transaction_id))?;
        if !txn.is_open() {
            return Err(format!("Transaction {} is not open", transaction_id));
        }
        txn.record(operation, rollback);
        Ok(())
    }

    /// Commit an open transaction.
    pub fn commit(&mut self, transaction_id: &str, finished_at_ms: u64) -> Result<(), String> {
        let txn = self
            .transactions
            .get_mut(transaction_id)
            .ok_or_else(|| format!("Transaction {} not found", transaction_id))?;
        if !txn.is_open() {
            return Err(format!("Transaction {} is not open", transaction_id));
        }
        txn.commit(finished_at_ms);
        Ok(())
    }

    /// Roll back an open or committed transaction.
    pub fn rollback(&mut self, transaction_id: &str, finished_at_ms: u64) -> Result<(), String> {
        let txn = self
            .transactions
            .get_mut(transaction_id)
            .ok_or_else(|| format!("Transaction {} not found", transaction_id))?;
        txn.rollback(finished_at_ms);
        Ok(())
    }

    /// Get a reference to a transaction by ID.
    pub fn get(&self, transaction_id: &str) -> Option<&EditTransaction> {
        self.transactions.get(transaction_id)
    }

    /// Get a mutable reference to a transaction by ID.
    pub fn get_mut(&mut self, transaction_id: &str) -> Option<&mut EditTransaction> {
        self.transactions.get_mut(transaction_id)
    }

    /// List all transactions for a given task.
    pub fn list_for_task(&self, task_id: u64) -> Vec<&EditTransaction> {
        self.transactions
            .values()
            .filter(|t| t.task_id == task_id)
            .collect()
    }

    /// Return the total number of stored transactions.
    pub fn len(&self) -> usize {
        self.transactions.len()
    }

    /// Returns `true` when no transactions are stored.
    pub fn is_empty(&self) -> bool {
        self.transactions.is_empty()
    }
}

impl Default for TransactionStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transaction_lifecycle() {
        let mut txn = EditTransaction::new("txn_1", 42, "step_1", 1000);
        assert!(txn.is_open());
        assert!(!txn.is_finished());

        txn.record(
            EditOperation::CreateEntity {
                entity_name: "Enemy".into(),
                components_json: serde_json::json!({"Sprite": {"color": [1,0,0,1]}}),
            },
            RollbackOperation::DeleteEntity {
                entity_name: "Enemy".into(),
            },
        );

        assert_eq!(txn.operations.len(), 1);
        assert_eq!(txn.rollback_plan.len(), 1);

        txn.commit(2000);
        assert!(txn.is_finished());
        assert_eq!(txn.status, TransactionStatus::Committed);
    }

    #[test]
    fn test_transaction_rollback() {
        let mut txn = EditTransaction::new("txn_2", 42, "step_2", 1000);
        txn.rollback(2000);
        assert!(txn.is_finished());
        assert_eq!(txn.status, TransactionStatus::RolledBack);
    }

    #[test]
    fn test_transaction_fail() {
        let mut txn = EditTransaction::new("txn_3", 42, "step_3", 1000);
        txn.fail(2000);
        assert!(txn.is_finished());
        assert_eq!(txn.status, TransactionStatus::Failed);
    }

    #[test]
    fn test_transaction_store_start_and_get() {
        let mut store = TransactionStore::new();
        assert!(store.is_empty());

        let txn = EditTransaction::new("txn_a", 1, "step_a", 100);
        store.start(txn);

        assert_eq!(store.len(), 1);
        let retrieved = store.get("txn_a").unwrap();
        assert_eq!(retrieved.task_id, 1);
        assert!(retrieved.is_open());
    }

    #[test]
    fn test_transaction_store_record_and_commit() {
        let mut store = TransactionStore::new();
        let txn = EditTransaction::new("txn_b", 2, "step_b", 100);
        store.start(txn);

        store
            .record_operation(
                "txn_b",
                EditOperation::CreateEntity {
                    entity_name: "Player".into(),
                    components_json: serde_json::json!({}),
                },
                RollbackOperation::DeleteEntity {
                    entity_name: "Player".into(),
                },
            )
            .expect("record should succeed");

        store.commit("txn_b", 200).expect("commit should succeed");
        assert_eq!(store.get("txn_b").unwrap().status, TransactionStatus::Committed);
    }

    #[test]
    fn test_transaction_store_list_for_task() {
        let mut store = TransactionStore::new();
        store.start(EditTransaction::new("txn_x", 10, "s1", 0));
        store.start(EditTransaction::new("txn_y", 10, "s2", 0));
        store.start(EditTransaction::new("txn_z", 20, "s3", 0));

        let task10 = store.list_for_task(10);
        assert_eq!(task10.len(), 2);

        let task20 = store.list_for_task(20);
        assert_eq!(task20.len(), 1);
    }

    #[test]
    fn test_transaction_store_record_on_closed_fails() {
        let mut store = TransactionStore::new();
        let txn = EditTransaction::new("txn_c", 3, "step_c", 100);
        store.start(txn);
        store.commit("txn_c", 200).unwrap();

        let result = store.record_operation(
            "txn_c",
            EditOperation::CreateEntity { entity_name: "X".into(), components_json: serde_json::json!({}) },
            RollbackOperation::DeleteEntity { entity_name: "X".into() },
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_transaction_store_commit_nonexistent_fails() {
        let mut store = TransactionStore::new();
        let result = store.commit("nonexistent", 100);
        assert!(result.is_err());
    }

    #[test]
    fn test_edit_operation_variants() {
        let op = EditOperation::SetTransform {
            entity_name: "Player".into(),
            translation_before: Some([0.0, 0.0, 0.0]),
            translation_after: [100.0, 50.0, 0.0],
        };
        let _ = format!("{:?}", op);

        let op2 = EditOperation::SetSpriteColor {
            entity_name: "Enemy".into(),
            rgba_before: None,
            rgba_after: [0.0, 0.0, 1.0, 1.0],
        };
        let _ = format!("{:?}", op2);
    }
}
