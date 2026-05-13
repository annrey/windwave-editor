//! OpenGame-Bench inspired evaluation system for AgentEdit
//!
//! Evaluates agent-generated game scenes along three dimensions:
//! - Build Health: Compilation success, warning count, dependency resolution
//! - Visual Usability: Scene renders correctly, entities visible, UI functional
//! - Intent Alignment: Generated scene matches user request

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;

// ---------------------------------------------------------------------------
// Score types
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Evaluator trait
// ---------------------------------------------------------------------------

pub trait Evaluator: Send + Sync {
    fn name(&self) -> &str;
    fn evaluate(&self, project_path: &Path, request: &str) -> Result<BenchScore, BenchError>;
}

// ---------------------------------------------------------------------------
// Build Health Evaluator
// ---------------------------------------------------------------------------

pub struct BuildHealthEvaluator;

impl BuildHealthEvaluator {
    pub fn new() -> Self {
        Self
    }
    
    fn evaluate_compilation(&self, project_path: &Path) -> Result<BuildHealthScore, BenchError> {
        let start = std::time::Instant::now();
        
        let output = Command::new("cargo")
            .current_dir(project_path)
            .args(&["check", "--message-format=short"])
            .output()
            .map_err(|e| BenchError::BuildFailed(e.to_string()))?;
        
        let build_time = start.elapsed().as_millis() as u64;
        let stderr = String::from_utf8_lossy(&output.stderr);
        let _stdout = String::from_utf8_lossy(&output.stdout);
        
        let error_count = stderr.matches("error:").count() as u32;
        let warning_count = stderr.matches("warning:").count() as u32;
        
        let dependency_errors: Vec<String> = stderr
            .lines()
            .filter(|l| l.contains("unresolved") || l.contains("could not find"))
            .map(|l| l.to_string())
            .collect();
        
        // Calculate score
        let mut score = 100.0f32;
        if !output.status.success() {
            score -= 50.0;
        }
        score -= error_count as f32 * 10.0;
        score -= warning_count as f32 * 2.0;
        score -= dependency_errors.len() as f32 * 15.0;
        score = score.max(0.0);
        
        Ok(BuildHealthScore {
            score,
            compilation_success: output.status.success(),
            warning_count,
            error_count,
            dependency_errors,
            build_time_ms: build_time,
        })
    }
}

impl Evaluator for BuildHealthEvaluator {
    fn name(&self) -> &str {
        "build_health"
    }
    
    fn evaluate(&self, project_path: &Path, _request: &str) -> Result<BenchScore, BenchError> {
        let build = self.evaluate_compilation(project_path)?;
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        
        Ok(BenchScore {
            overall: build.score,
            build_health: build,
            visual_usability: VisualUsabilityScore {
                score: 0.0,
                entities_rendered: 0,
                entities_expected: 0,
                camera_position_valid: false,
                lighting_present: false,
                ui_elements_visible: false,
                frame_rate_stable: false,
                screenshots: vec![],
            },
            intent_alignment: IntentAlignmentScore {
                score: 0.0,
                request_text: String::new(),
                detected_features: vec![],
                missing_features: vec![],
                extra_features: vec![],
                semantic_similarity: 0.0,
            },
            timestamp,
            scene_name: String::new(),
        })
    }
}

// ---------------------------------------------------------------------------
// Visual Usability Evaluator
// ---------------------------------------------------------------------------

pub struct VisualUsabilityEvaluator {
    /// Minimum entities expected for a valid scene
    min_entities: u32,
    /// Whether to capture screenshots
    capture_screenshots: bool,
}

impl VisualUsabilityEvaluator {
    pub fn new() -> Self {
        Self {
            min_entities: 1,
            capture_screenshots: true,
        }
    }
    
    pub fn with_min_entities(mut self, count: u32) -> Self {
        self.min_entities = count;
        self
    }
    
