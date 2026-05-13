//! Engine Adapter Trait
//!
//! Defines the abstract interface for game engine adapters.
//! This allows the agent system to work with different game engines
//! (Bevy, Unity, Godot, etc.) through a unified API.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Unique identifier for engine instances
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EngineId(pub u64);

impl Default for EngineId {
    fn default() -> Self {
        EngineId(0)
    }
}

/// Engine types supported by the system
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EngineType {
    Bevy,
    Unity,
    Godot,
    Unreal,
    Custom(String),
}

impl std::fmt::Display for EngineType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EngineType::Bevy => write!(f, "Bevy"),
            EngineType::Unity => write!(f, "Unity"),
            EngineType::Godot => write!(f, "Godot"),
            EngineType::Unreal => write!(f, "Unreal"),
            EngineType::Custom(name) => write!(f, "{}", name),
        }
    }
}

/// Engine capability flags
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EngineCapability {
    /// Support for entity creation/deletion
    EntityManagement,
    /// Support for component manipulation
    ComponentSystem,
    /// Support for scene hierarchy
    Hierarchy,
    /// Support for asset loading
    AssetSystem,
    /// Support for physics simulation
    Physics,
    /// Support for rendering
    Rendering,
    /// Support for screenshot capture
    Screenshot,
    /// Support for scripting
    Scripting,
    /// Support for undo/redo
    UndoRedo,
}

/// Status of an engine connection
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EngineStatus {
    /// Engine is not connected
    Disconnected,
    /// Engine is connecting
    Connecting,
    /// Engine is ready for commands
    Ready,
    /// Engine is busy processing
    Busy,
    /// Engine has encountered an error
    Error(EngineError),
}

/// Engine error types
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EngineError {
    /// Connection failed
    ConnectionFailed(String),
    /// Command execution failed
    CommandFailed(String),
    /// Entity not found
    EntityNotFound(u64),
    /// Component not found
    ComponentNotFound(String),
    /// Asset loading failed
    AssetLoadFailed(String),
    /// Not supported by this engine
    NotSupported(String),
    /// Generic error
    Other(String),
}

impl std::fmt::Display for EngineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EngineError::ConnectionFailed(msg) => write!(f, "Connection failed: {}", msg),
            EngineError::CommandFailed(msg) => write!(f, "Command failed: {}", msg),
            EngineError::EntityNotFound(id) => write!(f, "Entity not found: {}", id),
            EngineError::ComponentNotFound(name) => write!(f, "Component not found: {}", name),
            EngineError::AssetLoadFailed(path) => write!(f, "Asset load failed: {}", path),
            EngineError::NotSupported(feature) => write!(f, "Not supported: {}", feature),
            EngineError::Other(msg) => write!(f, "Error: {}", msg),
        }
    }
}

impl std::error::Error for EngineError {}

/// Result type for engine operations
pub type EngineResult<T> = Result<T, EngineError>;

/// Engine scene information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineSceneInfo {
    pub entity_count: usize,
    pub root_entities: Vec<u64>,
    pub scene_name: Option<String>,
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Entity information from engine
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineEntityInfo {
    pub id: u64,
    pub name: String,
    pub entity_type: String,
    pub components: Vec<EngineComponentInfo>,
    pub children: Vec<u64>,
}

/// Component information from engine
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineComponentInfo {
    pub name: String,
    pub properties: HashMap<String, serde_json::Value>,
}

/// Abstract trait for game engine adapters
///
/// This trait defines the common interface that all game engine adapters
/// must implement to be used by the agent system.
pub trait EngineAdapter: Send + Sync {
    /// Get the engine type
    fn engine_type(&self) -> EngineType;

    /// Get the engine ID
    fn engine_id(&self) -> EngineId;

    /// Get engine capabilities
    fn capabilities(&self) -> &[EngineCapability];

    /// Check if engine supports a specific capability
    fn supports(&self, capability: EngineCapability) -> bool {
        self.capabilities().contains(&capability)
    }

    /// Get current engine status
    fn status(&self) -> EngineStatus;

    /// Connect to the engine
    fn connect(&mut self) -> EngineResult<()>;

    /// Disconnect from the engine
    fn disconnect(&mut self) -> EngineResult<()>;

    /// Check if engine is connected and ready
    fn is_ready(&self) -> bool {
        matches!(self.status(), EngineStatus::Ready)
    }

    // ------------------------------------------------------------------
    // Entity Operations
    // ------------------------------------------------------------------

    /// Create a new entity
    fn create_entity(&mut self, name: &str, parent_id: Option<u64>) -> EngineResult<u64>;

    /// Delete an entity
    fn delete_entity(&mut self, entity_id: u64) -> EngineResult<()>;

    /// Get entity information
    fn get_entity(&self, entity_id: u64) -> EngineResult<EngineEntityInfo>;

    /// Find entities by name pattern
    fn find_entities(&self, name_pattern: &str) -> EngineResult<Vec<u64>>;

