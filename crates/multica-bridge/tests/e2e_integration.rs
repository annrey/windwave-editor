//! End-to-End Integration Tests
//!
//! 验证所有模块协同工作的集成测试
//! 包括场景管理、任务同步、记忆注入、消息处理等完整流程

use multica_bridge::memory_injector::{create_shared_memory_injector, MemoryInjectorConfig};
use multica_bridge::message_handler::{
    create_shared_message_handler_with_sync, DefaultMessageHandler, MessagePipeline,
};
use multica_bridge::scene_context::{ComponentData, SceneDiff, SceneEntity};
use multica_bridge::task_bridge::{BridgedTaskId, UnifiedTask, UnifiedTaskStatus};
use multica_bridge::task_sync_module::{
    create_shared_task_synchronizer, ConflictResolution, SyncDirection, TaskSyncConfig,
};
use multica_bridge::types::{Message, EVENT_DAEMON_HEARTBEAT};
use serde_json::json;
use std::collections::HashMap;

// ============================================================================
// Helper: set up a test scene with entities in the injector's MulticaDb
// ============================================================================

/// Helper to create a scene with two entities directly in the injector's DB.
/// Returns the scene_id.
fn setup_scene_in_injector(
    injector: &multica_bridge::memory_injector::MulticaMemoryInjector,
    scene_name: &str,
) -> String {
    let mut db = injector.db.lock().expect("mutex poisoned");
    let scene_id = db
        .create_scene(
            scene_name.to_string(),
            Some("Integration test scene".to_string()),
        )
        .unwrap();

    let mut props = HashMap::new();
    props.insert("x".to_string(), serde_json::json!(0.0));
    props.insert("y".to_string(), serde_json::json!(0.0));

    db.create_entity(
        scene_id.clone(),
        "Player".to_string(),
        "GameObject".to_string(),
        vec![ComponentData {
            type_name: "Transform".to_string(),
            properties: props,
        }],
        Some([0.0, 0.0, 0.0]),
    )
    .unwrap();

    db.create_entity(
        scene_id.clone(),
        "Enemy".to_string(),
        "GameObject".to_string(),
        vec![ComponentData {
            type_name: "Transform".to_string(),
            properties: {
                let mut m = HashMap::new();
                m.insert("x".to_string(), serde_json::json!(10.0));
                m
            },
        }],
        Some([10.0, 0.0, 0.0]),
    )
    .unwrap();

    scene_id
}

// ============================================================================
// Test 1: Scene memory workflow — create injector, inject a scene, verify
// ============================================================================

#[test]
fn test_e2e_scene_memory_workflow() {
    // 1. Create the memory injector (creates its own scene_memory + db internally)
    let config = MemoryInjectorConfig::default();
    let injector = create_shared_memory_injector(config);

    // 2. Set up a scene with entities in the injector's MulticaDb
    let scene_id = {
        let inj = injector.lock().unwrap();
        setup_scene_in_injector(&inj, "MainScene")
    };

    // 3. Inject the scene into memory
    let entries = {
        let mut inj = injector.lock().unwrap();
        inj.inject_scene(&scene_id).unwrap()
    };

    // 4. Verify entries were produced across all memory tiers
    assert!(!entries.is_empty(), "Should produce memory entries");

    let working_count = entries.iter().filter(|e| e.tier == "working").count();
    let episodic_count = entries.iter().filter(|e| e.tier == "episodic").count();
    let semantic_count = entries.iter().filter(|e| e.tier == "semantic").count();
    let procedural_count = entries.iter().filter(|e| e.tier == "procedural").count();

    assert_eq!(
        working_count, 2,
        "Should have 2 working entries (Player + Enemy)"
    );
    assert_eq!(
        episodic_count, 1,
        "Should have 1 episodic entry (scene load)"
    );
    assert!(semantic_count > 0, "Should have semantic entries");
    assert!(procedural_count > 0, "Should have procedural entries");

    // 5. Verify statistics
    let stats = injector.lock().unwrap().get_stats();
    assert_eq!(stats.injections_count, 1);
    assert_eq!(stats.errors_count, 0);
    assert!(stats.last_injection_time.is_some());
    assert_eq!(stats.entities_injected, entries.len() as u64);
}

