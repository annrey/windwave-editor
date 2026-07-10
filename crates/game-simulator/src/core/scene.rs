//! Scene management - collections of entities

use crate::core::component::*;
use crate::core::error::*;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

/// An entity in the simulation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entity {
    pub id: EntityId,
    pub name: String,
    pub position: Option<Position>,
    pub velocity: Option<Velocity>,
    pub health: Option<Health>,
    pub tags: Vec<Tag>,
}

impl Entity {
    pub fn new(name: &str) -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT_ID: AtomicU64 = AtomicU64::new(1);
        let id = EntityId(NEXT_ID.fetch_add(1, Ordering::Relaxed));

        Self {
            id,
            name: name.to_string(),
            position: None,
            velocity: None,
            health: None,
            tags: Vec::new(),
        }
    }
}

/// A scene contains a collection of entities
#[derive(Clone)]
pub struct Scene {
    name: String,
    entities: Arc<RwLock<HashMap<EntityId, Entity>>>,
    entity_name_map: Arc<RwLock<HashMap<String, EntityId>>>,
}

impl Scene {
    /// Create a new empty scene
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            entities: Arc::new(RwLock::new(HashMap::new())),
            entity_name_map: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Get the scene name
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Spawn a new entity in the scene
    pub fn spawn_entity(&self, name: &str) -> EntityId {
        let entity = Entity::new(name);
        let id = entity.id;

        self.entities.write().insert(id, entity);
        self.entity_name_map.write().insert(name.to_string(), id);

        id
    }

    /// Get an entity by ID
    pub fn get_entity(&self, id: EntityId) -> Option<Entity> {
        self.entities.read().get(&id).cloned()
    }

    /// Get an entity by name
    pub fn get_entity_by_name(&self, name: &str) -> Option<Entity> {
        let id = self.entity_name_map.read().get(name).copied()?;
        self.get_entity(id)
    }

    /// Update an entity
    pub fn update_entity(&self, entity: Entity) {
        self.entities.write().insert(entity.id, entity);
    }

    /// Get all entities
    pub fn entities(&self) -> Vec<Entity> {
        self.entities.read().values().cloned().collect()
    }

    /// Get number of entities
    pub fn entity_count(&self) -> usize {
        self.entities.read().len()
    }
}
