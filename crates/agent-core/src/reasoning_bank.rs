//! Reasoning Bank Lite - Inspired by Ruflo
//!
//! A lightweight version of Ruflo's ReasoningBank, which stores and retrieves
//! reasoning traces, supports pattern recognition, and enables reuse of
//! successful reasoning strategies.
//!
//! This complements the Skills Compound system perfectly.

use crate::squad::TaskId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// ID Types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ReasoningTraceId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ReasoningStepId(pub u64);

// ---------------------------------------------------------------------------
// Reasoning Step
// ---------------------------------------------------------------------------

/// A single step in a reasoning trace
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReasoningStep {
    pub id: ReasoningStepId,
    pub step_type: StepType,
    pub description: String,
    pub input: Option<String>,
    pub output: Option<String>,
    pub reasoning: Option<String>,
    pub timestamp: u64,
    pub duration_ms: Option<u64>,
    pub success: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum StepType {
    /// Thinking phase - planning or reasoning
    Think,
    /// Acting phase - executing a tool or action
    Act,
    /// Observing phase - reviewing the result
    Observe,
    /// Decision point - choosing between alternatives
    Decide,
    /// Reflecting phase - self-correction or review
    Reflect,
}

// ---------------------------------------------------------------------------
// Reasoning Trace
// ---------------------------------------------------------------------------

/// A complete reasoning trace for a task
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReasoningTrace {
    pub id: ReasoningTraceId,
    pub task_id: TaskId,
    pub title: String,
    pub description: String,
    pub steps: Vec<ReasoningStep>,
    pub success: bool,
    pub created_at: u64,
    pub completed_at: Option<u64>,
    pub total_duration_ms: Option<u64>,
    pub tags: Vec<String>,
    pub agent_name: Option<String>,
}

impl ReasoningTrace {
    /// Create a new empty reasoning trace
    pub fn new(id: ReasoningTraceId, task_id: TaskId, title: String, description: String) -> Self {
        Self {
            id,
            task_id,
            title,
            description,
            steps: Vec::new(),
            success: false,
            created_at: Self::now_secs(),
            completed_at: None,
            total_duration_ms: None,
            tags: Vec::new(),
            agent_name: None,
        }
    }

    /// Add a step to the trace
    pub fn add_step(&mut self, step: ReasoningStep) {
        self.steps.push(step);
    }

    /// Mark the trace as complete
    pub fn mark_complete(&mut self, success: bool) {
        self.success = success;
        self.completed_at = Some(Self::now_secs());
        if let Some(first_step) = self.steps.first() {
            if let Some(last_step) = self.steps.last() {
                self.total_duration_ms = Some(last_step.timestamp - first_step.timestamp);
            }
        }
    }

    /// Get a summary of the trace
    pub fn summary(&self) -> String {
        let success_str = if self.success { "✅" } else { "❌" };
        format!(
            "{} Trace for '{}' - {} steps{}",
            success_str,
            self.title,
            self.steps.len(),
            if let Some(duration) = self.total_duration_ms {
                format!(" ({}ms)", duration)
            } else {
                "".to_string()
            }
        )
    }

    /// Extract keywords from the trace
    pub fn extract_keywords(&self) -> Vec<String> {
        let mut keywords = Vec::new();

        for step in &self.steps {
            // Simple heuristic for keywords
            let content = format!(
                "{} {} {}",
                step.description,
                step.input.as_deref().unwrap_or(""),
                step.output.as_deref().unwrap_or("")
            );

            let words: Vec<String> = content
                .split_whitespace()
                .map(|w| w.to_lowercase())
                .filter(|w| w.len() > 3)
                .collect();

            keywords.extend(words);
        }

        keywords.extend(self.tags.clone());

        let mut seen = std::collections::HashSet::new();
        keywords
            .into_iter()
            .filter(|w| seen.insert(w.clone()))
            .collect()
    }

    fn now_secs() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }
}

// ---------------------------------------------------------------------------
// Reasoning Pattern
// ---------------------------------------------------------------------------

/// A recognized pattern from multiple traces
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReasoningPattern {
    pub name: String,
    pub description: String,
    pub source_traces: Vec<ReasoningTraceId>,
    pub step_patterns: Vec<StepType>,
    pub average_success_rate: f64,
    pub usage_count: u64,
    pub tags: Vec<String>,
}

// ---------------------------------------------------------------------------
// Reasoning Bank
// ---------------------------------------------------------------------------

pub struct ReasoningBank {
    traces: HashMap<ReasoningTraceId, ReasoningTrace>,
    task_to_trace: HashMap<TaskId, Vec<ReasoningTraceId>>,
    patterns: HashMap<String, ReasoningPattern>,
    id_counter: u64,
    step_id_counter: u64,
    tag_index: HashMap<String, Vec<ReasoningTraceId>>,
}

