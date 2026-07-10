use super::*;

// ===========================================================================
// IntegrationPlugin — wires DirectorRuntime ↔ ECS ↔ UI ↔ EventBus
// ===========================================================================

/// Global integration state that the DirectorRuntime can access.
///
/// In a real deployment, this is shared between Bevy systems and the
/// agent-core DirectorRuntime via Arc<Mutex<...>> or channels.
#[derive(Resource, Clone, Default)]
pub struct IntegrationState {
    /// Whether the engine is ready for agent commands.
    pub engine_ready: bool,
    /// Frame counter for time-based operations.
    pub frame_count: u64,
    /// Latest scene entity count (updated each rebuild).
    pub scene_entity_count: usize,
    /// Whether a scene index rebuild just completed.
    pub index_just_rebuilt: bool,
    /// Last vision observation summary (if any).
    pub last_vision_summary: Option<String>,
    /// Number of Bevy frame time samples collected.
    pub frame_time_sample_count: u64,
    /// Average Bevy frame delta in milliseconds.
    pub frame_time_avg_ms: f32,
    /// Maximum observed Bevy frame delta in milliseconds.
    pub frame_time_max_ms: f32,
    frame_time_total_ms: f32,
}

impl IntegrationState {
    pub fn record_frame_delta_secs(&mut self, delta_secs: f32) {
        let frame_ms = (delta_secs * 1000.0).max(0.0);
        self.frame_time_sample_count += 1;
        self.frame_time_total_ms += frame_ms;
        self.frame_time_avg_ms = self.frame_time_total_ms / self.frame_time_sample_count as f32;
        self.frame_time_max_ms = self.frame_time_max_ms.max(frame_ms);
    }

    pub fn frame_time_evidence(&self) -> Vec<String> {
        vec![
            format!("bevy_frame_count={}", self.frame_count),
            format!("bevy_frame_time_samples={}", self.frame_time_sample_count),
            format!("bevy_frame_time_avg_ms={:.3}", self.frame_time_avg_ms),
            format!("bevy_frame_time_max_ms={:.3}", self.frame_time_max_ms),
        ]
    }
}

/// Main integration plugin that wires all subsystems together.
///
/// Registers `IntegrationState` and a post-rebuild sync system that
/// keeps the DirectorRuntime informed about engine state changes.
pub struct IntegrationPlugin;

impl Plugin for IntegrationPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<IntegrationState>().add_systems(
            Update,
            (integration_frame_tick, integration_post_rebuild_sync).chain(),
        );
    }
}

/// Frame counter + engine readiness detection.
fn integration_frame_tick(
    mut state: ResMut<IntegrationState>,
    cache: Res<SceneIndexCache>,
    time: Res<Time>,
) {
    state.frame_count += 1;
    state.record_frame_delta_secs(time.delta_secs());

    // Mark engine ready after first SceneIndex rebuild
    if !state.engine_ready && !cache.0.entities_by_name.is_empty() {
        state.engine_ready = true;
        info!(
            "Integration engine ready — {} entities indexed",
            cache.0.entities_by_name.len()
        );
    }
}

/// Sync integration state after each SceneIndex rebuild.
fn integration_post_rebuild_sync(mut state: ResMut<IntegrationState>, cache: Res<SceneIndexCache>) {
    let count = cache.0.entities_by_name.len();
    if count != state.scene_entity_count {
        state.scene_entity_count = count;
        state.index_just_rebuilt = true;
    } else {
        state.index_just_rebuilt = false;
    }
}
