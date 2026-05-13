//! Memory Persistence Module - Phase 3.4
//!
//! Provides auto-save/load for PersistentMemory.
//! Note: PersistentMemory is not a Bevy Resource, so we use a wrapper approach.

use agent_core::memory::PersistentMemory;
use std::path::PathBuf;
use log::{info, warn, error};

/// Configuration for memory persistence
pub struct MemoryPersistenceConfig {
    /// Path to save/load memory file
    pub memory_file_path: PathBuf,
    /// Whether persistence is enabled
    pub enabled: bool,
}

impl Default for MemoryPersistenceConfig {
    fn default() -> Self {
        let mut path = std::env::temp_dir();
        path.push("AgentEdit");
        path.push("agent_memory.json");

        Self {
            memory_file_path: path,
            enabled: true,
        }
    }
}

/// Load persistent memory from disk
pub fn load_persistent_memory(config: &MemoryPersistenceConfig) -> PersistentMemory {
    if !config.enabled {
        info!("Memory persistence disabled, using defaults");
        return PersistentMemory::new();
    }

    // Ensure directory exists
    if let Some(parent) = config.memory_file_path.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            warn!("Failed to create memory directory: {}", e);
        }
    }

    match PersistentMemory::load(&config.memory_file_path) {
        Ok(loaded) => {
            info!("Loaded persistent memory from {:?}", config.memory_file_path);
            info!("  - {} learned patterns", loaded.learned_patterns.len());
            info!("  - {} entity knowledge entries", loaded.entity_knowledge.len());
            loaded
        }
        Err(e) => {
            warn!("Failed to load persistent memory: {}. Using defaults.", e);
            PersistentMemory::new()
        }
    }
}

/// Save persistent memory to disk
pub fn save_persistent_memory(memory: &PersistentMemory, config: &MemoryPersistenceConfig) {
    if !config.enabled {
        return;
    }

    match memory.save(&config.memory_file_path) {
        Ok(_) => {
            info!("Saved persistent memory to {:?}", config.memory_file_path);
        }
        Err(e) => {
            error!("Failed to save persistent memory: {}", e);
        }
    }
}

/// Get the default memory file path
pub fn default_memory_path() -> PathBuf {
    let mut path = std::env::temp_dir();
    path.push("AgentEdit");
    path.push("agent_memory.json");
    path
}
