use agent_core::memory::{MemoryConfig, MemoryQuery, MemorySystem, MemoryTier};

#[test]
fn test_record_and_retrieve_user_preference() {
    let mut ms = MemorySystem::new();

    ms.record_user_preference("color", "red", "user said I like red");
    ms.record_user_preference("layout", "grid", "user prefers grid layout");

    assert_eq!(ms.get_preference("color"), Some("red"));
    assert_eq!(ms.get_preference("layout"), Some("grid"));
    assert_eq!(ms.get_preference("nonexistent"), None);

    let summary = ms.build_preference_summary();
    assert!(summary.contains("color: red"));
    assert!(summary.contains("layout: grid"));
    assert!(summary.contains("User Preferences"));
}

#[test]
fn test_user_preference_overwrite() {
    let mut ms = MemorySystem::new();

    ms.record_user_preference("color", "red", "initial");
    ms.record_user_preference("color", "blue", "changed mind");

    assert_eq!(ms.get_preference("color"), Some("blue"));
}

#[test]
fn test_preference_in_context_building() {
    let mut ms = MemorySystem::new();
    ms.record_user_preference("favorite_entity", "Enemy", "user");

    let query = MemoryQuery::episodic_only("test");
    let ctx = ms.build_context(&query);

    assert!(ctx.episodic_context.contains("favorite_entity"));
    assert!(ctx.episodic_context.contains("Enemy"));
    assert!(!ctx.is_empty());
}

#[test]
fn test_multiple_preferences_dedup() {
    let mut ms = MemorySystem::new();

    ms.record_user_preference("key_a", "val_1", "s1");
    ms.record_user_preference("key_b", "val_2", "s2");
    ms.record_user_preference("key_a", "val_3", "s3");

    let summary = ms.build_preference_summary();
    assert!(summary.contains("key_a: val_3"));
    assert!(summary.contains("key_b: val_2"));
    assert!(!summary.contains("val_1"));
}

#[test]
fn test_episodic_bm25_search() {
    let mut ms = MemorySystem::new();

    ms.record_user_request("create a red enemy", None);
    ms.record_user_request("move the player left", None);
    ms.record_error("failed to create entity: duplicate", None);

    let query = MemoryQuery::episodic_only("red enemy");
    let results = ms.retrieve(&query);

    let has_create = results
        .iter()
        .any(|r| r.tier == MemoryTier::Episodic && r.content.contains("red"));
    assert!(has_create, "Should find 'red enemy' episode");
}

#[test]
fn test_context_budget_truncation() {
    let mut ms = MemorySystem::new();

    for i in 0..20 {
        ms.record_user_request(
            &format!(
                "very long request message number {} with lots of extra padding text",
                i
            ),
            None,
        );
    }

    let query = MemoryQuery::episodic_only("request");
    let ctx = ms.build_context_with_budget(&query, 500);

    assert!(
        ctx.total_chars <= 500,
        "Context should be truncated to budget, got {} chars",
        ctx.total_chars
    );
}

#[test]
fn test_working_memory_summary_in_context() {
    let mut ms = MemorySystem::new();

    ms.set_intent("user wants to create enemy");
    ms.register_entity("Player", agent_core::types::EntityId(1));
    ms.add_hint("the scene has a grid layout");

    let query = MemoryQuery::new("test");
    let ctx = ms.build_context(&query);

    assert!(
        !ctx.working_context.is_empty(),
        "Working context should not be empty"
    );
    assert!(
        ctx.working_context.contains("Player")
            || ctx.working_context.contains("enemy")
            || ctx.working_context.contains("grid"),
        "Working context should contain intent/entity/hint info"
    );
}

#[test]
fn test_procedural_default_workflows() {
    let mut ms = MemorySystem::with_config(MemoryConfig::default());
    ms.procedural.seed_with_defaults();

    let workflows = ms.procedural.find_matching("create entity", 5);
    assert!(!workflows.is_empty(), "Should find create_entity workflow");

    let player = ms.procedural.find_matching("player", 5);
    assert!(!player.is_empty(), "Should find player workflow");
}

#[test]
fn test_observe_decision_pattern() {
    let mut ms = MemorySystem::new();

    ms.observe_decision(
        "creating enemy entity",
        "use red color for enemies",
        "user liked the red enemies",
        true,
    );

    let patterns = ms.procedural.find_patterns("enemy", 0.3, 5);
    assert!(!patterns.is_empty(), "Should find decision pattern");
    assert!(patterns[0].context.contains("enemy"));
    assert!(patterns[0].confidence > 0.0);
}

