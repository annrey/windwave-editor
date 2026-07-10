use super::*;

// ===========================================================================
// VisionPlugin — basic visual observation pipeline
// ===========================================================================

/// Artifact representing a captured screenshot.
#[derive(Debug, Clone)]
pub struct ScreenshotArtifact {
    /// Path where the screenshot file is stored.
    pub path: String,
    /// Image dimensions (width, height).
    pub dimensions: (u32, u32),
    /// Timestamp of capture (millis since epoch).
    pub captured_at_ms: u64,
    /// Optional raw pixel data for in-memory analysis.
    pub raw_data: Option<Vec<u8>>,
}

/// Result of visual analysis on a screenshot.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct VisualObservation {
    /// Path to the screenshot that was analyzed.
    pub screenshot_path: String,
    /// Human-readable summary of what's visible.
    pub summary: String,
    /// Entities that the vision model detected.
    pub visible_entities: Vec<String>,
    /// Anomalies or unexpected visual states.
    pub anomalies: Vec<String>,
    /// Confidence score (0.0 – 1.0).
    pub confidence: f32,
}

impl Default for VisualObservation {
    fn default() -> Self {
        Self {
            screenshot_path: String::new(),
            summary: String::new(),
            visible_entities: Vec::new(),
            anomalies: Vec::new(),
            confidence: 1.0,
        }
    }
}

/// Error type for vision operations.
#[derive(Debug, Clone)]
pub enum VisionError {
    CaptureFailed(String),
    AnalysisFailed(String),
    NoProvider,
}

impl std::fmt::Display for VisionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VisionError::CaptureFailed(msg) => write!(f, "Capture failed: {}", msg),
            VisionError::AnalysisFailed(msg) => write!(f, "Analysis failed: {}", msg),
            VisionError::NoProvider => write!(f, "No vision provider configured"),
        }
    }
}

/// Trait for capturing screenshots from the engine (Bevy or other).
#[async_trait::async_trait]
pub trait ScreenshotProvider: Send + Sync {
    /// Capture a screenshot and return its artifact.
    async fn capture(&self) -> Result<ScreenshotArtifact, VisionError>;
}

/// Trait for running visual analysis on a screenshot (e.g. via vision LLM).
#[async_trait::async_trait]
pub trait VisionProvider: Send + Sync {
    /// Analyze a screenshot with an optional prompt.
    async fn analyze(
        &self,
        screenshot: &ScreenshotArtifact,
        prompt: &str,
    ) -> Result<VisualObservation, VisionError>;
}

/// Test helper that returns a fixed screenshot artifact without capturing.
pub struct MockScreenshotProvider {
    pub dimensions: (u32, u32),
    pub artifact_path: String,
}

impl Default for MockScreenshotProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl MockScreenshotProvider {
    pub fn new() -> Self {
        Self {
            dimensions: (1920, 1080),
            artifact_path: "mock_screenshot.png".to_string(),
        }
    }
}

#[async_trait::async_trait]
impl ScreenshotProvider for MockScreenshotProvider {
    async fn capture(&self) -> Result<ScreenshotArtifact, VisionError> {
        Ok(ScreenshotArtifact {
            path: self.artifact_path.clone(),
            dimensions: self.dimensions,
            captured_at_ms: 0,
            raw_data: None,
        })
    }
}

/// Test helper that returns a fixed visual observation without analysis.
pub struct MockVisionProvider {
    pub observation: VisualObservation,
}

impl MockVisionProvider {
    pub fn with_summary(summary: &str) -> Self {
        Self {
            observation: VisualObservation {
                screenshot_path: String::new(),
                summary: summary.to_string(),
                visible_entities: vec!["Player".to_string(), "Enemy".to_string()],
                anomalies: Vec::new(),
                confidence: 0.95,
            },
        }
    }
}

#[async_trait::async_trait]
impl VisionProvider for MockVisionProvider {
    async fn analyze(
        &self,
        _screenshot: &ScreenshotArtifact,
        _prompt: &str,
    ) -> Result<VisualObservation, VisionError> {
        Ok(self.observation.clone())
    }
}

/// Vision pipeline state (Bevy Resource).
#[derive(Resource, Default)]
pub struct VisionState {
    /// Optional screenshot provider (None = not configured).
    pub screenshot_provider: Option<Box<dyn ScreenshotProvider>>,
    /// Optional vision provider (None = not configured).
    pub vision_provider: Option<Box<dyn VisionProvider>>,
    /// Latest captured screenshot artifact.
    pub last_screenshot: Option<ScreenshotArtifact>,
    /// Latest visual observation result.
    pub last_observation: Option<VisualObservation>,
    /// Whether a capture is currently in progress (async guard).
    pub capture_in_progress: bool,
}

impl VisionState {
    /// Check if vision pipeline is fully configured.
    pub fn is_ready(&self) -> bool {
        self.screenshot_provider.is_some() && self.vision_provider.is_some()
    }

    /// Set mock providers for testing.
    pub fn with_mock_providers(&mut self) {
        self.screenshot_provider = Some(Box::new(MockScreenshotProvider::new()));
        self.vision_provider = Some(Box::new(MockVisionProvider::with_summary(
            "Mock scene analysis",
        )));
    }

    /// Set real (production) providers: Bevy screenshot + SceneIndex vision.
    pub fn with_real_providers(&mut self, output_dir: impl Into<String>) {
        self.screenshot_provider = Some(Box::new(BevyScreenshotProvider::new(output_dir)));
        self.vision_provider = Some(Box::new(SceneIndexVisionProvider::new()));
    }
}

/// Plugin for visual observation pipeline.
///
/// Initialises `VisionState` resource and registers optional systems
/// for screenshot capture and analysis.
pub struct VisionPlugin;

impl Plugin for VisionPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<VisionState>()
            .add_systems(Update, vision_status_system);
    }
}

/// Simple status system — logs vision pipeline readiness.
fn vision_status_system(vision: Res<VisionState>, mut logged: Local<bool>) {
    if !*logged && vision.is_ready() {
        info!(
            "Vision pipeline ready: screenshot={}, analysis={}",
            vision.screenshot_provider.is_some(),
            vision.vision_provider.is_some()
        );
        *logged = true;
    }
}
