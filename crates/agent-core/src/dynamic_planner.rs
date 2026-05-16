//! Dynamic Planner — Plan-and-Solve 动态修订引擎
//!
//! Sprint 1-A2: 在执行过程中根据中间结果智能调整计划。
//!
//! ## 核心能力
//!
//! 1. **观察驱动修订**: 根据工具执行结果自动调整后续步骤
//! 2. **模式识别**: 检测常见执行模式（实体已存在、权限不足等）
//! 3. **智能跳过**: 自动跳过冗余步骤（如重复创建）
//! 4. **前置条件插入**: 缺少依赖时自动插入前置步骤
//! 5. **降级策略**: 高风险操作失败时自动降级为低风险替代方案
//!
//! ## 架构
//!
//! ```text
//! ReAct Loop (Observe)
//!     ↓
//! DynamicPlanner::analyze_observation()
//!     ├─ Pattern Match → RevisionType
//!     └─ Confidence Score
//!         ↓
//! DynamicPlanner::apply_revision()
//!     ├─ Skip redundant steps
//!     ├─ Insert prerequisites
//!     ├─ Adapt failed steps
//!     └─ Log revision history
//! ```

use crate::plan::{EditPlan, EditPlanStep, TargetModule};
use crate::permission::OperationRisk;
use std::collections::HashMap;

// ===========================================================================
// Revision Type - 修订类型枚举
// ===========================================================================

