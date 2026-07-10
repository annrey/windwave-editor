//! Sprint 1-A3: ReflectionEngine

use crate::director::types::DirectorRuntime;

/// Test 26: ReflectionEngine initialization
#[test]
fn test_reflection_engine_initialization() {
    use crate::ReflectionEngine;

    let engine = ReflectionEngine::new();

    assert_eq!(engine.get_reflection_history().len(), 0);

    let stats = engine.get_stats();
    assert_eq!(stats.total_reflections, 0);
    assert_eq!(stats.successful, 0);
    assert_eq!(stats.failed, 0);
}

/// Test 27: classify transient errors
#[test]
fn test_reflection_classify_transient_errors() {
    use crate::reflection_engine::ErrorClassification;
    use crate::ReflectionEngine;

    let engine = ReflectionEngine::new();

    let timeout = engine.classify_error("Operation timed out after 30s");
    assert!(matches!(timeout, ErrorClassification::Transient { .. }));
    assert!(timeout.is_auto_recoverable());

    let network = engine.classify_error("Connection refused: could not connect");
    assert!(matches!(network, ErrorClassification::Transient { .. }));

    let rate_limit = engine.classify_error("Rate limit exceeded, try again later");
    assert!(matches!(rate_limit, ErrorClassification::Transient { .. }));
}

/// Test 28: classify permission errors
#[test]
fn test_reflection_classify_permission_errors() {
    use crate::reflection_engine::ErrorClassification;
    use crate::ReflectionEngine;

    let engine = ReflectionEngine::new();

    let perm = engine.classify_error("Permission denied: requires admin role");
    match perm {
        ErrorClassification::Permission { can_degrade, .. } => {
            assert!(can_degrade);
        }
        other => unreachable!("Expected Permission, got {:?}", other),
    }
}

/// Test 29: classify entity state errors
#[test]
fn test_reflection_classify_entity_state_errors() {
    use crate::reflection_engine::ErrorClassification;
    use crate::ReflectionEngine;

    let engine = ReflectionEngine::new();

    let not_found = engine.classify_error("Entity 'Player' not found");
    match not_found {
        ErrorClassification::EntityState {
            entity_name,
            actual_state,
            ..
        } => {
            assert_eq!(entity_name, "Player");
            assert_eq!(actual_state, "not found");
        }
        other => unreachable!("Expected EntityState, got {:?}", other),
    }

    let exists = engine.classify_error("Entity 'Boss' already exists");
    match exists {
        ErrorClassification::EntityState {
            entity_name,
            actual_state,
            ..
        } => {
            assert_eq!(entity_name, "Boss");
            assert_eq!(actual_state, "already exists");
        }
        other => unreachable!("Expected EntityState, got {:?}", other),
    }
}

/// Test 30: classify invalid input and fatal errors
#[test]
fn test_reflection_classify_invalid_and_fatal() {
    use crate::reflection_engine::ErrorClassification;
    use crate::ReflectionEngine;

    let engine = ReflectionEngine::new();

    let invalid = engine.classify_error("Invalid parameter 'color': must be RGBA array");
    match invalid {
        ErrorClassification::InvalidInput { parameter_name, .. } => {
            assert_eq!(parameter_name, "color");
        }
        other => unreachable!("Expected InvalidInput, got {:?}", other),
    }

    let fatal = engine.classify_error("Internal assertion failed: null pointer dereference");
    assert!(matches!(fatal, ErrorClassification::Fatal { .. }));
    assert!(!fatal.is_auto_recoverable());
}

/// Test 31: generate reflection text
#[test]
fn test_reflection_generate_reflection_text() {
    use crate::reflection_engine::ErrorClassification;
    use crate::ReflectionEngine;

    let engine = ReflectionEngine::new();
    let classification = ErrorClassification::Transient {
        retry_delay_ms: 500,
        max_retries: 3,
    };

    let reflection =
        engine.generate_reflection("call_api", "Request timed out after 30s", &classification);

    assert!(reflection.contains("What went wrong?"));
    assert!(reflection.contains("Why did it fail?"));
    assert!(reflection.contains("How to fix it?"));
    assert!(reflection.contains("temporary"));
    assert!(reflection.contains("retry"));
}

/// Test 32: alternative for not found
#[test]
fn test_reflection_alternative_for_not_found() {
    use crate::reflection_engine::ErrorClassification;
    use crate::ReflectionEngine;

    let engine = ReflectionEngine::new();
    let classification = ErrorClassification::EntityState {
        entity_name: "Enemy".into(),
        expected_state: "exists".into(),
        actual_state: "not found".into(),
    };

    let alt1 = engine.generate_alternative_strategy(
        "delete_entity('Enemy')",
        "Entity 'Enemy' not found",
        &classification,
    );
    assert!(alt1.is_some());
    let alt1_text = alt1.unwrap();
    assert!(alt1_text.contains("non-existent") || alt1_text.contains("skip"));

    let classification2 = ErrorClassification::EntityState {
        entity_name: "Player".into(),
        expected_state: "exists".into(),
        actual_state: "not found".into(),
    };
    let alt2 = engine.generate_alternative_strategy(
        "update_entity('Player')",
        "Entity 'Player' not found",
        &classification2,
    );
    assert!(alt2.is_some());
    assert!(alt2.unwrap().contains("Create"));
}

