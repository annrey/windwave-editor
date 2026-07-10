//! VisualUsabilityEvaluator — Scene rendering and UI checker

use super::types::*;
use std::path::Path;

pub struct VisualUsabilityEvaluator {
    /// Minimum entities expected for a valid scene
    min_entities: u32,
}

impl Default for VisualUsabilityEvaluator {
    fn default() -> Self {
        Self::new()
    }
}

impl VisualUsabilityEvaluator {
    pub fn new() -> Self {
        Self { min_entities: 1 }
    }

    pub fn with_min_entities(mut self, count: u32) -> Self {
        self.min_entities = count;
        self
    }

    /// Analyze scene file to count expected entities
    fn count_expected_entities(&self, project_path: &Path) -> u32 {
        let mut count = 0;

        // Walk scene files
        for entry in walkdir::WalkDir::new(project_path)
            .max_depth(3)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if path
                .extension()
                .map(|e| e == "json" || e == "ron")
                .unwrap_or(false)
            {
                if let Ok(content) = std::fs::read_to_string(path) {
                    count += content.matches("\"entity\"").count() as u32;
                    count += content.matches("Entity(").count() as u32;
                }
            }
        }

        count.max(self.min_entities)
    }

    /// Check if scene has basic lighting
    fn check_lighting(&self, project_path: &Path) -> bool {
        for entry in walkdir::WalkDir::new(project_path)
            .max_depth(3)
            .into_iter()
            .flatten()
        {
            let path = entry.path();
            if path.extension().map(|e| e == "rs").unwrap_or(false) {
                if let Ok(content) = std::fs::read_to_string(path) {
                    if content.contains("PointLight")
                        || content.contains("DirectionalLight")
                        || content.contains("AmbientLight")
                    {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Check if camera is positioned
    fn check_camera(&self, project_path: &Path) -> bool {
        for entry in walkdir::WalkDir::new(project_path)
            .max_depth(3)
            .into_iter()
            .flatten()
        {
            let path = entry.path();
            if path.extension().map(|e| e == "rs").unwrap_or(false) {
                if let Ok(content) = std::fs::read_to_string(path) {
                    if content.contains("Camera3dBundle") || content.contains("Camera2dBundle") {
                        return content.contains("transform") || content.contains("Transform");
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