    /// Set entity parent (reparent)
    fn set_parent(&mut self, child_id: u64, parent_id: Option<u64>) -> EngineResult<()>;

    // ------------------------------------------------------------------
    // Component Operations
    // ------------------------------------------------------------------

    /// Add component to entity
    fn add_component(&mut self, entity_id: u64, component_type: &str) -> EngineResult<()>;

    /// Remove component from entity
    fn remove_component(&mut self, entity_id: u64, component_type: &str) -> EngineResult<()>;

    /// Get component value
    fn get_component(&self, entity_id: u64, component_type: &str) -> EngineResult<EngineComponentInfo>;

    /// Set component property
    fn set_component_property(
        &mut self,
        entity_id: u64,
        component_type: &str,
        property: &str,
        value: serde_json::Value,
    ) -> EngineResult<()>;

    // ------------------------------------------------------------------
    // Scene Operations
    // ------------------------------------------------------------------

    /// Get scene information
    fn get_scene_info(&self) -> EngineResult<EngineSceneInfo>;

    /// Build scene index for agent reasoning
    fn build_scene_index(&self) -> EngineResult<crate::EntityId>;

    /// Capture screenshot
    fn capture_screenshot(&self) -> EngineResult<Vec<u8>>;

    // ------------------------------------------------------------------
    // Asset Operations
    // ------------------------------------------------------------------

    /// Load asset
    fn load_asset(&mut self, path: &str, asset_type: &str) -> EngineResult<String>;

    /// Unload asset
    fn unload_asset(&mut self, handle: &str) -> EngineResult<()>;

    // ------------------------------------------------------------------
    // Command Operations
    // ------------------------------------------------------------------

    /// Execute a raw command
    fn execute_command(&mut self, command: &str) -> EngineResult<serde_json::Value>;

    /// Batch execute commands
    fn execute_commands(&mut self, commands: &[String]) -> Vec<EngineResult<serde_json::Value>> {
        commands.iter().map(|cmd| self.execute_command(cmd)).collect()
    }

    // ------------------------------------------------------------------
    // Transaction Support
    // ------------------------------------------------------------------

    /// Begin transaction
    fn begin_transaction(&mut self) -> EngineResult<()>;

    /// Commit transaction
    fn commit_transaction(&mut self) -> EngineResult<()>;

    /// Rollback transaction
    fn rollback_transaction(&mut self) -> EngineResult<()>;

    /// Check if in transaction
    fn in_transaction(&self) -> bool;
}

/// Factory for creating engine adapters
pub trait EngineAdapterFactory: Send + Sync {
    /// Create a new engine adapter instance
    fn create(&self, config: EngineConfig) -> Box<dyn EngineAdapter>;

    /// Get the engine type this factory creates
    fn engine_type(&self) -> EngineType;
}

/// Configuration for engine adapters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineConfig {
    pub engine_type: EngineType,
    pub host: String,
    pub port: u16,
    pub project_path: Option<String>,
    pub extra: HashMap<String, serde_json::Value>,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            engine_type: EngineType::Bevy,
            host: "localhost".to_string(),
            port: 8080,
            project_path: None,
            extra: HashMap::new(),
        }
    }
}

/// Registry for engine adapters
#[derive(Default)]
pub struct EngineAdapterRegistry {
    factories: HashMap<EngineType, Box<dyn EngineAdapterFactory>>,
    adapters: HashMap<EngineId, Box<dyn EngineAdapter>>,
    next_id: u64,
}

impl EngineAdapterRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register an engine adapter factory
    pub fn register_factory(&mut self, factory: Box<dyn EngineAdapterFactory>) {
        let engine_type = factory.engine_type();
        self.factories.insert(engine_type, factory);
    }

    /// Create and register a new engine adapter
    pub fn create_adapter(&mut self, config: EngineConfig) -> EngineResult<EngineId> {
        let engine_type = config.engine_type.clone();
        let factory = self
            .factories
            .get(&engine_type)
            .ok_or_else(|| EngineError::NotSupported(format!("Engine type: {:?}", engine_type)))?;

        let id = EngineId(self.next_id);
        self.next_id += 1;

        let adapter = factory.create(config);
        self.adapters.insert(id, adapter);

        Ok(id)
    }

    /// Get an engine adapter by ID
    pub fn get_adapter(&self, id: EngineId) -> Option<&dyn EngineAdapter> {
        self.adapters.get(&id).map(|a| a.as_ref())
    }

    /// Get a mutable engine adapter by ID
    pub fn get_adapter_mut(&mut self, id: EngineId) -> Option<&mut (dyn EngineAdapter + '_)> {
        match self.adapters.get_mut(&id) {
            Some(adapter) => Some(adapter.as_mut()),
            None => None,
        }
    }

    /// Remove an engine adapter
    pub fn remove_adapter(&mut self, id: EngineId) -> Option<Box<dyn EngineAdapter>> {
        self.adapters.remove(&id)
    }

    /// List all registered adapters
    pub fn list_adapters(&self) -> Vec<(EngineId, EngineType, EngineStatus)> {
        self.adapters
            .iter()
            .map(|(id, adapter)| (*id, adapter.engine_type(), adapter.status()))
            .collect()
    }

    /// Get count of registered adapters
    pub fn adapter_count(&self) -> usize {
        self.adapters.len()
    }
}

