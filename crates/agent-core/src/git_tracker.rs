//! Git Tracker — File-level version control for AgentEdit projects
//!
//! Inspired by OpenGame's file-system approach, this module provides
//! Git-backed versioning for project files, enabling:
//! - Automatic commits on agent actions
//! - Rollback to any previous state
//! - Branch-per-experiment workflow
//! - Diff visualization for agent changes

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;

// ---------------------------------------------------------------------------
// Git configuration
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitConfig {
    /// Whether auto-commit is enabled
    pub auto_commit: bool,
    /// Commit message template
    pub commit_template: String,
    /// Whether to sign commits
    pub sign_commits: bool,
    /// Default branch name
    pub default_branch: String,
    /// Remote URL (optional)
    pub remote_url: Option<String>,
    /// Author name for commits
    pub author_name: String,
    /// Author email for commits
    pub author_email: String,
}

impl Default for GitConfig {
    fn default() -> Self {
        Self {
            auto_commit: true,
            commit_template: "[AgentEdit] {action}: {description}".to_string(),
            sign_commits: false,
            default_branch: "main".to_string(),
            remote_url: None,
            author_name: "AgentEdit".to_string(),
            author_email: "agent@agentedit.local".to_string(),
        }
    }
}

// ---------------------------------------------------------------------------
// Commit metadata
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitInfo {
    pub hash: String,
    pub message: String,
    pub author: String,
    pub timestamp: u64,
    pub files_changed: Vec<String>,
    pub action_type: AgentActionType,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AgentActionType {
    CreateEntity,
    ModifyEntity,
    DeleteEntity,
    CreateScene,
    ModifyScene,
    ImportAsset,
    GenerateCode,
    BatchOperation,
    ManualEdit,
    System,
}

// ---------------------------------------------------------------------------
// GitTracker — main API
// ---------------------------------------------------------------------------

pub struct GitTracker {
    project_path: PathBuf,
    config: GitConfig,
}

impl GitTracker {
    /// Initialize GitTracker for a project directory
    pub fn new(project_path: impl AsRef<Path>) -> Result<Self, GitError> {
        let path = project_path.as_ref().to_path_buf();
        
        // Ensure git repo exists
        if !path.join(".git").exists() {
            Self::init_repo(&path)?;
        }
        
        Ok(Self {
            project_path: path,
            config: GitConfig::default(),
        })
    }
    
    /// Initialize a new git repository
    fn init_repo(path: &Path) -> Result<(), GitError> {
        Self::git_cmd(path, &["init"])?;
        Self::git_cmd(path, &["config", "user.name", "AgentEdit"])?;
        Self::git_cmd(path, &["config", "user.email", "agent@agentedit.local"])?;
        Ok(())
    }
    
    /// Configure the tracker
    pub fn with_config(mut self, config: GitConfig) -> Self {
        self.config = config;
        self
    }
    
    // ------------------------------------------------------------------
    // Core operations
    // ------------------------------------------------------------------
    
    /// Stage files and commit with agent action metadata
    pub fn commit_action(
        &self,
        action: AgentActionType,
        description: &str,
        files: &[impl AsRef<Path>],
    ) -> Result<CommitInfo, GitError> {
        if !self.config.auto_commit {
            return Err(GitError::AutoCommitDisabled);
        }
        
        // Stage files
        for file in files {
            let relative = self.relative_path(file.as_ref())?;
            self.git(&["add", &relative])?;
        }
        
        // Build commit message
        let message = self.config.commit_template
            .replace("{action}", &format!("{:?}", action))
            .replace("{description}", description);
        
        // Commit
        self.git(&["commit", "-m", &message])?;
        
        // Get commit info
        let hash = self.get_last_commit_hash()?;
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        
        Ok(CommitInfo {
            hash,
            message,
            author: self.config.author_name.clone(),
            timestamp,
            files_changed: files.iter()
                .map(|f| self.relative_path(f.as_ref()).unwrap_or_default())
                .collect(),
            action_type: action,
        })
    }
    
    /// Commit all changes in the project
    pub fn commit_all(&self, message: &str) -> Result<CommitInfo, GitError> {
        self.git(&["add", "."])?;
        self.git(&["commit", "-m", message])?;
        
        let hash = self.get_last_commit_hash()?;
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        
        Ok(CommitInfo {
            hash,
            message: message.to_string(),
            author: self.config.author_name.clone(),
            timestamp,
            files_changed: vec!["*".to_string()],
            action_type: AgentActionType::BatchOperation,
        })
    }
    
    /// Rollback to a specific commit
    pub fn rollback_to(&self, commit_hash: &str) -> Result<(), GitError> {
        // Create a backup branch before rollback
        let backup_branch = format!("backup-{}", self.get_last_commit_hash()?.get(..7).unwrap_or("unknown"));
        self.git(&["branch", &backup_branch])?;
        
        // Reset to target commit
        self.git(&["reset", "--hard", commit_hash])?;
        
        Ok(())
    }
    
    /// Undo last commit (keep changes)
    pub fn undo_last_commit(&self) -> Result<(), GitError> {
        self.git(&["reset", "--soft", "HEAD~1"])?;
        Ok(())
    }
    
    /// Get diff between two commits
    pub fn diff(&self, from: &str, to: &str) -> Result<String, GitError> {
        let output = self.git_output(&["diff", from, to])?;
        Ok(output)
    }
    
    /// Get diff of current working directory
    pub fn diff_working(&self) -> Result<String, GitError> {
        let output = self.git_output(&["diff"])?;
        Ok(output)
    }
    
    /// List recent commits
    pub fn log(&self, limit: usize) -> Result<Vec<CommitInfo>, GitError> {
        let format = "%H|%s|%an|%at";
        let output = self.git_output(&[
            "log",
            &format!("--format={}", format),
            &format!("-n{}", limit),
        ])?;
        
        let mut commits = Vec::new();
        for line in output.lines() {
            let parts: Vec<&str> = line.split('|').collect();
            if parts.len() >= 4 {
                commits.push(CommitInfo {
                    hash: parts[0].to_string(),
                    message: parts[1].to_string(),
                    author: parts[2].to_string(),
                    timestamp: parts[3].parse().unwrap_or(0),
                    files_changed: vec![],
                    action_type: AgentActionType::System,
                });
            }
        }
        
        Ok(commits)
    }
    
    /// Create a new branch for an experiment
    pub fn create_experiment_branch(&self, name: &str) -> Result<(), GitError> {
        let branch_name = format!("experiment/{}", name);
        self.git(&["checkout", "-b", &branch_name])?;
        Ok(())
    }
    
    /// Switch back to main branch
    pub fn checkout_main(&self) -> Result<(), GitError> {
        self.git(&["checkout", &self.config.default_branch])?;
        Ok(())
    }
    
    /// Get current branch name
    pub fn current_branch(&self) -> Result<String, GitError> {
        let output = self.git_output(&["branch", "--show-current"])?;
        Ok(output.trim().to_string())
    }
    
    /// Check if working directory is clean
    pub fn is_clean(&self) -> Result<bool, GitError> {
        let output = self.git_output(&["status", "--porcelain"])?;
        Ok(output.trim().is_empty())
    }
    
    // ------------------------------------------------------------------
    // Helpers
    // ------------------------------------------------------------------
    
    fn git(&self, args: &[&str]) -> Result<(), GitError> {
        Self::git_cmd(&self.project_path, args)
    }
    
    fn git_output(&self, args: &[&str]) -> Result<String, GitError> {
        Self::git_cmd_output(&self.project_path, args)
    }
    
    fn git_cmd(path: &Path, args: &[&str]) -> Result<(), GitError> {
        let output = Command::new("git")
            .current_dir(path)
            .args(args)
            .output()
            .map_err(|e| GitError::GitNotAvailable(e.to_string()))?;
        
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(GitError::GitCommandFailed(stderr.to_string()));
        }
        
        Ok(())
    }
    
    fn git_cmd_output(path: &Path, args: &[&str]) -> Result<String, GitError> {
        let output = Command::new("git")
            .current_dir(path)
            .args(args)
            .output()
            .map_err(|e| GitError::GitNotAvailable(e.to_string()))?;
        
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(GitError::GitCommandFailed(stderr.to_string()));
        }
        
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
    
    fn get_last_commit_hash(&self) -> Result<String, GitError> {
        let output = self.git_output(&["rev-parse", "HEAD"])?;
        Ok(output.trim().to_string())
    }
    
    fn relative_path(&self, path: &Path) -> Result<String, GitError> {
        path.strip_prefix(&self.project_path)
            .map(|p| p.to_string_lossy().to_string())
            .map_err(|_| GitError::PathNotInProject(path.to_string_lossy().to_string()))
    }
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[derive(Debug, thiserror::Error)]
pub enum GitError {
    #[error("Git not available: {0}")]
    GitNotAvailable(String),
    #[error("Git command failed: {0}")]
    GitCommandFailed(String),
    #[error("Path not in project: {0}")]
    PathNotInProject(String),
    #[error("Auto-commit is disabled")]
    AutoCommitDisabled,
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

// ---------------------------------------------------------------------------
// Integration with RollbackManager
// ---------------------------------------------------------------------------

/// Bridge between GitTracker and RollbackManager for unified undo/redo
pub struct GitRollbackBridge {
    tracker: GitTracker,
    commit_stack: Vec<String>,
}

impl GitRollbackBridge {
    pub fn new(tracker: GitTracker) -> Self {
        Self {
            tracker,
            commit_stack: Vec::new(),
        }
    }
    
    /// Record a checkpoint before operation
    pub fn checkpoint(&mut self, action: AgentActionType, description: &str) -> Result<String, GitError> {
        let info = self.tracker.commit_action(action, description, &[] as &[&Path])?;
        self.commit_stack.push(info.hash.clone());
        Ok(info.hash)
    }
    
    /// Undo to previous checkpoint
    pub fn undo(&mut self) -> Result<(), GitError> {
        if let Some(hash) = self.commit_stack.pop() {
            self.tracker.rollback_to(&hash)?;
        }
        Ok(())
    }
    
    /// Get commit history
    pub fn history(&self, limit: usize) -> Result<Vec<CommitInfo>, GitError> {
        self.tracker.log(limit)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_git_tracker_init() {
        let temp_dir = std::env::temp_dir().join("agentedit_git_test");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();
        
        let tracker = GitTracker::new(&temp_dir).unwrap();
        assert!(tracker.is_clean().unwrap());
        
        let _ = std::fs::remove_dir_all(&temp_dir);
    }
    
    #[test]
    fn test_commit_and_log() {
        let temp_dir = std::env::temp_dir().join("agentedit_git_commit_test");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();
        
        let tracker = GitTracker::new(&temp_dir).unwrap();
        
        // Create a test file
        std::fs::write(temp_dir.join("test.txt"), "hello").unwrap();
        
        let info = tracker.commit_action(
            AgentActionType::CreateEntity,
            "Created test entity",
            &[temp_dir.join("test.txt")],
        ).unwrap();
        
        assert_eq!(info.action_type, AgentActionType::CreateEntity);
        
        let log = tracker.log(10).unwrap();
        assert!(!log.is_empty());
        
        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
