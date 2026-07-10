//! Integration tests for game simulator

use game_simulator::prelude::*;

#[test]
fn test_entity_spawn() {
    let scene = Scene::new("test");
    let id = scene.spawn_entity("player");

    assert_eq!(scene.entity_count(), 1);
    assert!(scene.get_entity(id).is_some());
    assert_eq!(scene.get_entity_by_name("player").unwrap().name, "player");
}

#[test]
fn test_physics_system() {
    let mut sim = GameSimulator::new();
    sim.add_system(PhysicsSystem::new());

    let id = sim.spawn_entity("test");
    let mut entity = sim.scene().get_entity(id).unwrap();
    entity.position = Some(Position::new(0.0, 0.0, 0.0));
    entity.velocity = Some(Velocity::new(1.0, 2.0, 3.0));
    sim.scene_mut().update_entity(entity);

    sim.run_for(60);

    let entity = sim.scene().get_entity(id).unwrap();
    let pos = entity.position.unwrap();
    assert!((pos.x - 1.0).abs() < 0.01);
    assert!((pos.y - 2.0).abs() < 0.01);
    assert!((pos.z - 3.0).abs() < 0.01);
}

#[test]
fn test_progress_tracking() {
    let mut progress = ProgressTracker::new();
    progress.start(100);

    assert_eq!(progress.progress(), 0.0);

    for _ in 0..50 {
        progress.advance_frame();
        progress.record_frame(0.016);
    }

    assert_eq!(progress.progress(), 0.5);
    assert_eq!(progress.current_frame(), 50);
}

#[test]
fn test_test_framework() {
    let mut runner = TestRunner::new();

    let result = runner.run_test(
        "simple_test",
        |sim| {
            sim.spawn_entity("test");
        },
        |sim| {
            if sim.scene().entity_count() == 1 {
                TestResult::pass("Ok")
            } else {
                TestResult::fail("Not ok")
            }
        },
    );

    assert!(result.passed);

    let summary = runner.summary();
    assert_eq!(summary.total, 1);
    assert_eq!(summary.passed, 1);
}
