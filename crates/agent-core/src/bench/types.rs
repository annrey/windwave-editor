//! Bench types — Score types, evaluator trait, and error types

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchScore {
    pub overall: f32,
    pub build_health: BuildHealthScore,
    pub visual_usability: VisualUsabilityScore,
    pub intent_alignment: IntentAlignmentScore,
    pub timestamp: u64,
    pub scene_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildHealthScore {
    pub score: f32,
    pub compilation_success: bool,
    pub warning_count: u32,
    pub error_count: u32,
    pub dependency_errors: Vec<String>,
    pub build_time_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisualUsabilityScore {
    pub score: f32,
    pub entities_rendered: u32,
    pub entities_expected: u32,
    pub camera_position_valid: bool,
    pub lighting_present: bool,
    pub ui_elements_visible: bool,
    pub frame_rate_stable: bool,
    pub screenshots: Vec<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntentAlignmentScore {
    pub score: f32,
    pub request_text: String,
    pub detected_features: Vec<String>,
    pub missing_features: Vec<String>,
    pub extra_features: Vec<String>,
    pub semantic_similarity: f32,
}

pub trait Evaluator: Send + Sync {
    fn name(&self) -> &str;
    fn evaluate(
        &self,
        project_path: &std::path::Path,
        request: &str,
    ) -> Result<BenchScore, BenchError>;
}

#[derive(Debug, thiserror::Error)]
pub enum BenchError {
    #[error("Build failed: {0}")]
    BuildFailed(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Walkdir error: {0}")]
    WalkDir(#[from] walkdir::Error),
    #[error("LLM error: {0}")]
    Llm(String),
}