impl Default for ReasoningBank {
    fn default() -> Self {
        Self::new()
    }
}

impl ReasoningBank {
    pub fn new() -> Self {
        Self {
            traces: HashMap::new(),
            task_to_trace: HashMap::new(),
            patterns: HashMap::new(),
            id_counter: 0,
            step_id_counter: 0,
            tag_index: HashMap::new(),
        }
    }

    /// Create a new reasoning trace for a task
    pub fn create_trace(
        &mut self,
        task_id: TaskId,
        title: String,
        description: String,
    ) -> ReasoningTraceId {
        let trace_id = ReasoningTraceId(self.id_counter);
        self.id_counter += 1;

        let trace = ReasoningTrace::new(trace_id, task_id, title, description);
        self.traces.insert(trace_id, trace);
        self.task_to_trace
            .entry(task_id)
            .or_default()
            .push(trace_id);
        trace_id
    }

    /// Get a trace by ID
    pub fn get_trace(&self, id: ReasoningTraceId) -> Option<&ReasoningTrace> {
        self.traces.get(&id)
    }

    /// Get a mutable trace
    pub fn get_trace_mut(&mut self, id: ReasoningTraceId) -> Option<&mut ReasoningTrace> {
        self.traces.get_mut(&id)
    }

    /// Add a step to an existing trace
    pub fn add_step_to_trace(
        &mut self,
        trace_id: ReasoningTraceId,
        step_type: StepType,
        description: String,
        input: Option<String>,
        output: Option<String>,
        reasoning: Option<String>,
        success: bool,
    ) -> Option<ReasoningStepId> {
        let trace = self.traces.get_mut(&trace_id)?;

        let step_id = ReasoningStepId(self.step_id_counter);
        self.step_id_counter += 1;

        let step = ReasoningStep {
            id: step_id,
            step_type,
            description,
            input,
            output,
            reasoning,
            timestamp: ReasoningTrace::now_secs(),
            duration_ms: None,
            success,
        };

        trace.add_step(step);
        Some(step_id)
    }

    /// Mark a trace as complete
    pub fn mark_trace_complete(
        &mut self,
        trace_id: ReasoningTraceId,
        success: bool,
        tags: Vec<String>,
    ) -> Option<()> {
        let trace = self.traces.get_mut(&trace_id)?;
        trace.mark_complete(success);
        trace.tags = tags.clone();

        for tag in tags {
            self.tag_index.entry(tag).or_default().push(trace_id);
        }

        Some(())
    }

    /// Get all traces for a task
    pub fn get_traces_for_task(&self, task_id: TaskId) -> Vec<&ReasoningTrace> {
        self.task_to_trace
            .get(&task_id)
            .map(|ids| ids.iter().filter_map(|id| self.traces.get(id)).collect())
            .unwrap_or_default()
    }

    /// Find similar traces by keywords
    pub fn find_similar_traces(&self, keywords: &[String]) -> Vec<&ReasoningTrace> {
        let mut matched_traces = HashMap::new();

        for keyword in keywords {
            if let Some(ids) = self.tag_index.get(keyword) {
                for &id in ids {
                    *matched_traces.entry(id).or_insert(0) += 1;
                }
            }
        }

        let mut sorted_traces: Vec<_> = matched_traces.into_iter().collect();
        sorted_traces.sort_by_key(|a| std::cmp::Reverse(a.1));

        sorted_traces
            .into_iter()
            .filter_map(|(id, _)| self.traces.get(&id))
            .collect()
    }

    /// Get successful traces
    pub fn get_successful_traces(&self) -> Vec<&ReasoningTrace> {
        self.traces.values().filter(|t| t.success).collect()
    }

    /// List all traces
    pub fn list_traces(&self) -> Vec<&ReasoningTrace> {
        self.traces.values().collect()
    }

    /// Quick convenience: record a single-step trace for a tool execution.
    pub fn record_trace(
        &mut self,
        tool_name: &str,
        result_text: &str,
        success: bool,
    ) -> Option<ReasoningTraceId> {
        let task_id = TaskId(self.id_counter);
        let trace_id = self.create_trace(
            task_id,
            tool_name.to_string(),
            format!("Tool execution: {}", tool_name),
        );
        self.add_step_to_trace(
            trace_id,
            StepType::Act,
            tool_name.to_string(),
            None,
            Some(result_text.to_string()),
            None,
            success,
        );
        self.mark_trace_complete(trace_id, success, vec![tool_name.to_string()]);
        Some(trace_id)
    }

