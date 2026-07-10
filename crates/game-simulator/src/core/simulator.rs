//! Main simulator orchestrator

use crate::core::error::*;
use crate::core::scene::*;
use crate::progress::*;
use crate::systems::*;
use std::time::Instant;

/// The main game simulator
pub struct GameSimulator {
    scene: Scene,
    systems: Vec<Box<dyn System>>,
    frame_count: u64,
    time_delta: f32,
    progress: ProgressTracker,
}

impl GameSimulator {
    /// Create a new simulator with default settings
    pub fn new() -> Self {
        Self::with_scene(Scene::new("main"))
    }

    /// Create a simulator with an existing scene
    pub fn with_scene(scene: Scene) -> Self {
        Self {
            scene,
            systems: Vec::new(),
            frame_count: 0,
            time_delta: 1.0 / 60.0,
            progress: ProgressTracker::new(),
        }
    }

    /// Get the current scene
    pub fn scene(&self) -> &Scene {
        &self.scene
    }

    /// Get a mutable reference to the scene
    pub fn scene_mut(&mut self) -> &mut Scene {
        &mut self.scene
    }

    /// Spawn an entity in the current scene
    pub fn spawn_entity(&mut self, name: &str) -> EntityId {
        self.scene.spawn_entity(name)
    }

    /// Add a system to the simulation
    pub fn add_system<S: System + 'static>(&mut self, system: S) {
        self.systems.push(Box::new(system));
    }

    /// Set the time delta per frame
    pub fn set_time_delta(&mut self, delta: f32) {
        self.time_delta = delta;
    }

    /// Get current frame count
    pub fn frame_count(&self) -> u64 {
        self.frame_count
    }

    /// Get progress tracker
    pub fn progress(&self) -> &ProgressTracker {
        &self.progress
    }

    /// Run simulation for one frame
    pub fn step(&mut self) {
        let start = Instant::now();
        self.frame_count += 1;

        // Run all systems
        for system in &self.systems {
            system.update(&mut self.scene, self.time_delta);
        }

        // Record frame time
        let frame_time = start.elapsed().as_secs_f64();
        self.progress.record_frame(frame_time);
        self.progress.advance_frame();
    }

    /// Run simulation for N frames
    pub fn run_for(&mut self, frames: u64) {
        self.progress.start(frames);

        for _ in 0..frames {
            self.step();
        }

        self.progress.finish();
    }
}

impl Default for GameSimulator {
    fn default() -> Self {
        Self::new()
    }
}
