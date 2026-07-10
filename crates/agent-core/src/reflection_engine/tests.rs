use super::*;
use std::time::Duration;

#[test]
fn test_reflection_engine_creation() {
    let engine = ReflectionEngine::new();
    assert_eq!(engine.get_reflection_history().len(), 0);
    assert_eq!(engine.get_stats().total_reflections, 0);
}

#[test]
fn test_classify_transient_errors() {
    let engine = ReflectionEngine::new();

    let timeout = engine.classify_error("Operation timed out after 30s");
    assert!(matches!(timeout, ErrorClassification::Transient { .. }));

    let network = engine.classify_error("Connection refused: could not connect");
    assert!(matches!(network, ErrorClassification::Transient { .. }));

    let rate_limit = engine.classify_error("Rate limit exceeded, try again later");
    assert!(matches!(rate_limit, ErrorClassification::Transient { .. }));
}

#[test]
fn test_classify_permission_errors() {
    let engine = ReflectionEngine::new();

    let perm = engine.classify_error("Permission denied: requires admin role");
    assert!(matches!(
        perm,
        ErrorClassification::Permission {
            can_degrade: true,
            ..
        }
    ));
    assert!(perm.is_auto_recoverable());

    let unauthorized = engine.classify_error("Unauthorized access to resource");
    assert!(matches!(
        unauthorized,
        ErrorClassification::Permission { .. }
    ));
}

#[test]
fn test_classify_entity_state_errors() {
    let engine = ReflectionEngine::new();

    let not_found = engine.classify_error("Entity 'Player' not found");
    match not_found {
        ErrorClassification::EntityState { entity_name, .. } => {
            assert_eq!(entity_name, "Player");
        }
        other => unreachable!("Expected EntityState, got {:?}", other),
    }

    let exists = engine.classify_error("Entity 'Boss' already exists");
    match exists {
        ErrorClassification::EntityState { actual_state, .. } => {
            assert_eq!(actual_state, "already exists");
        }
        other => unreachable!("Expected EntityState, got {:?}", other),
    }
}

#[test]
fn test_classify_invalid_input_errors() {
    let engine = ReflectionEngine::new();

    let invalid = engine.classify_error("Invalid parameter 'color': must be RGBA array");
    match invalid {
        ErrorClassification::InvalidInput { parameter_name, .. } => {
            assert_eq!(parameter_name, "color");
        }
        other => unreachable!("Expected InvalidInput, got {:?}", other),
    }
}

#[test]
fn test_classify_fatal_errors() {
    let engine = ReflectionEngine::new();

    let fatal = engine.classify_error("Internal assertion failed: null pointer dereference");
    assert!(matches!(fatal, ErrorClassification::Fatal { .. }));
    assert!(!fatal.is_auto_recoverable());
}

#[test]
fn test_generate_reflection_for_timeout() {
    let engine = ReflectionEngine::new();
    let classification = ErrorClassification::Transient {
        retry_delay_ms: 500,
        max_retries: 3,
    };

    let reflection =
        engine.generate_reflection("call_api", "Request timed out after 30s", &classification);

    assert!(reflection.contains("What went wrong?"));
    assert!(reflection.contains("temporary"));
    assert!(reflection.contains("How to fix it?"));
    assert!(reflection.contains("retry"));
}

#[test]
fn test_generate_alternative_for_not_found() {
    let engine = ReflectionEngine::new();
    let classification = ErrorClassification::EntityState {
        entity_name: "Enemy".into(),
        expected_state: "exists".into(),
        actual_state: "not found".into(),
    };

    let alt = engine.generate_alternative_strategy(
        "delete_entity('Enemy')",
        "Entity 'Enemy' not found",
        &classification,
    );

    assert!(
        alt.is_some(),
        "Should generate alternative for not-found entity"
    );
    let alt_text = alt.unwrap();
    assert!(
        alt_text.contains("non-existent") || alt_text.contains("skip"),
        "Alternative should handle deletion of non-existent entity"
    );
}

#[test]
fn test_generate_alternative_for_already_exists() {
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

    assert!(
        alt.is_some(),
        "Should generate alternative for duplicate entity"
    );
    let alt_text = alt.unwrap();
    assert!(
        alt_text.contains("Modify") || alt_text.contains("modify"),
        "Alternative should change creation to modification"
    );
}

#[test]
fn test_retry_config_backoff_calculation() {
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
    assert_eq!(delay2, Duration::from_millis(400)); // Not yet capped
}

#[test]
fn test_retry_config_max_delay_cap() {
    let config = RetryConfig {
        max_retries: 10,
        initial_delay_ms: 100,
        backoff_multiplier: 10.0,
        max_delay_ms: 500,
        jitter: false,
    };

    let delay2 = config.calculate_delay(2); // 100 * 10^2 = 10000 → capped at 500
    assert_eq!(delay2, Duration::from_millis(500));
}

#[test]
fn test_reflection_entry_summary() {
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

    let summary = entry.summary();
    assert!(summary.contains("✅ RESOLVED"));
    assert!(summary.contains("create_player"));
}

#[test]
fn test_stats_tracking() {
    let mut engine = ReflectionEngine::new();

    // Simulate some reflections
    engine.successful_reflections = 8;
    engine.failed_reflections = 2;

    let stats = engine.get_stats();
    assert_eq!(stats.total_reflections, 10);
    assert_eq!(stats.successful, 8);
    assert_eq!(stats.failed, 2);
    assert!(
        (stats.success_rate - 0.8).abs() < 0.01,
        "Success rate should be ~80%"
    );
}

#[test]
fn test_extract_entity_from_error() {
    assert_eq!(
        helpers::extract_entity_from_error("Entity 'Player' not found"),
        "Player"
    );
    assert_eq!(
        helpers::extract_entity_from_error("Entity \"Boss\" already exists"),
        "Boss"
    );
    assert_eq!(
        helpers::extract_entity_from_error("No entity mentioned"),
        "unknown"
    );
}

#[test]
fn test_error_classification_descriptions() {
    let transient = ErrorClassification::Transient {
        retry_delay_ms: 100,
        max_retries: 3,
    };
    assert!(transient.describe().contains("Transient"));
    assert!(transient.describe().contains("retry"));

    let fatal = ErrorClassification::Fatal {
        reason: "Test error".into(),
    };
    assert!(fatal.describe().contains("Fatal"));

    let permission = ErrorClassification::Permission {
        required_privilege: "admin".into(),
        can_degrade: true,
    };
    assert!(permission.describe().contains("Permission"));
    assert!(permission.describe().contains("degrade"));
}