    /// Create a simple pattern from similar traces
    pub fn create_pattern(&mut self, name: String, trace_ids: Vec<ReasoningTraceId>) -> Option<()> {
        let mut step_patterns = Vec::new();
        let mut total_success = 0.0;

        for &trace_id in &trace_ids {
            if let Some(trace) = self.traces.get(&trace_id) {
                let types: Vec<StepType> =
                    trace.steps.iter().map(|s| s.step_type.clone()).collect();
                if step_patterns.is_empty() {
                    step_patterns = types;
                }
                total_success += if trace.success { 1.0 } else { 0.0 };
            }
        }

        let avg_success_rate = if trace_ids.is_empty() {
            0.0
        } else {
            total_success / trace_ids.len() as f64
        };

        let pattern = ReasoningPattern {
            name: name.clone(),
            description: format!("Pattern created from {} traces", trace_ids.len()),
            source_traces: trace_ids,
            step_patterns,
            average_success_rate: avg_success_rate,
            usage_count: 0,
            tags: vec![],
        };

        self.patterns.insert(name, pattern);
        Some(())
    }
}

// ---------------------------------------------------------------------------
// Reasoning Bank Manager - High level interface
// ---------------------------------------------------------------------------

pub struct ReasoningBankManager {
    bank: ReasoningBank,
}

impl Default for ReasoningBankManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ReasoningBankManager {
    pub fn new() -> Self {
        Self {
            bank: ReasoningBank::new(),
        }
    }

    pub fn bank(&self) -> &ReasoningBank {
        &self.bank
    }

    pub fn bank_mut(&mut self) -> &mut ReasoningBank {
        &mut self.bank
    }

    /// Start recording a trace for a task
    pub fn start_trace(
        &mut self,
        task_id: TaskId,
        title: String,
        description: String,
    ) -> ReasoningTraceId {
        self.bank.create_trace(task_id, title, description)
    }

    /// Record a thinking step
    pub fn record_think(
        &mut self,
        trace_id: ReasoningTraceId,
        description: String,
        reasoning: String,
    ) {
        self.bank.add_step_to_trace(
            trace_id,
            StepType::Think,
            description,
            None,
            None,
            Some(reasoning),
            true,
        );
    }

    /// Record an action step
    pub fn record_act(
        &mut self,
        trace_id: ReasoningTraceId,
        description: String,
        input: String,
        output: String,
        success: bool,
    ) {
        self.bank.add_step_to_trace(
            trace_id,
            StepType::Act,
            description,
            Some(input),
            Some(output),
            None,
            success,
        );
    }

    /// Record an observation step
    pub fn record_observe(
        &mut self,
        trace_id: ReasoningTraceId,
        description: String,
        output: String,
    ) {
        self.bank.add_step_to_trace(
            trace_id,
            StepType::Observe,
            description,
            None,
            Some(output),
            None,
            true,
        );
    }

