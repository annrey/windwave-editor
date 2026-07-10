//! ShadowGitService - Lightweight file-level snapshot system for safe file editing
//!
//! Provides per-file undo/redo with in-memory snapshots. Before any file write,
//! call `snapshot_before_write()` to save the current state. Use `undo_file()`
//! and `redo_file()` to navigate history.

use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct FileSnapshot {
    pub timestamp: u64,
    pub content: String,
    pub description: String,
}

#[derive(Debug, Clone)]
pub struct FileContent {
    pub content: String,
    pub description: String,
}

pub struct ShadowGitService {
    shadow_dir: PathBuf,
    snapshots: HashMap<String, Vec<FileSnapshot>>,
    redo_stack: HashMap<String, Vec<FileContent>>,
}

fn current_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

impl ShadowGitService {
    pub fn new(project_root: PathBuf) -> Self {
        let shadow_dir = project_root.join(".shadow");
        let _ = std::fs::create_dir_all(&shadow_dir);
        Self {
            shadow_dir,
            snapshots: HashMap::new(),
            redo_stack: HashMap::new(),
        }
    }

    pub fn snapshot_before_write(&mut self, relative_path: &str, description: &str) {
        let full_path = self.shadow_dir.join(relative_path);
        let content = if full_path.exists() {
            std::fs::read_to_string(&full_path).unwrap_or_default()
        } else {
            String::new()
        };

        let snapshot = FileSnapshot {
            timestamp: current_timestamp(),
            content,
            description: description.to_string(),
        };

        self.snapshots
            .entry(relative_path.to_string())
            .or_default()
            .push(snapshot);

        self.redo_stack.remove(relative_path);
    }

    pub fn undo_file(&mut self, relative_path: &str) -> Result<String, String> {
        let snapshots = self
            .snapshots
            .get_mut(relative_path)
            .ok_or_else(|| format!("No snapshots for '{}'", relative_path))?;

        let snapshot = snapshots
            .pop()
            .ok_or_else(|| format!("No undo history for '{}'", relative_path))?;

        let full_path = self.shadow_dir.join(relative_path);
        let current_content = if full_path.exists() {
            std::fs::read_to_string(&full_path).unwrap_or_default()
        } else {
            String::new()
        };

        let redo_entry = FileContent {
            content: current_content,
            description: snapshot.description.clone(),
        };
        self.redo_stack
            .entry(relative_path.to_string())
            .or_default()
            .push(redo_entry);

        if let Some(parent) = full_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        std::fs::write(&full_path, &snapshot.content)
            .map_err(|e| format!("Failed to write '{}': {}", relative_path, e))?;

        Ok(snapshot.description)
    }

    pub fn redo_file(&mut self, relative_path: &str) -> Result<String, String> {
        let redo_entries = self
            .redo_stack
            .get_mut(relative_path)
            .ok_or_else(|| format!("No redo history for '{}'", relative_path))?;

        let redo_entry = redo_entries
            .pop()
            .ok_or_else(|| format!("No redo entries for '{}'", relative_path))?;

        let full_path = self.shadow_dir.join(relative_path);
        let current_content = if full_path.exists() {
            std::fs::read_to_string(&full_path).unwrap_or_default()
        } else {
            String::new()
        };

        let snapshot = FileSnapshot {
            timestamp: current_timestamp(),
            content: current_content,
            description: redo_entry.description.clone(),
        };
        self.snapshots
            .entry(relative_path.to_string())
            .or_default()
            .push(snapshot);

        if let Some(parent) = full_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        std::fs::write(&full_path, &redo_entry.content)
            .map_err(|e| format!("Failed to write '{}': {}", relative_path, e))?;

        Ok(redo_entry.description)
    }

    pub fn can_undo(&self, relative_path: &str) -> bool {
        self.snapshots
            .get(relative_path)
            .map(|v| !v.is_empty())
            .unwrap_or(false)
    }

    pub fn can_redo(&self, relative_path: &str) -> bool {
        self.redo_stack
            .get(relative_path)
            .map(|v| !v.is_empty())
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_snapshot_undo_redo_roundtrip() {
        let td = std::env::current_dir().unwrap().join("sg_test");
        let _ = std::fs::create_dir_all(&td);

        let mut svc = ShadowGitService::new(td.clone());
        let fpath = "test_file.txt";

        // Write initial content
        let full = td.join(".shadow").join(fpath);
        std::fs::create_dir_all(full.parent().unwrap()).unwrap();
        std::fs::write(&full, "version 1").unwrap();

        // Snapshot v1
        svc.snapshot_before_write(fpath, "Initial state");
        assert!(svc.can_undo(fpath));
        assert!(!svc.can_redo(fpath));

        // Write v2
        std::fs::write(&full, "version 2").unwrap();

        // Undo → should restore v1
        let desc = svc.undo_file(fpath).unwrap();
        assert_eq!(desc, "Initial state");
        let content = std::fs::read_to_string(&full).unwrap();
        assert_eq!(content, "version 1");
        assert!(!svc.can_undo(fpath));
        assert!(svc.can_redo(fpath));

        // Redo → should restore v2
        let desc = svc.redo_file(fpath).unwrap();
        assert_eq!(desc, "Initial state");
        let content = std::fs::read_to_string(&full).unwrap();
        assert_eq!(content, "version 2");
        assert!(svc.can_undo(fpath));
        assert!(!svc.can_redo(fpath));

        let _ = std::fs::remove_dir_all(&td);
    }

    #[test]
    fn test_can_undo_can_redo_empty() {
        let td = std::env::current_dir().unwrap().join("sg_test2");
        let _ = std::fs::create_dir_all(&td);
        let svc = ShadowGitService::new(td.clone());

        assert!(!svc.can_undo("nonexistent.txt"));
        assert!(!svc.can_redo("nonexistent.txt"));

        let _ = std::fs::remove_dir_all(&td);
    }

    #[test]
    fn test_undo_without_snapshot_fails() {
        let td = std::env::current_dir().unwrap().join("sg_test3");
        let _ = std::fs::create_dir_all(&td);
        let mut svc = ShadowGitService::new(td.clone());

        assert!(svc.undo_file("nonexistent.txt").is_err());
        let _ = std::fs::remove_dir_all(&td);
    }
}
