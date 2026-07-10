//! Reflection Statistics

/// Performance statistics for the reflection engine.
#[derive(Debug, Clone)]
pub struct ReflectionStats {
    /// Total number of reflections attempted.
    pub total_reflections: usize,
    /// Number of successful resolutions.
    pub successful: usize,
    /// Number of failures (all retries exhausted).
    pub failed: usize,
    /// Success rate (0.0 to 1.0).
    pub success_rate: f64,
}

impl std::fmt::Display for ReflectionStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Reflections: total={}, ✅={}, ❌={}, rate={:.1}%",
            self.total_reflections,
            self.successful,
            self.failed,
            self.success_rate * 100.0
        )
    }
}
