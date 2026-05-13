//! Context types — structured bundles of information that are handed to
//! the Planner and Executor agents so they can make informed decisions without
//! needing to query the engine repeatedly.

use serde::{Deserialize, Serialize};

/// Context passed to the Planner agent so it can design an appropriate edit
/// plan for the current task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlannerContext {
    /// The task this context was assembled for.
    pub task_id: u64,

    /// Absolute or relative path to the project root directory.
    pub project_root: String,

    /// Names of all agents currently available for delegation.
    pub available_agents: Vec<String>,

    /// Names of all tools currently registered in the tool registry.
    pub available_tools: Vec<String>,

    /// A summary of the current scene graph (entity count, notable entities).
    pub scene_summary: Option<String>,
}

impl PlannerContext {
    /// Create a minimal planner context.
    pub fn new(task_id: u64, project_root: impl Into<String>) -> Self {
        Self {
            task_id,
            project_root: project_root.into(),
            available_agents: Vec::new(),
            available_tools: Vec::new(),
            scene_summary: None,
        }
    }

    /// Attach a scene summary.
    pub fn with_scene_summary(mut self, summary: impl Into<String>) -> Self {
        self.scene_summary = Some(summary.into());
        self
    }

    /// Register available agents.
    pub fn with_agents(mut self, agents: Vec<String>) -> Self {
        self.available_agents = agents;
        self
    }

    /// Register available tools.
    pub fn with_tools(mut self, tools: Vec<String>) -> Self {
        self.available_tools = tools;
        self
    }
}

/// Context passed to Executor agents (SceneAgent, CodeAgent, etc.) so they can
/// work within the constraints of the current task without needing to re-derive
/// the environment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamContext {
    /// The task the executor is working on.
    pub task_id: u64,

    /// If the task has a goal state, its ID is stored here.
    pub goal_state_id: Option<String>,

    /// Summary of the current scene state (entities and their components).
    pub scene_summary: Option<String>,

    /// Summary of the project structure (files, modules, dependencies).
    pub project_summary: Option<String>,

    /// Hard constraints the executor must obey (e.g. "do not modify existing entities").
    pub constraints: Vec<String>,

    /// Entity names that are relevant to this step (helps the executor focus).
    pub relevant_entity_names: Vec<String>,
}

impl TeamContext {
    /// Create a minimal team context for a task.
    pub fn new(task_id: u64) -> Self {
        Self {
            task_id,
            goal_state_id: None,
            scene_summary: None,
            project_summary: None,
            constraints: Vec::new(),
            relevant_entity_names: Vec::new(),
        }
    }

    /// Attach a scene summary.
    pub fn with_scene_summary(mut self, summary: impl Into<String>) -> Self {
        self.scene_summary = Some(summary.into());
        self
    }

    /// Attach a project summary.
    pub fn with_project_summary(mut self, summary: impl Into<String>) -> Self {
        self.project_summary = Some(summary.into());
        self
    }

    /// Add a constraint.
    pub fn add_constraint(mut self, constraint: impl Into<String>) -> Self {
        self.constraints.push(constraint.into());
        self
    }

    /// Add relevant entity names.
    pub fn add_relevant_entities(mut self, names: Vec<String>) -> Self {
        self.relevant_entity_names.extend(names);
        self
    }
}

/// A builder that assembles a "context pack" — a serialisable snapshot of
/// everything an agent needs to know before performing a step.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextPackBuilder {
    project_root: String,
    scene_summary: Option<String>,
    available_tools: Vec<String>,
    available_agents: Vec<String>,
    constraints: Vec<String>,
}

impl ContextPackBuilder {
    pub fn new(project_root: impl Into<String>) -> Self {
        Self {
            project_root: project_root.into(),
            scene_summary: None,
            available_tools: Vec::new(),
            available_agents: Vec::new(),
            constraints: Vec::new(),
        }
    }

    pub fn with_scene_summary(mut self, summary: impl Into<String>) -> Self {
        self.scene_summary = Some(summary.into());
        self
    }

    pub fn with_tools(mut self, tools: Vec<String>) -> Self {
        self.available_tools = tools;
        self
    }

    pub fn with_agents(mut self, agents: Vec<String>) -> Self {
        self.available_agents = agents;
        self
    }

    pub fn add_constraint(mut self, constraint: impl Into<String>) -> Self {
        self.constraints.push(constraint.into());
        self
    }

    pub fn build(self, task_id: u64) -> PlannerContext {
        PlannerContext {
            task_id,
            project_root: self.project_root,
            available_agents: self.available_agents,
            available_tools: self.available_tools,
            scene_summary: self.scene_summary,
        }
    }
}

impl Default for ContextPackBuilder {
    fn default() -> Self {
        Self::new("")
    }
}
