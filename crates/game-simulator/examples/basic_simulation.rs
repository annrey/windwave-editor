//! Basic game simulation example

use game_simulator::prelude::*;

fn main() {
    println!("=== Game Simulator Demo ===\n");

    // Create simulator
    let mut sim = GameSimulator::new();

    // Add systems
    sim.add_system(PhysicsSystem::new());
    sim.add_system(HealthRegenSystem::new(2.0));

    // Spawn player entity
    let player_id = sim.spawn_entity("player");
    let mut player = sim.scene().get_entity(player_id).unwrap();
    player.position = Some(Position::new(0.0, 0.0, 0.0));
    player.velocity = Some(Velocity::new(1.0, 0.5, 0.0));
    player.health = Some(Health::new(100.0));
    sim.scene_mut().update_entity(player);

    // Spawn enemy entity
    let enemy_id = sim.spawn_entity("enemy");
    let mut enemy = sim.scene().get_entity(enemy_id).unwrap();
    enemy.position = Some(Position::new(10.0, 5.0, 0.0));
    enemy.velocity = Some(Velocity::new(-0.5, 0.2, 0.0));
    enemy.health = Some(Health::new(50.0));
    sim.scene_mut().update_entity(enemy);

    println!("Initial state:");
    print_scene(sim.scene());

    println!("\nSimulating 100 frames...");
    sim.run_for(100);

    println!("\nFinal state after 100 frames:");
    print_scene(sim.scene());

    // Print progress report
    let report = sim.progress().report();
    println!("\n{report}");

    // Run a test
    println!("\n=== Running Test ===");
    let mut runner = TestRunner::new();

    let result = runner.run_test(
        "player_moved",
        |sim| {
            let id = sim.spawn_entity("test_player");
            let mut entity = sim.scene().get_entity(id).unwrap();
            entity.position = Some(Position::new(0.0, 0.0, 0.0));
            entity.velocity = Some(Velocity::new(2.0, 0.0, 0.0));
            sim.scene_mut().update_entity(entity);
            sim.add_system(PhysicsSystem::new());
        },
        |sim| {
            let entity = sim.scene().get_entity_by_name("test_player").unwrap();
            let pos = entity.position.unwrap();

            if pos.x > 0.0 {
                TestResult::pass("Player moved successfully")
            } else {
                TestResult::fail("Player didn't move")
            }
        },
    );

    println!(
        "Test 'player_moved': {}",
        if result.passed { "PASSED" } else { "FAILED" }
    );
    println!("Message: {}", result.message);

    // Run benchmark
    println!("\n=== Running Benchmark ===");
    let mut bench_sim = GameSimulator::new();
    bench_sim.add_system(PhysicsSystem::new());

    // Spawn many entities for benchmark
    for i in 0..1000 {
        let id = bench_sim.spawn_entity(&format!("entity_{}", i));
        let mut entity = bench_sim.scene().get_entity(id).unwrap();
        entity.position = Some(Position::new(i as f32, 0.0, 0.0));
        entity.velocity = Some(Velocity::new(1.0, 0.0, 0.0));
        bench_sim.scene_mut().update_entity(entity);
    }

    let benchmark = run_benchmark("1000_entities_physics", bench_sim, 1000);
    println!("Benchmark '{}':", benchmark.name);
    println!("  FPS: {:.1}", benchmark.fps);
    println!("  Avg frame: {:.3}ms", benchmark.avg_frame_time_ms);
    println!("  Total time: {:.2}s", benchmark.total_time_ms / 1000.0);
}

fn print_scene(scene: &Scene) {
    println!("Scene: {}", scene.name());
    for entity in scene.entities() {
        println!("  - Entity: {}", entity.name);

        if let Some(pos) = &entity.position {
            println!("    Position: ({:.1}, {:.1}, {:.1})", pos.x, pos.y, pos.z);
        }

        if let Some(vel) = &entity.velocity {
            println!("    Velocity: ({:.1}, {:.1}, {:.1})", vel.x, vel.y, vel.z);
        }

        if let Some(health) = &entity.health {
            println!("    Health: {:.1} / {:.1}", health.current, health.max);
        }
    }
}
