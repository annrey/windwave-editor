//! Deep Integration Example
//!
//! This example demonstrates the full integration of:
//! - Scene context for entity management
//! - Game skill bridge for executing operations
//! - Scene event bus for notifications
//! - Task bridge for unified task management

use multica_bridge::*;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

/// Custom scene event subscriber for testing
struct ExampleSubscriber {
    event_count: AtomicUsize,
    name: String,
}

impl ExampleSubscriber {
    fn new(name: &str) -> Self {
        Self {
            event_count: AtomicUsize::new(0),
            name: name.to_string(),
        }
    }
}

impl SceneEventSubscriber for ExampleSubscriber {
    fn on_event(&self, event: &SceneEvent) {
        self.event_count.fetch_add(1, Ordering::SeqCst);
        println!(
            "[{}] Received event: {:?} for entity {}",
            self.name, event.event_type, event.entity_id
        );
    }
}

fn main() -> Result<()> {
    println!("=== WindWave + Multica Deep Integration Example ===\n");

    // 1. Initialize scene context
    println!("1. Initializing scene context...");
    let scene_context = create_shared_scene_context();
    println!("   ✓ Scene context created\n");

    // 2. Initialize event bus and subscribe
    println!("2. Setting up event bus...");
    let event_bus = create_shared_event_bus();

    let subscriber1 = Arc::new(ExampleSubscriber::new("GameEngine"));
    let subscriber2 = Arc::new(ExampleSubscriber::new("DebugMonitor"));

    let _id1 = event_bus.lock().unwrap().subscribe(subscriber1.clone());
    let _id2 = event_bus.lock().unwrap().subscribe(subscriber2.clone());

    println!(
        "   ✓ Event bus initialized with {} subscribers\n",
        event_bus.lock().unwrap().subscriber_count()
    );

    // 3. Initialize game skill bridge
    println!("3. Setting up game skill bridge...");
    let skill_bridge = GameSkillBridge::new(scene_context.clone());
    println!(
        "   ✓ Game skill bridge created with {} skills\n",
        skill_bridge.list_skills().len()
    );

    let scene_id = "MainScene".to_string();

    // 4. Create some entities using skills + publish events manually for this demo
    println!("4. Creating entities using game skills...");

    // Create first entity
    let create_params = CreateEntityParams {
        name: "Player".to_string(),
        position: Some([0.0, 0.0, 0.0]),
        components: vec![
            ComponentData {
                type_name: "Transform".to_string(),
                properties: {
                    let mut props = std::collections::HashMap::new();
                    props.insert("position".to_string(), serde_json::json!([0.0, 0.0, 0.0]));
                    props.insert(
                        "rotation".to_string(),
                        serde_json::json!([0.0, 0.0, 0.0, 1.0]),
                    );
                    props
                },
            },
            ComponentData {
                type_name: "Sprite".to_string(),
                properties: {
                    let mut props = std::collections::HashMap::new();
                    props.insert("color".to_string(), serde_json::json!([0.0, 1.0, 0.0, 1.0]));
                    props
                },
            },
        ],
    };

    let result = skill_bridge.execute_skill(
        "create_entity",
        &serde_json::to_value(create_params.clone())?,
    )?;
    let player_id = result["entity_id"].as_u64().unwrap();
    println!("   ✓ Created Player entity with ID: {}", player_id);

    // Publish event
    let entity = SceneEntity {
        id: player_id,
        name: create_params.name,
        components: create_params.components,
        position: create_params.position,
    };
    event_bus
        .lock()
        .unwrap()
        .publish_entity_created(scene_id.clone(), entity);

    // Create second entity
    let create_params2 = CreateEntityParams {
        name: "Enemy".to_string(),
        position: Some([5.0, 0.0, 0.0]),
        components: vec![ComponentData {
            type_name: "Sprite".to_string(),
            properties: {
                let mut props = std::collections::HashMap::new();
                props.insert("color".to_string(), serde_json::json!([1.0, 0.0, 0.0, 1.0]));
                props
            },
        }],
    };

    let result2 = skill_bridge.execute_skill(
        "create_entity",
        &serde_json::to_value(create_params2.clone())?,
    )?;
    let enemy_id = result2["entity_id"].as_u64().unwrap();
    println!("   ✓ Created Enemy entity with ID: {}\n", enemy_id);

    // Publish event
    let entity2 = SceneEntity {
        id: enemy_id,
        name: create_params2.name,
        components: create_params2.components,
        position: create_params2.position,
    };
    event_bus
        .lock()
        .unwrap()
        .publish_entity_created(scene_id.clone(), entity2);

    // 5. Query entities
    println!("5. Querying scene entities...");
    let query_params = QueryEntitiesParams {
        name_filter: None,
        component_type: None,
    };

    let query_result =
        skill_bridge.execute_skill("query_entities", &serde_json::to_value(query_params)?)?;
    let entities_count = query_result["count"].as_u64().unwrap();
    println!("   ✓ Found {} entities in the scene\n", entities_count);

    // 6. Update a component
    println!("6. Updating entity component...");
    let update_params = UpdateComponentParams {
        entity_id: enemy_id,
        component_type: "Sprite".to_string(),
        properties: {
            let mut props = std::collections::HashMap::new();
            props.insert("color".to_string(), serde_json::json!([0.0, 0.0, 1.0, 1.0]));
            props
        },
    };

    skill_bridge.execute_skill(
        "update_component",
        &serde_json::to_value(update_params.clone())?,
    )?;
    println!("   ✓ Updated Enemy's color to blue\n");

    // Publish event
    let component = ComponentData {
        type_name: update_params.component_type,
        properties: update_params.properties,
    };
    event_bus.lock().unwrap().publish_component_updated(
        scene_id.clone(),
        update_params.entity_id,
        component,
    );

    // 7. Set up task bridge
    println!("7. Setting up task bridge...");
    let config = BridgeConfig::default();
    let task_sync = TaskSync::new(config);
    let task_bridge = TaskBridge::new(task_sync);
    println!("   ✓ Task bridge created\n");

    // 8. Create a game task
    println!("8. Creating a game task...");
    let task = task_bridge.create_game_task(
        "Defeat the Enemy".to_string(),
        "Find and defeat the blue enemy at position (5, 0, 0)".to_string(),
        "MainScene".to_string(),
        None,
    )?;
    println!(
        "   ✓ Created task: {} (ID: {})\n",
        task.title, task.id.bridge_id
    );

    // 9. Delete an entity
    println!("9. Deleting an entity...");
    let delete_params = DeleteEntityParams {
        entity_id: enemy_id,
    };

    // Get entity before deletion for event
    let enemy_before = scene_context.lock().unwrap().get_entity(enemy_id);

    skill_bridge.execute_skill(
        "delete_entity",
        &serde_json::to_value(delete_params.clone())?,
    )?;
    println!("   ✓ Deleted Enemy entity\n");

    // Publish event
    if let Some(entity) = enemy_before {
        event_bus
            .lock()
            .unwrap()
            .publish_entity_deleted(scene_id.clone(), entity);
    }

    // 10. Update task status
    println!("10. Updating task status...");
    task_bridge.update_task_status(task.id.bridge_id, UnifiedTaskStatus::Running)?;
    println!("   ✓ Task status updated to Running");

    task_bridge.update_task_status(task.id.bridge_id, UnifiedTaskStatus::Done)?;
    println!("   ✓ Task status updated to Done\n");

    // 11. Check statistics
    println!("11. Final statistics:");
    let stats = task_bridge.get_stats();
    println!("   - Total tasks: {}", stats.total_tasks);
    println!("   - Multica tasks: {}", stats.multica_tasks);
    println!("   - Scene count: {}", stats.scene_count);

    let event_stats = event_bus.lock().unwrap();
    println!(
        "   - Events published: {}",
        event_stats.event_history().len()
    );
    println!(
        "   - Events received by GameEngine: {}",
        subscriber1.event_count.load(Ordering::SeqCst)
    );
    println!(
        "   - Events received by DebugMonitor: {}",
        subscriber2.event_count.load(Ordering::SeqCst)
    );

    println!("\n=== Integration Example Complete ===");
    println!("All systems are working together harmoniously!");

    Ok(())
}
