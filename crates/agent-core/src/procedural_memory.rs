//! Procedural memory — workflow templates and decision patterns.
//!
//! Inspired by agentmemory's procedural memory tier. Stores validated workflow
//! templates and decision patterns that the agent can recall and reuse.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// WorkflowTemplate
// ---------------------------------------------------------------------------

/// A validated workflow template — a named sequence of steps that can be
/// replayed when a trigger condition is detected.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowTemplate {
    pub name: String,
    /// Natural-language trigger condition (e.g. "用户要求创建新实体")
    pub trigger: String,
    /// Ordered list of steps
    pub steps: Vec<WorkflowStep>,
    /// Estimated success rate [0.0, 1.0]
    pub success_rate: f32,
    /// When this template was last used
    pub last_used: u64,
    /// How many times this template has been used
    pub use_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowStep {
    /// Description of what this step does
    pub description: String,
    /// Tool name to invoke
    pub tool_name: String,
    /// Parameters for the tool (JSON)
    pub parameters: Option<serde_json::Value>,
}

// ---------------------------------------------------------------------------
// DecisionPattern
// ---------------------------------------------------------------------------

/// A record of a past decision: in a given context, what decision was made,
/// what the outcome was, and the confidence level.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionPattern {
    /// Context description (e.g. "entity creation with color specification")
    pub context: String,
    /// The decision that was made
    pub decision: String,
    /// The outcome/result
    pub outcome: String,
    /// Confidence in this pattern [0.0, 1.0]
    pub confidence: f32,
    /// When this pattern was last observed
    pub last_seen: u64,
    /// How many times this pattern has been observed
    pub observation_count: u32,
}

// ---------------------------------------------------------------------------
// ProceduralMemory
// ---------------------------------------------------------------------------

/// Stores agent workflows and decision patterns for reuse.
pub struct ProceduralMemory {
    /// Indexed workflow templates by name.
    pub workflows: HashMap<String, WorkflowTemplate>,
    /// Decision patterns by context hash.
    pub patterns: Vec<DecisionPattern>,
}

impl ProceduralMemory {
    pub fn new() -> Self {
        Self {
            workflows: HashMap::new(),
            patterns: Vec::new(),
        }
    }

    // -----------------------------------------------------------------------
    // Workflow operations
    // -----------------------------------------------------------------------

    /// Add or update a workflow template.
    pub fn put_workflow(&mut self, wf: WorkflowTemplate) {
        self.workflows.insert(wf.name.clone(), wf);
    }

    /// Find workflows whose trigger matches the given text.
    /// Uses keyword overlap for matching.
    pub fn find_triggered(&self, request_text: &str, now: u64) -> Vec<&WorkflowTemplate> {
        let mut results: Vec<&WorkflowTemplate> = self
            .workflows
            .values()
            .filter(|wf| keyword_overlap(&wf.trigger, request_text) > 0.0)
            .collect();

        // Sort by success_rate * recency
        results.sort_by(|a, b| {
            let score_a = a.success_rate * (1.0 + 1.0 / (1.0 + (now.saturating_sub(a.last_used)) as f32));
            let score_b = b.success_rate * (1.0 + 1.0 / (1.0 + (now.saturating_sub(b.last_used)) as f32));
            score_b.partial_cmp(&score_a).unwrap_or(std::cmp::Ordering::Equal)
        });

        results
    }

    /// Record that a workflow was used (bump use_count + last_used).
    pub fn record_use(&mut self, name: &str, success: bool, now: u64) {
        if let Some(wf) = self.workflows.get_mut(name) {
            wf.use_count += 1;
            wf.last_used = now;
            // Adjust success rate with exponential moving average
            wf.success_rate = wf.success_rate * 0.9 + (if success { 1.0 } else { 0.0 }) * 0.1;
        }
    }

    /// Get top N workflows by success rate.
    pub fn top_workflows(&self, n: usize) -> Vec<&WorkflowTemplate> {
        let mut wfs: Vec<&WorkflowTemplate> = self.workflows.values().collect();
        wfs.sort_by(|a, b| b.success_rate.partial_cmp(&a.success_rate).unwrap_or(std::cmp::Ordering::Equal));
        wfs.truncate(n);
        wfs
    }

    // -----------------------------------------------------------------------
    // Decision pattern operations
    // -----------------------------------------------------------------------

