//! Goal checking and review methods for DirectorRuntime.

use crate::plan::{EditPlan, EditPlanStep, EditPlanStatus};
use crate::permission::OperationRisk;
use crate::types::now_millis;
use super::types::{DirectorRuntime, EditorEvent, DirectorTraceEntry, ReviewSummary};

impl DirectorRuntime {
    /// Run the goal checker on a task.
    ///
    /// When `goal_checker_enabled` is true and a `SceneBridge` is
    /// available, this method builds `GoalRequirementKind`s from the
    /// task's plan steps and validates them against the live scene
    /// snapshot. Otherwise falls back to checking plan completion status.
    ///
    /// # Arguments
    ///
    /// * `task_id` - The ID of the task to check.
    pub fn check_goal(&mut self, task_id: u64) -> Vec<EditorEvent> {
        let task_plans: Vec<&EditPlan> = self.plan_manager.find_by_task(task_id);

        let plan_count = task_plans.len();
        let all_completed = task_plans
            .iter()
            .all(|p| p.status == EditPlanStatus::Completed);

        if self.goal_checker_enabled && self.scene_bridge.is_some() && plan_count > 0 {
            let requirements: Vec<crate::goal::GoalRequirementKind> = task_plans
                .iter()
                .flat_map(|p| p.steps.iter())
                .filter_map(|step| self.build_step_requirements(step).first().cloned())
                .collect();

            if !requirements.is_empty() {
                let scene_snapshot = self
                    .scene_bridge
                    .as_ref()
                    .map(|b| b.get_scene_snapshot())
                    .unwrap_or_default();

                let checker = crate::goal_checker::GoalChecker::new();
                let result = checker.check(&requirements, &scene_snapshot);

                let summary = if result.all_matched {
                    format!(
                        "Goal check passed: {}/{} requirements matched for task {}.",
                        result.requirement_results.iter().filter(|r| r.matched).count(),
                        result.requirement_results.len(),
                        task_id
                    )
                } else {
                    let failed: Vec<String> = result
                        .requirement_results
                        .iter()
                        .filter(|r| !r.matched)
                        .filter_map(|r| r.message.clone())
                        .collect();
                    format!(
                        "Goal check incomplete for task {}: {} requirement(s) not met: {}",
                        task_id,
                        failed.len(),
                        failed.join("; ")
                    )
                };

                let events = vec![EditorEvent::GoalChecked {
                    task_id,
                    all_matched: result.all_matched,
                    summary: summary.clone(),
                }];
                self.events.extend(events.clone());

                self.trace_entries.push(DirectorTraceEntry {
                    timestamp_ms: now_millis(),
                    actor: "GoalChecker".into(),
                    summary,
                });

                return events;
            }
        }

        let summary = if plan_count == 0 {
            "No plans found for this task.".to_string()
        } else if all_completed {
            format!(
                "All {} plan(s) for task {} completed successfully.",
                plan_count, task_id
            )
        } else {
            format!(
                "{} plan(s) found for task {}; not all have completed.",
                plan_count, task_id
            )
        };

        let events = vec![EditorEvent::GoalChecked {
            task_id,
            all_matched: all_completed && plan_count > 0,
            summary: summary.clone(),
        }];
        self.events.extend(events.clone());

        self.trace_entries.push(DirectorTraceEntry {
            timestamp_ms: now_millis(),
            actor: "GoalChecker".into(),
            summary,
        });

        events
    }

    /// Run the reviewer on a task and produce a review summary.
    ///
    /// # Arguments
    ///
    /// * `task_id` - The ID of the task to review.
    pub fn review_task(&mut self, task_id: u64) -> ReviewSummary {
        let related_plans: Vec<&EditPlan> = self.plan_manager.find_by_task(task_id);

        let (decision, summary, issues) = if related_plans.is_empty() {
            (
                "no_data".to_string(),
                "No plans found for this task; nothing to review.".to_string(),
                vec!["No execution data available.".to_string()],
            )
        } else {
            let all_completed = related_plans
                .iter()
                .all(|p| p.status == EditPlanStatus::Completed);
            let high_risk = related_plans.iter().any(|p| {
                p.risk_level == OperationRisk::HighRisk
                    || p.risk_level == OperationRisk::Destructive
            });

            let mut iss = Vec::new();
            if high_risk {
                iss.push(
                    "Plan contained high-risk or destructive operations.".to_string(),
                );
            }
            if !all_completed {
                iss.push("Not all plans completed successfully.".to_string());
            }

            if all_completed && !high_risk {
                (
                    "approved".to_string(),
                    format!(
                        "All {} plan(s) completed without issues.",
                        related_plans.len()
                    ),
                    iss,
                )
            } else if all_completed {
                (
                    "needs_revision".to_string(),
                    "Plans completed but contained high-risk steps; manual review recommended."
                        .to_string(),
                    iss,
                )
            } else {
                (
                    "needs_revision".to_string(),
                    "Some plans did not complete; further action required.".to_string(),
                    iss,
                )
            }
        };

        let review = ReviewSummary {
            task_id,
            decision: decision.clone(),
            summary: summary.clone(),
            issues,
        };

        self.events.push(EditorEvent::ReviewCompleted {
            task_id,
            decision,
            summary,
        });

        self.trace_entries.push(DirectorTraceEntry {
            timestamp_ms: now_millis(),
            actor: "Reviewer".into(),
            summary: format!("Reviewed task {}: {:?}", task_id, review.decision),
        });

        review
    }

    // ------------------------------------------------------------------
    // Internal helpers
    // ------------------------------------------------------------------

    pub(crate) fn build_step_requirements(
        &self,
        step: &EditPlanStep,
    ) -> Vec<crate::goal::GoalRequirementKind> {
        let mut reqs = Vec::new();
        if step.title.contains("创建") || step.title.contains("Create") {
            let parts: Vec<&str> = step.title.split_whitespace().collect();
            if let Some(name) = parts.last() {
                reqs.push(crate::goal::GoalRequirementKind::EntityExists {
                    name: name.to_string(),
                });
            }
        }
        reqs
    }
}