// ============================================================================
// Test 2: Scene change detection — inject, then apply diff via inject_changes
// ============================================================================

#[test]
fn test_e2e_scene_change_triggers_memory_update() {
    // 1. Create injector and set up a scene
    let config = MemoryInjectorConfig::default();
    let injector = create_shared_memory_injector(config);

    let scene_id = {
        let inj = injector.lock().unwrap();
        setup_scene_in_injector(&inj, "ChangeScene")
    };

    // 2. Perform initial injection
    {
        let mut inj = injector.lock().unwrap();
        inj.inject_scene(&scene_id).unwrap();
    }

    // 3. Inject a scene change (diff-based) — entity added
    let diff = SceneDiff {
        added: vec![SceneEntity {
            id: 999,
            name: "NewObject".to_string(),
            components: vec![],
            position: Some([5.0, 5.0, 0.0]),
        }],
        removed: vec![],
        modified: vec![],
    };

    let change_entries = {
        let mut inj = injector.lock().unwrap();
        inj.inject_changes(&scene_id, &diff).unwrap()
    };

    // 4. Verify change entries
    assert_eq!(change_entries.len(), 1);
    assert_eq!(change_entries[0].tier, "episodic");
    assert!(change_entries[0].summary.contains("Entity created"));
    assert!(change_entries[0].summary.contains("NewObject"));

    // 5. Verify stats reflect the change
    let stats = injector.lock().unwrap().get_stats();
    assert_eq!(stats.changes_processed, 1);
    assert!(stats.injections_count >= 1);
}

// ============================================================================
// Test 3: Bidirectional task sync between local and remote tasks
// ============================================================================

#[test]
fn test_e2e_bidirectional_task_sync() {
    // 1. Create synchronizer with bidirectional + LastWriteWins
    let config = TaskSyncConfig {
        direction: SyncDirection::Bidirectional,
        conflict_resolution: ConflictResolution::LastWriteWins,
        ..TaskSyncConfig::default()
    };
    let synchronizer = create_shared_task_synchronizer(config);

    // 2. Add local task
    let local_task = UnifiedTask {
        id: BridgedTaskId {
            multica_id: Some(100),
            bridge_id: 1,
        },
        status: UnifiedTaskStatus::Running,
        title: "Local Task".to_string(),
        description: String::new(),
        scene_id: None,
        entity_ids: vec![],
        resource_ids: vec![],
        scene_snapshot: None,
        multica_task: None,
        created_at: "2024-01-01T00:00:00Z".to_string(),
        updated_at: "2024-01-01T00:00:00Z".to_string(),
    };
    synchronizer.add_local_task(local_task);

    // 3. Add remote task
    let remote_task = UnifiedTask {
        id: BridgedTaskId {
            multica_id: Some(200),
            bridge_id: 2,
        },
        status: UnifiedTaskStatus::Done,
        title: "Remote Task".to_string(),
        description: String::new(),
        scene_id: None,
        entity_ids: vec![],
        resource_ids: vec![],
        scene_snapshot: None,
        multica_task: None,
        created_at: "2024-01-01T00:01:00Z".to_string(),
        updated_at: "2024-01-01T00:01:00Z".to_string(),
    };
    synchronizer.add_remote_task(remote_task);

    // 4. Execute bidirectional sync
    let stats = synchronizer.sync().unwrap();
    assert_eq!(stats.total_syncs, 1);
    assert_eq!(stats.successful_syncs, 1);
    // Bidirectional sync: local->remote + remote->local = at least 2 tasks
    assert!(stats.synced_tasks_count >= 2);
}

// ============================================================================
// Test 4: Message handling integrated with task synchronizer
// ============================================================================

#[test]
fn test_e2e_message_handling_with_sync() {
    // 1. Create synchronizer
    let synchronizer = create_shared_task_synchronizer(TaskSyncConfig::default());

    // 2. Create message handler wired to the synchronizer
    let handler = create_shared_message_handler_with_sync(synchronizer);

    // 3. Process a task-created message
    let message = Message {
        message_type: "task:created".to_string(),
        payload: json!({
            "id": {"bridge_id": 1, "multica_id": 100},
            "title": "Synced Task",
            "description": "Test",
            "status": "running",
            "scene_id": null,
            "entity_ids": [],
            "resource_ids": [],
            "scene_snapshot": null,
            "multica_task": null,
            "created_at": "2024-01-01T00:00:00Z",
            "updated_at": "2024-01-01T00:00:00Z"
        }),
    };

    let result = handler.process_message(&message);
    assert!(result.success);
    assert_eq!(result.message_type, "task:created");

    // 4. Verify handler statistics
    let stats = handler.get_stats();
    assert_eq!(stats.total_processed, 1);
    assert_eq!(stats.successful, 1);
    assert_eq!(stats.failed, 0);
}