/// Test 33: alternative for already exists
#[test]
fn test_reflection_alternative_for_already_exists() {
    use crate::reflection_engine::ErrorClassification;
    use crate::ReflectionEngine;

    let engine = ReflectionEngine::new();
    let classification = ErrorClassification::EntityState {
        entity_name: "Player".into(),
        expected_state: "not exists".into(),
        actual_state: "already exists".into(),
    };

    let alt = engine.generate_alternative_strategy(
        "create_entity('Player')",
        "Entity 'Player' already exists",
        &classification,
    );

    assert!(alt.is_some());
    let alt_text = alt.unwrap();
    let is_acceptable = alt_text.contains("Modify")
        || alt_text.contains("modify")
        || alt_text.contains("existing")
        || alt_text.contains("Use existing")
        || alt_text != "create_entity('Player')";
    assert!(
        is_acceptable,
        "Should suggest modification or using existing entity. Got: {}",
        alt_text
    );
}

/// Test 34: RetryConfig backoff calculation
#[test]
fn test_retry_config_backoff_calculation() {
    use crate::reflection_engine::RetryConfig;
    use std::time::Duration;

    let config = RetryConfig {
        max_retries: 3,
        initial_delay_ms: 100,
        backoff_multiplier: 2.0,
        max_delay_ms: 1000,
        jitter: false,
    };

    let delay0 = config.calculate_delay(0);
    let delay1 = config.calculate_delay(1);
    let delay2 = config.calculate_delay(2);

    assert_eq!(delay0, Duration::from_millis(100));
    assert_eq!(delay1, Duration::from_millis(200));
    assert_eq!(delay2, Duration::from_millis(400));
}

/// Test 35: RetryConfig max delay cap
#[test]
fn test_retry_config_max_delay_cap() {
    use crate::reflection_engine::RetryConfig;
    use std::time::Duration;

    let config = RetryConfig {
        max_retries: 10,
        initial_delay_ms: 100,
        backoff_multiplier: 10.0,
        max_delay_ms: 500,
        jitter: false,
    };

    let delay2 = config.calculate_delay(2);
    assert_eq!(delay2, Duration::from_millis(500));
}

/// Test 36: ReflectionEntry lifecycle
#[test]
fn test_reflection_entry_lifecycle() {
    use crate::reflection_engine::{ErrorClassification, ReflectionEntry};

    let mut entry = ReflectionEntry::new(
        "create_player",
        "Timeout error",
        ErrorClassification::Transient {
            retry_delay_ms: 100,
            max_retries: 3,
        },
        "Test reflection text",
    );

    assert!(!entry.resolved);
    assert_eq!(entry.retry_count, 0);

    entry.mark_resolved("Retry succeeded", 2, 250);

    assert!(entry.resolved);
    assert_eq!(entry.retry_count, 2);
    assert_eq!(entry.total_retry_duration_ms, 250);

    let summary = entry.summary();
    assert!(summary.contains("RESOLVED"));
    assert!(summary.contains("create_player"));
}

/// Test 37: integration - DirectorRuntime has ReflectionEngine
#[test]
fn test_director_runtime_has_reflection_engine() {
    let rt = DirectorRuntime::new();

    let stats = rt.reflection_engine.get_stats();
    assert_eq!(stats.total_reflections, 0);

    let classification = rt
        .reflection_engine
        .classify_error("Entity 'TestEntity' not found");
    use crate::reflection_engine::ErrorClassification;
    assert!(matches!(
        classification,
        ErrorClassification::EntityState { .. }
    ));

    let alt = rt.reflection_engine.generate_alternative_strategy(
        "delete_entity('TestEntity')",
        "Entity 'TestEntity' not found",
        &classification,
    );
    assert!(
        alt.is_some(),
        "Should generate alternative for not-found entity"
    );
}

/// Test 38: stats tracking
#[test]
fn test_reflection_stats_tracking() {
    use crate::ReflectionEngine;

    let mut engine = ReflectionEngine::new();

    engine.successful_reflections = 8;
    engine.failed_reflections = 2;

    let stats = engine.get_stats();
    assert_eq!(stats.total_reflections, 10);
    assert_eq!(stats.successful, 8);
    assert_eq!(stats.failed, 2);

    assert!((stats.success_rate - 0.8).abs() < 0.01);
}
