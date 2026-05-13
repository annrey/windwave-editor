//! Procedural Memory (L0) - Workflow templates and decision patterns
//!
//! Stores validated workflows and decision patterns for reuse.
//! Inspired by agentmemory's procedural tier and the existing procedural_memory module.

use crate::memory::{MemoryEntryId, MemoryMetadata, MemoryTier};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A validated workflow template
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowTemplate {
    pub metadata: MemoryMetadata,
    pub name: String,
    /// Natural-language trigger condition
    pub trigger: String,
    /// Ordered list of steps
    pub steps: Vec<WorkflowStep>,
    /// Estimated success rate [0.0, 1.0]
    pub success_rate: f32,
    /// How many times this template has been used
    pub use_count: u32,
    /// Category for organization
    pub category: String,
}

impl WorkflowTemplate {
    pub fn new(id: u64, name: impl Into<String>, trigger: impl Into<String>) -> Self {
        Self {
            metadata: MemoryMetadata::new(id, MemoryTier::Procedural),
            name: name.into(),
            trigger: trigger.into(),
            steps: Vec::new(),
            success_rate: 0.5,
            use_count: 0,
            category: "general".to_string(),
        }
    }

    pub fn with_step(mut self, step: WorkflowStep) -> Self {
        self.steps.push(step);
        self
    }

    pub fn with_category(mut self, category: impl Into<String>) -> Self {
        self.category = category.into();
        self
    }