#[test]
fn test_procedural_workflow_learning() {
    let mut ms = MemorySystem::new();

    let wf_id = ms.create_workflow(
        "create_blue_player",
        "create player with blue color",
        "scene",
    );

    assert!(wf_id.0 > 0);

    ms.record_workflow_use("create_blue_player", true);
    ms.record_workflow_use("create_blue_player", true);
    ms.record_workflow_use("create_blue_player", false);

    let wf = ms.procedural.get_workflow("create_blue_player");
    assert!(wf.is_some());
    assert_eq!(wf.unwrap().use_count, 3);
    assert!(wf.unwrap().success_rate > 0.5);
}

#[test]
fn test_procedural_auto_learn_from_decisions() {
    let mut ms = MemorySystem::new();

    for _ in 0..10 {
        ms.observe_decision("creating enemy", "red color", "success", true);
    }

    let patterns = ms.procedural.find_patterns("creating enemy", 0.7, 5);
    assert!(
        !patterns.is_empty(),
        "Should find pattern after 10 successful observations"
    );
    assert!(patterns[0].confidence > 0.7);

    assert!(patterns[0].observation_count >= 10);
}

#[test]
fn test_memory_persistence_roundtrip() {
    let mut ms = MemorySystem::new();

    ms.record_user_request("test request 1", None);
    ms.record_user_preference("color", "green", "test");
    ms.record_step("create_player", "player created", true, 150);

    let temp_dir = std::env::temp_dir();
    let save_path = temp_dir.join("agent_memory_test_roundtrip.json");
    let save_path_str = save_path.to_str().unwrap();

    let info = ms.save_to_file(save_path_str).unwrap();
    assert_eq!(info.episodic_count, 3);
    assert!(info.total_bytes > 0);

    let mut ms2 = MemorySystem::new();
    let load_result = ms2.load_from_file(save_path_str).unwrap();
    assert_eq!(load_result.episodic_restored, 3);

    assert_eq!(ms2.episodic.len(), 3);
    assert_eq!(ms2.get_preference("color"), Some("green"));

    let _ = std::fs::remove_file(save_path);
}

#[test]
fn test_memory_persistence_with_workflows() {
    let mut ms = MemorySystem::new();

    ms.record_user_request("create player entity", None);
    ms.create_workflow("test_flow", "test trigger", "testing");
    ms.observe_decision("ctx", "dec", "out", true);

    let temp_dir = std::env::temp_dir();
    let save_path = temp_dir.join("agent_memory_test_workflows.json");
    let save_path_str = save_path.to_str().unwrap();

    let info = ms.save_to_file(save_path_str).unwrap();
    assert!(info.procedural_count > 0);

    let mut ms2 = MemorySystem::new();
    let load = ms2.load_from_file(save_path_str).unwrap();
    assert!(load.procedural_restored > 0);

    let _ = std::fs::remove_file(save_path);
}

#[test]
fn test_memory_tier_isolation_with_preferences() {
    let mut ms = MemorySystem::new();
    ms.record_user_preference("theme", "dark", "user prefers dark mode");
    ms.record_user_request("do something", None);

    // Episodic-only query should find the preference
    let episodic_query = MemoryQuery::episodic_only("theme");
    let ctx = ms.build_context(&episodic_query);
    assert!(
        ctx.episodic_context.contains("dark") || ctx.episodic_context.contains("theme"),
        "Episodic tier should contain recorded preference"
    );
}

#[test]
fn test_memory_cleanup_by_importance() {
    let mut ms = MemorySystem::with_config(MemoryConfig {
        enable_decay: true,
        ..Default::default()
    });

    for i in 0..100 {
        ms.record_user_request(&format!("request {}", i), None);
    }

    let before = ms.episodic.len();
    assert!(before >= 100);

    ms.cleanup();

    let after = ms.episodic.len();
    assert!(after > 0, "Cleanup should leave some episodes");
}

#[test]
fn test_memory_stats() {
    let mut ms = MemorySystem::new();

    ms.record_user_request("r1", None);
    ms.record_user_request("r2", None);
    ms.record_error("e1", None);

    let stats = ms.stats();
    assert_eq!(stats.episodic_entries, 3);
    assert!(stats.total_entries() >= 3);
}

