use crate::permission::OperationRisk;

// ===========================================================================
// Revision Type - 修订类型枚举
// ===========================================================================

/// Types of plan revisions that can be applied dynamically.
#[derive(Debug, Clone, PartialEq)]
pub enum RevisionType {
    /// Skip the next N steps (e.g., entity already exists).
    Skip { count: usize, reason: String },
    /// Insert a new step before the current position.
    InsertBefore {
        index: usize,
        step: crate::plan::EditPlanStep,
        reason: String,
    },
    /// Insert a new step after the current position.
    InsertAfter {
        index: usize,
        step: crate::plan::EditPlanStep,
        reason: String,
    },
    /// Replace a step's action with an alternative.
    Replace {
        index: usize,
        original_title: String,
        new_step: crate::plan::EditPlanStep,
        reason: String,
    },
    /// Adapt a failed step to use a lower-risk approach.
    Adapt {
        index: usize,
        adaptation: String,
        from_risk: OperationRisk,
        to_risk: OperationRisk,
    },
    /// Abort the entire plan (unrecoverable error).
    Abort { reason: String },
}

impl RevisionType {
    /// Get human-readable description of this revision.
    pub fn describe(&self) -> String {
        match self {
            RevisionType::Skip { count, reason } => {
                format!("Skip next {} steps: {}", count, reason)
            }
            RevisionType::InsertBefore {
                index,
                step,
                reason,
            } => {
                format!("Insert '{}' before step {}: {}", step.title, index, reason)
            }
            RevisionType::InsertAfter {
                index,
                step,
                reason,
            } => {
                format!("Insert '{}' after step {}: {}", step.title, index, reason)
            }
            RevisionType::Replace {
                index, new_step, ..
            } => {
                format!("Replace step {} with '{}'", index, new_step.title)
            }
            RevisionType::Adapt {
                index, adaptation, ..
            } => {
                format!("Adapt step {}: {}", index, adaptation)
            }
            RevisionType::Abort { reason } => {
                format!("ABORT: {}", reason)
            }
        }
    }

    /// Check if this revision is safe to apply automatically.
    pub fn is_safe_auto_apply(&self) -> bool {
        matches!(
            self,
            RevisionType::Skip { .. }
                | RevisionType::InsertBefore { .. }
                | RevisionType::InsertAfter { .. }
                | RevisionType::Adapt { .. }
        )
    }
}
