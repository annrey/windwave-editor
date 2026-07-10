//! Screenshot system for Bevy engine — powered by Bevy 0.17's built-in ScreenshotPlugin.
//!
//! Provides on-demand viewport capture via [`bevy::render::view::screenshot::Screenshot`].
//! Captured images are saved to disk as PNG and optionally encoded to base64 for vision APIs.

use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Screenshot, ScreenshotCaptured};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// Result of a screenshot operation.
#[derive(Debug, Clone)]
pub enum ScreenshotResult {
    Success {
        path: PathBuf,
        dimensions: (u32, u32),
        base64: String,
    },
    Failure {
        error: String,
    },
}

#[derive(Debug, Default)]
struct ScreenshotTelemetry {
    requests_total: u64,
    success_total: u64,
    failure_total: u64,
    last_result_summary: Option<String>,
}

/// Resource for requesting and receiving screenshots.
/// Screenshots are captured via Bevy's built-in ScreenshotPlugin and
/// results are stored in the shared `results` queue.
/// Captures are rate-limited to prevent performance degradation.
#[derive(Resource)]
pub struct ScreenshotQueue {
    pub results: Arc<Mutex<Vec<ScreenshotResult>>>,
    telemetry: Arc<Mutex<ScreenshotTelemetry>>,
    pub counter: u64,
    pub requested: bool,
    pub last_capture: Option<Instant>,
    pub cooldown_secs: f32,
}

impl ScreenshotQueue {
    pub fn new() -> Self {
        Self {
            results: Arc::new(Mutex::new(Vec::new())),
            telemetry: Arc::new(Mutex::new(ScreenshotTelemetry::default())),
            counter: 0,
            requested: false,
            last_capture: None,
            cooldown_secs: 0.5,
        }
    }

    pub fn pop_result(&self) -> Option<ScreenshotResult> {
        self.results.lock().ok()?.pop()
    }

    pub fn record_result(&self, result: ScreenshotResult) {
        Self::record_result_with_refs(&self.results, &self.telemetry, result);
    }

    pub fn runtime_readback_evidence(&self) -> Vec<String> {
        let result_count = self
            .results
            .lock()
            .map(|results| results.len())
            .unwrap_or(0);
        let telemetry = self.telemetry.lock().ok();
        let requests_total = telemetry
            .as_ref()
            .map(|telemetry| telemetry.requests_total)
            .unwrap_or_default();
        let success_total = telemetry
            .as_ref()
            .map(|telemetry| telemetry.success_total)
            .unwrap_or_default();
        let failure_total = telemetry
            .as_ref()
            .map(|telemetry| telemetry.failure_total)
            .unwrap_or_default();
        let last_result = telemetry
            .as_ref()
            .and_then(|telemetry| telemetry.last_result_summary.clone())
            .unwrap_or_else(|| "none".to_string());

        vec![
            format!("bevy_screenshot_requested={}", self.requested),
            format!("bevy_screenshot_requests_total={requests_total}"),
            format!("bevy_screenshot_success_total={success_total}"),
            format!("bevy_screenshot_failure_total={failure_total}"),
            format!("bevy_screenshot_result_count={result_count}"),
            format!("bevy_screenshot_last_result={last_result}"),
        ]
    }

    /// Request a screenshot capture. The capture will happen on the next frame
    /// only if the cooldown period has elapsed.
    pub fn request_capture(&mut self) {
        self.requested = true;
        if let Ok(mut telemetry) = self.telemetry.lock() {
            telemetry.requests_total += 1;
        }
    }

    fn record_result_with_refs(
        results_ref: &Arc<Mutex<Vec<ScreenshotResult>>>,
        telemetry_ref: &Arc<Mutex<ScreenshotTelemetry>>,
        result: ScreenshotResult,
    ) {
        if let Ok(mut telemetry) = telemetry_ref.lock() {
            match &result {
                ScreenshotResult::Success { .. } => telemetry.success_total += 1,
                ScreenshotResult::Failure { .. } => telemetry.failure_total += 1,
            }
            telemetry.last_result_summary = Some(Self::summarize_result(&result));
        }
        if let Ok(mut results) = results_ref.lock() {
            results.push(result);
        }
    }

    fn summarize_result(result: &ScreenshotResult) -> String {
        match result {
            ScreenshotResult::Success {
                path, dimensions, ..
            } => format!(
                "success path={} dimensions={}x{}",
                path.display(),
                dimensions.0,
                dimensions.1
            ),
            ScreenshotResult::Failure { error } => format!("failure error={error}"),
        }
    }
}

impl Default for ScreenshotQueue {
    fn default() -> Self {
        Self::new()
    }
}

/// The captured screenshot artifact (raw RGBA pixel data).
#[derive(Debug, Clone, Component)]
pub struct ScreenshotArtifact {
    pub data: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub captured_at: Instant,
    pub base64: Option<String>,
}