    /// Analyze scene file to count expected entities
    fn count_expected_entities(&self, project_path: &Path) -> u32 {
        let mut count = 0;
        
        // Walk scene files
        for entry in walkdir::WalkDir::new(project_path).max_depth(3) {
            if let Ok(entry) = entry {
                let path = entry.path();
                if path.extension().map(|e| e == "json" || e == "ron").unwrap_or(false) {
                    if let Ok(content) = std::fs::read_to_string(path) {
                        // Count entity definitions
                        count += content.matches("\"entity\"").count() as u32;
                        count += content.matches("Entity(").count() as u32;
                    }
                }
            }
        }
        
        count.max(self.min_entities)
    }
    
    /// Check if scene has basic lighting
    fn check_lighting(&self, project_path: &Path) -> bool {
        for entry in walkdir::WalkDir::new(project_path).max_depth(3) {
            if let Ok(entry) = entry {
                let path = entry.path();
                if path.extension().map(|e| e == "rs").unwrap_or(false) {
                    if let Ok(content) = std::fs::read_to_string(path) {
                        if content.contains("PointLight") 
                            || content.contains("DirectionalLight")
                            || content.contains("AmbientLight") {
                            return true;
                        }
                    }
                }
            }
        }
        false
    }
    
    /// Check if camera is positioned
    fn check_camera(&self, project_path: &Path) -> bool {
        for entry in walkdir::WalkDir::new(project_path).max_depth(3) {
            if let Ok(entry) = entry {
                let path = entry.path();
                if path.extension().map(|e| e == "rs").unwrap_or(false) {
                    if let Ok(content) = std::fs::read_to_string(path) {
                        if content.contains("Camera3dBundle") || content.contains("Camera2dBundle") {
                            return content.contains("transform") || content.contains("Transform");
                        }
                    }
                }
            }
        }
        false
    }
}

impl Evaluator for VisualUsabilityEvaluator {
    fn name(&self) -> &str {
        "visual_usability"
    }
    
    fn evaluate(&self, project_path: &Path, _request: &str) -> Result<BenchScore, BenchError> {
        let expected = self.count_expected_entities(project_path);
        let lighting = self.check_lighting(project_path);
        let camera = self.check_camera(project_path);
        
        // Score calculation
        let mut score = 0.0f32;
        
        if expected >= self.min_entities {
            score += 30.0;
        }
        if lighting {
            score += 25.0;
        }
        if camera {
            score += 25.0;
        }
        // Assume UI visible if there are UI components
        let ui_visible = walkdir::WalkDir::new(project_path)
            .max_depth(3)
            .into_iter()
            .filter_map(|e| e.ok())
            .any(|e| {
                if let Some(ext) = e.path().extension() {
                    if ext == "rs" {
                        if let Ok(content) = std::fs::read_to_string(e.path()) {
                            return content.contains("TextBundle") 
                                || content.contains("ButtonBundle");
                        }
                    }
                }
                false
            });
        
        if ui_visible {
            score += 20.0;
        }
        
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        
        Ok(BenchScore {
            overall: score,
            build_health: BuildHealthScore {
                score: 0.0,
                compilation_success: false,
                warning_count: 0,
                error_count: 0,
                dependency_errors: vec![],
                build_time_ms: 0,
            },
            visual_usability: VisualUsabilityScore {
                score,
                entities_rendered: expected, // Static analysis estimate
                entities_expected: expected,
                camera_position_valid: camera,
                lighting_present: lighting,
                ui_elements_visible: ui_visible,
                frame_rate_stable: true, // Assume stable without runtime
                screenshots: vec![],
            },
            intent_alignment: IntentAlignmentScore {
                score: 0.0,
                request_text: String::new(),
                detected_features: vec![],
                missing_features: vec![],
                extra_features: vec![],
                semantic_similarity: 0.0,
            },
            timestamp,
            scene_name: String::new(),
        })
    }
}

// ---------------------------------------------------------------------------
// Intent Alignment Evaluator (VLM-based)
// ---------------------------------------------------------------------------

pub struct IntentAlignmentEvaluator;

impl IntentAlignmentEvaluator {
    pub fn new() -> Self {
        Self
    }
    
