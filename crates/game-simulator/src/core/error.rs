//! Error types for the game simulator

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Unique identifier for an entity
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EntityId(pub u64);

impl std::fmt::Display for EntityId {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Result type for simulator operations
pub type SimResult<T> = Result<T, SimError>;

/// Errors that can occur during simulation
#[derive(Error, Debug)]
pub enum SimError {
    #[error("Component not found: {0}")]
    ComponentNotFound(String),

    #[error("Entity not found: {0}")]
    EntityNotFound(EntityId),

    #[error("System error: {0}")]
    SystemError(String),

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Already has component: {0}")]
    ComponentAlreadyExists(String),

    #[error("Invalid operation: {0}")]
    InvalidOperation(String),
}
