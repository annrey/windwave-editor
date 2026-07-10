use super::*;

#[test]
fn test_build_health_evaluator() {
    let temp_dir = std::env::temp_dir().join("agentedit_bench_test");
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();

    // Create a minimal Cargo.toml
    std::fs::write(
        temp_dir.join("Cargo.toml"),
        r#"
[package]
name = "test"
version = "0.1.0"
edition = "2021"

[dependencies]
"#,
    )
    .unwrap();

    std::fs::create_dir_all(temp_dir.join("src")).unwrap();
    std::fs::write(temp_dir.join("src/main.rs"), "fn main() {}").unwrap();

    let evaluator = BuildHealthEvaluator::new();
    let result = evaluator.evaluate(&temp_dir, "");

    assert!(result.is_ok());
    let score = result.unwrap();
    assert!(score.build_health.compilation_success);

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_visual_evaluator() {
    let temp_dir = std::env::temp_dir().join("agentedit_visual_test");
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();

    std::fs::create_dir_all(temp_dir.join("src")).unwrap();
    std::fs::write(
        temp_dir.join("src/main.rs"),
        r#"
fn main() {
    // Camera3dBundle with Transform
    // PointLight
}
"#,
    )
    .unwrap();

    let evaluator = VisualUsabilityEvaluator::new();
    let result = evaluator.evaluate(&temp_dir, "");

    assert!(result.is_ok());
    let score = result.unwrap();
    assert!(score.visual_usability.camera_position_valid);

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_bench_runner() {
    let temp_dir = std::env::temp_dir().join("agentedit_runner_test");
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();

    let results_dir = temp_dir.join("results");
    let runner = BenchRunner::new(&results_dir);

    assert!(!runner.evaluators.is_empty());

    let _ = std::fs::remove_dir_all(&temp_dir);
}