// ============================================================================
// Test 5: Scene lifecycle — inject scene, record changes, verify history
// ============================================================================

#[test]
fn test_e2e_scene_lifecycle_with_changes() {
    // 1. Create injector and set up a scene
    let config = MemoryInjectorConfig::default();
    let injector = create_shared_memory_injector(config);

    let scene_id = {
        let inj = injector.lock().unwrap();
        setup_scene_in_injector(&inj, "LifecycleScene")
    };

    // 2. Inject initial scene state
    {
        let mut inj = injector.lock().unwrap();
        let entries = inj.inject_scene(&scene_id).unwrap();
        assert!(!entries.is_empty());
    }

    // 3. Record a scene change directly in scene context memory
    let diff = SceneDiff {
        added: vec![SceneEntity {
            id: 100,
            name: "SpawnedEntity".to_string(),
            components: vec![],
            position: None,
        }],
        removed: vec![],
        modified: vec![],
    };

    {
        let inj = injector.lock().unwrap();
        let mut mem = inj.scene_memory.lock().unwrap();
        mem.record_scene_change(diff, scene_id.clone()).unwrap();
    }

    // 4. Verify change history was recorded
    {
        let inj = injector.lock().unwrap();
        let mem = inj.scene_memory.lock().unwrap();
        let changes = mem.get_change_history(&scene_id);
        assert_eq!(changes.len(), 1, "Should have 1 recorded change");
    }

    // 5. Verify injector stats after all operations
    let stats = injector.lock().unwrap().get_stats();
    assert_eq!(stats.injections_count, 1);
    assert_eq!(stats.errors_count, 0);
}

// ============================================================================
// Test 6: Multiple scenes workflow — inject all scenes
// ============================================================================

#[test]
fn test_e2e_multiple_scenes_workflow() {
    // 1. Create injector
    let config = MemoryInjectorConfig::default();
    let injector = create_shared_memory_injector(config);

    // 2. Set up multiple scenes in the DB
    {
        let inj = injector.lock().unwrap();
        setup_scene_in_injector(&inj, "Scene1");
        setup_scene_in_injector(&inj, "Scene2");
        setup_scene_in_injector(&inj, "Scene3");
    }

    // 3. Inject all scenes at once
    let all_entries = {
        let mut inj = injector.lock().unwrap();
        inj.inject_all_scenes().unwrap()
    };

    // 4. Verify entries were produced for all scenes
    assert!(
        !all_entries.is_empty(),
        "Should produce entries for multiple scenes"
    );

    // Count scenes by looking at unique scene_ids in episodic entries
    let scene_ids: std::collections::HashSet<&str> = all_entries
        .iter()
        .filter(|e| e.tier == "episodic")
        .map(|e| e.scene_id.as_str())
        .collect();
    assert!(
        scene_ids.len() >= 3,
        "Should have episodic entries for at least 3 scenes"
    );

    // 5. Verify statistics
    let stats = injector.lock().unwrap().get_stats();
    assert!(
        stats.injections_count >= 3,
        "Should have at least 3 injections"
    );
    assert_eq!(stats.errors_count, 0);
    assert!(stats.entities_injected > 0);
}

// ============================================================================
// Test 7: Task conflict resolution with LastWriteWins
// ============================================================================