/// No-op engine adapter for testing or when no engine is available
pub struct NullEngineAdapter {
    id: EngineId,
    status: EngineStatus,
}

impl NullEngineAdapter {
    pub fn new() -> Self {
        Self {
            id: EngineId(0),
            status: EngineStatus::Disconnected,
        }
    }
}

impl Default for NullEngineAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl EngineAdapter for NullEngineAdapter {
    fn engine_type(&self) -> EngineType {
        EngineType::Custom("Null".to_string())
    }

    fn engine_id(&self) -> EngineId {
        self.id
    }

    fn capabilities(&self) -> &[EngineCapability] {
        &[]
    }

    fn status(&self) -> EngineStatus {
        self.status.clone()
    }

    fn connect(&mut self) -> EngineResult<()> {
        self.status = EngineStatus::Ready;
        Ok(())
    }

    fn disconnect(&mut self) -> EngineResult<()> {
        self.status = EngineStatus::Disconnected;
        Ok(())
    }

    fn create_entity(&mut self, _name: &str, _parent_id: Option<u64>) -> EngineResult<u64> {
        Err(EngineError::NotSupported("Null engine".to_string()))
    }

    fn delete_entity(&mut self, _entity_id: u64) -> EngineResult<()> {
        Err(EngineError::NotSupported("Null engine".to_string()))
    }

    fn get_entity(&self, _entity_id: u64) -> EngineResult<EngineEntityInfo> {
        Err(EngineError::NotSupported("Null engine".to_string()))
    }

    fn find_entities(&self, _name_pattern: &str) -> EngineResult<Vec<u64>> {
        Err(EngineError::NotSupported("Null engine".to_string()))
    }

    fn set_parent(&mut self, _child_id: u64, _parent_id: Option<u64>) -> EngineResult<()> {
        Err(EngineError::NotSupported("Null engine".to_string()))
    }

    fn add_component(&mut self, _entity_id: u64, _component_type: &str) -> EngineResult<()> {
        Err(EngineError::NotSupported("Null engine".to_string()))
    }

    fn remove_component(&mut self, _entity_id: u64, _component_type: &str) -> EngineResult<()> {
        Err(EngineError::NotSupported("Null engine".to_string()))
    }

    fn get_component(&self, _entity_id: u64, _component_type: &str) -> EngineResult<EngineComponentInfo> {
        Err(EngineError::NotSupported("Null engine".to_string()))
    }

    fn set_component_property(
        &mut self,
        _entity_id: u64,
        _component_type: &str,
        _property: &str,
        _value: serde_json::Value,
    ) -> EngineResult<()> {
        Err(EngineError::NotSupported("Null engine".to_string()))
    }

    fn get_scene_info(&self) -> EngineResult<EngineSceneInfo> {
        Err(EngineError::NotSupported("Null engine".to_string()))
    }

    fn build_scene_index(&self) -> EngineResult<crate::EntityId> {
        Err(EngineError::NotSupported("Null engine".to_string()))
    }

    fn capture_screenshot(&self) -> EngineResult<Vec<u8>> {
        Err(EngineError::NotSupported("Null engine".to_string()))
    }

    fn load_asset(&mut self, _path: &str, _asset_type: &str) -> EngineResult<String> {
        Err(EngineError::NotSupported("Null engine".to_string()))
    }

    fn unload_asset(&mut self, _handle: &str) -> EngineResult<()> {
        Err(EngineError::NotSupported("Null engine".to_string()))
    }

    fn execute_command(&mut self, _command: &str) -> EngineResult<serde_json::Value> {
        Err(EngineError::NotSupported("Null engine".to_string()))
    }

    fn begin_transaction(&mut self) -> EngineResult<()> {
        Err(EngineError::NotSupported("Null engine".to_string()))
    }

    fn commit_transaction(&mut self) -> EngineResult<()> {
        Err(EngineError::NotSupported("Null engine".to_string()))
    }

    fn rollback_transaction(&mut self) -> EngineResult<()> {
        Err(EngineError::NotSupported("Null engine".to_string()))
    }

    fn in_transaction(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_null_engine() {
        let mut engine = NullEngineAdapter::new();
        assert_eq!(engine.status(), EngineStatus::Disconnected);
        assert!(engine.connect().is_ok());
        assert_eq!(engine.status(), EngineStatus::Ready);
        assert!(engine.create_entity("test", None).is_err());
    }

    #[test]
    fn test_engine_registry() {
        let registry = EngineAdapterRegistry::new();
        assert_eq!(registry.adapter_count(), 0);
    }
}
