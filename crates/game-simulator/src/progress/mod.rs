//! Progress tracking and reporting

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::fmt;
use std::time::Duration;

/// Progress tracker for simulation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgressTracker {
    total_frames: Option<u64>,
    current_frame: u64,
    frame_times: VecDeque<f64>,
    start_time: Option<DateTime<Utc>>,
    end_time: Option<DateTime<Utc>>,
    is_running: bool,
    milestones: Vec<Milestone>,
}

/// A milestone in the simulation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Milestone {
    pub name: String,
    pub frame: u64,
    pub timestamp: DateTime<Utc>,
    pub data: serde_json::Value,
}

impl ProgressTracker {
    /// Create a new progress tracker
    pub fn new() -> Self {
        Self {
            total_frames: None,
            current_frame: 0,
            frame_times: VecDeque::with_capacity(100),
            start_time: None,
            end_time: None,
            is_running: false,
            milestones: Vec::new(),
        }
    }

    /// Start tracking with expected total frames
    pub fn start(&mut self, total: u64) {
        self.total_frames = Some(total);
        self.current_frame = 0;
        self.start_time = Some(Utc::now());
        self.end_time = None;
        self.is_running = true;
    }

    /// Start tracking without frame limit
    pub fn start_unbounded(&mut self) {
        self.total_frames = None;
        self.current_frame = 0;
        self.start_time = Some(Utc::now());
        self.end_time = None;
        self.is_running = true;
    }

    /// Finish tracking
    pub fn finish(&mut self) {
        self.is_running = false;
        self.end_time = Some(Utc::now());
    }

    /// Advance to next frame
    pub fn advance_frame(&mut self) {
        self.current_frame += 1;
    }

    /// Record frame time
    pub fn record_frame(&mut self, time_sec: f64) {
        self.frame_times.push_back(time_sec);
        if self.frame_times.len() > 100 {
            self.frame_times.pop_front();
        }
    }

    /// Add a milestone
    pub fn add_milestone(&mut self, name: &str, data: serde_json::Value) {
        self.milestones.push(Milestone {
            name: name.to_string(),
            frame: self.current_frame,
            timestamp: Utc::now(),
            data,
        });
    }

    /// Get current progress (0.0 - 1.0)
    pub fn progress(&self) -> f64 {
        match self.total_frames {
            Some(total) if total > 0 => self.current_frame as f64 / total as f64,
            _ => 0.0,
        }
    }

    /// Get current frame
    pub fn current_frame(&self) -> u64 {
        self.current_frame
    }

    /// Get average frame time (last 100 frames)
    pub fn avg_frame_time(&self) -> f64 {
        if self.frame_times.is_empty() {
            return 0.0;
        }
        self.frame_times.iter().sum::<f64>() / self.frame_times.len() as f64
    }

    /// Get estimated time remaining
    pub fn estimated_remaining(&self) -> Option<Duration> {
        let total = self.total_frames?;
        let remaining = total.saturating_sub(self.current_frame);
        let avg = self.avg_frame_time();
        if avg > 0.0 {
            Some(Duration::from_secs_f64(avg * remaining as f64))
        } else {
            None
        }
    }

    /// Get total elapsed time
    pub fn elapsed(&self) -> Option<Duration> {
        let start = self.start_time?;
        let end = self.end_time.unwrap_or_else(Utc::now);
        Some((end - start).to_std().unwrap_or(Duration::ZERO))
    }

    /// Get milestones
    pub fn milestones(&self) -> &[Milestone] {
        &self.milestones
    }

    /// Generate a progress report
    pub fn report(&self) -> ProgressReport {
        ProgressReport {
            current_frame: self.current_frame,
            total_frames: self.total_frames,
            progress: self.progress(),
            avg_frame_time_ms: self.avg_frame_time() * 1000.0,
            estimated_remaining_ms: self.estimated_remaining().map(|d| d.as_millis() as u64),
            elapsed_ms: self.elapsed().map(|d| d.as_millis() as u64),
            milestone_count: self.milestones.len(),
        }
    }
}

impl Default for ProgressTracker {
    fn default() -> Self {
        Self::new()
    }
}

/// Progress report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgressReport {
    pub current_frame: u64,
    pub total_frames: Option<u64>,
    pub progress: f64,
    pub avg_frame_time_ms: f64,
    pub estimated_remaining_ms: Option<u64>,
    pub elapsed_ms: Option<u64>,
    pub milestone_count: usize,
}

impl fmt::Display for ProgressReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let total_str = self.total_frames.map_or("?".to_string(), |t| t.to_string());
        let progress_pct = (self.progress * 100.0).round();
        let remaining_str = self
            .estimated_remaining_ms
            .map(|ms| format!("{:.1}s", ms as f64 / 1000.0))
            .unwrap_or_else(|| "?".to_string());
        let elapsed_str = self
            .elapsed_ms
            .map(|ms| format!("{:.1}s", ms as f64 / 1000.0))
            .unwrap_or_else(|| "?".to_string());

        write!(
            f,
            "Progress: {}/{} frames ({:.0}%) | Frame: {:.2}ms | Elapsed: {} | Remaining: {} | Milestones: {}",
            self.current_frame, total_str, progress_pct,
            self.avg_frame_time_ms, elapsed_str, remaining_str,
            self.milestone_count
        )
    }
}
