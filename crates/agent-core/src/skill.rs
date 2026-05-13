//! Skill graph executor — DAG-based workflow engine for agent orchestration.
//!
//! Design reference: Section 12.5 of gpt-agent-team-task-event-skill-architecture.md
//!
//! Skills are directed acyclic graphs where nodes represent agent actions
//! and edges define execution order. The executor validates, dispatches,
//! and tracks progress through the DAG.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};

use crate::registry::CapabilityKind;

// ---------------------------------------------------------------------------
// Skill ID
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SkillId(pub u64);

// ---------------------------------------------------------------------------
// Skill Definition
// ---------------------------------------------------------------------------


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillDefinition {
    pub id: SkillId,
    pub name: String,
    pub description: String,
    pub inputs: Vec<SkillInput>,
    pub nodes: Vec<SkillNode>,
    pub edges: Vec<SkillEdge>,
}

impl SkillDefinition {
    pub fn to_mcp_description(&self) -> serde_json::Value {
        let properties: serde_json::Map<String, serde_json::Value> = self.inputs.iter()
            .map(|i| {
                (i.name.clone(), serde_json::json!({
                    "type": match i.input_type {
                        SkillInputType::String => "string",
                        SkillInputType::Number => "number",
                        SkillInputType::EntityId => "string",
                        SkillInputType::Bool => "boolean",
                        SkillInputType::Json => "object",
                    },
                    "description": i.description,
                }))
            })
            .collect();

        serde_json::json!({
            "name": self.name,
            "description": self.description,
            "inputSchema": {
                "type": "object",
                "properties": properties,
                "required": self.inputs.iter()
                    .filter(|i| i.required)
                    .map(|i| i.name.clone())
                    .collect::<Vec<_>>(),
            }
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillInput {
    pub name: String,
    pub description: String,
    pub input_type: SkillInputType,
    pub required: bool,
    pub default: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SkillInputType {
    String,
    Number,
    EntityId,
    Bool,
    Json,
}

/// Handler that maps skill-node actions to concrete operations.
///
/// Implementations translate action strings (e.g. "spawn_entity", "set_transform")
/// into real engine calls — typically via a `SceneBridge`.
#[allow(unused_variables)]
pub trait SkillActionHandler {
    fn handle(
        &mut self,
        action: &str,
        _params: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<serde_json::Value, String>;
}

/// Mock handler for testing — always succeeds.
pub struct MockSkillActionHandler;

impl SkillActionHandler for MockSkillActionHandler {
    fn handle(
        &mut self,
        _action: &str,
        _params: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<serde_json::Value, String> {
        Ok(serde_json::json!({"action": _action, "handled": true}))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillNode {
    pub id: String,
    pub title: String,
    pub required_capability: CapabilityKind,
    pub tool_name: Option<String>,
    pub input_mapping: serde_json::Value,
    pub retry: RetryPolicy,
    pub rollback: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillNodeResult {
    pub node_id: String,
    pub title: String,
    pub tool_name: Option<String>,
    pub output: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryPolicy {
    pub max_retries: u32,
    pub delay_ms: u64,
    pub backoff_multiplier: f64,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: 3,
            delay_ms: 1000,
            backoff_multiplier: 2.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillEdge {
    pub from: String,
    pub to: String,
    pub condition: SkillEdgeCondition,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SkillEdgeCondition {
    Always,
    OnSuccess,
    OnFailure,
    OnOutput { key: String, expected: serde_json::Value },
}

// ---------------------------------------------------------------------------
// Skill Instance (runtime)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillInstance {
    pub id: String,
    pub skill_id: SkillId,
    pub status: SkillInstanceStatus,
    pub node_states: HashMap<String, NodeState>,
    pub results: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SkillInstanceStatus {
    Pending,
    Running,
    Completed,
    Failed(String),
    WaitingForUser,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeState {
    Pending,
    Ready,
    Running,
    Complete,
    Failed(String),
    Skipped,
}

// ---------------------------------------------------------------------------
// Skill Executor
// ---------------------------------------------------------------------------

pub struct SkillExecutor;

impl SkillExecutor {
    pub fn new() -> Self {
        Self
    }

    /// Validate that all node IDs referenced in edges exist.
    pub fn validate(&self, skill: &SkillDefinition) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();

        let node_ids: HashSet<&str> = skill.nodes.iter().map(|n| n.id.as_str()).collect();

        for edge in &skill.edges {
            if !node_ids.contains(edge.from.as_str()) {
                errors.push(format!("Edge references unknown node '{}' (from)", edge.from));
            }
            if !node_ids.contains(edge.to.as_str()) {
                errors.push(format!("Edge references unknown node '{}' (to)", edge.to));
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Build an execution order via topological sort.
    pub fn build_execution_order(&self, skill: &SkillDefinition) -> Result<Vec<String>, String> {
        let mut in_degree: HashMap<&str, u32> = HashMap::new();
        let mut adjacency: HashMap<&str, Vec<&str>> = HashMap::new();

        for node in &skill.nodes {
            in_degree.entry(&node.id).or_insert(0);
            adjacency.entry(&node.id).or_default();
        }

        for edge in &skill.edges {
            *in_degree.entry(&edge.to).or_insert(0) += 1;
            adjacency.entry(&edge.from).or_default().push(&edge.to);
        }

        let mut queue: VecDeque<&str> = VecDeque::new();
        for node in &skill.nodes {
            if in_degree[node.id.as_str()] == 0 {
                queue.push_back(&node.id);
            }
        }

        let mut order = Vec::new();
        while let Some(current) = queue.pop_front() {
            order.push(current.to_string());
            if let Some(neighbors) = adjacency.get(current) {
                for &neighbor in neighbors {
                    let deg = in_degree.get_mut(neighbor).unwrap();
                    *deg -= 1;
                    if *deg == 0 {
                        queue.push_back(neighbor);
                    }
                }
            }
        }

        if order.len() != skill.nodes.len() {
            return Err("Skill graph contains a cycle".to_string());
        }

        Ok(order)
    }

    /// Find nodes that are ready to execute based on current node states and edges.
    pub fn find_ready_nodes(
        &self,
        skill: &SkillDefinition,
        instance: &SkillInstance,
    ) -> Vec<String> {
        let mut ready = Vec::new();

        for node in &skill.nodes {
            let state = instance
                .node_states
                .get(&node.id)
                .unwrap_or(&NodeState::Pending);

            if *state != NodeState::Pending {
                continue;
            }

            let upstream_complete = skill
                .edges
                .iter()
                .filter(|e| e.to == node.id)
                .all(|e| {
                    let upstream_state = instance
                        .node_states
                        .get(&e.from)
                        .unwrap_or(&NodeState::Pending);
                    matches!(upstream_state, NodeState::Complete | NodeState::Skipped)
                });

            if upstream_complete {
                ready.push(node.id.clone());
            }
        }

        ready
    }

    /// Execute the full skill DAG with a handler.
    ///
    /// Runs nodes in topological order, calling `handler.handle()` for each.
    /// Outputs from upstream nodes are passed as `upstream_output` in the
    /// params of downstream nodes.
    pub fn execute_with_handler(
        &self,
        skill: &SkillDefinition,
        handler: &mut dyn SkillActionHandler,
    ) -> Result<Vec<SkillNodeResult>, String> {
        self.validate(skill).map_err(|errs| errs.join("; "))?;
        let order = self.build_execution_order(skill)?;

        let mut outputs: HashMap<String, serde_json::Value> = HashMap::new();
        let mut results = Vec::new();

        for node_id in &order {
            let node = skill.nodes.iter().find(|n| &n.id == node_id)
                .ok_or_else(|| format!("Node '{}' not found in skill definition", node_id))?;

            // Convert input_mapping JSON → params HashMap
            let mut merged_params: HashMap<String, serde_json::Value> =
                if let Some(obj) = node.input_mapping.as_object() {
                    obj.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
                } else {
                    HashMap::new()
                };

            // Inject upstream outputs
            for edge in &skill.edges {
                if &edge.to == node_id {
                    if let Some(upstream_output) = outputs.get(&edge.from) {
                        merged_params.insert("upstream_output".into(), upstream_output.clone());
                    }
                }
            }

            let action = node.tool_name.as_deref().unwrap_or("noop");
            let result = handler.handle(action, &merged_params)
                .map_err(|e| format!("Node '{}' ({}) failed: {}", node.id, node.title, e))?;

            outputs.insert(node.id.clone(), result.clone());
            results.push(SkillNodeResult {
                node_id: node.id.clone(),
                title: node.title.clone(),
                tool_name: node.tool_name.clone(),
                output: result,
            });
        }

        Ok(results)
    }
}

impl Default for SkillExecutor {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Skill Registry
// ---------------------------------------------------------------------------

pub struct SkillRegistry {
    skills: HashMap<SkillId, SkillDefinition>,
    name_index: HashMap<String, SkillId>,
    capability_index: HashMap<CapabilityKind, Vec<SkillId>>,
}

impl SkillRegistry {
    pub fn new() -> Self {
        Self {
            skills: HashMap::new(),
            name_index: HashMap::new(),
            capability_index: HashMap::new(),
        }
    }

    pub fn register(&mut self, skill: SkillDefinition) {
        let id = skill.id;
        self.name_index.insert(skill.name.clone(), id);

        for node in &skill.nodes {
            self.capability_index
                .entry(node.required_capability.clone())
                .or_default()
                .push(id);
        }

        self.skills.insert(id, skill);
    }

    pub fn get(&self, id: &SkillId) -> Option<&SkillDefinition> {
        self.skills.get(id)
    }

    pub fn find_by_name(&self, name: &str) -> Option<&SkillDefinition> {
        self.name_index.get(name).and_then(|id| self.skills.get(id))
    }

    pub fn find_by_capability(
        &self,
        capability: &CapabilityKind,
    ) -> Vec<&SkillDefinition> {
        self.capability_index
            .get(capability)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| self.skills.get(id))
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn list_skill_names(&self) -> Vec<&str> {
        self.skills.values().map(|s| s.name.as_str()).collect()
    }

    pub fn len(&self) -> usize {
        self.skills.len()
    }

    pub fn is_empty(&self) -> bool {
        self.skills.is_empty()
    }

    /// Generate MCP (Model Context Protocol) -style tool descriptions for all skills.
    /// Useful for LLM tool selection: the LLM can see available skills and their schemas.
    pub fn all_mcp_descriptions(&self) -> Vec<serde_json::Value> {
        self.skills.values().map(|s| s.to_mcp_description()).collect()
    }

    /// MCP descriptions filtered by capability
    pub fn mcp_descriptions_for_capability(
        &self,
        capability: &CapabilityKind,
    ) -> Vec<serde_json::Value> {
        self.capability_index
            .get(capability)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| self.skills.get(id))
                    .map(|s| s.to_mcp_description())
                    .collect()
            })
            .unwrap_or_default()
    }
}

impl Default for SkillRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_create_enemy_skill() -> SkillDefinition {
        SkillDefinition {
            id: SkillId(1),
            name: "create_enemy_ai".to_string(),
            description: "Create an enemy with AI component".to_string(),
            inputs: vec![
                SkillInput {
                    name: "enemy_type".to_string(),
                    description: "Type of enemy".to_string(),
                    input_type: SkillInputType::String,
                    required: true,
                    default: None,
                },
            ],
            nodes: vec![
                SkillNode {
                    id: "analyze_scene".to_string(),
                    title: "Analyze current scene".to_string(),
                    required_capability: CapabilityKind::SceneRead,
                    tool_name: Some("query_entities".to_string()),
                    input_mapping: serde_json::json!({}),
                    retry: RetryPolicy::default(),
                    rollback: None,
                },
                SkillNode {
                    id: "create_entity".to_string(),
                    title: "Create enemy entity".to_string(),
                    required_capability: CapabilityKind::SceneWrite,
                    tool_name: Some("create_entity".to_string()),
                    input_mapping: serde_json::json!({}),
                    retry: RetryPolicy::default(),
                    rollback: Some("delete_entity".to_string()),
                },
                SkillNode {
                    id: "add_ai".to_string(),
                    title: "Add AI component".to_string(),
                    required_capability: CapabilityKind::SceneWrite,
                    tool_name: Some("update_component".to_string()),
                    input_mapping: serde_json::json!({}),
                    retry: RetryPolicy { max_retries: 5, ..RetryPolicy::default() },
                    rollback: Some("remove_component".to_string()),
                },
            ],
            edges: vec![
                SkillEdge {
                    from: "analyze_scene".to_string(),
                    to: "create_entity".to_string(),
                    condition: SkillEdgeCondition::Always,
                },
                SkillEdge {
                    from: "create_entity".to_string(),
                    to: "add_ai".to_string(),
                    condition: SkillEdgeCondition::OnSuccess,
                },
            ],
        }
    }

    #[test]
    fn test_validate_valid_skill() {
        let skill = make_create_enemy_skill();
        let executor = SkillExecutor::new();
        assert!(executor.validate(&skill).is_ok());
    }

    #[test]
    fn test_validate_invalid_edge() {
        let mut skill = make_create_enemy_skill();
        skill.edges.push(SkillEdge {
            from: "nonexistent".to_string(),
            to: "add_ai".to_string(),
            condition: SkillEdgeCondition::Always,
        });

        let executor = SkillExecutor::new();
        let result = executor.validate(&skill);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors.iter().any(|e| e.contains("nonexistent")));
    }

    #[test]
    fn test_topological_sort() {
        let skill = make_create_enemy_skill();
        let executor = SkillExecutor::new();
        let order = executor.build_execution_order(&skill).unwrap();
        // analyze_scene must come before create_entity
        let pos_analyze = order.iter().position(|n| n == "analyze_scene").unwrap();
        let pos_create = order.iter().position(|n| n == "create_entity").unwrap();
        assert!(pos_analyze < pos_create);
    }

    #[test]
    fn test_cycle_detection() {
        let mut skill = make_create_enemy_skill();
        skill.edges.push(SkillEdge {
            from: "add_ai".to_string(),
            to: "analyze_scene".to_string(),
            condition: SkillEdgeCondition::Always,
        });

        let executor = SkillExecutor::new();
        let result = executor.build_execution_order(&skill);
        assert!(result.is_err());
    }

    #[test]
    fn test_find_ready_nodes_all_pending() {
        let skill = make_create_enemy_skill();
        let instance = SkillInstance {
            id: "inst_1".to_string(),
            skill_id: skill.id,
            status: SkillInstanceStatus::Running,
            node_states: HashMap::new(),
            results: HashMap::new(),
        };

        let executor = SkillExecutor::new();
        let ready = executor.find_ready_nodes(&skill, &instance);
        // Only nodes with no upstream dependencies should be ready
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0], "analyze_scene");
    }

    #[test]
    fn test_skill_registry_register_and_find() {
        let mut registry = SkillRegistry::new();
        let skill = make_create_enemy_skill();
        registry.register(skill);

        assert_eq!(registry.len(), 1);
        let found = registry.find_by_name("create_enemy_ai");
        assert!(found.is_some());
        assert_eq!(found.unwrap().name, "create_enemy_ai");
    }

    #[test]
    fn test_skill_registry_find_by_capability() {
        let mut registry = SkillRegistry::new();
        registry.register(make_create_enemy_skill());

        let found = registry.find_by_capability(&CapabilityKind::SceneRead);
        assert!(!found.is_empty());
    }

    #[test]
    fn test_retry_policy_default() {
        let policy = RetryPolicy::default();
        assert_eq!(policy.max_retries, 3);
        assert_eq!(policy.delay_ms, 1000);
        assert_eq!(policy.backoff_multiplier, 2.0);
    }
}