    /// Get reasoning recommendations for a new task
    pub fn get_recommendations(&self, keywords: &[String]) -> Vec<&ReasoningTrace> {
        let mut recommendations = self.bank.find_similar_traces(keywords);

        // Sort by success rate
        recommendations.sort_by(|a, b| {
            let score_a = if a.success { 1.0 } else { 0.0 } * a.steps.len() as f64;
            let score_b = if b.success { 1.0 } else { 0.0 } * b.steps.len() as f64;
            score_b
                .partial_cmp(&score_a)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        recommendations
    }

    // -----------------------------------------------------------------------
    // Phase 3: Automatic Pattern Discovery
    // -----------------------------------------------------------------------

    /// Discover patterns from successful traces
    pub fn discover_patterns(&mut self, min_traces: usize) -> Vec<ReasoningPattern> {
        let successful_traces = self.bank.get_successful_traces();

        if successful_traces.len() < min_traces {
            return Vec::new();
        }

        // Group traces by similar step sequences
        let mut pattern_groups: std::collections::HashMap<String, Vec<ReasoningTraceId>> =
            HashMap::new();

        for trace in successful_traces {
            let step_signature = self.get_step_signature(trace);
            pattern_groups
                .entry(step_signature)
                .or_default()
                .push(trace.id);
        }

        // Create patterns from groups with sufficient traces
        let mut patterns = Vec::new();
        for (signature, trace_ids) in pattern_groups {
            if trace_ids.len() >= min_traces {
                if let Some(pattern) = self.create_pattern_from_traces(&signature, &trace_ids) {
                    patterns.push(pattern);
                }
            }
        }

        // Store patterns in the bank
        for pattern in &patterns {
            self.bank
                .patterns
                .insert(pattern.name.clone(), pattern.clone());
        }

        patterns
    }

    /// Get a signature representing the sequence of step types
    fn get_step_signature(&self, trace: &ReasoningTrace) -> String {
        trace
            .steps
            .iter()
            .map(|s| format!("{:?}", s.step_type))
            .collect::<Vec<_>>()
            .join("|")
    }

    /// Create a pattern from a group of traces
    fn create_pattern_from_traces(
        &self,
        signature: &str,
        trace_ids: &[ReasoningTraceId],
    ) -> Option<ReasoningPattern> {
        let traces = trace_ids
            .iter()
            .filter_map(|&id| self.bank.get_trace(id))
            .collect::<Vec<_>>();

        if traces.is_empty() {
            return None;
        }

        let success_rate = traces.iter().filter(|t| t.success).count() as f64 / traces.len() as f64;

        let first_trace = traces.first()?;

        Some(ReasoningPattern {
            name: format!("Pattern_{}", signature.len()),
            description: format!("Step sequence: {}", signature),
            source_traces: trace_ids.to_vec(),
            step_patterns: first_trace
                .steps
                .iter()
                .map(|s| s.step_type.clone())
                .collect(),
            average_success_rate: success_rate,
            usage_count: 0,
            tags: Vec::new(),
        })
    }

    /// Get patterns that match a new request
    pub fn get_matching_patterns(&self, _request: &str) -> Vec<&ReasoningPattern> {
        let mut matches = Vec::new();

        for pattern in self.bank.patterns.values() {
            // Check if pattern has been successful
            if pattern.average_success_rate >= 0.5 {
                matches.push(pattern);
            }
        }

        // Sort by success rate
        matches.sort_by(|a, b| {
            b.average_success_rate
                .partial_cmp(&a.average_success_rate)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        matches
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::squad::TaskId;

    #[test]
    fn test_create_trace() {
        let mut bank = ReasoningBank::new();
        let trace_id = bank.create_trace(TaskId(1), "Test Task".into(), "Description".into());

        let trace = bank.get_trace(trace_id).unwrap();
        assert_eq!(trace.title, "Test Task");
        assert_eq!(trace.task_id, TaskId(1));
    }

    #[test]
    fn test_add_steps() {
        let mut bank = ReasoningBank::new();
        let trace_id = bank.create_trace(TaskId(1), "Test".into(), "".into());

        let step1 = bank.add_step_to_trace(
            trace_id,
            StepType::Think,
            "Think step".into(),
            None,
            None,
            Some("Reasoning".into()),
            true,
        );

        let step2 = bank.add_step_to_trace(
            trace_id,
            StepType::Act,
            "Act step".into(),
            Some("Input".into()),
            Some("Output".into()),
            None,
            true,
        );

        assert!(step1.is_some());
        assert!(step2.is_some());

        let trace = bank.get_trace(trace_id).unwrap();
        assert_eq!(trace.steps.len(), 2);
    }

    #[test]
    fn test_mark_complete() {
        let mut bank = ReasoningBank::new();
        let trace_id = bank.create_trace(TaskId(1), "Test".into(), "".into());
        bank.add_step_to_trace(
            trace_id,
            StepType::Think,
            "Step".into(),
            None,
            None,
            None,
            true,
        );
        bank.mark_trace_complete(trace_id, true, vec!["test".into()]);

        let trace = bank.get_trace(trace_id).unwrap();
        assert!(trace.success);
        assert_eq!(trace.tags, vec!["test"]);
    }

    #[test]
    fn test_manager() {
        let mut manager = ReasoningBankManager::new();

        let trace_id = manager.start_trace(TaskId(1), "Test Task".into(), "".into());
        manager.record_think(trace_id, "Thinking".into(), "Let's do this".into());
        manager.record_act(
            trace_id,
            "Acting".into(),
            "Input".into(),
            "Output".into(),
            true,
        );
        manager.record_observe(trace_id, "Observing".into(), "Result looks good".into());

        manager
            .bank_mut()
            .mark_trace_complete(trace_id, true, vec!["test".into()]);

        let traces = manager.bank().list_traces();
        assert_eq!(traces.len(), 1);
        assert_eq!(traces[0].steps.len(), 3);
    }

    #[test]
    fn test_extract_keywords() {
        let mut bank = ReasoningBank::new();
        let trace_id = bank.create_trace(TaskId(1), "Create entity".into(), "".into());
        bank.add_step_to_trace(
            trace_id,
            StepType::Think,
            "Creating player entity".into(),
            None,
            None,
            None,
            true,
        );
        bank.mark_trace_complete(trace_id, true, vec!["entity".into(), "player".into()]);

        let trace = bank.get_trace(trace_id).unwrap();
        let keywords = trace.extract_keywords();
        assert!(!keywords.is_empty());
    }
}
