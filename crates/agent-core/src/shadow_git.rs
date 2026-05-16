//! ShadowGit - In-memory file snapshot system for editor
//!
//! Provides lightweight file-level snapshots that don't touch the real git repository.
//! Unlike the EditHistory (which tracks entity operations), ShadowGit tracks actual file content.
//!
//! # Features
//! - Create snapshots of file contents
//! - Navigate history with undo/redo
//! - Branch support (in-memory branches)
//! - Compare snapshots

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileSnapshot {
    pub path: PathBuf,
    pub content: String,
    pub timestamp: u64,
    pub hash: String,
}

impl FileSnapshot {
    pub fn new(path: PathBuf, content: String) -> Self {
        let hash = compute_hash(&content);
        Self {
            path,
            content,
            timestamp: current_timestamp(),
            hash,
        }
    }

    pub fn is_modified(&self, current: &str) -> bool {
        self.hash != compute_hash(current)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    pub id: String,
    pub message: String,
    pub files: Vec<FileSnapshot>,
    pub parent_id: Option<String>,
    pub timestamp: u64,
    pub branch: String,
}

impl Snapshot {
    pub fn new(id: String, message: String, files: Vec<FileSnapshot>, parent_id: Option<String>, branch: String) -> Self {
        Self {
            id,
            message,
            files,
            parent_id,
            timestamp: current_timestamp(),
            branch,
        }
    }

