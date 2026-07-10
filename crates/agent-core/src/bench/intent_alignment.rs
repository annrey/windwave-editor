//! IntentAlignmentEvaluator — VLM-based request-to-implementation matcher

use super::types::*;
use std::path::Path;

pub struct IntentAlignmentEvaluator;

impl Default for IntentAlignmentEvaluator {
    fn default() -> Self {
        Self::new()
    }
}

impl IntentAlignmentEvaluator {
    pub fn new() -> Self {
        Self
    }

    /// Detect features implemented in code
    fn detect_code_features(&self, project_path: &Path) -> Vec<String> {
        let mut features = Vec::new();

        for entry in walkdir::WalkDir::new(project_path)
            .max_depth(3)
            .into_iter()
            .flatten()
        {
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
            .filter(|e| {
                !detected
                    .iter()
                    .any(|d| d.to_lowercase().contains(&e.to_lowercase()))
            })
            .cloned()
            .collect();

        let extra: Vec<String> = detected
            .iter()
            .filter(|d| {
                !expected
                    .iter()
                    .any(|e| d.to_lowercase().contains(&e.to_lowercase()))
            })
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
