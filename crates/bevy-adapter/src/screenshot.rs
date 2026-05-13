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
    Success { path: PathBuf, dimensions: (u32, u32), base64: String },
    Failure { error: String },
}

/// Resource for requesting and receiving screenshots.
/// Screenshots are captured via Bevy's built-in ScreenshotPlugin and
/// results are stored in the shared `results` queue.
#[derive(Resource)]
pub struct ScreenshotQueue {
    pub results: Arc<Mutex<Vec<ScreenshotResult>>>,
    pub counter: u64,
}

impl ScreenshotQueue {
    pub fn new() -> Self {
        Self { results: Arc::new(Mutex::new(Vec::new())), counter: 0 }
    }
    pub fn pop_result(&self) -> Option<ScreenshotResult> {
        self.results.lock().ok()?.pop()
    }
}

impl Default for ScreenshotQueue {
    fn default() -> Self { Self::new() }
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
        img.save(path).map_err(|e| ScreenshotError::EncodeError(e.to_string()))?;
        Ok(())
    }

    pub fn to_base64(&self) -> Result<String, ScreenshotError> {
        if let Some(ref b64) = self.base64 {
            return Ok(b64.clone());
        }
        encode_image_to_base64(&self.data, self.width, self.height)
    }

    pub fn dimensions(&self) -> (u32, u32) { (self.width, self.height) }
    pub fn estimated_size(&self) -> usize { self.data.len() / 3 }
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
        Self { default_output_dir: PathBuf::from("screenshots"), ..Default::default() }
    }
    pub fn set_output_dir(&mut self, dir: impl Into<PathBuf>) {
        self.default_output_dir = dir.into();
    }
}

pub struct ScreenshotPlugin;

impl Plugin for ScreenshotPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(bevy::render::view::screenshot::ScreenshotPlugin);
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
    let results_ref = queue.results.clone();
    let counter = queue.counter;
    queue.counter += 1;

    let output_dir = state.default_output_dir.clone();
    let path = output_dir.join(format!("screenshot_{}.png", counter));

    commands.spawn(Screenshot::primary_window())
        .observe(save_to_disk(path.clone()))
        .observe(
            move |capture: On<ScreenshotCaptured>| {
                let image = &capture.image;
                let (width, height) = (image.width(), image.height());
                let Some(data) = image.data.clone() else { return };
                let b64 = encode_image_to_base64(&data, width, height).ok();
                if let Ok(mut results) = results_ref.lock() {
                    results.push(ScreenshotResult::Success {
                        path: path.clone(),
                        dimensions: (width, height),
                        base64: b64.unwrap_or_default(),
                    });
                }
            }
        );
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

pub fn manual_screenshot_placeholder(
    _size: (u32, u32),
) -> Result<ScreenshotArtifact, ScreenshotError> {
    Err(ScreenshotError::NotInitialized)
}

pub fn encode_for_vision_placeholder(
    artifact: &ScreenshotArtifact,
) -> Result<(String, (u32, u32)), ScreenshotError> {
    let b64 = artifact.to_base64()?;
    Ok((b64, artifact.dimensions()))
}
