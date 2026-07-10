use super::helpers::{extract_component_name_from_obs, extract_entity_name_from_obs, truncate_str};
use super::{ObservationPattern, RevisionEntry, RevisionType};
use crate::permission::OperationRisk;
use crate::plan::{EditPlan, EditPlanStep, TargetModule};
use std::collections::HashMap;

// ===========================================================================
// DynamicPlanner - 核心修订引擎
// ===========================================================================

/// Intelligent planner that dynamically revises execution plans based on
/// intermediate results.
///
/// ## Usage
///
/// ```rust
/// # use agent_core::{DynamicPlanner, EditPlan, ExecutionMode, OperationRisk, TargetModule, EditPlanStep};
/// let mut planner = DynamicPlanner::new();
/// let mut plan = EditPlan::new("plan_1", 1, "test", "", ExecutionMode::Plan);
/// plan.steps.push(EditPlanStep {
///     id: "s1".into(), title: "Create Player".into(),
///     target_module: TargetModule::Scene,
///     action_description: "Create player entity".into(),
///     risk: OperationRisk::LowRisk,
///     validation_requirements: vec![],
/// });
///
/// // After each tool execution:
/// if let Some(revision) = planner.analyze_observation("Entity already exists", 0, "plan_1") {
///     planner.apply_revision(&mut plan, revision, 0, "Entity already exists", true).unwrap();
/// }
/// ```
pub struct DynamicPlanner {
    /// Registered observation patterns.
    pub(super) patterns: Vec<ObservationPattern>,
    /// History of all applied revisions.
    revision_history: Vec<RevisionEntry>,
    /// Maximum number of revisions per plan (to prevent infinite loops).
    pub(super) max_revisions_per_plan: usize,
    /// Current revision count per plan.
    pub(super) revision_counts: HashMap<String, usize>,
}

impl DynamicPlanner {
    /// Create a new DynamicPlanner with default patterns.
    pub fn new() -> Self {
        let mut planner = Self {
            patterns: Vec::new(),
            revision_history: Vec::new(),
            max_revisions_per_plan: 10,
            revision_counts: HashMap::new(),
        };

        // Register default patterns
        planner.register_default_patterns();
        planner
    }