    /// Extract features from user request using LLM
    async fn extract_features(&self, request: &str) -> Result<Vec<String>, BenchError> {
        let _prompt = format!(
            "Extract key game features from this request as a JSON array of strings:\n\n{}\n\n\
             Example output: [\"player movement\", \"enemy AI\", \"score system\"]",
            request
        );
        
        // This would call the LLM - simplified for now
        let features: Vec<String> = request
            .to_lowercase()
            .split_whitespace()
            .filter(|w| w.len() > 4)
            .map(|w| w.to_string())
            .collect();
        
        Ok(features)
    }
    
    /// Detect features implemented in code
    fn detect_code_features(&self, project_path: &Path) -> Vec<String> {
        let mut features = Vec::new();
        
        for entry in walkdir::WalkDir::new(project_path).max_depth(3) {
            if let Ok(entry) = entry {
                let path = entry.path();
                if path.extension().map(|e| e == "rs").unwrap_or(false) {
                    if let Ok(content) = std::fs::read_to_string(path) {
                        let content_lower = content.to_lowercase();
                        
                        if content_lower.contains("player") {
                            features.push("player".to_string());
                        }
                        if content_lower.contains("enemy") || content_lower.contains("ai") {
                            features.push("enemy AI".to_string());
                        }
                        if content_lower.contains("score") || content_lower.contains("point") {
                            features.push("scoring".to_string());
                        }
                        if content_lower.contains("camera") {
                            features.push("camera".to_string());
                        }
                        if content_lower.contains("physics") || content_lower.contains("rapier") {
                            features.push("physics".to_string());
                        }
                        if content_lower.contains("ui") || content_lower.contains("textbundle") {
                            features.push("UI".to_string());
                        }
                        if content_lower.contains("sound") || content_lower.contains("audio") {
                            features.push("audio".to_string());
                        }
                        if content_lower.contains("animation") || content_lower.contains("animate") {
                            features.push("animation".to_string());
                        }
                    }
                }
            }
        }
        
        features.sort();
        features.dedup();
        features
    }
    
    /// Calculate semantic similarity between request and implementation
    fn calculate_similarity(&self, request: &str, detected: &[String]) -> f32 {
        let request_lower = request.to_lowercase();
        let mut matches = 0;
        
        for feature in detected {
            if request_lower.contains(&feature.to_lowercase()) {
                matches += 1;
            }
        }
        
        if detected.is_empty() {
            0.0
        } else {
            (matches as f32 / detected.len() as f32) * 100.0
        }
    }
}

impl Evaluator for IntentAlignmentEvaluator {
    fn name(&self) -> &str {
        "intent_alignment"
    }
    
    fn evaluate(&self, project_path: &Path, request: &str) -> Result<BenchScore, BenchError> {
        let detected = self.detect_code_features(project_path);
        let similarity = self.calculate_similarity(request, &detected);
        
        // Simple keyword matching for expected features
        let expected: Vec<String> = request
            .to_lowercase()
            .split_whitespace()
            .filter(|w| w.len() > 4)
            .map(|w| w.to_string())
            .collect();
        
        let missing: Vec<String> = expected
            .iter()
            .filter(|e| !detected.iter().any(|d| d.to_lowercase().contains(&e.to_lowercase())))
            .cloned()
            .collect();
        
        let extra: Vec<String> = detected
            .iter()
            .filter(|d| !expected.iter().any(|e| d.to_lowercase().contains(&e.to_lowercase())))
            .cloned()
            .collect();
        
        let score = if expected.is_empty() {
            0.0
        } else {
            let match_count = expected.len() - missing.len();
            (match_count as f32 / expected.len() as f32) * 100.0
        };
        
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        
        Ok(BenchScore {
            overall: score,
            build_health: BuildHealthScore {
                score: 0.0,
                compilation_success: false,
                warning_count: 0,
                error_count: 0,
                dependency_errors: vec![],
                build_time_ms: 0,
            },
            visual_usability: VisualUsabilityScore {
                score: 0.0,
                entities_rendered: 0,
                entities_expected: 0,
                camera_position_valid: false,
                lighting_present: false,
                ui_elements_visible: false,
                frame_rate_stable: false,
                screenshots: vec![],
            },
            intent_alignment: IntentAlignmentScore {
                score,
                request_text: request.to_string(),
                detected_features: detected.clone(),
                missing_features: missing,
                extra_features: extra,
                semantic_similarity: similarity,
            },
            timestamp,
            scene_name: String::new(),
        })
    }
}