    pub fn full_text(&self) -> String {
        let steps_text = self.steps.iter()
            .map(|s| format!("{}:{}", s.tool_name, s.description))
            .collect::<Vec<_>>()
            .join(" ");
        format!("{} {} {} {}", self.name, self.trigger, self.category, steps_text)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowStep {
    pub description: String,
    pub tool_name: String,
    pub parameters: Option<serde_json::Value>,
}

/// A learned decision pattern
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionPattern {
    pub metadata: MemoryMetadata,
    pub context: String,
    pub decision: String,
    pub outcome: String,
    pub confidence: f32,
    pub observation_count: u32,
}

impl DecisionPattern {
    pub fn new(id: u64, context: impl Into<String>, decision: impl Into<String>, outcome: impl Into<String>) -> Self {
        Self {
            metadata: MemoryMetadata::new(id, MemoryTier::Procedural),
            context: context.into(),
            decision: decision.into(),
            outcome: outcome.into(),
            confidence: 0.5,
            observation_count: 1,
        }
    }

    pub fn full_text(&self) -> String {
        format!("{} {} {}", self.context, self.decision, self.outcome)
    }
}

/// Procedural Memory - L0 workflows and patterns
///
/// Design principles:
/// - Workflows are reusable action sequences
/// - Decision patterns capture cause-effect relationships
/// - Success rate tracking for quality ranking
/// - Keyword-based matching for workflow triggering
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProceduralMemory {
    workflows: Vec<WorkflowTemplate>,
    patterns: Vec<DecisionPattern>,
    next_id: u64,
    /// Workflow name -> index
    name_index: HashMap<String, usize>,
    /// Category -> workflow indices
    category_index: HashMap<String, Vec<usize>>,
}

impl ProceduralMemory {
    pub fn new() -> Self {
        Self {
            workflows: Vec::new(),
            patterns: Vec::new(),
            next_id: 1,
            name_index: HashMap::new(),
            category_index: HashMap::new(),
        }
    }

    /// Add a workflow template
    pub fn add_workflow(&mut self, workflow: WorkflowTemplate) -> MemoryEntryId {
        let id = workflow.metadata.id;
        let idx = self.workflows.len();
        self.name_index.insert(workflow.name.clone(), idx);
        self.category_index.entry(workflow.category.clone()).or_default().push(idx);
        self.workflows.push(workflow);
        id
    }

    /// Create and add a workflow
    pub fn create_workflow(
        &mut self,
        name: impl Into<String>,
        trigger: impl Into<String>,
        category: impl Into<String>,
    ) -> MemoryEntryId {
        let id = self.next_id();
        let workflow = WorkflowTemplate::new(id.0, name, trigger)
            .with_category(category);
        self.add_workflow(workflow)
    }

    /// Find workflows matching a request (keyword overlap)
    pub fn find_matching(&self, request: &str, top_k: usize) -> Vec<&WorkflowTemplate> {
        let request_lower = request.to_lowercase();
        let request_tokens: Vec<String> = request_lower.split_whitespace().map(|s| s.to_string()).collect();

        let mut scored: Vec<(usize, f32)> = self.workflows.iter()
            .enumerate()
            .map(|(idx, wf)| {
                let wf_text = wf.full_text().to_lowercase();
                let overlap = request_tokens.iter()
                    .filter(|token| wf_text.contains(*token))
                    .count() as f32;
                let score = overlap * wf.success_rate;
                (idx, score)
            })
            .filter(|(_, score)| *score > 0.0)
            .collect();

        scored.sort_by(|a, b| {
            b.1.partial_cmp(&a.1)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        scored.into_iter()
            .take(top_k)
            .filter_map(|(idx, _)| self.workflows.get(idx))
            .collect()
    }

    /// Find workflows by category
    pub fn find_by_category(&self, category: &str) -> Vec<&WorkflowTemplate> {
        self.category_index.get(category)
            .map(|indices| {
                indices.iter()
                    .filter_map(|&idx| self.workflows.get(idx))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Get workflow by name
    pub fn get_workflow(&self, name: &str) -> Option<&WorkflowTemplate> {
        self.name_index.get(name)
            .and_then(|&idx| self.workflows.get(idx))
    }

    /// Record workflow use (update success rate)
    pub fn record_use(&mut self, name: &str, success: bool) {
        if let Some(idx) = self.name_index.get(name).copied() {
            if let Some(wf) = self.workflows.get_mut(idx) {
                wf.use_count += 1;
                // Exponential moving average
                let alpha = 0.1;
                wf.success_rate = wf.success_rate * (1.0 - alpha) + (if success { 1.0 } else { 0.0 }) * alpha;
                wf.metadata.touch();
            }
        }
    }

    /// Add a decision pattern
    pub fn add_pattern(&mut self, pattern: DecisionPattern) -> MemoryEntryId {
        let id = pattern.metadata.id;
        self.patterns.push(pattern);
        id
    }

    /// Observe a decision and update/create pattern
    pub fn observe_decision(
        &mut self,
        context: &str,
        decision: &str,
        outcome: &str,
        success: bool,
    ) -> MemoryEntryId {
        // Try to find existing pattern
        if let Some(idx) = self.patterns.iter().position(|p| {
            p.context == context && p.decision == decision
        }) {
            let pattern = &mut self.patterns[idx];
            pattern.observation_count += 1;
            pattern.outcome = outcome.to_string();
            let alpha = 0.1;
            pattern.confidence = pattern.confidence * (1.0 - alpha)
                + (if success { 1.0 } else { 0.0 }) * alpha;
            pattern.metadata.touch();
            return pattern.metadata.id;
        }

        // Create new pattern
        let id = self.next_id();
        let pattern = DecisionPattern::new(id.0, context, decision, outcome);
        self.add_pattern(pattern)
    }

    /// Find decision patterns for a context
    pub fn find_patterns(&self, context: &str, min_confidence: f32, top_k: usize) -> Vec<&DecisionPattern> {
        let context_lower = context.to_lowercase();
        let mut scored: Vec<(usize, f32)> = self.patterns.iter()
            .enumerate()
            .filter(|(_, p)| p.confidence >= min_confidence)
            .map(|(idx, p)| {
                let overlap = if p.context.to_lowercase().contains(&context_lower) { 1.0 } else { 0.0 };
                let score = overlap * p.confidence * (p.observation_count as f32).sqrt();
                (idx, score)
            })
            .filter(|(_, score)| *score > 0.0)
            .collect();

        scored.sort_by(|a, b| {
            b.1.partial_cmp(&a.1)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        scored.into_iter()
            .take(top_k)
            .filter_map(|(idx, _)| self.patterns.get(idx))
            .collect()
    }

    /// Build summary for LLM context
    pub fn build_summary(&self, request: &str, max_workflows: usize, max_patterns: usize) -> String {
        let mut parts = Vec::new();

        // Matching workflows
        let workflows = self.find_matching(request, max_workflows);
        if !workflows.is_empty() {
            parts.push("## Suggested Workflows".to_string());
            for wf in workflows {
                parts.push(format!(
                    "- {} ({}): {} steps, {:.0}% success rate",
                    wf.name,
                    wf.category,
                    wf.steps.len(),
                    wf.success_rate * 100.0
                ));
            }
        }

        // Relevant patterns
        let patterns = self.find_patterns(request, 0.5, max_patterns);
        if !patterns.is_empty() {
            parts.push("## Learned Patterns".to_string());
            for p in patterns {
                parts.push(format!(
                    "- In '{}', '{}' → {} (confidence: {:.0}%, {} observations)",
                    p.context,
                    p.decision,
                    p.outcome,
                    p.confidence * 100.0,
                    p.observation_count
                ));
            }
        }

        if parts.is_empty() {
            String::new()
        } else {
            parts.join("\n")
        }
    }

    /// Seed with default Bevy workflows
    pub fn seed_with_defaults(&mut self) {
        let create_entity = WorkflowTemplate::new(self.next_id().0, "create_entity", "create spawn new entity")
            .with_category("scene")
            .with_step(WorkflowStep {
                description: "Create entity with name".to_string(),
                tool_name: "create_entity".to_string(),
                parameters: Some(serde_json::json!({"name": "{entity_name}"})),
            })
            .with_step(WorkflowStep {
                description: "Add Transform component".to_string(),
                tool_name: "add_component".to_string(),
                parameters: Some(serde_json::json!({"component": "Transform", "position": [0, 0, 0]})),
            });
        self.add_workflow(create_entity);

        let create_player = WorkflowTemplate::new(self.next_id().0, "create_player", "create player character")
            .with_category("scene")
            .with_step(WorkflowStep {
                description: "Create player entity".to_string(),
                tool_name: "create_entity".to_string(),
                parameters: Some(serde_json::json!({"name": "Player"})),
            })
            .with_step(WorkflowStep {
                description: "Add Player component".to_string(),
                tool_name: "add_component".to_string(),
                parameters: Some(serde_json::json!({"component": "Player"})),
            })
            .with_step(WorkflowStep {
                description: "Add Sprite component".to_string(),
                tool_name: "add_component".to_string(),
                parameters: Some(serde_json::json!({"component": "Sprite", "color": "blue"})),
            });
        self.add_workflow(create_player);

        let add_movement = WorkflowTemplate::new(self.next_id().0, "add_movement", "add movement system wasd")
            .with_category("code")
            .with_step(WorkflowStep {
                description: "Create movement system".to_string(),
                tool_name: "create_system".to_string(),
                parameters: Some(serde_json::json!({
                    "name": "player_movement",
                    "query": "&mut Transform, With<Player>",
                    "input": "WASD"
                })),
            });
        self.add_workflow(add_movement);
    }

    /// Workflow count
    pub fn workflow_count(&self) -> usize {
        self.workflows.len()
    }

    /// Pattern count
    pub fn pattern_count(&self) -> usize {
        self.patterns.len()
    }

    fn next_id(&mut self) -> MemoryEntryId {
        let id = MemoryEntryId(self.next_id);
        self.next_id += 1;
        id
    }
}

impl Default for ProceduralMemory {
    fn default() -> Self {
        let mut mem = Self::new();
        mem.seed_with_defaults();
        mem
    }
}