    /// Register default observation patterns for common scenarios.
    fn register_default_patterns(&mut self) {
        // Pattern 1: Entity already exists → Skip creation steps
        self.patterns.push(ObservationPattern {
            name: "entity_already_exists".into(),
            keywords: vec![
                "already exists".into(),
                "已存在".into(),
                "duplicate".into(),
                "重复".into(),
            ],
            regex_pattern: Some(r"entity\s+'?(\w+)'?\s+already\s+exists".into()),
            revision_type_fn: |obs, _| {
                let entity_name =
                    extract_entity_name_from_obs(obs).unwrap_or_else(|| "unknown".into());
                Some(RevisionType::Skip {
                    count: 1,
                    reason: format!("Entity '{}' already exists, skipping creation", entity_name),
                })
            },
            confidence: 0.95,
        });

        // Pattern 2: Entity not found → Insert creation or adapt deletion
        self.patterns.push(ObservationPattern {
            name: "entity_not_found".into(),
            keywords: vec![
                "not found".into(),
                "未找到".into(),
                "does not exist".into(),
                "不存在".into(),
                "no such entity".into(),
            ],
            regex_pattern: Some(r"entity\s+'?(\w+)'?\s+(not\s+found|does\s+not\s+exist)".into()),
            revision_type_fn: |obs, idx| {
                let entity_name =
                    extract_entity_name_from_obs(obs).unwrap_or_else(|| "unknown".into());

                // If current step is delete/modify, adapt to create
                if obs.to_lowercase().contains("delete") || obs.to_lowercase().contains("删除") {
                    Some(RevisionType::Adapt {
                        index: idx,
                        adaptation: format!("Entity '{}' not found, creating instead", entity_name),
                        from_risk: OperationRisk::MediumRisk,
                        to_risk: OperationRisk::LowRisk,
                    })
                } else {
                    // Otherwise insert prerequisite
                    Some(RevisionType::InsertBefore {
                        index: idx,
                        step: EditPlanStep {
                            id: format!("prereq_create_{}", entity_name),
                            title: format!("Create missing entity '{}'", entity_name),
                            target_module: TargetModule::Scene,
                            action_description: format!(
                                "Prerequisite: create '{}' before continuing",
                                entity_name
                            ),
                            risk: OperationRisk::LowRisk,
                            validation_requirements: vec![format!(
                                "Entity '{}' exists",
                                entity_name
                            )],
                        },
                        reason: format!("Entity '{}' not found, need to create first", entity_name),
                    })
                }
            },
            confidence: 0.90,
        });

        // Pattern 3: Permission denied → Adapt to lower-risk approach
        self.patterns.push(ObservationPattern {
            name: "permission_denied".into(),
            keywords: vec![
                "permission denied".into(),
                "权限不足".into(),
                "access denied".into(),
                "拒绝访问".into(),
                "unauthorized".into(),
                "未授权".into(),
            ],
            regex_pattern: None,
            revision_type_fn: |obs, idx| {
                Some(RevisionType::Adapt {
                    index: idx,
                    adaptation: format!(
                        "Permission denied, requesting user approval: {}",
                        truncate_str(obs, 100)
                    ),
                    from_risk: OperationRisk::HighRisk,
                    to_risk: OperationRisk::LowRisk,
                })
            },
            confidence: 0.85,
        });

        // Pattern 4: Component not found → Add component first
        self.patterns.push(ObservationPattern {
            name: "component_not_found".into(),
            keywords: vec![
                "component not found".into(),
                "组件不存在".into(),
                "no such component".into(),
                "找不到组件".into(),
            ],
            regex_pattern: None,
            revision_type_fn: |obs, idx| {
                let comp_name =
                    extract_component_name_from_obs(obs).unwrap_or_else(|| "unknown".into());
                Some(RevisionType::InsertBefore {
                    index: idx,
                    step: EditPlanStep {
                        id: format!("prereq_add_comp_{}", comp_name),
                        title: format!("Add component '{}'", comp_name),
                        target_module: TargetModule::Scene,
                        action_description: format!(
                            "Prerequisite: add '{}' component before modifying it",
                            comp_name
                        ),
                        risk: OperationRisk::LowRisk,
                        validation_requirements: vec![format!("Component '{}' exists", comp_name)],
                    },
                    reason: format!("Component '{}' not found, adding first", comp_name),
                })
            },
            confidence: 0.88,
        });

        // Pattern 5: Operation succeeded but with warning → Continue (no revision)
        self.patterns.push(ObservationPattern {
            name: "success_with_warning".into(),
            keywords: vec![
                "success".into(),
                "成功".into(),
                "completed".into(),
                "完成".into(),
                "warning".into(),
                "警告".into(),
            ],
            regex_pattern: None,
            revision_type_fn: |_obs, _idx| None, // No revision needed
            confidence: 1.0,
        });

        // Pattern 6: Network/timeout error → Retry (handled by Reflection, not here)
        self.patterns.push(ObservationPattern {
            name: "transient_error".into(),
            keywords: vec![
                "timeout".into(),
                "超时".into(),
                "network error".into(),
                "网络错误".into(),
                "connection refused".into(),
                "连接被拒".into(),
                "rate limit".into(),
                "速率限制".into(),
            ],
            regex_pattern: None,
            revision_type_fn: |obs, _idx| {
                Some(RevisionType::Skip {
                    count: 0, // Don't skip, just note for retry logic
                    reason: format!("Transient error detected: {}", truncate_str(obs, 80)),
                })
            },
            confidence: 0.70,
        });
    }

    /// Analyze an observation and determine if a revision is needed.
    ///
    /// Returns the highest-confidence revision if any pattern matches.
    ///
    /// # Arguments
    /// * `observation` - The tool execution result text
    /// * `step_index` - Current step index in the plan
    /// * `plan_id` - ID of the plan being executed
    ///
    /// # Returns
    /// * `Some(RevisionType)` - A revision should be applied
    /// * `None` - No revision needed
    pub fn analyze_observation(
        &mut self,
        observation: &str,
        step_index: usize,
        plan_id: &str,
    ) -> Option<RevisionType> {
        // Check revision limit
        let current_count = self.revision_counts.get(plan_id).copied().unwrap_or(0);
        if current_count >= self.max_revisions_per_plan {
            eprintln!(
                "[DynamicPlanner] Max revisions ({}) reached for plan '{}', skipping",
                self.max_revisions_per_plan, plan_id
            );
            return None;
        }

        // Find best matching pattern
        let mut best_match: Option<(RevisionType, f32)> = None;

        for pattern in &self.patterns {
            if let Some((revision, confidence)) = pattern.matches(observation) {
                match &best_match {
                    None => best_match = Some((revision, confidence)),
                    Some((_, best_conf)) => {
                        if confidence > *best_conf {
                            best_match = Some((revision, confidence));
                        }
                    }
                }
            }
        }

        if let Some((revision, _confidence)) = best_match {
            eprintln!(
                "[DynamicPlanner] Detected revision at step {}: {} (confidence: {:.2})",
                step_index,
                revision.describe(),
                _confidence
            );
            Some(revision)
        } else {
            None
        }
    }

