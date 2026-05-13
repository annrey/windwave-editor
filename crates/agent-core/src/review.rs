//! Review types — the output of the Reviewer agent. After every plan step
//! (or at task completion), the Reviewer inspects the current state against the
//! goal and produces a decision: accept, retry, rollback, or escalate to the user.

use serde::{Deserialize, Serialize};

/// A structured summary produced by the Reviewer agent after evaluating a
/// completed (or partially completed) task.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReviewSummary {
    /// The task being reviewed.
    pub task_id: u64,

    /// The Reviewer's final decision.
    pub decision: ReviewerDecision,

    /// Narrative summary of the review findings.
    pub summary: String,

    /// Any issues that were identified (empty if the review passed).
    pub issues: Vec<String>,
}

impl ReviewSummary {
    /// Create a new review summary with the given decision and summary text.
    pub fn new(
        task_id: u64,
        decision: ReviewerDecision,
        summary: impl Into<String>,
    ) -> Self {
        Self {
            task_id,
            decision,
            summary: summary.into(),
            issues: Vec::new(),
        }
    }

    /// Create an error summary — a review that failed to complete (e.g. the
    /// Reviewer itself encountered an error).
    ///
    /// The decision is set to `AskUser` so a human can intervene.
    pub fn error(
        task_id: u64,
        summary: impl Into<String>,
        issues: Vec<String>,
    ) -> Self {
        Self {
            task_id,
            decision: ReviewerDecision::AskUser,
            summary: summary.into(),
            issues,
        }
    }

    /// Add an issue to the review.
    pub fn add_issue(&mut self, issue: impl Into<String>) {
        self.issues.push(issue.into());
    }

    /// Returns `true` when the review passed without issues.
    pub fn is_clean(&self) -> bool {
        self.issues.is_empty() && matches!(self.decision, ReviewerDecision::Accept)
    }
}

/// The action the Reviewer recommends after evaluating a task or step.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReviewerDecision {
    /// The task met all its goals — no further action needed.
    Accept,

    /// The task did not fully meet its goals, but the agent can retry
    /// autonomously without user input.
    Retry,

    /// The task needs another attempt. If retries are exhausted, escalate to
    /// the user.
    RetryOrAskUser,

    /// The changes made so far are incorrect; roll back the last transaction
    /// and try a different approach.
    Rollback,

    /// The Reviewer cannot determine the right course of action; present the
    /// situation to the user for a manual decision.
    AskUser,
}

// ============================================================================
// Reviewer — the business-logic reviewer engine
// ============================================================================

/// Review decision engine.
///
/// The Reviewer inspects the goal-check results and decides the next step:
/// accept, retry, rollback, or escalate to the user.
///
/// # Review logic
/// - All requirements met -> Accept
/// - Few failures (<= 2)   -> RetryOrAskUser
/// - Many failures (> 2)   -> AskUser
pub struct Reviewer;

impl Reviewer {
    /// Create a new Reviewer instance.
    pub fn new() -> Self {
        Self
    }

    /// Review a task based on its goal-check outcome.
    ///
    /// # Parameters
    /// - `task_id`: The task being reviewed.
    /// - `all_requirements_met`: Whether every goal requirement passed.
    /// - `failed_requirements`: Descriptions of the failures.
    pub fn review(
        &self,
        task_id: u64,
        all_requirements_met: bool,
        failed_requirements: &[String],
    ) -> ReviewSummary {
        if all_requirements_met {
            ReviewSummary {
                task_id,
                decision: ReviewerDecision::Accept,
                summary: "All goal requirements met; task completed successfully.".into(),
                issues: Vec::new(),
            }
        } else if failed_requirements.is_empty() {
            ReviewSummary {
                task_id,
                decision: ReviewerDecision::AskUser,
                summary: "Goal-check state is inconsistent; manual confirmation needed.".into(),
                issues: vec!["Internal inconsistency: all_matched=false but no failures listed.".into()],
            }
        } else {
            let total = failed_requirements.len();
            if total <= 2 {
                ReviewSummary {
                    task_id,
                    decision: ReviewerDecision::RetryOrAskUser,
                    summary: format!(
                        "{} requirement(s) not met; retry or ask the user.",
                        total
                    ),
                    issues: failed_requirements.to_vec(),
                }
            } else {
                ReviewSummary {
                    task_id,
                    decision: ReviewerDecision::AskUser,
                    summary: format!(
                        "{} requirements not met; too many issues, user intervention needed.",
                        total
                    ),
                    issues: failed_requirements.to_vec(),
                }
            }
        }
    }

    /// Strict review — every requirement must pass; no retry allowed.
    ///
    /// Used for high-risk operations.
    pub fn review_strict(
        &self,
        task_id: u64,
        all_requirements_met: bool,
        failed_requirements: &[String],
    ) -> ReviewSummary {
        if all_requirements_met {
            ReviewSummary {
                task_id,
                decision: ReviewerDecision::Accept,
                summary: "Strict review passed: all requirements satisfied.".into(),
                issues: Vec::new(),
            }
        } else {
            ReviewSummary {
                task_id,
                decision: ReviewerDecision::AskUser,
                summary: "Strict review failed: unmet requirements; manual intervention required.".into(),
                issues: failed_requirements.to_vec(),
            }
        }
    }
}

impl Default for Reviewer {
    fn default() -> Self {
        Self::new()
    }
}