// ---------------------------------------------------------------------------
// BenchRunner — Orchestrates all evaluators
// ---------------------------------------------------------------------------

pub struct BenchRunner {
    evaluators: Vec<Box<dyn Evaluator>>,
    results_dir: PathBuf,
}

impl BenchRunner {
    pub fn new(results_dir: impl AsRef<Path>) -> Self {
        let dir = results_dir.as_ref().to_path_buf();
        let _ = std::fs::create_dir_all(&dir);
        
        Self {
            evaluators: vec![
                Box::new(BuildHealthEvaluator::new()),
                Box::new(VisualUsabilityEvaluator::new()),
                Box::new(IntentAlignmentEvaluator::new()),
            ],
            results_dir: dir,
        }
    }
    
    pub fn with_evaluator(mut self, evaluator: Box<dyn Evaluator>) -> Self {
        self.evaluators.push(evaluator);
        self
    }
    
    /// Run full evaluation suite
    pub fn evaluate(&self, project_path: &Path, request: &str, scene_name: &str) -> Result<BenchScore, BenchError> {
        let mut combined = BenchScore {
            overall: 0.0,
            build_health: BuildHealthScore {
                score: 0.0,
                compilation_success: false,
                warning_count: 0,
                error_count: 0,
                dependency_errors: vec![],
                build_time_ms: 0,
            },
            visual_usability: VisualUsabilityScore {
                score: 0.0,
                entities_rendered: 0,
                entities_expected: 0,
                camera_position_valid: false,
                lighting_present: false,
                ui_elements_visible: false,
                frame_rate_stable: false,
                screenshots: vec![],
            },
            intent_alignment: IntentAlignmentScore {
                score: 0.0,
                request_text: request.to_string(),
                detected_features: vec![],
                missing_features: vec![],
                extra_features: vec![],
                semantic_similarity: 0.0,
            },
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            scene_name: scene_name.to_string(),
        };
        
        let mut total_score = 0.0f32;
        let mut evaluator_count = 0;
        
        for evaluator in &self.evaluators {
            match evaluator.evaluate(project_path, request) {
                Ok(score) => {
                    total_score += score.overall;
                    evaluator_count += 1;
                    
                    // Merge scores
                    if score.build_health.score > 0.0 {
                        combined.build_health = score.build_health;
                    }
                    if score.visual_usability.score > 0.0 {
                        combined.visual_usability = score.visual_usability;
                    }
                    if score.intent_alignment.score > 0.0 {
                        combined.intent_alignment = score.intent_alignment;
                    }
                }
                Err(e) => {
                    eprintln!("Evaluator {} failed: {}", evaluator.name(), e);
                }
            }
        }
        
        if evaluator_count > 0 {
            combined.overall = total_score / evaluator_count as f32;
        }
        
        // Save result
        self.save_result(&combined)?;
        
        Ok(combined)
    }
    
    /// Save evaluation result to disk
    fn save_result(&self, score: &BenchScore) -> Result<(), BenchError> {
        let filename = format!(
            "{}_{}.json",
            score.scene_name,
            score.timestamp
        );
        
        let path = self.results_dir.join(filename);
        let json = serde_json::to_string_pretty(score)?;
        std::fs::write(path, json)?;
        
        Ok(())
    }
    