    /// Apply a revision to an edit plan.
    ///
    /// Modifies the plan in-place and records the revision in history.
    pub fn apply_revision(
        &mut self,
        plan: &mut EditPlan,
        revision: RevisionType,
        step_index: usize,
        trigger_observation: &str,
        auto_applied: bool,
    ) -> Result<(), String> {
        // Record revision in history
        let entry = RevisionEntry::new(
            revision.clone(),
            &plan.id,
            step_index,
            trigger_observation,
            auto_applied,
        );
        self.revision_history.push(entry.clone());

        // Update revision count
        *self.revision_counts.entry(plan.id.clone()).or_insert(0) += 1;

        // Apply the actual revision to the plan
        match &revision {
            RevisionType::Skip { count, reason } => {
                self.apply_skip(plan, step_index, *count, reason)?;
            }
            RevisionType::InsertBefore { index, step, .. } => {
                self.apply_insert_before(plan, *index, step.clone())?;
            }
            RevisionType::InsertAfter { index, step, .. } => {
                self.apply_insert_after(plan, *index, step.clone())?;
            }
            RevisionType::Replace {
                index, new_step, ..
            } => {
                self.apply_replace(plan, *index, new_step.clone())?;
            }
            RevisionType::Adapt {
                index,
                adaptation,
                to_risk,
                ..
            } => {
                self.apply_adapt(plan, *index, adaptation, *to_risk)?;
            }
            RevisionType::Abort { reason } => {
                return Err(format!("Plan aborted: {}", reason));
            }
        }

        eprintln!("[DynamicPlanner] Applied revision: {}", entry.summary());

        Ok(())
    }

    // ------------------------------------------------------------------
    // Private revision application methods
    // ------------------------------------------------------------------

    fn apply_skip(
        &self,
        plan: &mut EditPlan,
        _start_index: usize,
        count: usize,
        reason: &str,
    ) -> Result<(), String> {
        let mut skipped = 0;
        for step in &mut plan.steps {
            if skipped >= count {
                break;
            }
            if !step.title.starts_with("[SKIPPED]") {
                step.title = format!("[SKIPPED] {}", step.title);
                step.action_description =
                    format!("[SKIPPED] {} ({})", step.action_description, reason);
                skipped += 1;
            }
        }
        Ok(())
    }

    fn apply_insert_before(
        &self,
        plan: &mut EditPlan,
        index: usize,
        step: EditPlanStep,
    ) -> Result<(), String> {
        let insert_idx = index.min(plan.steps.len());
        plan.steps.insert(insert_idx, step);
        Ok(())
    }

    fn apply_insert_after(
        &self,
        plan: &mut EditPlan,
        index: usize,
        step: EditPlanStep,
    ) -> Result<(), String> {
        let insert_idx = (index + 1).min(plan.steps.len());
        plan.steps.insert(insert_idx, step);
        Ok(())
    }

    fn apply_replace(
        &self,
        plan: &mut EditPlan,
        index: usize,
        new_step: EditPlanStep,
    ) -> Result<(), String> {
        if index < plan.steps.len() {
            plan.steps[index] = new_step;
            Ok(())
        } else {
            Err(format!(
                "Cannot replace step {}: index out of bounds (len={})",
                index,
                plan.steps.len()
            ))
        }
    }

    fn apply_adapt(
        &self,
        plan: &mut EditPlan,
        index: usize,
        adaptation: &str,
        new_risk: OperationRisk,
    ) -> Result<(), String> {
        if index < plan.steps.len() {
            let step = &mut plan.steps[index];
            step.title = format!("[ADAPTED] {}", step.title);
            step.action_description = format!("{} ({})", step.action_description, adaptation);
            step.risk = new_risk;
            Ok(())
        } else {
            Err(format!("Cannot adapt step {}: index out of bounds", index))
        }
    }

    /// Get all revision history entries for a specific plan.
    pub fn get_plan_revisions(&self, plan_id: &str) -> Vec<&RevisionEntry> {
        self.revision_history
            .iter()
            .filter(|entry| entry.plan_id == plan_id)
            .collect()
    }

    /// Get total number of revisions applied across all plans.
    pub fn total_revisions(&self) -> usize {
        self.revision_history.len()
    }

    /// Get number of registered patterns (for testing).
    pub fn pattern_count(&self) -> usize {
        self.patterns.len()
    }

    /// Set max revisions per plan (for testing).
    pub fn set_max_revisions_per_plan(&mut self, max: usize) {
        self.max_revisions_per_plan = max;
    }

    /// Clear revision history (for testing or new session).
    pub fn clear_history(&mut self) {
        self.revision_history.clear();
        self.revision_counts.clear();
    }

    /// Reset revision count for a specific plan.
    pub fn reset_plan_revisions(&mut self, plan_id: &str) {
        self.revision_counts.remove(plan_id);
    }
}

impl Default for DynamicPlanner {
    fn default() -> Self {
        Self::new()
    }
}
