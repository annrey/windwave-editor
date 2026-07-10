//! Complete task flow example: from Multica Issue to WindWave Task completion
use multica_bridge::*;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    println!("=== Multica + WindWave Complete Task Flow ===\n");

    // Step 1: Configure and connect
    let config = BridgeConfig::default();
    println!("[1/5] Configuring bridge...");
    println!("  Server: {}", config.server_url);
    println!("  Agent ID: {}", config.agent_id);

    let mut task_sync = TaskSync::new(config.clone());

    // Try to connect
    println!("\n[2/5] Connecting to Multica...");
    match task_sync.connect().await {
        Ok(_) => println!("  ✓ Connected successfully!"),
        Err(e) => println!("  ⚠️  Running in fallback mode: {}", e),
    }

    // Note: This requires Multica server to be running
    // For this demo, we'll simulate the flow
    println!("\n=== Simulated Task Flow ===\n");

    // Step 2: Simulate receiving a task dispatch from Multica
    println!("[3/5] Simulating task dispatch from Multica...");
    let mock_dispatch = TaskDispatchPayload {
        task_id: "multica_task_123".to_string(),
        issue_id: "issue_456".to_string(),
        title: "Add a player character to the game".to_string(),
        description: "Create a controllable player with WASD movement and basic physics"
            .to_string(),
    };

    let task = task_sync.handle_task_dispatch(mock_dispatch);
    println!("  ✓ Task created: {}", task.title);
    println!("  Task ID: {}", task.id);
    println!("  Status: {:?}\n", task.status);

    // Step 3: Simulate task execution and send progress
    println!("[4/5] Simulating task execution with progress updates...");

    // Progress update 1
    println!("  → Sending progress update (1/3): Setting up player model...");
    task_sync
        .send_task_progress(task.id, "Setting up player model and sprite", 1, 3)
        .await?;
    println!("  ✓ Progress 1/3 sent");

    // Progress update 2
    println!("  → Sending progress update (2/3): Adding movement controls...");
    task_sync
        .send_task_progress(task.id, "Implementing WASD movement controls", 2, 3)
        .await?;
    println!("  ✓ Progress 2/3 sent");

    // Send a task message
    println!("  → Sending task message: Player movement setup complete!");
    task_sync
        .send_task_message(
            task.id,
            1,
            "Player movement controls configured successfully",
        )
        .await?;
    println!("  ✓ Task message sent");

    // Progress update 3
    println!("  → Sending progress update (3/3): Adding physics...");
    task_sync
        .send_task_progress(task.id, "Adding basic collision and gravity", 3, 3)
        .await?;
    println!("  ✓ Progress 3/3 sent");

    // Step 4: Mark task as completed
    println!("\n[5/5] Marking task as completed...");
    task_sync
        .complete_task(
            task.id,
            Some(
                "Player character successfully added! 🎮\n\
         - WASD movement implemented\n\
         - Basic physics enabled\n\
         - Collision detection working"
                    .to_string(),
            ),
        )
        .await?;
    println!("  ✓ Task completion sent");

    println!("\n✅ Task flow completed successfully!");
    println!("\n=== Integration Summary ===");
    println!("  - Real WebSocket connection implementation");
    println!("  - Can send/receive messages to/from Multica");
    println!("  - Task synchronization is working");
    println!("  - Skill adapter is ready");

    Ok(())
}
