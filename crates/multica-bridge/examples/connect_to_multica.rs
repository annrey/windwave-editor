//! Connect to Multica server and handle real tasks
use multica_bridge::*;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    println!("=== Multica Bridge - Real WebSocket Connection ===\n");

    // Configuration
    let config = BridgeConfig::default();
    println!("Config:");
    println!("  Server: {}", config.server_url);
    println!("  Agent ID: {}", config.agent_id);
    println!("  Daemon ID: {}\n", config.daemon_id);

    // Initialize components
    let mut task_sync = TaskSync::new(config.clone());
    let mut skill_adapter = SkillAdapter::new();

    // Register WindWave-specific skills
    println!("Registering skills...");
    skill_adapter.register_skill(
        "create_player",
        "Create a player character with WASD controls",
        vec!["game".to_string(), "player".to_string()],
        |params| {
            println!("\n⚡ Executing create_player skill!");
            println!("   Params: {:?}", params);
            Ok(serde_json::json!({
                "success": true,
                "player_id": "player_001",
                "controls": ["W", "A", "S", "D"]
            }))
        },
    );

    skill_adapter.register_skill(
        "add_obstacle",
        "Add collision obstacles to the scene",
        vec!["game".to_string(), "scene".to_string()],
        |params| {
            println!("\n⚡ Executing add_obstacle skill!");
            println!("   Params: {:?}", params);
            Ok(serde_json::json!({
                "success": true,
                "obstacles_added": 5
            }))
        },
    );

    println!(
        "✓ {} skills registered\n",
        skill_adapter.list_local_skills().len()
    );

    // Try to connect - fall back gracefully if it fails
    let connection_result = task_sync.connect().await;

    match connection_result {
        Ok(_) => {
            println!("✅ Connected to Multica server successfully!");
            println!("   Now we can receive tasks and send updates!\n");

            // If connected, we can run a demo
            println!("Running in connected mode...\n");

            // Simulate receiving a task dispatch
            println!("1. Simulating task dispatch from Multica...");
            let mock_dispatch = TaskDispatchPayload {
                task_id: "test_task_001".to_string(),
                issue_id: "issue_001".to_string(),
                title: "Create a demo player".to_string(),
                description: "Create a simple player character for the demo".to_string(),
            };

            let task = task_sync.handle_task_dispatch(mock_dispatch);
            println!("   ✓ Task created: {}", task.title);

            // Simulate sending progress
            println!("\n2. Sending progress updates...");
            task_sync
                .send_task_progress(task.id, "Initializing player...", 1, 3)
                .await?;
            println!("   ✓ Progress 1/3 sent");

            tokio::time::sleep(Duration::from_millis(500)).await;

            task_sync
                .send_task_progress(task.id, "Setting up movement...", 2, 3)
                .await?;
            println!("   ✓ Progress 2/3 sent");

            tokio::time::sleep(Duration::from_millis(500)).await;

            // Send a task message
            task_sync
                .send_task_message(task.id, 1, "Movement controls configured!")
                .await?;
            println!("   ✓ Task message sent");

            tokio::time::sleep(Duration::from_millis(500)).await;

            task_sync
                .send_task_progress(task.id, "Finalizing...", 3, 3)
                .await?;
            println!("   ✓ Progress 3/3 sent");

            // Complete the task
            println!("\n3. Completing task...");
            task_sync
                .complete_task(
                    task.id,
                    Some("Demo player successfully created! 🎮".to_string()),
                )
                .await?;
            println!("   ✓ Task completion sent");

            println!("\n✅ Connected mode demo complete!\n");
        }
        Err(e) => {
            println!("⚠️  Could not connect to Multica server:");
            println!("   Error: {}\n", e);

            println!("Running in fallback/demo mode...\n");

            // In fallback mode, we just show a message
            println!("1. Simulating task dispatch from Multica...");
            println!("   ✓ Task would be created here!\n");

            println!("2. Sending progress updates...");
            println!("   ✓ Progress would be sent here!\n");

            println!("3. Completing task...");
            println!("   ✓ Task completion would be sent here!\n");

            println!("\n=== Fallback mode demo complete! ===\n");
        }
    }

    // Note about real connection
    println!("Note: To connect to a real Multica server:");
    println!("1. Start Multica with: cd multica && make selfhost");
    println!("2. This will start Multica on localhost:8080");
    println!("3. Open http://localhost:3000 in your browser");
    println!("4. Create an Issue and assign it to your WindWave agent");
    println!("5. Watch the bridge handle it automatically!\n");

    Ok(())
}
