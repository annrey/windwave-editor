//! Module types — the registry and capability descriptors for every subsystem
//! in the editor. Modules are the building blocks of the agent pipeline:
//! AgentDispatch, TaskManage, EventStore, GoalCheck, etc.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Well-known capabilities that a module can expose. These correspond to the
/// named subsystems in the editor architecture.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ModuleCapability {
    /// Dispatch work to the appropriate agent (Director's responsibility).
    AgentDispatch,

    /// Create, track, and manage tasks.
    TaskManage,

    /// Store and replay events (the EventBus).
    EventStore,

    /// Check goal requirements against engine state (Reviewer).
    GoalCheck,

    /// Execute a tool from the tool registry.
    SkillExecute,

    /// Index and query the current scene graph.
    SceneIndex,

    /// Index and query the project file tree.
    ProjectIndex,

    /// Analyse visual output (screenshot, layout).
    VisionAnalyze,

    /// Adapt editor commands to engine-specific operations (Bevy adapter).
    EngineAdapt,

    /// Render the editor UI.
    UiRender,

    /// Roll back a transaction to a previous state.
    Rollback,
}

/// A lightweight snapshot of a module's identity and health. Used by the
/// Director to discover which modules are available and whether they are
/// functioning correctly.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleSnapshot {
    /// Unique module identifier (e.g. `"scene_index"`).
    pub module_id: String,

    /// Human-readable display name (e.g. `"Scene Index"`).
    pub module_name: String,

    /// Current status string: `"ok"`, `"degraded"`, `"offline"`, etc.
    pub status: String,
}

impl ModuleSnapshot {
    pub fn new(
        module_id: impl Into<String>,
        module_name: impl Into<String>,
        status: impl Into<String>,
    ) -> Self {
        Self {
            module_id: module_id.into(),
            module_name: module_name.into(),
            status: status.into(),
        }
    }

    /// Returns `true` when the module is reporting a healthy status.
    pub fn is_healthy(&self) -> bool {
        self.status == "ok"
    }
}

/// Global configuration context shared by all modules (project root, etc.).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleContext {
    /// Absolute path to the project root directory.
    pub project_root: String,
}

impl ModuleContext {
    pub fn new(project_root: impl Into<String>) -> Self {
        Self {
            project_root: project_root.into(),
        }
    }
}

// ---------------------------------------------------------------------------
// ModuleRegistry — runtime catalogue of all available modules
// ---------------------------------------------------------------------------

/// A registry that maps module IDs to `ModuleSnapshot` instances. The Director
/// queries this registry to know which capabilities are available before
/// dispatching work.
#[derive(Debug, Clone)]
pub struct ModuleRegistry {
    /// module_id -> snapshot
    modules: HashMap<String, ModuleSnapshot>,
}

impl ModuleRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            modules: HashMap::new(),
        }
    }

    /// Register (or update) a module snapshot.
    ///
    /// If a module with the same `module_id` already exists, its snapshot is
    /// replaced.
    pub fn register(&mut self, snapshot: ModuleSnapshot) {
        self.modules.insert(snapshot.module_id.clone(), snapshot);
    }

    /// Get a module snapshot by ID.
    pub fn get(&self, module_id: &str) -> Option<&ModuleSnapshot> {
        self.modules.get(module_id)
    }

    /// Remove a module from the registry.
    pub fn unregister(&mut self, module_id: &str) -> Option<ModuleSnapshot> {
        self.modules.remove(module_id)
    }

    /// Return all registered module snapshots.
    pub fn list_all(&self) -> Vec<&ModuleSnapshot> {
        self.modules.values().collect()
    }

    /// Return only modules that are currently healthy.
    pub fn list_healthy(&self) -> Vec<&ModuleSnapshot> {
        self.modules.values().filter(|m| m.is_healthy()).collect()
    }

    /// Total number of registered modules.
    pub fn len(&self) -> usize {
        self.modules.len()
    }

    /// Returns `true` when no modules are registered.
    pub fn is_empty(&self) -> bool {
        self.modules.is_empty()
    }
}

impl Default for ModuleRegistry {
    fn default() -> Self {
        Self::new()
    }
}