#[test]
fn test_e2e_task_conflict_resolution() {
    // 1. Create synchronizer with LastWriteWins strategy
    let config = TaskSyncConfig {
        direction: SyncDirection::Bidirectional,
        conflict_resolution: ConflictResolution::LastWriteWins,
        ..TaskSyncConfig::default()
    };
    let synchronizer = create_shared_task_synchronizer(config);

    // 2. Add conflicting tasks (same bridge_id and multica_id, different status)
    let local_task = UnifiedTask {
        id: BridgedTaskId {
            multica_id: Some(100),
            bridge_id: 1,
        },
        status: UnifiedTaskStatus::Running,
        title: "Conflicted Task".to_string(),
        description: String::new(),
        scene_id: None,
        entity_ids: vec![],
        resource_ids: vec![],
        scene_snapshot: None,
        multica_task: None,
        created_at: "2024-01-01T00:00:00Z".to_string(),
        updated_at: "2024-01-01T00:00:00Z".to_string(),
    };

    let remote_task = UnifiedTask {
        id: BridgedTaskId {
            multica_id: Some(100),
            bridge_id: 1,
        },
        status: UnifiedTaskStatus::Done,
        title: "Conflicted Task".to_string(),
        description: String::new(),
        scene_id: None,
        entity_ids: vec![],
        resource_ids: vec![],
        scene_snapshot: None,
        multica_task: None,
        created_at: "2024-01-01T00:00:00Z".to_string(),
        updated_at: "2024-01-01T00:01:00Z".to_string(), // newer version wins
    };

    synchronizer.add_local_task(local_task);
    synchronizer.add_remote_task(remote_task);

    // 3. Sync — conflict should be resolved automatically with LastWriteWins
    let stats = synchronizer.sync().unwrap();
    assert_eq!(stats.successful_syncs, 1);
    // No conflicts recorded because LastWriteWins resolves them silently
    assert_eq!(synchronizer.get_stats().total_syncs, 1);
}

// ============================================================================
// Test 8: Manual conflict resolution
// ============================================================================

#[test]
fn test_e2e_manual_conflict_resolution() {
    // 1. Create synchronizer with Manual conflict resolution
    let config = TaskSyncConfig {
        direction: SyncDirection::Bidirectional,
        conflict_resolution: ConflictResolution::Manual,
        ..TaskSyncConfig::default()
    };
    let synchronizer = create_shared_task_synchronizer(config);

    // 2. Add conflicting tasks
    let local_task = UnifiedTask {
        id: BridgedTaskId {
            multica_id: Some(100),
            bridge_id: 1,
        },
        status: UnifiedTaskStatus::Failed,
        title: "Manual Conflict".to_string(),
        description: String::new(),
        scene_id: None,
        entity_ids: vec![],
        resource_ids: vec![],
        scene_snapshot: None,
        multica_task: None,
        created_at: "2024-01-01T00:00:00Z".to_string(),
        updated_at: "2024-01-01T00:00:00Z".to_string(),
    };

    let remote_task = UnifiedTask {
        id: BridgedTaskId {
            multica_id: Some(100),
            bridge_id: 1,
        },
        status: UnifiedTaskStatus::Cancelled,
        title: "Manual Conflict".to_string(),
        description: String::new(),
        scene_id: None,
        entity_ids: vec![],
        resource_ids: vec![],
        scene_snapshot: None,
        multica_task: None,
        created_at: "2024-01-01T00:00:00Z".to_string(),
        updated_at: "2024-01-01T00:01:00Z".to_string(),
    };

    synchronizer.add_local_task(local_task.clone());
    synchronizer.add_remote_task(remote_task.clone());

    // 3. Sync — conflicts should be recorded (bidirectional sync encounters conflict twice)
    let stats = synchronizer.sync().unwrap();
    assert_eq!(stats.successful_syncs, 1);

    // Manual conflict resolution records conflicts
    let final_stats = synchronizer.get_stats();
    assert!(
        final_stats.conflict_count >= 1,
        "Manual mode should record conflicts"
    );
}

// ============================================================================
// Test 9: Message pipeline with multiple handlers
// ============================================================================

#[test]
fn test_e2e_message_pipeline() {
    // 1. Create message processing pipeline
    let mut pipeline = MessagePipeline::new();

    // 2. Add multiple handlers to the pipeline
    let synchronizer = create_shared_task_synchronizer(TaskSyncConfig::default());
    pipeline.add_handler(Box::new(DefaultMessageHandler::with_task_synchronizer(
        synchronizer.clone(),
    )));
    pipeline.add_handler(Box::new(DefaultMessageHandler::new()));

    // 3. Send a heartbeat message through the pipeline
    let message = Message {
        message_type: EVENT_DAEMON_HEARTBEAT.to_string(),
        payload: json!({
            "runtime_id": "test-runtime",
            "supports_batch_import": false
        }),
    };

    let results = pipeline.process_message(&message);
    assert_eq!(results.len(), 2, "Should process through both handlers");
    assert!(
        results.iter().all(|r| r.success),
        "All handlers should succeed"
    );

    // 4. Verify pipeline statistics
    let stats = pipeline.get_stats();
    assert_eq!(stats.total_processed, 2);
    assert_eq!(stats.successful, 2);
    assert_eq!(stats.failed, 0);
}

