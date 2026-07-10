//! Retry Configuration - 重试配置

use std::time::Duration;

/// Configuration for retry behavior with exponential backoff.
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Maximum number of retry attempts.
    pub max_retries: usize,
    /// Initial delay before first retry (milliseconds).
    pub initial_delay_ms: u64,
    /// Multiplier for delay between retries (exponential backoff).
    pub backoff_multiplier: f64,
    /// Maximum delay cap (milliseconds).
    pub max_delay_ms: u64,
    /// Whether to add jitter to prevent thundering herd.
    pub jitter: bool,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_delay_ms: 100,
            backoff_multiplier: 2.0,
            max_delay_ms: 5000,
            jitter: true,
        }
    }
}

impl RetryConfig {
    /// Calculate delay for a specific retry attempt (with optional jitter).
    pub fn calculate_delay(&self, attempt: usize) -> Duration {
        let base_delay =
            self.initial_delay_ms as f64 * self.backoff_multiplier.powi(attempt as i32);
        let capped_delay = base_delay.min(self.max_delay_ms as f64) as u64;

        let final_delay = if self.jitter && capped_delay > 0 {
            // Add ±20% random jitter (simple implementation)
            let jitter_range = (capped_delay as f64 * 0.2) as u64;
            if jitter_range > 0 {
                // Use timestamp-based pseudo-random to avoid external dependency
                let pseudo_random = crate::types::now_millis() % (jitter_range + 1);
                capped_delay.saturating_add(pseudo_random)
            } else {
                capped_delay
            }
        } else {
            capped_delay
        };

        Duration::from_millis(final_delay)
    }
}
