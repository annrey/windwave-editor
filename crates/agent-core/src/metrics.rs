//! Agent Metrics & Performance Tracer
//!
//! Implements Design Document Section 10.1: structured collection of timing,
//! call-count, success-rate, and memory-metrics for every Agent invocation,
//! plus a generic `PerformanceTracer` helper for ad-hoc timing.

use std::time::{Duration, Instant};

/// Aggregate metrics captured across one or more Agent runs.
///
/// Callers should periodically feed data into a shared `AgentMetrics`
/// instance and then query it for dashboards / logging.
#[derive(Debug, Clone, Default)]
pub struct AgentMetrics {
    // --- timing ---
    pub total_execution_time: Duration,
    pub llm_call_time: Duration,
    pub tool_execution_time: Duration,
    pub thinking_time: Duration,

    // --- call statistics ---
    pub llm_calls: usize,
    pub tool_calls: usize,
    pub tokens_sent: usize,
    pub tokens_received: usize,

    // --- success rates ---
    pub successful_operations: usize,
    pub failed_operations: usize,
    pub retried_operations: usize,

    // --- memory ---
    pub peak_memory_mb: usize,
    pub context_window_size: usize,
}

impl AgentMetrics {
    pub fn new() -> Self {
        Self::default()
    }

    // ------------------------------------------------------------------
    // Accumulators
    // ------------------------------------------------------------------

    /// Record wall-clock time spent inside an LLM call.
    pub fn record_llm_call(&mut self, duration: Duration, tokens_in: usize, tokens_out: usize) {
        self.llm_call_time += duration;
        self.llm_calls += 1;
        self.tokens_sent += tokens_in;
        self.tokens_received += tokens_out;
    }

    /// Record tool execution time.
    pub fn record_tool_call(&mut self, duration: Duration, success: bool) {
        self.tool_execution_time += duration;
        self.tool_calls += 1;
        if success {
            self.successful_operations += 1;
        } else {
            self.failed_operations += 1;
        }
    }

    /// Record a retried operation.
    pub fn record_retry(&mut self) {
        self.retried_operations += 1;
    }

    /// Register thinking time.
    pub fn record_thinking(&mut self, duration: Duration) {
        self.thinking_time += duration;
    }

    /// Update peak memory estimate.
    pub fn record_peak_memory(&mut self, mb: usize) {
        if mb > self.peak_memory_mb {
            self.peak_memory_mb = mb;
        }
    }

    /// Set the current context window size.
    pub fn set_context_window(&mut self, size: usize) {
        self.context_window_size = size;
    }

    // ------------------------------------------------------------------
    // Derived stats
    // ------------------------------------------------------------------

    /// Average LLM latency (ms).
    pub fn avg_llm_latency_ms(&self) -> f64 {
        if self.llm_calls == 0 {
            return 0.0;
        }
        self.llm_call_time.as_millis() as f64 / self.llm_calls as f64
    }

    /// Success ratio [0, 1].
    pub fn success_ratio(&self) -> f64 {
        let total = self.successful_operations + self.failed_operations;
        if total == 0 {
            return 1.0;
        }
        self.successful_operations as f64 / total as f64
    }

    /// Reset accumulated counters (IDs and totals stay).
    pub fn reset_counters(&mut self) {
        *self = Self::new();
    }
}

// ============================================================================
// PerformanceTracer — ad-hoc timing helper
// ============================================================================

/// Convenience wrapper that times an async closure and returns the result
/// together with the elapsed `Duration`.
pub struct PerformanceTracer;

impl PerformanceTracer {
    /// Execute an async closure `f`, returning `(T, Duration)`.
    ///
    /// # Example
    /// ```ignore
    /// let (result, elapsed) = PerformanceTracer::trace("llm_call", || async {
    ///     llm.complete("hello").await
    /// }).await;
    /// metrics.record_llm_call(elapsed, 10, 50);
    /// ```
    pub async fn trace<F, Fut, T>(_name: &str, f: F) -> (T, Duration)
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = T>,
    {
        let start = Instant::now();
        let result = f().await;
        let duration = start.elapsed();
        (result, duration)
    }

    /// Synchronous variant for non-async closures.
    pub fn trace_sync<F, T>(_name: &str, f: F) -> (T, Duration)
    where
        F: FnOnce() -> T,
    {
        let start = Instant::now();
        let result = f();
        let duration = start.elapsed();
        (result, duration)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_metrics() {
        let m = AgentMetrics::new();
        assert_eq!(m.llm_calls, 0);
        assert_eq!(m.failed_operations, 0);
        assert!((m.success_ratio() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_record_llm_call() {
        let mut m = AgentMetrics::new();
        m.record_llm_call(Duration::from_millis(200), 50, 100);
        assert_eq!(m.llm_calls, 1);
        assert_eq!(m.tokens_sent, 50);
        assert_eq!(m.tokens_received, 100);
        assert!((m.avg_llm_latency_ms() - 200.0).abs() < 0.01);
    }

    #[test]
    fn test_record_tool_success() {
        let mut m = AgentMetrics::new();
        m.record_tool_call(Duration::from_millis(10), true);
        m.record_tool_call(Duration::from_millis(20), true);
        m.record_tool_call(Duration::from_millis(30), false);
        assert_eq!(m.tool_calls, 3);
        assert_eq!(m.successful_operations, 2);
        assert_eq!(m.failed_operations, 1);
        assert!((m.success_ratio() - 2.0 / 3.0).abs() < 0.001);
    }

    #[test]
    fn test_record_retry() {
        let mut m = AgentMetrics::new();
        m.record_retry();
        m.record_retry();
        assert_eq!(m.retried_operations, 2);
    }

    #[test]
    fn test_peak_memory() {
        let mut m = AgentMetrics::new();
        m.record_peak_memory(100);
        m.record_peak_memory(200);
        m.record_peak_memory(150);
        assert_eq!(m.peak_memory_mb, 200);
    }

    #[test]
    fn test_avg_llm_latency_zero_calls() {
        let m = AgentMetrics::new();
        assert_eq!(m.avg_llm_latency_ms(), 0.0);
    }

    #[test]
    fn test_reset_counters() {
        let mut m = AgentMetrics::new();
        m.record_llm_call(Duration::from_secs(1), 10, 20);
        assert_eq!(m.llm_calls, 1);
        m.reset_counters();
        assert_eq!(m.llm_calls, 0);
    }

    #[tokio::test]
    async fn test_performance_tracer_async() {
        let (val, dur) = PerformanceTracer::trace("test", || async {
            tokio::time::sleep(Duration::from_millis(5)).await;
            42
        })
        .await;
        assert_eq!(val, 42);
        assert!(dur.as_millis() >= 4); // allow slight clock variance
    }

    #[test]
    fn test_performance_tracer_sync() {
        let (val, dur) = PerformanceTracer::trace_sync("test", || {
            let mut s = String::new();
            for _ in 0..1000 {
                s.push('a');
            }
            s
        });
        assert_eq!(val.len(), 1000);
        assert!(dur.as_nanos() > 0);
    }
}