    /// Record a decision pattern observation.
    pub fn observe_decision(
        &mut self,
        context: &str,
        decision: &str,
        outcome: &str,
        now: u64,
    ) {
        // Try to update existing pattern
        if let Some(dp) = self.patterns.iter_mut()
            .find(|dp| dp.context == context && dp.decision == decision) {
            dp.observation_count += 1;
            dp.last_seen = now;
            dp.confidence = dp.confidence * 0.9 + 0.1;
            return;
        }

        // New pattern
        self.patterns.push(DecisionPattern {
            context: context.to_string(),
            decision: decision.to_string(),
            outcome: outcome.to_string(),
            confidence: 0.5,
            last_seen: now,
            observation_count: 1,
        });
    }

    /// Find decision patterns relevant to a given context.
    pub fn find_decisions(&self, context: &str, min_confidence: f32) -> Vec<&DecisionPattern> {
        let mut results: Vec<&DecisionPattern> = self
            .patterns
            .iter()
            .filter(|dp| dp.confidence >= min_confidence && keyword_overlap(&dp.context, context) > 0.0)
            .collect();
        results.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap_or(std::cmp::Ordering::Equal));
        results
    }

    // -----------------------------------------------------------------------
    // Serialization
    // -----------------------------------------------------------------------

    /// Export all workflows and patterns as JSON.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        let data = serde_json::json!({
            "workflows": self.workflows.values().collect::<Vec<_>>(),
            "patterns": self.patterns,
        });
        serde_json::to_string_pretty(&data)
    }

    /// Import workflows and patterns from JSON.
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        let data: serde_json::Value = serde_json::from_str(json)?;
        let mut pm = Self::new();
        if let Some(wfs) = data["workflows"].as_array() {
            for wf in wfs {
                if let Ok(t) = serde_json::from_value::<WorkflowTemplate>(wf.clone()) {
                    pm.put_workflow(t);
                }
            }
        }
        if let Some(pats) = data["patterns"].as_array() {
            for p in pats {
                if let Ok(dp) = serde_json::from_value::<DecisionPattern>(p.clone()) {
                    pm.patterns.push(dp);
                }
            }
        }
        Ok(pm)
    }
}

impl Default for ProceduralMemory {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Simple keyword overlap ratio between two texts.
fn keyword_overlap(a: &str, b: &str) -> f32 {
    let tokens_a: Vec<String> = a.split_whitespace().map(|s| s.to_lowercase()).collect();
    let tokens_b: Vec<String> = b.split_whitespace().map(|s| s.to_lowercase()).collect();

    if tokens_a.is_empty() || tokens_b.is_empty() {
        return 0.0;
    }

    let common = tokens_a.iter().filter(|t| tokens_b.contains(t)).count();
    common as f32 / tokens_a.len().min(tokens_b.len()) as f32
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_put_and_find_workflow() {
        let mut pm = ProceduralMemory::new();
        pm.put_workflow(WorkflowTemplate {
            name: "create_entity".into(),
            trigger: "create entity spawn".into(),
            steps: vec![WorkflowStep {
                description: "创建实体".into(),
                tool_name: "create_entity".into(),
                parameters: None,
            }],
            success_rate: 0.9,
            last_used: 1000,
            use_count: 5,
        });

        let triggered = pm.find_triggered("create enemy spawn", 2000);
        assert!(!triggered.is_empty());
        assert_eq!(triggered[0].name, "create_entity");
    }

    #[test]
    fn test_record_use_updates_success_rate() {
        let mut pm = ProceduralMemory::new();
        pm.put_workflow(WorkflowTemplate {
            name: "test".into(),
            trigger: "test".into(),
            steps: vec![],
            success_rate: 1.0,
            last_used: 0,
            use_count: 0,
        });

        pm.record_use("test", false, 1000);
        if let Some(wf) = pm.workflows.get("test") {
            assert!(wf.success_rate < 1.0);
            assert_eq!(wf.use_count, 1);
        }
    }

    #[test]
    fn test_decision_pattern() {
        let mut pm = ProceduralMemory::new();
        pm.observe_decision("entity creation", "use blue color", "good", 1000);
        pm.observe_decision("entity creation", "use blue color", "good", 1001);

        let patterns = pm.find_decisions("entity creation", 0.4);
        assert!(!patterns.is_empty());
        assert!(patterns[0].confidence > 0.5);
    }

    #[test]
    fn test_json_roundtrip() {
        let mut pm = ProceduralMemory::new();
        pm.put_workflow(WorkflowTemplate {
            name: "wf1".into(),
            trigger: "test trigger".into(),
            steps: vec![],
            success_rate: 0.5,
            last_used: 42,
            use_count: 3,
        });
        pm.observe_decision("ctx", "dec", "out", 123);

        let json = pm.to_json().unwrap();
        let restored = ProceduralMemory::from_json(&json).unwrap();

        assert_eq!(restored.workflows.len(), 1);
        assert_eq!(restored.patterns.len(), 1);
    }
}
