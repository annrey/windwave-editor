//! BenchRunner — Orchestrates all evaluators + DirectorRuntime integration

use super::build_health::BuildHealthEvaluator;
use super::intent_alignment::IntentAlignmentEvaluator;
use super::types::*;
use super::visual_usability::VisualUsabilityEvaluator;
use crate::director::DirectorRuntime;
use std::path::{Path, PathBuf};

pub struct BenchRunner {
    pub(crate) evaluators: Vec<Box<dyn Evaluator>>,
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
    pub fn evaluate(
        &self,
        project_path: &Path,
        request: &str,
        scene_name: &str,
    ) -> Result<BenchScore, BenchError> {
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
        let filename = format!("{}_{}.json", score.scene_name, score.timestamp);

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

            if path
                .file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.starts_with(scene_name))
                .unwrap_or(false)
            {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    if let Ok(score) = serde_json::from_str::<BenchScore>(&content) {
                        results.push(score);
                    }
                }
            }
        }

        results.sort_by_key(|a| std::cmp::Reverse(a.timestamp));
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

            report.push_str("\n## Trend\n\n");
            report.push_str(&format!(
                "Overall improvement: {:.1}% {}\n",
                improvement.abs(),
                if improvement > 0.0 { "📈" } else { "📉" }
            ));
        }

        Ok(report)
    }
}

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
