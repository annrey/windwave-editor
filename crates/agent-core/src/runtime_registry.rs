//! Runtime Registry - Multi-engine support for Agent Core
//!
//! Inspired by Multica's runtime unification, this module provides a unified
//! interface for managing multiple game engines and runtime environments.
//!
//! Supported engines:
//! - Bevy (native)
//! - Unity (planned)
//! - Godot (planned)
//! - Custom engines

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

// ---------------------------------------------------------------------------
// Engine ID & Type
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RuntimeId(pub u64);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EngineType {
    Bevy,
    Unity,
    Godot,
    Unreal,
    Custom(String),
}

impl std::fmt::Display for EngineType {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            EngineType::Bevy => write!(f, "Bevy"),
            EngineType::Unity => write!(f, "Unity"),
            EngineType::Godot => write!(f, "Godot"),
            EngineType::Unreal => write!(f, "Unreal"),
            EngineType::Custom(name) => write!(f, "Custom({})", name),
        }
    }
}

// ---------------------------------------------------------------------------
// Runtime Capabilities
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RuntimeCapability {
    SceneEditing,
    CodeHotReload,
    AssetImport,
    PhysicsSimulation,
    Rendering,
    Networking,
    Audio,
    InputHandling,
    SaveLoadSystem,
    DebuggingTools,
}

// ---------------------------------------------------------------------------
// Runtime Info
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeInfo {
    pub id: RuntimeId,
    pub engine_type: EngineType,
    pub version: String,
    pub name: String,
    pub description: String,
    pub capabilities: HashSet<RuntimeCapability>,
    pub is_active: bool,
    pub connected: bool,
    pub last_heartbeat: Option<u64>,
}

// ---------------------------------------------------------------------------
// Runtime Configuration
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeConfig {
    pub name: String,
    pub engine_type: EngineType,
    pub version: String,
    pub capabilities: Vec<RuntimeCapability>,
    pub connection_url: Option<String>,
    pub local_path: Option<String>,
    pub auto_start: bool,
}

// ---------------------------------------------------------------------------
// Runtime Registry
// ---------------------------------------------------------------------------

pub struct RuntimeRegistry {
    runtimes: HashMap<RuntimeId, RuntimeInfo>,
    configs: HashMap<RuntimeId, RuntimeConfig>,
    id_counter: u64,
    active_runtime: Option<RuntimeId>,
    engine_index: HashMap<EngineType, Vec<RuntimeId>>,
    capability_index: HashMap<RuntimeCapability, Vec<RuntimeId>>,
}

impl Default for RuntimeRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl RuntimeRegistry {
    pub fn new() -> Self {
        Self {
            runtimes: HashMap::new(),
            configs: HashMap::new(),
            id_counter: 0,
            active_runtime: None,
            engine_index: HashMap::new(),
            capability_index: HashMap::new(),
        }
    }

    /// Register a new runtime
    pub fn register_runtime(&mut self, config: RuntimeConfig) -> RuntimeId {
        let id = RuntimeId(self.id_counter);
        self.id_counter += 1;

        let capabilities: HashSet<RuntimeCapability> =
            config.capabilities.clone().into_iter().collect();

        let engine_type = config.engine_type.clone();

        let info = RuntimeInfo {
            id,
            engine_type: engine_type.clone(),
            version: config.version.clone(),
            name: config.name.clone(),
            description: String::new(),
            capabilities,
            is_active: false,
            connected: false,
            last_heartbeat: None,
        };

        self.engine_index.entry(engine_type).or_default().push(id);

        for &cap in &config.capabilities {
            self.capability_index.entry(cap).or_default().push(id);
        }

        self.runtimes.insert(id, info);
        self.configs.insert(id, config);
        id
    }

    /// Get runtime info by ID
    pub fn get_runtime(&self, id: RuntimeId) -> Option<&RuntimeInfo> {
        self.runtimes.get(&id)
    }