impl ScreenshotArtifact {
    pub fn save_png(&self, path: impl AsRef<std::path::Path>) -> Result<(), ScreenshotError> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| ScreenshotError::IoError(e.to_string()))?;
        }
        let img: image::ImageBuffer<image::Rgba<u8>, Vec<u8>> =
            image::ImageBuffer::from_raw(self.width, self.height, self.data.clone())
                .ok_or(ScreenshotError::InvalidDimensions)?;
        img.save(path)
            .map_err(|e| ScreenshotError::EncodeError(e.to_string()))?;
        Ok(())
    }

    pub fn to_base64(&self) -> Result<String, ScreenshotError> {
        if let Some(ref b64) = self.base64 {
            return Ok(b64.clone());
        }
        encode_image_to_base64(&self.data, self.width, self.height)
    }

    pub fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }
    pub fn estimated_size(&self) -> usize {
        self.data.len() / 3
    }
}

#[derive(Debug, Clone)]
pub enum ScreenshotError {
    ReadError(String),
    EncodeError(String),
    InvalidDimensions,
    IoError(String),
    NotInitialized,
}

impl std::fmt::Display for ScreenshotError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ReadError(e) => write!(f, "Failed to read render target: {}", e),
            Self::EncodeError(e) => write!(f, "Failed to encode image: {}", e),
            Self::InvalidDimensions => write!(f, "Invalid dimensions"),
            Self::IoError(e) => write!(f, "IO error: {}", e),
            Self::NotInitialized => write!(f, "Screenshot system not initialized"),
        }
    }
}
impl std::error::Error for ScreenshotError {}

#[derive(Resource, Default)]
pub struct ScreenshotState {
    pub completed_artifacts: Vec<ScreenshotArtifact>,
    pub screenshot_counter: u64,
    pub default_output_dir: PathBuf,
}

impl ScreenshotState {
    pub fn new() -> Self {
        Self {
            default_output_dir: PathBuf::from("screenshots"),
            ..Default::default()
        }
    }
    pub fn set_output_dir(&mut self, dir: impl Into<PathBuf>) {
        self.default_output_dir = dir.into();
    }
}

pub struct ScreenshotPlugin;

impl Plugin for ScreenshotPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<bevy::render::view::screenshot::ScreenshotPlugin>() {
            app.add_plugins(bevy::render::view::screenshot::ScreenshotPlugin);
        }
        app.init_resource::<ScreenshotQueue>();
        app.init_resource::<ScreenshotState>();
        app.add_systems(Update, process_screenshot_requests);
    }
}

fn process_screenshot_requests(
    mut commands: Commands,
    mut queue: ResMut<ScreenshotQueue>,
    state: Res<ScreenshotState>,
) {
    // Rate-limit: only capture when explicitly requested and cooldown elapsed
    if !queue.requested {
        return;
    }
    if let Some(last) = queue.last_capture {
        if last.elapsed().as_secs_f32() < queue.cooldown_secs {
            return;
        }
    }
    queue.requested = false;
    queue.last_capture = Some(Instant::now());

    let results_ref = queue.results.clone();
    let telemetry_ref = queue.telemetry.clone();
    let counter = queue.counter;
    queue.counter += 1;

    let output_dir = state.default_output_dir.clone();
    let path = output_dir.join(format!("screenshot_{}.png", counter));

    commands
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(path.clone()))
        .observe(move |capture: On<ScreenshotCaptured>| {
            let image = &capture.image;
            let (width, height) = (image.width(), image.height());
            let Some(data) = image.data.clone() else {
                ScreenshotQueue::record_result_with_refs(
                    &results_ref,
                    &telemetry_ref,
                    ScreenshotResult::Failure {
                        error: "captured screenshot has no image data".to_string(),
                    },
                );
                return;
            };
            let b64 = encode_image_to_base64(&data, width, height).ok();
            ScreenshotQueue::record_result_with_refs(
                &results_ref,
                &telemetry_ref,
                ScreenshotResult::Success {
                    path: path.clone(),
                    dimensions: (width, height),
                    base64: b64.unwrap_or_default(),
                },
            );
        });
}

fn encode_image_to_base64(data: &[u8], width: u32, height: u32) -> Result<String, ScreenshotError> {
    let img: image::ImageBuffer<image::Rgba<u8>, Vec<u8>> =
        image::ImageBuffer::from_raw(width, height, data.to_vec())
            .ok_or(ScreenshotError::InvalidDimensions)?;
    let mut buffer = Vec::new();
    let mut cursor = std::io::Cursor::new(&mut buffer);
    img.write_to(&mut cursor, image::ImageOutputFormat::Png)
        .map_err(|e| ScreenshotError::EncodeError(e.to_string()))?;
    use base64::Engine;
    Ok(base64::engine::general_purpose::STANDARD.encode(&buffer))
}
