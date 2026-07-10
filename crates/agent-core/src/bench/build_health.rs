//! BuildHealthEvaluator — Compilation health checker

use super::types::*;
use std::path::Path;
use std::process::Command;

pub struct BuildHealthEvaluator;

impl Default for BuildHealthEvaluator {
    fn default() -> Self {
        Self::new()
    }
}

impl BuildHealthEvaluator {
    pub fn new() -> Self {
        Self
    }

    fn evaluate_compilation(&self, project_path: &Path) -> Result<BuildHealthScore, BenchError> {
        let start = std::time::Instant::now();

        let output = Command::new("cargo")
            .current_dir(project_path)
            .args(["check", "--message-format=short"])
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