    /// Get mutable runtime info
    pub fn get_runtime_mut(&mut self, id: RuntimeId) -> Option<&mut RuntimeInfo> {
        self.runtimes.get_mut(&id)
    }

    /// Get runtime config
    pub fn get_config(&self, id: RuntimeId) -> Option<&RuntimeConfig> {
        self.configs.get(&id)
    }

    /// List all registered runtimes
    pub fn list_runtimes(&self) -> Vec<&RuntimeInfo> {
        self.runtimes.values().collect()
    }

    /// Find runtimes by engine type
    pub fn find_by_engine(&self, engine_type: EngineType) -> Vec<&RuntimeInfo> {
        self.engine_index
            .get(&engine_type)
            .map(|ids| ids.iter().filter_map(|id| self.runtimes.get(id)).collect())
            .unwrap_or_default()
    }

    /// Find runtimes by capability
    pub fn find_by_capability(&self, cap: RuntimeCapability) -> Vec<&RuntimeInfo> {
        self.capability_index
            .get(&cap)
            .map(|ids| ids.iter().filter_map(|id| self.runtimes.get(id)).collect())
            .unwrap_or_default()
    }

    /// Set the active runtime
    pub fn set_active_runtime(&mut self, id: RuntimeId) -> Result<(), String> {
        if !self.runtimes.contains_key(&id) {
            return Err("Runtime not registered".to_string());
        }

        // Deactivate current
        if let Some(current) = self.active_runtime {
            if let Some(rt) = self.runtimes.get_mut(&current) {
                rt.is_active = false;
            }
        }

        // Activate new
        if let Some(rt) = self.runtimes.get_mut(&id) {
            rt.is_active = true;
        }
        self.active_runtime = Some(id);
        Ok(())
    }

    /// Get the active runtime
    pub fn get_active_runtime(&self) -> Option<&RuntimeInfo> {
        self.active_runtime.and_then(|id| self.runtimes.get(&id))
    }

    /// Mark a runtime as connected
    pub fn mark_connected(&mut self, id: RuntimeId) -> Result<(), String> {
        if let Some(rt) = self.runtimes.get_mut(&id) {
            rt.connected = true;
            rt.last_heartbeat = Some(Self::now_secs());
            Ok(())
        } else {
            Err("Runtime not found".to_string())
        }
    }

    /// Mark a runtime as disconnected
    pub fn mark_disconnected(&mut self, id: RuntimeId) -> Result<(), String> {
        if let Some(rt) = self.runtimes.get_mut(&id) {
            rt.connected = false;
            Ok(())
        } else {
            Err("Runtime not found".to_string())
        }
    }

    /// Check if a runtime has a specific capability
    pub fn has_capability(&self, id: RuntimeId, cap: RuntimeCapability) -> bool {
        self.runtimes
            .get(&id)
            .map(|rt| rt.capabilities.contains(&cap))
            .unwrap_or(false)
    }

    fn now_secs() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }
}

// ---------------------------------------------------------------------------
// Runtime Manager - Higher-level interface
// ---------------------------------------------------------------------------

pub struct RuntimeManager {
    registry: RuntimeRegistry,
}

impl Default for RuntimeManager {
    fn default() -> Self {
        Self::new()
    }
}

impl RuntimeManager {
    pub fn new() -> Self {
        let mut manager = Self {
            registry: RuntimeRegistry::new(),
        };

        // Register default Bevy runtime (built-in)
        let default_config = RuntimeConfig {
            name: "Bevy Native".to_string(),
            engine_type: EngineType::Bevy,
            version: "0.17".to_string(),
            capabilities: vec![
                RuntimeCapability::SceneEditing,
                RuntimeCapability::PhysicsSimulation,
                RuntimeCapability::Rendering,
                RuntimeCapability::InputHandling,
                RuntimeCapability::AssetImport,
            ],
            connection_url: None,
            local_path: None,
            auto_start: true,
        };

        let bevy_id = manager.registry.register_runtime(default_config);

        // Mark as active
        manager.registry.set_active_runtime(bevy_id).ok();

        manager
    }