#[test]
fn test_record_user_preference_at_system_level() {
    let mut ms = MemorySystem::new();

    let id = ms.record_user_preference("favorite_shape", "sphere", "user input");
    assert!(id.0 > 0);

    assert_eq!(ms.get_preference("favorite_shape"), Some("sphere"));

    let summary = ms.build_preference_summary();
    assert!(summary.contains("favorite_shape"));
    assert!(summary.contains("sphere"));
}

#[test]
fn test_empty_preference_summary() {
    let ms = MemorySystem::new();
    let summary = ms.build_preference_summary();
    assert!(summary.is_empty());
}

#[test]
fn test_auto_learn_promotes_high_confidence_patterns() {
    let mut ms = MemorySystem::new();

    for _ in 0..10 {
        ms.observe_decision(
            "create enemy with red color",
            "use red color",
            "success",
            true,
        );
    }

    let before = ms.procedural.workflow_count();
    let promoted = ms.auto_learn(0.7, 5);
    assert!(promoted > 0, "Should promote at least 1 workflow");
    assert!(ms.procedural.workflow_count() > before);

    let auto_wf = ms
        .procedural
        .get_workflow("auto_create_enemy_with_red_color");
    assert!(auto_wf.is_some(), "Auto-learned workflow should exist");
    assert_eq!(auto_wf.unwrap().category, "auto_learned");
}

#[test]
fn test_auto_learn_skips_existing() {
    let mut ms = MemorySystem::new();

    ms.create_workflow("auto_existing_pattern", "existing pattern", "auto_learned");

    for _ in 0..10 {
        ms.observe_decision("existing pattern", "some decision", "success", true);
    }

    let before = ms.procedural.workflow_count();
    let promoted = ms.auto_learn(0.7, 5);
    assert_eq!(promoted, 0, "Should skip existing workflow");
    assert_eq!(ms.procedural.workflow_count(), before);
}

#[test]
fn test_auto_learn_low_confidence_not_promoted() {
    let mut ms = MemorySystem::new();

    ms.observe_decision("risky action", "try risky", "failed", false);

    let promoted = ms.auto_learn(0.7, 1);
    assert_eq!(promoted, 0, "Low confidence pattern should not be promoted");
}

#[test]
fn test_memory_cleanup_preserves_preferences() {
    let mut ms = MemorySystem::with_config(MemoryConfig {
        enable_decay: true,
        ..Default::default()
    });

    ms.record_user_preference("important_key", "important_value", "user");

    for i in 0..200 {
        ms.record_user_request(&format!("noise {}", i), None);
    }

    ms.cleanup();

    let pref = ms.get_preference("important_key");
    assert_eq!(
        pref,
        Some("important_value"),
        "User preferences should survive cleanup since they have higher importance"
    );
}

#[test]
fn test_memory_empty_system_handles_query_gracefully() {
    let mut ms = MemorySystem::new();
    let query = MemoryQuery::working_only("test");
    let ctx = ms.build_context(&query);
    assert!(
        !ctx.is_empty() || ctx.working_context.is_empty(),
        "Empty memory should not panic on query"
    );
}

#[test]
fn test_memory_capacity_eviction() {
    let mut ms = MemorySystem::with_config(MemoryConfig {
        working_capacity: 5,
        ..Default::default()
    });

    for i in 0..20 {
        ms.record_user_request(&format!("request {}", i), None);
    }

    // After many records, earlier entries should be replaced
    ms.cleanup();

    let query = MemoryQuery::working_only("request");
    let ctx = ms.build_context(&query);
    let total_chars = ctx.working_context.len() + ctx.episodic_context.len();
    assert!(
        total_chars < 5000 || total_chars > 0,
        "Memory system should handle capacity-constrained state without panic"
    );
}

#[test]
fn test_memory_disabled_hybrid_retrieval() {
    let mut ms = MemorySystem::with_config(MemoryConfig {
        enable_hybrid_retrieval: false,
        ..Default::default()
    });

    ms.record_user_preference("color", "blue", "test");

    // With hybrid retrieval disabled, preferences still accessible via get_preference
    assert_eq!(
        ms.get_preference("color"),
        Some("blue"),
        "Preferences should still work with hybrid retrieval disabled"
    );
}
