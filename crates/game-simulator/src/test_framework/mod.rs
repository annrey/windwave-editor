//! Test framework for game simulation

use crate::core::simulator::GameSimulator;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

/// Result of a test
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestResult {
    pub passed: bool,
    pub message: String,
    pub details: serde_json::Value,
}

impl TestResult {
    pub fn pass(message: &str) -> Self {
        Self {
            passed: true,
            message: message.to_string(),
            details: serde_json::json!({}),
        }
    }

    pub fn fail(message: &str) -> Self {
        Self {
            passed: false,
            message: message.to_string(),
            details: serde_json::json!({}),
        }
    }
}

/// Test runner
pub struct TestRunner {
    results: HashMap<String, TestResult>,
}

impl TestRunner {
    pub fn new() -> Self {
        Self {
            results: HashMap::new(),
        }
    }

    pub fn run_test(
        &mut self,
        name: &str,
        setup: impl Fn(&mut GameSimulator),
        check: impl Fn(&GameSimulator) -> TestResult,
    ) -> &TestResult {
        let mut sim = GameSimulator::new();
        setup(&mut sim);
        let result = check(&sim);
        self.results.insert(name.to_string(), result);
        self.results.get(name).unwrap()
    }

    pub fn results(&self) -> &HashMap<String, TestResult> {
        &self.results
    }

    pub fn summary(&self) -> TestSummary {
        let passed = self.results.values().filter(|r| r.passed).count();
        let failed = self.results.values().filter(|r| !r.passed).count();

        TestSummary {
            total: self.results.len(),
            passed,
            failed,
        }
    }
}

impl Default for TestRunner {
    fn default() -> Self {
        Self::new()
    }
}

/// Summary of test results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestSummary {
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
}

impl fmt::Display for TestSummary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.total == 0 {
            return write!(f, "Tests: 0 passed, 0 failed, 0 total (0.0% success)");
        }
        write!(
            f,
            "Tests: {} passed, {} failed, {} total ({:.1}% success)",
            self.passed,
            self.failed,
            self.total,
            (self.passed as f64 / self.total as f64 * 100.0)
        )
    }
}

/// Benchmark result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkResult {
    pub name: String,
    pub frame_count: u64,
    pub avg_frame_time_ms: f64,
    pub total_time_ms: f64,
    pub fps: f64,
}

/// Run a benchmark
pub fn run_benchmark(name: &str, mut sim: GameSimulator, frames: u64) -> BenchmarkResult {
    let start = std::time::Instant::now();

    sim.run_for(frames);

    let total_time = start.elapsed().as_secs_f64() * 1000.0;
    let avg_frame_time = total_time / frames as f64;
    let fps = frames as f64 / (total_time / 1000.0);

    BenchmarkResult {
        name: name.to_string(),
        frame_count: frames,
        avg_frame_time_ms: avg_frame_time,
        total_time_ms: total_time,
        fps,
    }
}
