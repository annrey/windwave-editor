//! Scene Change Detection Module
//!
//! Tracks changes to the scene state and provides incremental updates.
//! Supports both polling-based and event-driven change detection.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Instant;

/// Represents a change to a single entity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityChange {
    pub entity_id: crate::EntityId,
    pub change_type: ChangeType,
    pub component_name: Option<String>,
    pub old_value: Option<serde_json::Value>,
    pub new_value: Option<serde_json::Value>,
    pub timestamp: f64,
}

/// Type of change
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ChangeType {
    Created,
    Modified,
    Deleted,
}

/// Scene change summary
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SceneChangeSummary {
    pub entities_created: Vec<crate::EntityId>,
    pub entities_modified: Vec<crate::EntityId>,
    pub entities_deleted: Vec<crate::EntityId>,
    pub component_changes: Vec<ComponentChangeSummary>,
    pub timestamp: f64,
    pub scene_hash_before: String,
    pub scene_hash_after: String,
}

/// Component-level change summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentChangeSummary {
    pub entity_id: crate::EntityId,
    pub component_type: String,
    pub properties_changed: Vec<String>,
}

/// Change detection strategy
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "config")]
pub enum ChangeDetectionStrategy {
    /// Poll scene state at intervals
    Polling { interval_ms: u64 },
    /// Listen for engine change events
    EventDriven,
    /// Hybrid: events with periodic polling backup
    Hybrid { interval_ms: u64 },
}

impl Default for ChangeDetectionStrategy {
    fn default() -> Self {
        ChangeDetectionStrategy::Hybrid { interval_ms: 100 }
    }
}

/// Scene change tracker
#[derive(Debug)]
pub struct SceneChangeTracker {
    last_scene_hash: String,
    last_check_time: Instant,
    strategy: ChangeDetectionStrategy,
    change_history: Vec<SceneChangeSummary>,
    max_history_size: usize,
}

impl SceneChangeTracker {
    pub fn new(strategy: ChangeDetectionStrategy) -> Self {
        Self {
            last_scene_hash: String::new(),
            last_check_time: Instant::now(),
            strategy,
            change_history: Vec::new(),
            max_history_size: 100,
        }
    }

    /// Check if we should poll for changes based on strategy
    pub fn should_check(&self) -> bool {
        match self.strategy {
            ChangeDetectionStrategy::Polling { interval_ms } |
            ChangeDetectionStrategy::Hybrid { interval_ms } => {
                self.last_check_time.elapsed().as_millis() as u64 >= interval_ms
            }
            ChangeDetectionStrategy::EventDriven => false,
        }
    }

    /// Update the last check time
    pub fn mark_checked(&mut self) {
        self.last_check_time = Instant::now();
    }

    /// Detect changes between old and new scene state
    pub fn detect_changes(
        &mut self,
        old_entities: &HashMap<crate::EntityId, crate::EntityInfo>,
        new_entities: &HashMap<crate::EntityId, crate::EntityInfo>,
    ) -> SceneChangeSummary {
        let mut summary = SceneChangeSummary::default();
        summary.timestamp = self.last_check_time.elapsed().as_secs_f64();

        // Find created entities
        for (id, info) in new_entities {
            if !old_entities.contains_key(id) {
                summary.entities_created.push(*id);
                let comp_names: Vec<_> = info.components.iter().map(|c| c.name.clone()).collect();
                summary.component_changes.push(ComponentChangeSummary {
                    entity_id: *id,
                    component_type: "Entity".to_string(),
                    properties_changed: comp_names,
                });
            }
        }

        // Find deleted entities
        for id in old_entities.keys() {
            if !new_entities.contains_key(id) {
                summary.entities_deleted.push(*id);
            }
        }

        // Find modified entities
        for (id, new_info) in new_entities {
            if let Some(old_info) = old_entities.get(id) {
                let mut properties_changed = Vec::new();

                let old_types: std::collections::HashSet<String> =
                    old_info.components.iter().map(|c| c.name.clone()).collect();
                let new_types: std::collections::HashSet<String> =
                    new_info.components.iter().map(|c| c.name.clone()).collect();

                // Compare components
                for new_comp in &new_info.components {
                    let old_comp = old_info.components.iter().find(|c| c.name == new_comp.name);
                    if let Some(old_comp) = old_comp {
                        // Check if component properties changed
                        for (prop_name, new_value) in &new_comp.properties {
                            let old_value = old_comp.properties.get(prop_name);
                            if old_value.map(|v| v == new_value) != Some(true) {
                                properties_changed.push(format!("{}.{}", new_comp.name, prop_name));
                            }
                        }
                    } else {
                        // New component added
                        properties_changed.push(format!("{} (new)", new_comp.name));
                    }
                }

                // Check for removed components
                for old_type in &old_types {
                    if !new_types.contains(old_type) {
                        properties_changed.push(format!("{} (removed)", old_type));
                    }
                }

                if !properties_changed.is_empty() {
                    summary.entities_modified.push(*id);
                    for prop in &properties_changed {
                        let parts: Vec<&str> = prop.split('.').collect();
                        if parts.len() >= 2 {
                            summary.component_changes.push(ComponentChangeSummary {
                                entity_id: *id,
                                component_type: parts[0].to_string(),
                                properties_changed: vec![parts[1..].join(".")],
                            });
                        }
                    }
                }
            }
        }

        // Store in history
        self.change_history.push(summary.clone());
        if self.change_history.len() > self.max_history_size {
            self.change_history.remove(0);
        }

        summary
    }

