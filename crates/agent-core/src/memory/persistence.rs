//! Persistence operations for MemorySystem

use super::system::MemorySystem;
use super::types::{MemoryConfig, MemoryLoadResult, MemoryPersistenceInfo, MemoryStats};
use crate::memory::EpisodeType;
use serde::{Deserialize, Serialize};

// Private persistence data structure
#[derive(Debug, Clone, Serialize, Deserialize)]
struct MemoryPersistedData {
    working_entries: Vec<serde_json::Value>,
    episodes: Vec<crate::memory::Episode>,
    semantic_nodes: Vec<crate::memory::SemanticNode>,
    procedural_workflows: Vec<crate::memory::WorkflowTemplate>,
    config: MemoryConfig,
    saved_at: String,
    version: String,
}

impl MemorySystem {
    /// Save memory state to disk (JSON format)
    pub fn save_to_file(&self, path: &str) -> Result<MemoryPersistenceInfo, String> {
        let dir = std::path::Path::new(path)
            .parent()
            .unwrap_or(std::path::Path::new("."));

        std::fs::create_dir_all(dir)
            .map_err(|e| format!("Failed to create directory {:?}: {}", dir, e))?;

        let data = MemoryPersistedData {
            working_entries: self.working.get_entries_for_persistence(),
            episodes: self
                .episodic
                .get_all_episodes()
                .into_iter()
                .filter(|e| {
                    matches!(
                        e.episode_type,
                        EpisodeType::UserRequest
                            | EpisodeType::ToolCalled
                            | EpisodeType::ErrorOccurred
                            | EpisodeType::Summary
                            | EpisodeType::StepExecuted
                            | EpisodeType::UserPreference
                    )
                })
                .collect(),
            semantic_nodes: self.semantic.export_nodes(),
            procedural_workflows: self.procedural.export_workflows(),
            config: self.config.clone(),
            saved_at: chrono::Utc::now().to_rfc3339(),
            version: "1.0".to_string(),
        };

        let json = serde_json::to_string_pretty(&data)
            .map_err(|e| format!("Serialization failed: {}", e))?;

        let total_bytes = json.len();

        std::fs::write(path, &json).map_err(|e| format!("Failed to write to {}: {}", path, e))?;

        Ok(MemoryPersistenceInfo {
            file_path: path.to_string(),
            total_bytes,
            working_count: data.working_entries.len(),
            episodic_count: data.episodes.len(),
            semantic_count: data.semantic_nodes.len(),
            procedural_count: data.procedural_workflows.len(),
        })
    }

    /// Load memory state from disk
    pub fn load_from_file(&mut self, path: &str) -> Result<MemoryLoadResult, String> {
        if !std::path::Path::new(path).exists() {
            return Err(format!("File not found: {}", path));
        }

        let json =
            std::fs::read_to_string(path).map_err(|e| format!("Failed to read {}: {}", path, e))?;

        let data: MemoryPersistedData =
            serde_json::from_str(&json).map_err(|e| format!("Deserialization failed: {}", e))?;

        let working_count = data.working_entries.len();
        let episodic_count = data.episodes.len();
        let semantic_count = data.semantic_nodes.len();
        let procedural_count = data.procedural_workflows.len();

        for entry in data.working_entries {
            self.working.restore_entry(entry);
        }
        for episode in data.episodes {
            self.episodic.restore_episode(episode);
        }
        for node in data.semantic_nodes {
            self.semantic.import_node(node);
        }
        for workflow in data.procedural_workflows {
            self.procedural.import_workflow(workflow);
        }

        self.config = data.config;

        Ok(MemoryLoadResult {
            file_path: path.to_string(),
            working_restored: working_count,
            episodic_restored: episodic_count,
            semantic_restored: semantic_count,
            procedural_restored: procedural_count,
            saved_at: data.saved_at,
        })
    }

    /// Auto-save with backup rotation (keeps last N backups)
    pub fn auto_save(&self, base_path: &str, max_backups: usize) -> Result<Vec<String>, String> {
        let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
        let current_path = format!("{}_{}.json", base_path, timestamp);

        self.save_to_file(&current_path)?;

        let latest_path = format!("{}_latest.json", base_path);
        let _ = std::fs::remove_file(&latest_path);
        #[cfg(unix)]
        {
            let _ = std::os::unix::fs::symlink(
                std::path::Path::new(&current_path).file_name().unwrap(),
                &latest_path,
            );
        }

        Self::cleanup_old_backups(base_path, max_backups)?;
        Ok(vec![current_path])
    }

    fn cleanup_old_backups(base_path: &str, keep: usize) -> Result<(), String> {
        let pattern = format!("{}_*.json", base_path);

        let mut files: Vec<std::path::PathBuf> = glob::glob(&pattern)
            .map_err(|e| format!("Glob error: {}", e))?
            .filter_map(Result::ok)
            .collect();

        files.sort();
        files.reverse();

        for old_file in files.into_iter().skip(keep) {
            std::fs::remove_file(&old_file)
                .map_err(|e| format!("Failed to remove old backup {:?}: {}", old_file, e))?;
        }

        Ok(())
    }

    /// Get memory statistics
    pub fn stats(&self) -> MemoryStats {
        MemoryStats {
            working_entries: self.working.len(),
            working_capacity: self.config.working_capacity,
            episodic_entries: self.episodic.len(),
            semantic_nodes: self.semantic.node_count(),
            semantic_relations: self.semantic.relation_count(),
            procedural_workflows: self.procedural.workflow_count(),
            procedural_patterns: self.procedural.pattern_count(),
        }
    }
}