    pub fn registry(&self) -> &RuntimeRegistry {
        &self.registry
    }

    pub fn registry_mut(&mut self) -> &mut RuntimeRegistry {
        &mut self.registry
    }

    /// Register a custom runtime
    pub fn register_custom_runtime(
        &mut self,
        name: &str,
        engine_type: EngineType,
        capabilities: Vec<RuntimeCapability>,
    ) -> RuntimeId {
        let config = RuntimeConfig {
            name: name.to_string(),
            engine_type,
            version: "1.0".to_string(),
            capabilities,
            connection_url: None,
            local_path: None,
            auto_start: false,
        };
        self.registry.register_runtime(config)
    }

    /// Select the best runtime for a set of capabilities
    pub fn select_runtime_for(&self, required_caps: &[RuntimeCapability]) -> Option<RuntimeId> {
        let candidates: Vec<_> = self
            .registry
            .list_runtimes()
            .into_iter()
            .filter(|rt| rt.connected || rt.engine_type == EngineType::Bevy) // Prefer Bevy as fallback
            .map(|rt| {
                let score = required_caps
                    .iter()
                    .filter(|&cap| rt.capabilities.contains(cap))
                    .count();
                (score, rt)
            })
            .filter(|(score, _)| *score > 0)
            .collect();

        candidates
            .iter()
            .max_by_key(|(score, _)| score)
            .map(|(_, rt)| rt.id)
    }

    /// Get the scene bridge for the active runtime
    pub fn get_scene_bridge(&self) -> Option<String> {
        let active = self.registry.get_active_runtime()?;
        Some(format!("SceneBridge::{}", active.engine_type))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_runtime_registration() {
        let mut registry = RuntimeRegistry::new();

        let config = RuntimeConfig {
            name: "Test Engine".to_string(),
            engine_type: EngineType::Bevy,
            version: "1.0".to_string(),
            capabilities: vec![RuntimeCapability::SceneEditing],
            connection_url: None,
            local_path: None,
            auto_start: true,
        };

        let id = registry.register_runtime(config);
        assert!(registry.get_runtime(id).is_some());
    }

    #[test]
    fn test_active_runtime() {
        let mut registry = RuntimeRegistry::new();

        let config = RuntimeConfig {
            name: "Test".to_string(),
            engine_type: EngineType::Bevy,
            version: "1.0".to_string(),
            capabilities: vec![],
            connection_url: None,
            local_path: None,
            auto_start: true,
        };

        let id = registry.register_runtime(config);
        registry.set_active_runtime(id).unwrap();

        let active = registry.get_active_runtime().unwrap();
        assert_eq!(active.id, id);
        assert_eq!(active.name, "Test");
    }

    #[test]
    fn test_runtime_manager() {
        let manager = RuntimeManager::new();

        // Should have default Bevy runtime
        let runtimes = manager.registry().list_runtimes();
        assert!(!runtimes.is_empty());

        // Check active runtime
        let active = manager.registry().get_active_runtime();
        assert!(active.is_some());
        assert_eq!(active.unwrap().engine_type, EngineType::Bevy);
    }

    #[test]
    fn test_capability_check() {
        let mut registry = RuntimeRegistry::new();

        let config = RuntimeConfig {
            name: "Test".to_string(),
            engine_type: EngineType::Bevy,
            version: "1.0".to_string(),
            capabilities: vec![RuntimeCapability::SceneEditing],
            connection_url: None,
            local_path: None,
            auto_start: true,
        };

        let id = registry.register_runtime(config);
        assert!(registry.has_capability(id, RuntimeCapability::SceneEditing));
        assert!(!registry.has_capability(id, RuntimeCapability::Networking));
    }
}
