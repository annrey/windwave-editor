//! Skills Compound System - Learning from Completed Tasks
//!
//! Inspired by Multica's approach to skill compounding. This module enables
//! extracting reusable patterns from completed tasks and converting them
//! into new, compound skills that can be reused.
//!
//! Core idea: Every successful task completion contributes to the agent's
//! collective knowledge, creating compound skills that can be applied
//! to similar future tasks.

use crate::registry::CapabilityKind;
use crate::squad::{SquadTask, TaskId};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

pub type SkillCompound = SkillCompoundManager;

// ---------------------------------------------------------------------------
// Compound Skill ID
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CompoundSkillId(pub u64);

// ---------------------------------------------------------------------------
// Task Pattern
// ---------------------------------------------------------------------------

/// A pattern extracted from a completed task
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskPattern {
    pub id: TaskId,
    pub title_pattern: String,
    pub description_pattern: String,
    pub capabilities_used: Vec<CapabilityKind>,
    pub steps: Vec<PatternStep>,
    pub success_score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternStep {
    pub order: u32,
    pub action: String,
    pub parameters: HashMap<String, String>,
    pub outcome: String,
}

// ---------------------------------------------------------------------------
// Compound Skill
// ---------------------------------------------------------------------------

/// A compound skill created from one or more task patterns
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompoundSkill {
    pub id: CompoundSkillId,
    pub name: String,
    pub description: String,
    pub source_tasks: Vec<TaskId>,
    pub patterns: Vec<TaskPattern>,
    pub required_capabilities: Vec<CapabilityKind>,
    pub steps: Vec<SkillCompoundStep>,
    pub usage_count: u64,
    pub success_rate: f64,
    pub created_at: u64,
    pub last_used: Option<u64>,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillCompoundStep {
    pub order: u32,
    pub action: String,
    pub description: String,
    pub parameters: Vec<StepParameter>,
    pub optional: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepParameter {
    pub name: String,
    pub param_type: String,
    pub required: bool,
    pub default: Option<String>,
    pub description: String,
}

// ---------------------------------------------------------------------------
// Skill Compound Registry
// ---------------------------------------------------------------------------

pub struct SkillCompoundRegistry {
    skills: HashMap<CompoundSkillId, CompoundSkill>,
    task_patterns: HashMap<TaskId, TaskPattern>,
    id_counter: u64,
    task_skill_mapping: HashMap<TaskId, Vec<CompoundSkillId>>,
    tag_index: HashMap<String, Vec<CompoundSkillId>>,
    capability_index: HashMap<CapabilityKind, Vec<CompoundSkillId>>,
}

impl Default for SkillCompoundRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl SkillCompoundRegistry {
    pub fn new() -> Self {
        Self {
            skills: HashMap::new(),
            task_patterns: HashMap::new(),
            id_counter: 0,
            task_skill_mapping: HashMap::new(),
            tag_index: HashMap::new(),
            capability_index: HashMap::new(),
        }
    }

    /// Extract a pattern from a completed task
    pub fn extract_pattern(
        &mut self,
        task: &SquadTask,
        success: bool,
        execution_steps: Vec<PatternStep>,
    ) -> TaskPattern {
        let success_score = if success { 1.0 } else { 0.2 };

        let pattern = TaskPattern {
            id: task.id,
            title_pattern: Self::simplify_pattern(&task.title),
            description_pattern: Self::simplify_pattern(&task.description),
            capabilities_used: task.required_capabilities.clone(),
            steps: execution_steps,
            success_score,
        };

        self.task_patterns.insert(task.id, pattern.clone());
        pattern
    }

    /// Simplify text to create a searchable pattern
    fn simplify_pattern(text: &str) -> String {
        let lower = text.to_lowercase();
        let words: Vec<&str> = lower.split_whitespace().collect();
        words.join(" ")
    }

    /// Create a compound skill from one or more patterns
    pub fn create_compound_skill(
        &mut self,
        name: String,
        description: String,
        patterns: Vec<TaskPattern>,
        tags: Vec<String>,
    ) -> Result<CompoundSkillId, String> {
        if patterns.is_empty() {
            return Err("Need at least one pattern to create a skill".to_string());
        }

        // Collect all required capabilities from patterns
        let mut all_caps = HashSet::new();
        for p in &patterns {
            for cap in &p.capabilities_used {
                all_caps.insert(*cap);
            }
        }
        let required_capabilities: Vec<CapabilityKind> = all_caps.into_iter().collect();

        // Merge steps from all patterns (simple merging for now)
        let mut steps = Vec::new();
        for (idx, step) in patterns[0].steps.iter().enumerate() {
            steps.push(SkillCompoundStep {
                order: idx as u32,
                action: step.action.clone(),
                description: format!("Step {}: {}", idx + 1, step.action),
                parameters: vec![],
                optional: false,
            });
        }

        // Calculate average success rate
        let avg_success_rate: f64 =
            patterns.iter().map(|p| p.success_score).sum::<f64>() / patterns.len() as f64;

        let skill_id = CompoundSkillId(self.id_counter);
        self.id_counter += 1;

        let source_tasks: Vec<TaskId> = patterns.iter().map(|p| p.id).collect();

        let skill = CompoundSkill {
            id: skill_id,
            name,
            description,
            source_tasks: source_tasks.clone(),
            patterns,
            required_capabilities,
            steps,
            usage_count: 0,
            success_rate: avg_success_rate,
            created_at: Self::now_secs(),
            last_used: None,
            tags: tags.clone(),
        };

        // Update indices
        for &task_id in &source_tasks {
            self.task_skill_mapping
                .entry(task_id)
                .or_default()
                .push(skill_id);
        }

        for tag in &tags {
            self.tag_index
                .entry(tag.clone())
                .or_default()
                .push(skill_id);
        }

        for &cap in &skill.required_capabilities {
            self.capability_index.entry(cap).or_default().push(skill_id);
        }

        self.skills.insert(skill_id, skill);
        Ok(skill_id)
    }

    /// Get a compound skill by ID
    pub fn get_skill(&self, id: CompoundSkillId) -> Option<&CompoundSkill> {
        self.skills.get(&id)
    }

    /// Get all compound skills
    pub fn list_skills(&self) -> Vec<&CompoundSkill> {
        self.skills.values().collect()
    }

    /// Find skills by capability
    pub fn find_by_capability(&self, cap: CapabilityKind) -> Vec<&CompoundSkill> {
        self.capability_index
            .get(&cap)
            .map(|ids| ids.iter().filter_map(|id| self.skills.get(id)).collect())
            .unwrap_or_default()
    }

    /// Find skills by tag
    pub fn find_by_tag(&self, tag: &str) -> Vec<&CompoundSkill> {
        self.tag_index
            .get(tag)
            .map(|ids| ids.iter().filter_map(|id| self.skills.get(id)).collect())
            .unwrap_or_default()
    }

    /// Record usage of a skill (updates metrics)
    pub fn record_usage(&mut self, skill_id: CompoundSkillId, success: bool) -> Option<()> {
        let skill = self.skills.get_mut(&skill_id)?;
        skill.usage_count += 1;
        skill.last_used = Some(Self::now_secs());

        // Update success rate using exponential moving average
        let weight = 0.1;
        let success_val = if success { 1.0 } else { 0.0 };
        skill.success_rate = (1.0 - weight) * skill.success_rate + weight * success_val;

        Some(())
    }

    /// Search for similar patterns to a given task
    pub fn find_similar_patterns(&self, title: &str, description: &str) -> Vec<&TaskPattern> {
        let search_pattern = Self::simplify_pattern(&format!("{} {}", title, description));
        let search_words: HashSet<&str> = search_pattern.split_whitespace().collect();

        let mut matches = Vec::new();

        for pattern in self.task_patterns.values() {
            let pattern_words: HashSet<&str> = pattern
                .title_pattern
                .split_whitespace()
                .chain(pattern.description_pattern.split_whitespace())
                .collect();
            let overlap = search_words.intersection(&pattern_words).count();
            if overlap > 0 {
                matches.push((overlap, pattern));
            }
        }

        // Sort by overlap count
        matches.sort_by(|a, b| b.0.cmp(&a.0));
        matches.into_iter().map(|(_, p)| p).collect()
    }

    fn now_secs() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }
}

// ---------------------------------------------------------------------------
// Skill Compound Manager - Orchestrates skill compounding
// ---------------------------------------------------------------------------

pub struct SkillCompoundManager {
    registry: SkillCompoundRegistry,
    auto_compound: bool,
    min_patterns_for_compound: usize,
}

impl Default for SkillCompoundManager {
    fn default() -> Self {
        Self::new()
    }
}

impl SkillCompoundManager {
    pub fn new() -> Self {
        Self {
            registry: SkillCompoundRegistry::new(),
            auto_compound: true,
            min_patterns_for_compound: 2,
        }
    }

    /// Get the registry
    pub fn registry(&self) -> &SkillCompoundRegistry {
        &self.registry
    }

    /// Get mutable registry
    pub fn registry_mut(&mut self) -> &mut SkillCompoundRegistry {
        &mut self.registry
    }

    /// Process a completed task and optionally auto-compound
    pub fn process_completed_task(
        &mut self,
        task: &SquadTask,
        success: bool,
        steps: Vec<PatternStep>,
    ) -> Option<CompoundSkillId> {
        // Extract the pattern first
        let pattern = self.registry.extract_pattern(task, success, steps);

        // Auto-compound if enabled
        if self.auto_compound && success {
            return self.try_auto_compound(&pattern);
        }

        None
    }

    /// Try to create a compound skill from similar patterns
    fn try_auto_compound(&mut self, new_pattern: &TaskPattern) -> Option<CompoundSkillId> {
        // Find similar patterns
        let similar = self
            .registry
            .find_similar_patterns(&new_pattern.title_pattern, &new_pattern.description_pattern);

        if similar.len() >= self.min_patterns_for_compound {
            let mut all_patterns = vec![new_pattern.clone()];
            all_patterns.extend(similar.into_iter().cloned());

            // Generate a skill name
            let skill_name = format!("Compound: {}", new_pattern.title_pattern);
            let skill_desc = format!(
                "Auto-generated skill from {} similar tasks",
                all_patterns.len()
            );

            // Create tags from capabilities
            let tags: Vec<String> = new_pattern
                .capabilities_used
                .iter()
                .map(|c| format!("{:?}", c))
                .collect();

            return self
                .registry
                .create_compound_skill(skill_name, skill_desc, all_patterns, tags)
                .ok();
        }

        None
    }

    /// Record a successful skill execution, auto-compounding if patterns match.
    pub fn record_success(
        &mut self,
        skill_name: &str,
        context: &str,
        result: &str,
    ) -> Option<CompoundSkillId> {
        let task = SquadTask::new(
            TaskId(0),
            skill_name.to_string(),
            format!("{}: {}", context, result),
            vec![CapabilityKind::SceneWrite],
        );
        let steps = vec![PatternStep {
            order: 1,
            action: skill_name.to_string(),
            parameters: HashMap::new(),
            outcome: result.to_string(),
        }];
        self.process_completed_task(&task, true, steps)
    }

    /// Get skill recommendations for a new task
    pub fn recommend_skills(&self, task: &SquadTask) -> Vec<&CompoundSkill> {
        let mut recommendations = Vec::new();

        // Find by capabilities
        for &cap in &task.required_capabilities {
            recommendations.extend(self.registry.find_by_capability(cap));
        }

        // Find by similar patterns
        let similar = self
            .registry
            .find_similar_patterns(&task.title, &task.description);
        for pattern in similar {
            if let Some(skill_ids) = self.registry.task_skill_mapping.get(&pattern.id) {
                for &skill_id in skill_ids {
                    if let Some(skill) = self.registry.get_skill(skill_id) {
                        if !recommendations
                            .iter()
                            .any(|s: &&CompoundSkill| s.id == skill_id)
                        {
                            recommendations.push(skill);
                        }
                    }
                }
            }
        }

        // Sort by success rate and usage
        recommendations.sort_by(|a, b| {
            let score_a = a.success_rate * a.usage_count as f64;
            let score_b = b.success_rate * b.usage_count as f64;
            score_b
                .partial_cmp(&score_a)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        recommendations
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::CapabilityKind;

    #[test]
    fn test_pattern_extraction() {
        let mut registry = SkillCompoundRegistry::new();

        let task = SquadTask::new(
            TaskId(1),
            "Create player".to_string(),
            "Create a player entity with mesh and physics".to_string(),
            vec![CapabilityKind::SceneRead, CapabilityKind::SceneWrite],
        );

        let steps = vec![PatternStep {
            order: 1,
            action: "spawn_entity".to_string(),
            parameters: HashMap::new(),
            outcome: "Entity created".to_string(),
        }];

        let pattern = registry.extract_pattern(&task, true, steps);
        assert_eq!(pattern.id, TaskId(1));
        assert_eq!(pattern.success_score, 1.0);
    }

    #[test]
    fn test_create_compound_skill() {
        let mut registry = SkillCompoundRegistry::new();

        let task1 = SquadTask::new(
            TaskId(1),
            "Create player".to_string(),
            "Create entity".to_string(),
            vec![CapabilityKind::SceneWrite],
        );

        let task2 = SquadTask::new(
            TaskId(2),
            "Create enemy".to_string(),
            "Create entity".to_string(),
            vec![CapabilityKind::SceneWrite],
        );

        let pattern1 = registry.extract_pattern(&task1, true, vec![]);
        let pattern2 = registry.extract_pattern(&task2, true, vec![]);

        let skill_id = registry
            .create_compound_skill(
                "Create Entity".to_string(),
                "Create any entity".to_string(),
                vec![pattern1, pattern2],
                vec!["entity".to_string()],
            )
            .unwrap();

        let skill = registry.get_skill(skill_id).unwrap();
        assert_eq!(skill.name, "Create Entity");
        assert_eq!(skill.success_rate, 1.0);
    }

    #[test]
    fn test_skill_compound_manager() {
        let mut manager = SkillCompoundManager::new();

        let task = SquadTask::new(
            TaskId(1),
            "Test task".to_string(),
            "Description".to_string(),
            vec![CapabilityKind::SceneWrite],
        );

        manager.process_completed_task(&task, true, vec![]);
        assert_eq!(manager.registry().list_skills().len(), 0); // Need more patterns to auto-compound
    }
}