    pub fn file_content(&self, path: &PathBuf) -> Option<&str> {
        self.files.iter().find(|f| &f.path == path).map(|f| f.content.as_str())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Branch {
    pub name: String,
    pub head_snapshot_id: Option<String>,
    pub created_at: u64,
}

impl Branch {
    pub fn new(name: String) -> Self {
        Self {
            name,
            head_snapshot_id: None,
            created_at: current_timestamp(),
        }
    }
}

pub struct ShadowGit {
    snapshots: Vec<Snapshot>,
    current_branch: String,
    branches: HashMap<String, Branch>,
    undo_stack: Vec<String>,
    redo_stack: Vec<String>,
    staging: HashMap<PathBuf, String>,
    max_snapshots: usize,
}

impl ShadowGit {
    pub fn new() -> Self {
        let mut branches = HashMap::new();
        branches.insert("main".to_string(), Branch::new("main".to_string()));
        
        Self {
            snapshots: Vec::new(),
            current_branch: "main".to_string(),
            branches,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            staging: HashMap::new(),
            max_snapshots: 100,
        }
    }

    pub fn add_to_staging(&mut self, path: PathBuf, content: String) {
        self.staging.insert(path, content);
    }

    pub fn remove_from_staging(&mut self, path: &PathBuf) -> Option<String> {
        self.staging.remove(path)
    }

    pub fn staged_files(&self) -> Vec<&PathBuf> {
        self.staging.keys().collect()
    }

    pub fn create_snapshot(&mut self, message: &str) -> Option<String> {
        if self.staging.is_empty() {
            return None;
        }

        let files: Vec<FileSnapshot> = self.staging.drain()
            .map(|(path, content)| FileSnapshot::new(path, content))
            .collect();

        let id = format!("{:x}", current_timestamp());
        let parent_id = self.current_head_id();

        let snapshot = Snapshot::new(
            id.clone(),
            message.to_string(),
            files,
            parent_id,
            self.current_branch.clone(),
        );

        self.snapshots.push(snapshot.clone());
        self.undo_stack.push(id.clone());
        self.redo_stack.clear();

        if let Some(branch) = self.branches.get_mut(&self.current_branch) {
            branch.head_snapshot_id = Some(id.clone());
        }

        self.trim_snapshots();

        Some(id)
    }

    pub fn get_snapshot(&self, id: &str) -> Option<&Snapshot> {
        self.snapshots.iter().find(|s| s.id == id)
    }

    pub fn get_current_head(&self) -> Option<&Snapshot> {
        self.current_head_id().and_then(|id| self.get_snapshot(&id))
    }

    pub fn undo(&mut self) -> Option<String> {
        if let Some(snapshot_id) = self.undo_stack.pop() {
            self.redo_stack.push(snapshot_id.clone());
            if let Some(branch) = self.branches.get_mut(&self.current_branch) {
                if let Some(prev_id) = self.undo_stack.last() {
                    branch.head_snapshot_id = Some(prev_id.clone());
                } else {
                    branch.head_snapshot_id = None;
                }
            }
            Some(snapshot_id)
        } else {
            None
        }
    }

    pub fn redo(&mut self) -> Option<String> {
        if let Some(snapshot_id) = self.redo_stack.pop() {
            self.undo_stack.push(snapshot_id.clone());
            if let Some(branch) = self.branches.get_mut(&self.current_branch) {
                branch.head_snapshot_id = Some(snapshot_id.clone());
            }
            Some(snapshot_id)
        } else {
            None
        }
    }

    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    pub fn create_branch(&mut self, name: &str) -> Option<String> {
        if self.branches.contains_key(name) {
            return None;
        }

        let parent_id = self.current_head_id();
        let mut branch = Branch::new(name.to_string());
        branch.head_snapshot_id = parent_id;

        let id = format!("branch-{}", name);
        self.branches.insert(name.to_string(), branch);

        Some(id)
    }

    pub fn checkout_branch(&mut self, name: &str) -> Result<(), ShadowGitError> {
        if !self.branches.contains_key(name) {
            return Err(ShadowGitError::BranchNotFound(name.to_string()));
        }

        self.current_branch = name.to_string();
        
        if let Some(branch) = self.branches.get(name) {
            self.undo_stack.clear();
            if let Some(head_id) = &branch.head_snapshot_id {
                self.undo_stack.push(head_id.clone());
            }
            self.redo_stack.clear();
        }

        Ok(())
    }

    pub fn current_branch_name(&self) -> &str {
        &self.current_branch
    }

    pub fn list_branches(&self) -> Vec<&str> {
        self.branches.keys().map(|s| s.as_str()).collect()
    }

    pub fn snapshot_count(&self) -> usize {
        self.snapshots.len()
    }

    pub fn history(&self, limit: usize) -> Vec<&Snapshot> {
        self.snapshots.iter().rev().take(limit).collect()
    }

    pub fn find_file_at_snapshot(&self, path: &PathBuf, snapshot_id: &str) -> Option<String> {
        self.get_snapshot(snapshot_id)
            .and_then(|s| s.file_content(path))
            .map(|s| s.to_string())
    }

    pub fn diff_snapshot_with_current(&self, snapshot_id: &str) -> Vec<DiffEntry> {
        let mut diffs = Vec::new();

        if let Some(snapshot) = self.get_snapshot(snapshot_id) {
            for file in &snapshot.files {
                diffs.push(DiffEntry {
                    path: file.path.clone(),
                    old_content: Some(file.content.clone()),
                    new_content: self.staging.get(&file.path).cloned(),
                    status: if self.staging.contains_key(&file.path) {
                        DiffStatus::Modified
                    } else {
                        DiffStatus::Unchanged
                    },
                });
            }
        }

        for (path, content) in &self.staging {
            if !diffs.iter().any(|d| &d.path == path) {
                diffs.push(DiffEntry {
                    path: path.clone(),
                    old_content: None,
                    new_content: Some(content.clone()),
                    status: DiffStatus::Added,
                });
            }
        }

        diffs
    }

    fn current_head_id(&self) -> Option<String> {
        self.branches.get(&self.current_branch)
            .and_then(|b| b.head_snapshot_id.clone())
    }

    fn trim_snapshots(&mut self) {
        if self.snapshots.len() > self.max_snapshots {
            let to_remove = self.snapshots.len() - self.max_snapshots;
            self.snapshots.drain(0..to_remove);
        }
    }
}

impl Default for ShadowGit {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffEntry {
    pub path: PathBuf,
    pub old_content: Option<String>,
    pub new_content: Option<String>,
    pub status: DiffStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiffStatus {
    Added,
    Modified,
    Deleted,
    Unchanged,
}

#[derive(Debug, Clone)]
pub enum ShadowGitError {
    BranchNotFound(String),
    SnapshotNotFound(String),
    FileNotFound(PathBuf),
}

impl std::fmt::Display for ShadowGitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ShadowGitError::BranchNotFound(name) => write!(f, "Branch not found: {}", name),
            ShadowGitError::SnapshotNotFound(id) => write!(f, "Snapshot not found: {}", id),
            ShadowGitError::FileNotFound(path) => write!(f, "File not found: {:?}", path),
        }
    }
}

impl std::error::Error for ShadowGitError {}

fn compute_hash(content: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    content.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn current_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_snapshot() {
        let mut sg = ShadowGit::new();
        sg.add_to_staging(PathBuf::from("test.txt"), "hello".to_string());
        
        let id = sg.create_snapshot("Initial commit");
        assert!(id.is_some());
        assert_eq!(sg.snapshot_count(), 1);
    }

    #[test]
    fn test_undo_redo() {
        let mut sg = ShadowGit::new();
        sg.add_to_staging(PathBuf::from("test.txt"), "content".to_string());
        sg.create_snapshot("First");
        
        assert!(sg.can_undo());
        assert!(!sg.can_redo());
        
        sg.undo();
        assert!(!sg.can_undo());
        assert!(sg.can_redo());
        
        sg.redo();
        assert!(sg.can_undo());
        assert!(!sg.can_redo());
    }

    #[test]
    fn test_branch_operations() {
        let mut sg = ShadowGit::new();
        sg.add_to_staging(PathBuf::from("test.txt"), "content".to_string());
        sg.create_snapshot("Main commit");
        
        sg.create_branch("feature").unwrap();
        sg.checkout_branch("feature").unwrap();
        
        assert_eq!(sg.current_branch_name(), "feature");
        assert_eq!(sg.list_branches().len(), 2);
    }

    #[test]
    fn test_diff() {
        let mut sg = ShadowGit::new();
        sg.add_to_staging(PathBuf::from("test.txt"), "original".to_string());
        let snapshot_id = sg.create_snapshot("Initial").unwrap();
        
        sg.add_to_staging(PathBuf::from("test.txt"), "modified".to_string());
        
        let diffs = sg.diff_snapshot_with_current(&snapshot_id);
        assert_eq!(diffs.len(), 1);
        assert_eq!(diffs[0].status, DiffStatus::Modified);
    }

    #[test]
    fn test_diff_new_file() {
        let mut sg = ShadowGit::new();
        sg.add_to_staging(PathBuf::from("existing.txt"), "content".to_string());
        let snapshot_id = sg.create_snapshot("Initial").unwrap();
        
        sg.add_to_staging(PathBuf::from("new.txt"), "new content".to_string());
        
        let diffs = sg.diff_snapshot_with_current(&snapshot_id);
        let new_file = diffs.iter().find(|d| d.path.to_str() == Some("new.txt")).unwrap();
        assert_eq!(new_file.status, DiffStatus::Added);
    }
}
