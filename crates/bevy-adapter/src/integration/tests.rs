use super::*;

// ===========================================================================
// Tests
// ===========================================================================

// -------------------------------------------------------------------
// SceneIndexCache
// -------------------------------------------------------------------

#[test]
fn test_empty_cache() {
    let cache = SceneIndexCache::default();
    assert_eq!(cache.root_count(), 0);
    assert_eq!(cache.total_count(), 0);
    assert!(cache.entity_names().is_empty());
    let snapshot = cache.to_goal_checker_snapshot();
    assert!(snapshot.is_empty());
}

#[test]
fn test_incremental_plugin_reconciles_deleted_entities_without_waiting_for_fallback() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.init_resource::<BevyAdapter>();
    app.init_resource::<SceneIndexCache>();
    app.add_plugins(SceneIndexIncrementalPlugin::new(10_000, 10_000));

    let entity = app
        .world_mut()
        .spawn((
            Name::new("TransientEnemy"),
            Transform::default(),
            Sprite::default(),
        ))
        .id();
    app.world_mut()
        .resource_mut::<BevyAdapter>()
        .register_entity(entity);

    app.update();
    assert!(
        app.world()
            .resource::<SceneIndexCache>()
            .find_by_name("TransientEnemy")
            .is_some(),
        "initial changed components should add the entity to SceneIndex"
    );

    app.world_mut().despawn(entity);
    app.update();

    let cache = app.world().resource::<SceneIndexCache>();
    assert!(
        cache.find_by_name("TransientEnemy").is_none(),
        "deleted entities should be reconciled before the fallback interval"
    );
    assert_eq!(cache.total_count(), 0);
}

// -------------------------------------------------------------------
// VisualObservation
// -------------------------------------------------------------------

#[test]
fn test_visual_observation_default() {
    let obs = VisualObservation::default();
    assert!(obs.screenshot_path.is_empty());
    assert!(obs.summary.is_empty());
    assert!(obs.visible_entities.is_empty());
    assert!(obs.anomalies.is_empty());
    assert!((obs.confidence - 1.0).abs() < f32::EPSILON);
}

#[test]
fn test_visual_observation_with_data() {
    let obs = VisualObservation {
        screenshot_path: "screen.png".into(),
        summary: "3 entities visible".into(),
        visible_entities: vec!["Player".into(), "Enemy_01".into()],
        anomalies: vec![],
        confidence: 0.92,
    };
    assert_eq!(obs.visible_entities.len(), 2);
    assert!(obs.summary.contains("entities"));
}

// -------------------------------------------------------------------
// VisionError
// -------------------------------------------------------------------

#[test]
fn test_vision_error_display() {
    let err = VisionError::CaptureFailed("timeout".into());
    assert!(err.to_string().contains("timeout"));

    let err2 = VisionError::NoProvider;
    assert!(err2.to_string().contains("provider"));
}
