use super::RevisionType;

// ===========================================================================
// Revision History - 修订历史记录
// ===========================================================================

/// A single revision applied to a plan.
#[derive(Debug, Clone)]
pub struct RevisionEntry {
    /// Timestamp when revision was applied (ms since epoch).
    pub timestamp_ms: u64,
    /// Type of revision applied.
    pub revision_type: RevisionType,
    /// Plan ID this revision was applied to.
    pub plan_id: String,
    /// Step index where revision was applied.
    pub step_index: usize,
    /// Original observation that triggered revision.
    pub trigger_observation: String,
    /// Whether this revision was auto-applied or user-approved.
    pub auto_applied: bool,
}

impl RevisionEntry {
    /// Create a new revision entry.
    pub fn new(
        revision_type: RevisionType,
        plan_id: &str,
        step_index: usize,
        trigger_observation: &str,
        auto_applied: bool,
    ) -> Self {
        Self {
            timestamp_ms: crate::types::now_millis(),
            revision_type,
            plan_id: plan_id.to_string(),
            step_index,
            trigger_observation: trigger_observation.to_string(),
            auto_applied,
        }
    }

    /// Get human-readable summary.
    pub fn summary(&self) -> String {
        format!(
            "[{}] {} @ step {} (auto={})",
            self.timestamp_ms,
            self.revision_type.describe(),
            self.step_index,
            self.auto_applied
        )
    }
}
