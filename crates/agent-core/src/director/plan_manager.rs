//! PlanManager — manages the lifecycle of edit plans.
//!
//! Extracted from DirectorRuntime to avoid the god-object antipattern.
//! Handles plan CRUD, approval/rejection, permission checking,
//! and plan ID generation — independently of execution machinery.

use std::collections::HashMap;

use crate::plan::{EditPlan, EditPlanStep, EditPlanStatus, ExecutionMode, TargetModule};
use crate::permission::{OperationRisk, PermissionEngine, PermissionRequirement};
use crate::planner::{Planner, PlannerContext};
use crate::fallback::FallbackEngine;

/// Manages the lifecycle of edit plans: creation, storage, approval,
/// rejection, permission checks, and ID assignment.
///
/// Owns the plan dictionary and pending-approval queue so that
/// execution logic can be separated into its own module.
pub struct PlanManager {
    /// Map from plan ID to plan data.
    plans: HashMap<String, EditPlan>,
    /// IDs of plans waiting for user approval.
    pending_approvals: Vec<String>,
    /// Monotonically increasing counter for task/plan IDs.
    next_task_id: u64,
    /// The planner implementation used to create edit plans.
    planner: Box<dyn Planner>,
}

impl std::fmt::Debug for PlanManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PlanManager")
            .field("plans", &self.plans)
            .field("pending_approvals", &self.pending_approvals)
            .field("next_task_id", &self.next_task_id)
            .field("planner", &"<dyn Planner>")
            .finish()
    }
}

impl PlanManager {
    pub fn new(planner: Box<dyn Planner>) -> Self {
        Self {
            plans: HashMap::new(),
            pending_approvals: Vec::new(),
            next_task_id: 0,
            planner,
        }
    }

    pub fn set_planner(&mut self, planner: Box<dyn Planner>) {
        self.planner = planner;
    }

    // ------------------------------------------------------------------
    // ID generation
    // ------------------------------------------------------------------

    /// Allocate and advance the next task ID atomically.
    /// Returns the *previous* value (the newly assigned ID).
    pub fn allocate_task_id(&mut self) -> u64 {
        let id = self.next_task_id;
        self.next_task_id += 1;
        id
    }

    /// Generate a unique plan ID string.
    pub fn generate_plan_id(&self, prefix: &str, task_id: u64) -> String {
        format!("plan_{}_{}_{}", prefix, task_id, self.next_task_id)
    }

    // ------------------------------------------------------------------
    // CRUD
    // ------------------------------------------------------------------

    pub fn insert(&mut self, plan_id: String, plan: EditPlan) {
        self.plans.insert(plan_id, plan);
    }

    pub fn get(&self, plan_id: &str) -> Option<&EditPlan> {
        self.plans.get(plan_id)
    }

    pub fn get_mut(&mut self, plan_id: &str) -> Option<&mut EditPlan> {
        self.plans.get_mut(plan_id)
    }

    pub fn list(&self) -> Vec<&EditPlan> {
        self.plans.values().collect()
    }

    pub fn plan_count(&self) -> usize {
        self.plans.len()
    }

    pub fn current_step_count(&self) -> usize {
        self.plans
            .values()
            .max_by_key(|p| p.task_id)
            .map(|p| p.steps.len())
            .unwrap_or(0)
    }

    /// Find plans belonging to a specific task.
    pub fn find_by_task(&self, task_id: u64) -> Vec<&EditPlan> {
        self.plans
            .values()
            .filter(|p| p.task_id == task_id)
            .collect()
    }

    /// Count completed plans.  Returns `(completed, total)`.
    pub fn completion_counts(&self) -> (usize, usize) {
        let total = self.plans.len();
        let completed = self
            .plans
            .values()
            .filter(|p| p.status == EditPlanStatus::Completed)
            .count();
        (completed, total)
    }

    // ------------------------------------------------------------------
    // Approval queue
    // ------------------------------------------------------------------

    pub fn has_pending_approvals(&self) -> bool {
        !self.pending_approvals.is_empty()
    }

    pub fn pending_approval_ids(&self) -> Vec<String> {
        self.pending_approvals.clone()
    }

    pub fn add_pending(&mut self, plan_id: String) {
        self.pending_approvals.push(plan_id);
    }

    pub fn remove_pending(&mut self, plan_id: &str) {
        self.pending_approvals.retain(|id| id != plan_id);
    }

    // ------------------------------------------------------------------
    // Plan creation
    // ------------------------------------------------------------------

    /// Create an `EditPlan` from raw user text using the configured planner.
    pub fn create_plan(
        &mut self,
        text: &str,
        context: PlannerContext,
        memory_context: Option<crate::memory::MemoryContext>,
    ) -> EditPlan {
        let task_id = self.allocate_task_id();
        let mut context = context;
        context.memory_context = memory_context;
        self.planner.create_plan(text, task_id, context)
    }

    // ------------------------------------------------------------------
    // Permission
    // ------------------------------------------------------------------

    /// Check whether a plan should be auto-approved, needs user confirmation,
    /// or is forbidden.
    pub fn check_permission(&self, plan_id: &str) -> PermissionRequirement {
        let engine = PermissionEngine::new();
        let plan = match self.plans.get(plan_id) {
            Some(p) => p,
            None => {
                return PermissionRequirement::Forbidden {
                    reason: format!("Plan '{}' not found", plan_id),
                };
            }
        };
        engine.decide_for_plan(plan.risk_level)
    }

    // ------------------------------------------------------------------
    // Approval / Rejection
    // ------------------------------------------------------------------

    /// Approve a plan: remove from pending queue and mark as Approved.
    ///
    /// Returns `Err(plan_id)` if the plan wasn't found.
    pub fn approve(&mut self, plan_id: &str) -> Result<(), String> {
        self.remove_pending(plan_id);
        match self.plans.get_mut(plan_id) {
            Some(plan) => {
                plan.status = EditPlanStatus::Approved;
                Ok(())
            }
            None => Err(plan_id.to_string()),
        }
    }

    /// Reject a plan: remove from pending queue and mark as Rejected.
    ///
    /// Returns `Err(plan_id)` if the plan wasn't found.
    pub fn reject(&mut self, plan_id: &str) -> Result<(), String> {
        self.remove_pending(plan_id);
        match self.plans.get_mut(plan_id) {
            Some(plan) => {
                plan.status = EditPlanStatus::Rejected;
                Ok(())
            }
            None => Err(plan_id.to_string()),
        }
    }

    // ------------------------------------------------------------------
    // Status transitions
    // ------------------------------------------------------------------

    pub fn set_status(&mut self, plan_id: &str, status: EditPlanStatus) {
        if let Some(plan) = self.plans.get_mut(plan_id) {
            plan.status = status;
        }
    }
}
