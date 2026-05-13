//! Scene Serializer - Phase 4.1
//!
//! Enhanced scene save/load functionality with:
//! - JSON scene format
//! - Component serialization/deserialization
//! - Scene versioning
//! - Metadata support

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// Current scene format version
pub const SCENE_VERSION: u32 = 1;

/// Serializable scene file structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneFile {
    /// Format version for migration support
    pub version: u32,
    /// Scene name
    pub name: String,
    /// All entities in the scene
    pub entities: Vec<SceneEntityData>,
    /// Custom metadata
    pub metadata: HashMap<String, serde_json::Value>,
    /// Creation timestamp
    pub created_at: u64,
    /// Last modified timestamp
    pub modified_at: u64,
}

impl Default for SceneFile {
    fn default() -> Self {
        Self {
            version: SCENE_VERSION,
            name: "Untitled".to_string(),
            entities: Vec::new(),
            metadata: HashMap::new(),
            created_at: current_timestamp(),
            modified_at: current_timestamp(),
        }
    }
}

/// Serializable entity data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneEntityData {
    /// Unique entity ID (stable across saves)
    pub id: u64,
    /// Entity name
    pub name: String,
    /// All components attached to this entity
    pub components: Vec<SerializedComponent>,
    /// Child entities
    pub children: Vec<SceneEntityData>,
    /// Parent ID (if any)
    pub parent: Option<u64>,
}

/// Serialized component data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializedComponent {
    /// Component type name (fully qualified)
    pub type_name: String,
    /// Component data as JSON
    pub data: serde_json::Value,
}

impl SerializedComponent {
    /// Create a new serialized component
    pub fn new(type_name: &str, data: impl Serialize) -> Result<Self, serde_json::Error> {
        Ok(Self {
            type_name: type_name.to_string(),
            data: serde_json::to_value(data)?,
        })
    }
}

/// Scene save/load result
pub type SceneResult<T> = Result<T, SceneError>;

/// Scene serialization errors
#[derive(Debug, thiserror::Error)]
pub enum SceneError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON serialization error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Unsupported scene version: {0}")]
    UnsupportedVersion(u32),
    #[error("Invalid scene data: {0}")]
    InvalidData(String),
}

impl SceneFile {
    /// Create a new empty scene
    pub fn new(name: impl Into<String>) -> Self {
        let now = current_timestamp();
        Self {
            version: SCENE_VERSION,
            name: name.into(),
            entities: Vec::new(),
            metadata: HashMap::new(),
            created_at: now,
            modified_at: now,
        }
    }

    /// Add an entity to the scene
    pub fn add_entity(&mut self, entity: SceneEntityData) {
        self.entities.push(entity);
        self.modified_at = current_timestamp();
    }

    /// Set metadata value
    pub fn set_metadata(&mut self, key: impl Into<String>, value: impl Serialize) -> SceneResult<()> {
        self.metadata.insert(key.into(), serde_json::to_value(value)?);
        self.modified_at = current_timestamp();
        Ok(())
    }

    /// Get metadata value
    pub fn get_metadata(&self, key: &str) -> Option<&serde_json::Value> {
        self.metadata.get(key)
    }