// ============================================================================
// Test 10: Full integration scenario — all components working together
// ============================================================================

#[test]
fn test_e2e_full_integration_scenario() {
    // A complete integration test simulating a real workflow:
    // scene setup -> memory injection -> message handling -> task sync -> scene changes

    // Phase 1: Create memory injector and set up scene data
    let injector_config = MemoryInjectorConfig::default();
    let injector = create_shared_memory_injector(injector_config);

    let scene_id = {
        let inj = injector.lock().unwrap();
        setup_scene_in_injector(&inj, "MainScene")
    };

    // Phase 2: Inject main scene into memory
    {
        let mut inj = injector.lock().unwrap();
        let entries = inj.inject_scene(&scene_id).unwrap();
        assert!(
            !entries.is_empty(),
            "Scene injection should produce entries"
        );
    }

    // Phase 3: Create task synchronizer
    let sync_config = TaskSyncConfig {
        direction: SyncDirection::Bidirectional,
        ..TaskSyncConfig::default()
    };
    let synchronizer = create_shared_task_synchronizer(sync_config);

    // Phase 4: Create message handler wired to synchronizer
    let handler = create_shared_message_handler_with_sync(synchronizer.clone());

    // Phase 5: Process multiple messages
    let messages = vec![
        Message {
            message_type: "task:created".to_string(),
            payload: json!({
                "id": {"bridge_id": 1, "multica_id": 100},
                "title": "Task 1",
                "description": "Description 1",
                "status": "pending",
                "scene_id": scene_id,
                "entity_ids": [1, 2],
                "resource_ids": [],
                "scene_snapshot": null,
                "multica_task": null,
                "created_at": "2024-01-01T00:00:00Z",
                "updated_at": "2024-01-01T00:00:00Z"
            }),
        },
        Message {
            message_type: EVENT_DAEMON_HEARTBEAT.to_string(),
            payload: json!({"runtime_id": "runtime-1"}),
        },
        Message {
            message_type: "task:updated".to_string(),
            payload: json!({
                "id": {"bridge_id": 1, "multica_id": 100},
                "title": "Task 1 Updated",
                "status": "running"
            }),
        },
    ];

    for message in &messages {
        let result = handler.process_message(message);
        assert!(
            result.success,
            "Message handling failed for type: {}",
            message.message_type
        );
    }

    // Phase 6: Execute task synchronization
    let sync_stats = synchronizer.sync().unwrap();
    assert_eq!(sync_stats.successful_syncs, 1);

    // Phase 7: Apply a scene change and inject it
    let diff = SceneDiff {
        added: vec![SceneEntity {
            id: 300,
            name: "LateEntity".to_string(),
            components: vec![],
            position: Some([15.0, 0.0, 0.0]),
        }],
        removed: vec![],
        modified: vec![],
    };

    {
        let mut inj = injector.lock().unwrap();
        let change_entries = inj.inject_changes(&scene_id, &diff).unwrap();
        assert_eq!(change_entries.len(), 1);
    }

    // Phase 8: Final verification of all stats
    let memory_stats = injector.lock().unwrap().get_stats();
    // 1 from inject_scene + 1 from inject_changes
    assert_eq!(memory_stats.injections_count, 2);
    assert_eq!(memory_stats.changes_processed, 1);
    assert_eq!(memory_stats.errors_count, 0);

    let handler_stats = handler.get_stats();
    assert_eq!(handler_stats.total_processed, 3);
    assert_eq!(handler_stats.successful, 3);
    assert_eq!(handler_stats.failed, 0);

    let final_sync_stats = synchronizer.get_stats();
    assert_eq!(final_sync_stats.total_syncs, 1);
    assert_eq!(final_sync_stats.successful_syncs, 1);
}
