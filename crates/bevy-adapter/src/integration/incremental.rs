use super::*;

// ===========================================================================
// SceneIndexIncrementalRebuildPlugin — incremental over periodic full rebuild
// ===========================================================================

/// Tracks scene generation so SceneIndex is only rebuilt when ECS state changes.
#[derive(Resource, Default)]
pub struct SceneIndexGenerationTracker {
    /// Hash of entity count + entity names (lightweight change detection).
    pub last_entity_hash: u64,
    /// Number of rebuilds performed since startup.
    pub rebuild_count: u64,
    /// Number of rebuilds skipped (no change detected).
    pub skipped_count: u64,
    /// Whether this frame triggered a rebuild.
    pub rebuilt_this_frame: bool,
}

/// Incremental SceneIndex plugin — rebuilds only when entities change.
///
/// Compares a quick hash of (entity_count, sorted_entity_names) against
/// the previous frame. Rebuilds only when the hash differs — saving
/// significant CPU in scenes with few or no mutations.
pub struct SceneIndexIncrementalPlugin {
    /// Fallback interval: rebuild at this frame interval even if hash matches.
    pub fallback_interval: usize,
    /// Enforce full rebuild every N frames regardless of change detection.
    pub full_rebuild_interval: usize,
}

impl SceneIndexIncrementalPlugin {
    pub fn new(fallback_interval: usize, full_rebuild_interval: usize) -> Self {
        Self {
            fallback_interval,
            full_rebuild_interval,
        }
    }
}

impl Default for SceneIndexIncrementalPlugin {
    fn default() -> Self {
        Self {
            fallback_interval: 120,     // force check every ~2s
            full_rebuild_interval: 300, // full rebuild every ~5s
        }
    }
}

impl Plugin for SceneIndexIncrementalPlugin {
    fn build(&self, app: &mut App) {
        let fallback = self.fallback_interval;
        let full = self.full_rebuild_interval;
        app.init_resource::<SceneIndexGenerationTracker>()
            .insert_resource(IncrementalConfig {
                fallback_interval: fallback,
                full_rebuild_interval: full,
                frame_counter: 0,
            })
            .add_systems(Update, incremental_scene_index_update);
    }
}

#[derive(Resource)]
#[allow(dead_code)]
struct IncrementalConfig {
    fallback_interval: usize,
    full_rebuild_interval: usize,
    frame_counter: usize,
}

/// Quick hash of entity names for change detection.
pub fn compute_scene_hash(cache: &SceneIndexCache) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut names: Vec<&String> = cache.get().entities_by_name.keys().collect();
    names.sort();

    let mut hasher = DefaultHasher::new();
    names.len().hash(&mut hasher);
    for name in &names {
        name.hash(&mut hasher);
    }
    hasher.finish()
}

/// Component-level incremental scene index update using Bevy Change Detection.
///
/// Uses `Changed<Transform>`, `Changed<Sprite>`, `Changed<ChildOf>`, and
/// `Changed<Children>` to only rebuild entities whose components actually
/// changed. Falls back to full rebuild periodically.
fn incremental_scene_index_update(
    mut cache: ResMut<SceneIndexCache>,
    mut config: ResMut<IncrementalConfig>,
    adapter: Res<BevyAdapter>,
    // Use Changed<T> for component-level change detection
    transform_changes: Query<Entity, Changed<Transform>>,
    sprite_changes: Query<Entity, Changed<Sprite>>,
    hierarchy_changes: Query<Entity, Or<(Changed<ChildOf>, Changed<Children>)>>,
    all_entities: Query<(
        Entity,
        Option<&Name>,
        Option<&Transform>,
        Option<&Sprite>,
        Option<&Children>,
        Option<&ChildOf>,
    )>,
) {
    config.frame_counter += 1;

    // Periodic full rebuild
    if config
        .frame_counter
        .is_multiple_of(config.full_rebuild_interval)
    {
        info!("SceneIndex: full rebuild (frame {})", config.frame_counter);
        rebuild_entities_full(&mut cache, &adapter, &all_entities);
        return;
    }

    // Collect changed entity IDs
    let mut changed: std::collections::HashSet<Entity> = std::collections::HashSet::new();
    for e in transform_changes.iter() {
        changed.insert(e);
    }
    for e in sprite_changes.iter() {
        changed.insert(e);
    }
    for e in hierarchy_changes.iter() {
        changed.insert(e);
    }

    // Force update at fallback interval even if no changes detected
    let force = config
        .frame_counter
        .is_multiple_of(config.fallback_interval);

    if !force {
        let live_ids: std::collections::HashSet<u64> = all_entities
            .iter()
            .filter_map(|(entity, ..)| adapter.get_agent_id(entity).map(|id| id.0))
            .filter(|id| *id != 0)
            .collect();
        let has_stale_index_entries = cache
            .get()
            .entities_by_name
            .values()
            .any(|id| *id != 0 && !live_ids.contains(id));

        if has_stale_index_entries {
            cache.0.reconcile_deletions(&live_ids);
        }
    }

    if changed.is_empty() && !force {
        return;
    }

    // Incremental update: only rebuild changed entities
    cache.incremental_update(&adapter, &all_entities, &changed, force);
}

/// Full rebuild of all entities into SceneIndex.
fn rebuild_entities_full(
    cache: &mut SceneIndexCache,
    adapter: &BevyAdapter,
    all_entities: &Query<(
        Entity,
        Option<&Name>,
        Option<&Transform>,
        Option<&Sprite>,
        Option<&Children>,
        Option<&ChildOf>,
    )>,
) {
    let changed: std::collections::HashSet<Entity> = all_entities.iter().map(|(e, ..)| e).collect();
    cache.incremental_update(adapter, all_entities, &changed, true);
}