    /// Record a scene hash for next comparison
    pub fn record_hash(&mut self, hash: impl Into<String>) {
        self.last_scene_hash = hash.into();
    }

    /// Get the last recorded scene hash
    pub fn last_hash(&self) -> &str {
        &self.last_scene_hash
    }

    /// Get change history
    pub fn history(&self) -> &[SceneChangeSummary] {
        &self.change_history
    }

    /// Clear change history
    pub fn clear_history(&mut self) {
        self.change_history.clear();
    }

    /// Get the change detection strategy
    pub fn strategy(&self) -> ChangeDetectionStrategy {
        self.strategy
    }

    /// Set a new change detection strategy
    pub fn set_strategy(&mut self, strategy: ChangeDetectionStrategy) {
        self.strategy = strategy;
        self.last_check_time = Instant::now();
    }
}

impl Default for SceneChangeTracker {
    fn default() -> Self {
        Self::new(ChangeDetectionStrategy::default())
    }
}

/// Trait for types that can track scene changes
pub trait SceneChangeProvider {
    /// Get the current scene state hash
    fn scene_hash(&self) -> String;

    /// Get detailed change summary since last check
    fn get_changes(&mut self) -> Option<SceneChangeSummary>;

    /// Enable/disable change tracking
    fn set_tracking_enabled(&mut self, enabled: bool);

    /// Check if tracking is enabled
    fn is_tracking_enabled(&self) -> bool;
}

/// Configuration for change detection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeDetectionConfig {
    pub enabled: bool,
    pub strategy: ChangeDetectionStrategy,
    pub track_component_changes: bool,
    pub track_entity_lifecycle: bool,
    pub max_history_entries: usize,
}

impl Default for ChangeDetectionConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            strategy: ChangeDetectionStrategy::default(),
            track_component_changes: true,
            track_entity_lifecycle: true,
            max_history_entries: 100,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn create_test_entity(id: u64, components: Vec<(&str, &str)>) -> (crate::EntityId, crate::EntityInfo) {
        let entity_id = crate::EntityId(id);
        let mut comp_vec = Vec::new();
        
        for (name, value) in components {
            let mut props = HashMap::new();
            props.insert("value".to_string(), crate::PropertyValue::String(value.to_string()));
            comp_vec.push(
                crate::ComponentInfo {
                    name: name.to_string(),
                    properties: props,
                },
            );
        }
        
        let info = crate::EntityInfo {
            id: entity_id,
            name: format!("Entity_{}", id),
            entity_type: "test".to_string(),
            components: comp_vec,
            children: Vec::new(),
        };
        
        (entity_id, info)
    }

    #[test]
    fn test_detect_created_entity() {
        let mut tracker = SceneChangeTracker::new(ChangeDetectionStrategy::Polling { interval_ms: 100 });
        
        let old = HashMap::new();
        let mut new = HashMap::new();
        let (id, info) = create_test_entity(1, vec![("Transform", "pos:0,0,0")]);
        new.insert(id, info);
        
        let changes = tracker.detect_changes(&old, &new);
        
        assert_eq!(changes.entities_created.len(), 1);
        assert_eq!(changes.entities_deleted.len(), 0);
        assert_eq!(changes.entities_modified.len(), 0);
    }

    #[test]
    fn test_detect_deleted_entity() {
        let mut tracker = SceneChangeTracker::new(ChangeDetectionStrategy::Polling { interval_ms: 100 });
        
        let mut old = HashMap::new();
        let new = HashMap::new();
        let (id, info) = create_test_entity(1, vec![("Transform", "pos:0,0,0")]);
        old.insert(id, info);
        
        let changes = tracker.detect_changes(&old, &new);
        
        assert_eq!(changes.entities_created.len(), 0);
        assert_eq!(changes.entities_deleted.len(), 1);
        assert_eq!(changes.entities_modified.len(), 0);
    }
}