/// Types of plan revisions that can be applied dynamically.
#[derive(Debug, Clone, PartialEq)]
pub enum RevisionType {
    /// Skip the next N steps (e.g., entity already exists).
    Skip {
        count: usize,
        reason: String,
    },
    /// Insert a new step before the current position.
    InsertBefore {
        index: usize,
        step: EditPlanStep,
        reason: String,
    },
    /// Insert a new step after the current position.
    InsertAfter {
        index: usize,
        step: EditPlanStep,
        reason: String,
    },
    /// Replace a step's action with an alternative.
    Replace {
        index: usize,
        original_title: String,
        new_step: EditPlanStep,
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
    Abort {
        reason: String,
    },
}

impl RevisionType {
    /// Get human-readable description of this revision.
    pub fn describe(&self) -> String {
        match self {
            RevisionType::Skip { count, reason } => {
                format!("Skip next {} steps: {}", count, reason)
            }
            RevisionType::InsertBefore { index, step, reason } => {
                format!("Insert '{}' before step {}: {}",
                    step.title, index, reason)
            }
            RevisionType::InsertAfter { index, step, reason } => {
                format!("Insert '{}' after step {}: {}",
                    step.title, index, reason)
            }
            RevisionType::Replace { index, new_step, .. } => {
                format!("Replace step {} with '{}'", index, new_step.title)
            }
            RevisionType::Adapt { index, adaptation, .. } => {
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

// ===========================================================================
// Observation Pattern - 观察结果模式
// ===========================================================================

/// A pattern that can be matched against tool execution observations.
#[derive(Debug, Clone)]
pub struct ObservationPattern {
    /// Pattern name for logging and debugging.
    pub name: String,
    /// Keywords that trigger this pattern (case-insensitive).
    pub keywords: Vec<String>,
    /// Regex pattern for more complex matching (optional).
    pub regex_pattern: Option<String>,
    /// The type of revision to apply when matched.
    pub revision_type_fn: fn(&str, usize) -> Option<RevisionType>,
    /// Confidence score (0.0-1.0) for this pattern match.
    pub confidence: f32,
}

impl ObservationPattern {
    /// Check if this observation matches the pattern.
    pub fn matches(&self, observation: &str) -> Option<(RevisionType, f32)> {
        let obs_lower = observation.to_lowercase();

        // Keyword matching
        let keyword_match = self.keywords.iter()
            .any(|kw| obs_lower.contains(&kw.to_lowercase()));

        if keyword_match {
            // Try to generate revision using the function
            if let Some(revision) = (self.revision_type_fn)(observation, 0) {
                return Some((revision, self.confidence));
            }
        }

        None
    }
}

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
        format!("[{}] {} @ step {} (auto={})",
            self.timestamp_ms,
            self.revision_type.describe(),
            self.step_index,
            self.auto_applied
        )
    }
}

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
    patterns: Vec<ObservationPattern>,
    /// History of all applied revisions.
    revision_history: Vec<RevisionEntry>,
    /// Maximum number of revisions per plan (to prevent infinite loops).
    max_revisions_per_plan: usize,
    /// Current revision count per plan.
    revision_counts: HashMap<String, usize>,
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
                "already exists".into(), "已存在".into(),
                "duplicate".into(), "重复".into(),
            ],
            regex_pattern: Some(r"entity\s+'?(\w+)'?\s+already\s+exists".into()),
            revision_type_fn: |obs, _| {
                let entity_name = extract_entity_name_from_obs(obs)
                    .unwrap_or_else(|| "unknown".into());
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
                "not found".into(), "未找到".into(),
                "does not exist".into(), "不存在".into(),
                "no such entity".into(),
            ],
            regex_pattern: Some(r"entity\s+'?(\w+)'?\s+(not\s+found|does\s+not\s+exist)".into()),
            revision_type_fn: |obs, idx| {
                let entity_name = extract_entity_name_from_obs(obs)
                    .unwrap_or_else(|| "unknown".into());

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
                            validation_requirements: vec![
                                format!("Entity '{}' exists", entity_name)
                            ],
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
                "permission denied".into(), "权限不足".into(),
                "access denied".into(), "拒绝访问".into(),
                "unauthorized".into(), "未授权".into(),
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
                "component not found".into(), "组件不存在".into(),
                "no such component".into(), "找不到组件".into(),
            ],
            regex_pattern: None,
            revision_type_fn: |obs, idx| {
                let comp_name = extract_component_name_from_obs(obs)
                    .unwrap_or_else(|| "unknown".into());
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
                        validation_requirements: vec![
                            format!("Component '{}' exists", comp_name)
                        ],
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
                "success".into(), "成功".into(),
                "completed".into(), "完成".into(),
                "warning".into(), "警告".into(),
            ],
            regex_pattern: None,
            revision_type_fn: |_obs, _idx| None, // No revision needed
            confidence: 1.0,
        });

        // Pattern 6: Network/timeout error → Retry (handled by Reflection, not here)
        self.patterns.push(ObservationPattern {
            name: "transient_error".into(),
            keywords: vec![
                "timeout".into(), "超时".into(),
                "network error".into(), "网络错误".into(),
                "connection refused".into(), "连接被拒".into(),
                "rate limit".into(), "速率限制".into(),
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
                step_index, revision.describe(), _confidence
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
            RevisionType::Replace { index, new_step, .. } => {
                self.apply_replace(plan, *index, new_step.clone())?;
            }
            RevisionType::Adapt { index, adaptation, to_risk, .. } => {
                self.apply_adapt(plan, *index, adaptation, *to_risk)?;
            }
            RevisionType::Abort { reason } => {
                return Err(format!("Plan aborted: {}", reason));
            }
        }

        eprintln!(
            "[DynamicPlanner] Applied revision: {}",
            entry.summary()
        );

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
                step.action_description = format!("[SKIPPED] {} ({})", step.action_description, reason);
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
            Err(format!("Cannot replace step {}: index out of bounds (len={})", index, plan.steps.len()))
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
        self.revision_history.iter()
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

// ===========================================================================
// Helper functions for pattern matching
// ===========================================================================

/// Extract entity name from observation text.
fn extract_entity_name_from_obs(obs: &str) -> Option<String> {
    // Try common patterns like "Entity 'Player' already exists"
    if let Some(start) = obs.find('\'') {
        if let Some(end) = obs[start + 1..].find('\'') {
            return Some(obs[start + 1..start + 1 + end].trim().to_string());
        }
    }

    // Try quoted strings
    if let Some(start) = obs.find('"') {
        if let Some(end) = obs[start + 1..].find('"') {
            return Some(obs[start + 1..start + 1 + end].trim().to_string());
        }
    }

    None
}

/// Extract component name from observation text.
fn extract_component_name_from_obs(obs: &str) -> Option<String> {
    let lower = obs.to_lowercase();

    if lower.contains("component") || lower.contains("组件") {
        // Look for word after "component"
        let patterns = ["component '", "component \"", "component ", "组件 '", "组件 \""];
        for pattern in &patterns {
            if let Some(start) = lower.find(pattern) {
                let after = &obs[start + pattern.len()..];
                let name: String = after.chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect();
                if !name.is_empty() {
                    return Some(name);
                }
            }
        }
    }

    None
}

/// Truncate string to max length with ellipsis.
fn truncate_str(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len])
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan::ExecutionMode;

    #[test]
    fn test_dynamic_planner_creation() {
        let planner = DynamicPlanner::new();
        assert!(planner.patterns.len() >= 6, "Should have at least 6 default patterns");
        assert_eq!(planner.total_revisions(), 0);
    }

    #[test]
    fn test_pattern_entity_already_exists() {
        let mut planner = DynamicPlanner::new();
        let revision = planner.analyze_observation(
            "Entity 'Player' already exists in scene",
            0,
            "plan_1",
        );

        assert!(revision.is_some(), "Should detect 'already exists' pattern");
        match revision.unwrap() {
            RevisionType::Skip { reason, .. } => {
                assert!(reason.contains("Player"), "Reason should mention entity name");
                assert!(reason.contains("already exists"), "Reason should explain skip");
            }
            other => unreachable!("Expected Skip revision, got {:?}", other),
        }
    }

    #[test]
    fn test_pattern_entity_not_found() {
        let mut planner = DynamicPlanner::new();
        let revision = planner.analyze_observation(
            "Entity 'Enemy' not found in scene",
            0,
            "plan_2",
        );

        assert!(revision.is_some(), "Should detect 'not found' pattern");
        // Should either insert prerequisite or adapt
        match revision.unwrap() {
            RevisionType::InsertBefore { step, .. } => {
                assert!(step.title.contains("Create"), "Should suggest creating entity");
                assert!(step.title.contains("Enemy"), "Should mention missing entity");
            }
            RevisionType::Adapt { adaptation, .. } => {
                assert!(adaptation.contains("not found"), "Adaptation should mention error");
            }
            other => unreachable!("Expected InsertBefore or Adapt, got {:?}", other),
        }
    }

    #[test]
    fn test_pattern_permission_denied() {
        let mut planner = DynamicPlanner::new();
        let revision = planner.analyze_observation(
            "Permission denied: operation requires admin privileges",
            2,
            "plan_3",
        );

        assert!(revision.is_some(), "Should detect 'permission denied' pattern");
        match revision.unwrap() {
            RevisionType::Adapt { from_risk, to_risk, .. } => {
                assert_eq!(from_risk, OperationRisk::HighRisk);
                assert_eq!(to_risk, OperationRisk::LowRisk);
            }
            other => unreachable!("Expected Adapt revision, got {:?}", other),
        }
    }

    #[test]
    fn test_apply_skip_revision() {
        let mut planner = DynamicPlanner::new();
        let mut plan = EditPlan::new(
            "test_plan", 1, "Test", "Test plan", ExecutionMode::Plan
        );
        plan.steps.push(EditPlanStep {
            id: "step1".into(),
            title: "Create Player".into(),
            target_module: TargetModule::Scene,
            action_description: "Create player entity".into(),
            risk: OperationRisk::LowRisk,
            validation_requirements: vec![],
        });
        plan.steps.push(EditPlanStep {
            id: "step2".into(),
            title: "Create Enemy".into(),
            target_module: TargetModule::Scene,
            action_description: "Create enemy entity".into(),
            risk: OperationRisk::LowRisk,
            validation_requirements: vec![],
        });

        let revision = RevisionType::Skip {
            count: 1,
            reason: "Entity already exists".into(),
        };
        planner.apply_revision(&mut plan, revision, 0, "Test obs", true).unwrap();

        assert!(plan.steps[0].title.starts_with("[SKIPPED]"));
        assert!(!plan.steps[1].title.starts_with("[SKIPPED]"));
        assert_eq!(planner.total_revisions(), 1);
    }

    #[test]
    fn test_apply_insert_before_revision() {
        let mut planner = DynamicPlanner::new();
        let mut plan = EditPlan::new(
            "test_plan", 1, "Test", "Test plan", ExecutionMode::Plan
        );
        plan.steps.push(EditPlanStep {
            id: "step1".into(),
            title: "Update Player".into(),
            target_module: TargetModule::Scene,
            action_description: "Update player".into(),
            risk: OperationRisk::MediumRisk,
            validation_requirements: vec![],
        });

        let prereq = EditPlanStep {
            id: "prereq".into(),
            title: "Create Player".into(),
            target_module: TargetModule::Scene,
            action_description: "Create player first".into(),
            risk: OperationRisk::LowRisk,
            validation_requirements: vec![],
        };
        let revision = RevisionType::InsertBefore {
            index: 0,
            step: prereq,
            reason: "Entity not found".into(),
        };
        planner.apply_revision(&mut plan, revision, 0, "Not found", true).unwrap();

        assert_eq!(plan.steps.len(), 2);
        assert_eq!(plan.steps[0].title, "Create Player"); // Prerequisite inserted first
        assert_eq!(planner.total_revisions(), 1);
    }

    #[test]
    fn test_max_revisions_limit() {
        let mut planner = DynamicPlanner::new();
        planner.max_revisions_per_plan = 2; // Set low limit for testing

        let mut plan = EditPlan::new(
            "limited_plan", 1, "Tracked", "", ExecutionMode::Plan
        );
        plan.steps.push(EditPlanStep {
            id: "s1".into(),
            title: "Step 1".into(),
            target_module: TargetModule::Scene,
            action_description: "".into(),
            risk: OperationRisk::LowRisk,
            validation_requirements: vec![],
        });

        // Apply 2 revisions (should succeed)
        for i in 0..2 {
            let rev = RevisionType::Skip {
                count: 1,
                reason: format!("Test revision {}", i),
            };
            let result = planner.analyze_observation("Entity already exists", 0, "limited_plan");
            assert!(result.is_some(), "Revision {} should be allowed", i);
            let _ = planner.apply_revision(&mut plan, result.unwrap(), 0, "Obs", true);
        }

        // Third revision should be blocked
        let result = planner.analyze_observation("Entity already exists", 0, "limited_plan");
        assert!(result.is_none(), "Third revision should be blocked by limit");
    }

    #[test]
    fn test_revision_history_tracking() {
        let mut planner = DynamicPlanner::new();
        let mut plan = EditPlan::new(
            "tracked_plan", 1, "Tracked", "", ExecutionMode::Plan
        );
        plan.steps.push(EditPlanStep {
            id: "s1".into(),
            title: "Step 1".into(),
            target_module: TargetModule::Scene,
            action_description: "".into(),
            risk: OperationRisk::LowRisk,
            validation_requirements: vec![],
        });

        // Apply revision
        let revision = RevisionType::Skip {
            count: 1,
            reason: "Test".into(),
        };
        planner.apply_revision(&mut plan, revision, 0, "Obs", true).unwrap();

        // Check history
        let history = planner.get_plan_revisions("tracked_plan");
        assert_eq!(history.len(), 1);
        assert!(history[0].auto_applied);
        assert_eq!(history[0].step_index, 0);
        println!("Revision entry: {}", history[0].summary());
    }

    #[test]
    fn test_extract_entity_name() {
        assert_eq!(
            extract_entity_name_from_obs("Entity 'Player' already exists"),
            Some("Player".into())
        );
        assert_eq!(
            extract_entity_name_from_obs("Entity \"Boss\" not found"),
            Some("Boss".into())
        );
        assert_eq!(
            extract_entity_name_from_obs("No entity mentioned here"),
            None
        );
    }

    #[test]
    fn test_revision_type_describe() {
        let skip = RevisionType::Skip {
            count: 2,
            reason: "Duplicate".into(),
        };
        assert!(skip.describe().contains("Skip"));
        assert!(skip.describe().contains("2"));

        let abort = RevisionType::Abort {
            reason: "Fatal error".into(),
        };
        assert!(abort.describe().contains("ABORT"));
    }

    #[test]
    fn test_is_safe_auto_apply() {
        assert!(RevisionType::Skip {
            count: 1, reason: "".into()
        }.is_safe_auto_apply());

        assert!(RevisionType::InsertBefore {
            index: 0,
            step: unimplemented_step(),
            reason: "".into()
        }.is_safe_auto_apply());

        assert!(!RevisionType::Abort {
            reason: "".into()
        }.is_safe_auto_apply());
    }

    fn unimplemented_step() -> EditPlanStep {
        EditPlanStep {
            id: "test".into(),
            title: "Test".into(),
            target_module: TargetModule::Scene,
            action_description: "".into(),
            risk: OperationRisk::LowRisk,
            validation_requirements: vec![],
        }
    }
}
