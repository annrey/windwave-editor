//! Shared types for the game skill system

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum GameEngine {
    Bevy,
    Unity,
    Godot,
    Phaser,
    ThreeJs,
    Custom(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectSkeleton {
    pub name: String,
    pub engine: GameEngine,
    pub description: String,
    pub files: Vec<SkeletonFile>,
    pub dependencies: Vec<String>,
    pub success_count: u64,
    pub failure_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkeletonFile {
    pub path: String,
    pub template: String,
    pub is_entry_point: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifiedFix {
    pub id: String,
    pub error_pattern: String,
    pub error_signature: ErrorSignature,
    pub fix_strategy: FixStrategy,
    pub verification_method: VerificationMethod,
    pub success_count: u64,
    pub last_used: u64,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorSignature {
    pub error_type: String,
    pub stack_trace_pattern: Option<String>,
    pub console_output_pattern: Option<String>,
    pub component_hint: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FixStrategy {
    CodeReplace {
        file_pattern: String,
        search: String,
        replace: String,
    },
    ComponentAdd {
        entity_pattern: String,
        component_type: String,
        properties: HashMap<String, serde_json::Value>,
    },
    DependencyAdd {
        crate_name: String,
        version: String,
    },
    SystemReorder {
        system_name: String,
        before: Vec<String>,
        after: Vec<String>,
    },
    Custom {
        description: String,
        script: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VerificationMethod {
    CompileCheck,
    RuntimeTest { test_command: String },
    VisualInspection { criteria: String },
    ConsoleClean { max_warnings: u32 },
}

#[derive(Debug, Clone)]
pub struct FixResult {
    pub fix_id: String,
    pub success: bool,
    pub files_modified: u32,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct ScaffoldResult {
    pub template_name: String,
    pub files_created: Vec<PathBuf>,
    pub fixes_applied: Vec<FixResult>,
    pub success: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum SkillError {
    #[error("Template not found: {0}")]
    TemplateNotFound(String),
    #[error("No Cargo.toml found in project")]
    NoCargoToml,
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Walkdir error: {0}")]
    WalkDir(#[from] walkdir::Error),
}