    /// Save scene to file
    pub fn save(&self, path: impl AsRef<Path>) -> SceneResult<()> {
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)?;
        Ok(())
    }

    /// Load scene from file
    pub fn load(path: impl AsRef<Path>) -> SceneResult<Self> {
        let json = std::fs::read_to_string(path)?;
        let scene: Self = serde_json::from_str(&json)?;

        // Version check
        if scene.version > SCENE_VERSION {
            return Err(SceneError::UnsupportedVersion(scene.version));
        }

        Ok(scene)
    }

    /// Find entity by ID
    pub fn find_entity(&self, id: u64) -> Option<&SceneEntityData> {
        Self::find_entity_recursive(&self.entities, id)
    }

    fn find_entity_recursive(entities: &[SceneEntityData], id: u64) -> Option<&SceneEntityData> {
        for entity in entities {
            if entity.id == id {
                return Some(entity);
            }
            if let Some(found) = Self::find_entity_recursive(&entity.children, id) {
                return Some(found);
            }
        }
        None
    }

    /// Get total entity count (including children)
    pub fn total_entity_count(&self) -> usize {
        Self::count_entities_recursive(&self.entities)
    }

    fn count_entities_recursive(entities: &[SceneEntityData]) -> usize {
        entities.len()
            + entities
                .iter()
                .map(|e| Self::count_entities_recursive(&e.children))
                .sum::<usize>()
    }

    /// Validate scene integrity
    pub fn validate(&self) -> Result<(), SceneError> {
        // Check for duplicate IDs
        let mut ids = std::collections::HashSet::new();
        Self::collect_ids_recursive(&self.entities, &mut ids)?;

        // Check parent references
        Self::validate_parents(&self.entities, None)?;

        Ok(())
    }

    fn collect_ids_recursive(
        entities: &[SceneEntityData],
        ids: &mut std::collections::HashSet<u64>,
    ) -> Result<(), SceneError> {
        for entity in entities {
            if !ids.insert(entity.id) {
                return Err(SceneError::InvalidData(format!(
                    "Duplicate entity ID: {}",
                    entity.id
                )));
            }
            Self::collect_ids_recursive(&entity.children, ids)?;
        }
        Ok(())
    }

    fn validate_parents(
        entities: &[SceneEntityData],
        expected_parent: Option<u64>,
    ) -> Result<(), SceneError> {
        for entity in entities {
            if entity.parent != expected_parent {
                return Err(SceneError::InvalidData(format!(
                    "Entity {} has mismatched parent (expected {:?}, got {:?})",
                    entity.id, expected_parent, entity.parent
                )));
            }
            Self::validate_parents(&entity.children, Some(entity.id))?;
        }
        Ok(())
    }
}

impl SceneEntityData {
    /// Create a new entity data
    pub fn new(id: u64, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            components: Vec::new(),
            children: Vec::new(),
            parent: None,
        }
    }

    /// Add a component
    pub fn add_component(&mut self, component: SerializedComponent) {
        self.components.push(component);
    }

    /// Add a child entity
    pub fn add_child(&mut self, mut child: SceneEntityData) {
        child.parent = Some(self.id);
        self.children.push(child);
    }

    /// Find component by type
    pub fn find_component(&self, type_name: &str) -> Option<&SerializedComponent> {
        self.components.iter().find(|c| c.type_name == type_name)
    }

    /// Get component data as typed value
    pub fn get_component<T: for<'de> Deserialize<'de>>(
        &self,
        type_name: &str,
    ) -> Option<Result<T, serde_json::Error>> {
        self.find_component(type_name)
            .map(|c| serde_json::from_value(c.data.clone()))
    }
}

/// Utility function to get current timestamp
fn current_timestamp() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Scene file extension
pub const SCENE_FILE_EXTENSION: &str = ".scene";

/// Check if path has scene extension
pub fn is_scene_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("scene"))
        .unwrap_or(false)
}

/// Auto-detect scene version from JSON string
pub fn detect_scene_version(json: &str) -> Option<u32> {
    let value: serde_json::Value = serde_json::from_str(json).ok()?;
    value.get("version")?.as_u64().map(|v| v as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scene_save_load() {
        let mut scene = SceneFile::new("Test Scene");
        scene.set_metadata("author", "AgentEdit").unwrap();

        let entity = SceneEntityData::new(1, "Player");
        scene.add_entity(entity);

        let path = std::env::temp_dir().join("test_scene.json");
        scene.save(&path).unwrap();

        let loaded = SceneFile::load(&path).unwrap();
        assert_eq!(loaded.name, "Test Scene");
        assert_eq!(loaded.entities.len(), 1);
        assert_eq!(loaded.get_metadata("author").unwrap(), "AgentEdit");

        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn test_entity_hierarchy() {
        let mut parent = SceneEntityData::new(1, "Parent");
        let child = SceneEntityData::new(2, "Child");
        parent.add_child(child);

        assert_eq!(parent.children.len(), 1);
        assert_eq!(parent.children[0].parent, Some(1));
    }

    #[test]
    fn test_version_detection() {
        let json = r#"{"version": 1, "name": "Test"}"#;
        assert_eq!(detect_scene_version(json), Some(1));
    }
}