    /// Load historical results
    pub fn load_history(&self, scene_name: &str) -> Result<Vec<BenchScore>, BenchError> {
        let mut results = Vec::new();
        
        for entry in std::fs::read_dir(&self.results_dir)? {
            let entry = entry?;
            let path = entry.path();
            
            if path.file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.starts_with(scene_name))
                .unwrap_or(false) {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    if let Ok(score) = serde_json::from_str::<BenchScore>(&content) {
                        results.push(score);
                    }
                }
            }
        }
        
        results.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        Ok(results)
    }
    
    /// Generate report comparing iterations
    pub fn generate_report(&self, scene_name: &str) -> Result<String, BenchError> {
        let history = self.load_history(scene_name)?;
        
        if history.is_empty() {
            return Ok("No evaluation history found.".to_string());
        }
        
        let mut report = format!("# Evaluation Report: {}\n\n", scene_name);
        report.push_str("| Iteration | Overall | Build | Visual | Intent |\n");
        report.push_str("|-----------|---------|-------|--------|--------|\n");
        
        for (i, score) in history.iter().enumerate() {
            report.push_str(&format!(
                "| {} | {:.1} | {:.1} | {:.1} | {:.1} |\n",
                i + 1,
                score.overall,
                score.build_health.score,
                score.visual_usability.score,
                score.intent_alignment.score
            ));
        }
        
        // Trend analysis
        if history.len() >= 2 {
            let first = history.last().unwrap();
            let latest = history.first().unwrap();
            let improvement = latest.overall - first.overall;
            
            report.push_str(&format!("\n## Trend\n\n"));
            report.push_str(&format!(
                "Overall improvement: {:.1}% {}\n",
                improvement.abs(),
                if improvement > 0.0 { "📈" } else { "📉" }
            ));
        }
        
        Ok(report)
    }
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Integration with DirectorRuntime
// ---------------------------------------------------------------------------

use crate::director::DirectorRuntime;

pub trait BenchIntegration {
    fn evaluate_current_scene(&self, request: &str) -> Result<BenchScore, BenchError>;
    fn should_continue_iteration(&self, score: &BenchScore) -> bool;
}

impl BenchIntegration for DirectorRuntime {
    fn evaluate_current_scene(&self, _request: &str) -> Result<BenchScore, BenchError> {
        // This would integrate with the actual project path
        Err(BenchError::BuildFailed("Not yet implemented".to_string()))
    }
    
    fn should_continue_iteration(&self, score: &BenchScore) -> bool {
        // Continue if score is below threshold
        score.overall < 80.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_build_health_evaluator() {
        let temp_dir = std::env::temp_dir().join("agentedit_bench_test");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();
        
        // Create a minimal Cargo.toml
        std::fs::write(
            temp_dir.join("Cargo.toml"),
            r#"
[package]
name = "test"
version = "0.1.0"
edition = "2021"

[dependencies]
"#
        ).unwrap();
        
        std::fs::create_dir_all(temp_dir.join("src")).unwrap();
        std::fs::write(
            temp_dir.join("src/main.rs"),
            "fn main() {}"
        ).unwrap();
        
        let evaluator = BuildHealthEvaluator::new();
        let result = evaluator.evaluate(&temp_dir, "");
        
        assert!(result.is_ok());
        let score = result.unwrap();
        assert!(score.build_health.compilation_success);
        
        let _ = std::fs::remove_dir_all(&temp_dir);
    }
    
    #[test]
    fn test_visual_evaluator() {
        let temp_dir = std::env::temp_dir().join("agentedit_visual_test");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();
        
        std::fs::create_dir_all(temp_dir.join("src")).unwrap();
        std::fs::write(
            temp_dir.join("src/main.rs"),
            r#"
fn main() {
    // Camera3dBundle with Transform
    // PointLight
}
"#
        ).unwrap();
        
        let evaluator = VisualUsabilityEvaluator::new();
        let result = evaluator.evaluate(&temp_dir, "");
        
        assert!(result.is_ok());
        let score = result.unwrap();
        assert!(score.visual_usability.camera_position_valid);
        
        let _ = std::fs::remove_dir_all(&temp_dir);
    }
    
    #[test]
    fn test_bench_runner() {
        let temp_dir = std::env::temp_dir().join("agentedit_runner_test");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();
        
        let results_dir = temp_dir.join("results");
        let runner = BenchRunner::new(&results_dir);
        
        assert!(!runner.evaluators.is_empty());
        
        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
