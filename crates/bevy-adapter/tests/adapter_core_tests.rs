use agent_core::runtime_agent::RuntimeBehaviorSpec;
use agent_core::{
    DirectorRuntime, EnemyRuntimeState, EntityId, LootRuntimeState, OpenWorldPlan,
    OpenWorldRuntimeState, OpenWorldSceneTemplate,
};
use bevy::prelude::Visibility;
use bevy_adapter::scene_index::{ComponentSummary, SceneIndex};
use bevy_adapter::*;
use serde_json::json;

#[test]
fn test_integration_state_records_frame_time_evidence() {
    let mut app = bevy::prelude::App::new();
    app.add_plugins(bevy::MinimalPlugins)
        .insert_resource(bevy_adapter::integration::SceneIndexCache::default())
        .add_plugins(bevy_adapter::integration::IntegrationPlugin);

    app.update();
    app.update();

    let state = app
        .world()
        .resource::<bevy_adapter::integration::IntegrationState>();
    assert!(state.frame_count >= 2);
    assert!(state.frame_time_sample_count >= 2);
    assert!(state.frame_time_avg_ms >= 0.0);
    assert!(state.frame_time_max_ms >= state.frame_time_avg_ms);
    assert!(state
        .frame_time_evidence()
        .iter()
        .any(|row| row.starts_with("bevy_frame_time_avg_ms=")));
}

#[test]
fn test_screenshot_queue_records_runtime_readback_evidence() {
    let mut queue = ScreenshotQueue::default();

    assert!(queue
        .runtime_readback_evidence()
        .contains(&"bevy_screenshot_requested=false".to_string()));
    assert!(queue
        .runtime_readback_evidence()
        .contains(&"bevy_screenshot_result_count=0".to_string()));

    queue.request_capture();

    let evidence = queue.runtime_readback_evidence();
    assert!(evidence.contains(&"bevy_screenshot_requested=true".to_string()));
    assert!(evidence.contains(&"bevy_screenshot_requests_total=1".to_string()));

    queue.record_result(ScreenshotResult::Success {
        path: std::path::PathBuf::from("framebuffer.png"),
        dimensions: (800, 450),
        base64: "encoded".into(),
    });
    queue.record_result(ScreenshotResult::Failure {
        error: "primary window unavailable".into(),
    });

    let evidence = queue.runtime_readback_evidence();
    assert!(evidence.contains(&"bevy_screenshot_result_count=2".to_string()));
    assert!(evidence.contains(&"bevy_screenshot_success_total=1".to_string()));
    assert!(evidence.contains(&"bevy_screenshot_failure_total=1".to_string()));
    assert!(evidence.contains(
        &"bevy_screenshot_last_result=failure error=primary window unavailable".to_string()
    ));
}

// ============================================================================
// EngineCommand serialization tests
// ============================================================================

#[test]
fn test_engine_command_create_entity_serialization() {
    let cmd = EngineCommand::CreateEntity {
        name: "Player".into(),
        components: vec![ComponentPatch {
            type_name: "Transform".into(),
            value: json!({"translation": [0.0, 0.0, 0.0]}),
        }],
    };

    let json = serde_json::to_string(&cmd).unwrap();
    assert!(json.contains("Player"));
    assert!(json.contains("CreateEntity"));

    let deserialized: EngineCommand = serde_json::from_str(&json).unwrap();
    match deserialized {
        EngineCommand::CreateEntity { name, components } => {
            assert_eq!(name, "Player");
            assert_eq!(components.len(), 1);
            assert_eq!(components[0].type_name, "Transform");
        }
        _ => panic!("Expected CreateEntity"),
    }
}

#[test]
fn test_engine_command_delete_entity_serialization() {
    let cmd = EngineCommand::DeleteEntity { entity_id: 42 };

    let json = serde_json::to_string(&cmd).unwrap();
    let deserialized: EngineCommand = serde_json::from_str(&json).unwrap();

    match deserialized {
        EngineCommand::DeleteEntity { entity_id } => assert_eq!(entity_id, 42),
        _ => panic!("Expected DeleteEntity"),
    }
}

#[test]
fn test_engine_command_set_transform_serialization() {
    let cmd = EngineCommand::SetTransform {
        entity_id: 1,
        translation: Some([1.0, 2.0, 3.0]),
        rotation: None,
        scale: Some([2.0, 2.0, 2.0]),
    };

    let json = serde_json::to_string(&cmd).unwrap();
    let deserialized: EngineCommand = serde_json::from_str(&json).unwrap();

    match deserialized {
        EngineCommand::SetTransform {
            entity_id,
            translation,
            scale,
            ..
        } => {
            assert_eq!(entity_id, 1);
            assert_eq!(translation, Some([1.0, 2.0, 3.0]));
            assert_eq!(scale, Some([2.0, 2.0, 2.0]));
        }
        _ => panic!("Expected SetTransform"),
    }
}

#[test]
fn test_engine_command_set_sprite_color_serialization() {
    let cmd = EngineCommand::SetSpriteColor {
        entity_id: 5,
        rgba: [1.0, 0.0, 0.0, 1.0],
    };

    let json = serde_json::to_string(&cmd).unwrap();
    let deserialized: EngineCommand = serde_json::from_str(&json).unwrap();

    match deserialized {
        EngineCommand::SetSpriteColor { entity_id, rgba } => {
            assert_eq!(entity_id, 5);
            assert_eq!(rgba, [1.0, 0.0, 0.0, 1.0]);
        }
        _ => panic!("Expected SetSpriteColor"),
    }
}

#[test]
fn test_engine_command_set_visibility_serialization() {
    let cmd = EngineCommand::SetVisibility {
        entity_id: 3,
        visible: false,
    };

    let json = serde_json::to_string(&cmd).unwrap();
    let deserialized: EngineCommand = serde_json::from_str(&json).unwrap();

    match deserialized {
        EngineCommand::SetVisibility { entity_id, visible } => {
            assert_eq!(entity_id, 3);
            assert!(!visible);
        }
        _ => panic!("Expected SetVisibility"),
    }
}

#[test]
fn test_engine_command_add_component_serialization() {
    let cmd = EngineCommand::AddComponent {
        entity_id: 1,
        component: ComponentPatch {
            type_name: "RigidBody".into(),
            value: json!({"mass": 10.0}),
        },
    };

    let json = serde_json::to_string(&cmd).unwrap();
    let deserialized: EngineCommand = serde_json::from_str(&json).unwrap();

    match deserialized {
        EngineCommand::AddComponent {
            entity_id,
            component,
        } => {
            assert_eq!(entity_id, 1);
            assert_eq!(component.type_name, "RigidBody");
            assert_eq!(component.value, json!({"mass": 10.0}));
        }
        _ => panic!("Expected AddComponent"),
    }
}

#[test]
fn test_engine_command_modify_component_serialization() {
    let cmd = EngineCommand::ModifyComponent {
        entity_id: 2,
        component_type: "Transform".into(),
        property: "scale".into(),
        value: json!([3.0, 3.0, 3.0]),
    };

    let json = serde_json::to_string(&cmd).unwrap();
    let deserialized: EngineCommand = serde_json::from_str(&json).unwrap();

    match deserialized {
        EngineCommand::ModifyComponent {
            entity_id,
            component_type,
            property,
            value,
        } => {
            assert_eq!(entity_id, 2);
            assert_eq!(component_type, "Transform");
            assert_eq!(property, "scale");
            assert_eq!(value, json!([3.0, 3.0, 3.0]));
        }
        _ => panic!("Expected ModifyComponent"),
    }
}

#[test]
fn test_engine_command_set_parent_serialization() {
    let cmd = EngineCommand::SetParent {
        child_entity_id: 10,
        parent_entity_id: 5,
    };

    let json = serde_json::to_string(&cmd).unwrap();
    let deserialized: EngineCommand = serde_json::from_str(&json).unwrap();

    match deserialized {
        EngineCommand::SetParent {
            child_entity_id,
            parent_entity_id,
        } => {
            assert_eq!(child_entity_id, 10);
            assert_eq!(parent_entity_id, 5);
        }
        _ => panic!("Expected SetParent"),
    }
}

#[test]
fn test_engine_command_remove_from_parent() {
    let cmd = EngineCommand::RemoveFromParent { entity_id: 7 };

    let json = serde_json::to_string(&cmd).unwrap();
    let deserialized: EngineCommand = serde_json::from_str(&json).unwrap();

    match deserialized {
        EngineCommand::RemoveFromParent { entity_id } => assert_eq!(entity_id, 7),
        _ => panic!("Expected RemoveFromParent"),
    }
}

#[test]
fn test_engine_command_reparent_children() {
    let cmd = EngineCommand::ReparentChildren {
        source_parent_id: 1,
        target_parent_id: 2,
    };

    let json = serde_json::to_string(&cmd).unwrap();
    let deserialized: EngineCommand = serde_json::from_str(&json).unwrap();

    match deserialized {
        EngineCommand::ReparentChildren {
            source_parent_id,
            target_parent_id,
        } => {
            assert_eq!(source_parent_id, 1);
            assert_eq!(target_parent_id, 2);
        }
        _ => panic!("Expected ReparentChildren"),
    }
}

#[test]
fn test_open_world_small_island_template_indexes_required_zones() {
    let plan = OpenWorldPlan::open_world_slice01_fixture();
    let template = OpenWorldSceneTemplate::small_island_from_plan(&plan);
    let mut index = SceneIndex::new();

    for (offset, entity) in template.entities.iter().enumerate() {
        let components: Vec<ComponentSummary> = entity
            .components
            .iter()
            .map(|component| ComponentSummary {
                type_name: component.type_name.clone(),
                properties: component.properties.clone(),
            })
            .collect();
        index.add_entity(entity.name.clone(), (offset + 1) as u64, components);
    }

    for zone_id in OpenWorldSceneTemplate::required_zone_ids() {
        let node = index
            .get_entity_by_name(zone_id)
            .unwrap_or_else(|| panic!("expected SceneIndex entity for {zone_id}"));
        assert!(node
            .components
            .iter()
            .any(|component| component.type_name == "ZoneMarker"));
    }

    let zone_markers = index.get_entities_with_component("ZoneMarker");
    let marker_names: Vec<&str> = zone_markers.iter().map(|node| node.name.as_str()).collect();
    assert!(marker_names.contains(&"spawn_zone"));
    assert!(marker_names.contains(&"puzzle_zone"));
    assert!(marker_names.contains(&"camp_zone"));
    assert!(marker_names.contains(&"reward_zone"));

    for object_id in OpenWorldSceneTemplate::required_gameplay_object_ids() {
        index
            .get_entity_by_name(object_id)
            .unwrap_or_else(|| panic!("expected SceneIndex entity for {object_id}"));
    }

    assert_entity_has_component(&index, "player", "PlayerController");
    assert_entity_has_component(&index, "puzzle_switch", "PuzzleSwitch");
    assert_entity_has_component(&index, "reward_chest", "LootContainer");
    assert_entity_has_component(&index, "camp_enemy_01", "EnemyBrain");
}

fn assert_entity_has_component(index: &SceneIndex, entity_name: &str, component_type: &str) {
    let node = index
        .get_entity_by_name(entity_name)
        .unwrap_or_else(|| panic!("expected SceneIndex entity for {entity_name}"));
    assert!(
        node.components
            .iter()
            .any(|component| component.type_name == component_type),
        "expected {entity_name} to include {component_type}"
    );
}

#[test]
fn test_open_world_template_components_apply_to_bevy_world_and_scene_index() {
    let plan = OpenWorldPlan::open_world_slice01_fixture();
    let template = OpenWorldSceneTemplate::small_island_from_plan(&plan);
    let mut adapter = BevyAdapter::default();
    let mut world = bevy::prelude::World::new();

    for stable_id in ["player", "puzzle_switch", "reward_chest", "camp_enemy_01"] {
        let entity = template
            .entities
            .iter()
            .find(|entity| entity.stable_id == stable_id)
            .unwrap_or_else(|| panic!("expected template entity {stable_id}"));
        let result = adapter
            .apply_engine_command(template_entity_to_create_command(entity), &mut world)
            .unwrap();
        let bevy_entity = adapter
            .get_bevy_entity(EntityId(result.entity_id.expect("created entity id")))
            .expect("created entity should be registered");

        assert!(
            world.get::<OpenWorldObject>(bevy_entity).is_some(),
            "{stable_id} should have OpenWorldObject"
        );
    }

    let index = BevyAdapter::build_scene_index(&mut adapter, &mut world);
    assert_entity_has_component(&index, "player", "PlayerController");
    assert_entity_has_component(&index, "player", "Combatant");
    assert_entity_has_component(&index, "puzzle_switch", "PuzzleSwitch");
    assert_entity_has_component(&index, "reward_chest", "LootContainer");
    assert_entity_has_component(&index, "camp_enemy_01", "EnemyBrain");
}

#[test]
fn test_open_world_replay_world_state_applies_to_bevy_components() {
    let plan = OpenWorldPlan::open_world_slice01_fixture();
    let template = OpenWorldSceneTemplate::small_island_from_plan(&plan);
    let mut adapter = BevyAdapter::default();
    let mut world = bevy::prelude::World::new();

    for stable_id in ["player", "reward_chest", "camp_enemy_01", "main_quest"] {
        let entity = template
            .entities
            .iter()
            .find(|entity| entity.stable_id == stable_id)
            .unwrap_or_else(|| panic!("expected template entity {stable_id}"));
        adapter
            .apply_engine_command(template_entity_to_create_command(entity), &mut world)
            .unwrap();
    }

    let mut replay_state = agent_core::OpenWorldReplayWorldState::default();
    replay_state
        .actor_zones
        .insert("player".into(), "camp_zone".into());
    replay_state
        .loot_states
        .insert("reward_chest".into(), "Opened".into());
    replay_state
        .enemy_states
        .insert("camp_enemy_01".into(), "Dead".into());
    replay_state
        .quest_states
        .insert("main_quest".into(), "Completed".into());
    replay_state
        .inventory
        .entry("player".into())
        .or_default()
        .insert("reward_item".into());

    apply_open_world_replay_state_to_world(&mut world, &replay_state);

    let player = find_open_world_entity(&mut world, "player");
    assert_eq!(
        world
            .get::<OpenWorldReplayActorState>(player)
            .unwrap()
            .zone_id,
        "camp_zone"
    );
    assert!(world
        .get::<OpenWorldReplayInventoryState>(player)
        .unwrap()
        .items
        .contains("reward_item"));

    let chest = find_open_world_entity(&mut world, "reward_chest");
    assert_eq!(
        world.get::<OpenWorldReplayLootState>(chest).unwrap().state,
        "Opened"
    );

    let enemy = find_open_world_entity(&mut world, "camp_enemy_01");
    assert_eq!(
        world.get::<OpenWorldReplayEnemyState>(enemy).unwrap().state,
        "Dead"
    );

    let quest = find_open_world_entity(&mut world, "main_quest");
    assert_eq!(
        world.get::<OpenWorldReplayQuestState>(quest).unwrap().state,
        "Completed"
    );

    let index = BevyAdapter::build_scene_index(&mut adapter, &mut world);
    let enemy_node = index
        .get_entity_by_name("camp_enemy_01")
        .expect("enemy should be indexed");
    let enemy_replay_state = enemy_node
        .components
        .iter()
        .find(|component| component.type_name == "OpenWorldReplayEnemyState")
        .expect("enemy replay state should be indexed");
    assert_eq!(
        enemy_replay_state.properties.get("state"),
        Some(&json!("Dead"))
    );

    let player_node = index
        .get_entity_by_name("player")
        .expect("player should be indexed");
    assert!(player_node.components.iter().any(|component| {
        component.type_name == "OpenWorldReplayInventoryState"
            && component.properties.get("items") == Some(&json!(["reward_item"]))
    }));
}

#[test]
fn test_open_world_replay_gameplay_state_drives_visibility_and_interaction() {
    let plan = OpenWorldPlan::open_world_slice01_fixture();
    let template = OpenWorldSceneTemplate::small_island_from_plan(&plan);
    let mut adapter = BevyAdapter::default();
    let mut world = bevy::prelude::World::new();

    for stable_id in ["reward_chest", "camp_enemy_01"] {
        let entity = template
            .entities
            .iter()
            .find(|entity| entity.stable_id == stable_id)
            .unwrap_or_else(|| panic!("expected template entity {stable_id}"));
        adapter
            .apply_engine_command(template_entity_to_create_command(entity), &mut world)
            .unwrap();
    }

    let mut replay_state = agent_core::OpenWorldReplayWorldState::default();
    replay_state
        .loot_states
        .insert("reward_chest".into(), "Opened".into());
    replay_state
        .enemy_states
        .insert("camp_enemy_01".into(), "Dead".into());

    apply_open_world_replay_state_to_world(&mut world, &replay_state);
    sync_open_world_replay_gameplay_state(&mut world);

    let enemy = find_open_world_entity(&mut world, "camp_enemy_01");
    assert_eq!(world.get::<Visibility>(enemy), Some(&Visibility::Hidden));
    assert_eq!(
        world
            .get::<OpenWorldReplayGameplayState>(enemy)
            .expect("enemy should have replay gameplay state"),
        &OpenWorldReplayGameplayState {
            visible: false,
            interaction_enabled: false,
        }
    );

    let chest = find_open_world_entity(&mut world, "reward_chest");
    assert_ne!(world.get::<Visibility>(chest), Some(&Visibility::Hidden));
    assert_eq!(
        world
            .get::<OpenWorldReplayGameplayState>(chest)
            .expect("chest should have replay gameplay state"),
        &OpenWorldReplayGameplayState {
            visible: true,
            interaction_enabled: false,
        }
    );

    let index = BevyAdapter::build_scene_index(&mut adapter, &mut world);
    let enemy_node = index
        .get_entity_by_name("camp_enemy_01")
        .expect("enemy should be indexed");
    assert!(enemy_node.components.iter().any(|component| {
        component.type_name == "OpenWorldReplayGameplayState"
            && component.properties.get("visible") == Some(&json!(false))
            && component.properties.get("interaction_enabled") == Some(&json!(false))
    }));
}

#[test]
fn test_open_world_loot_interaction_bridges_bevy_entity_to_core_runtime() {
    let plan = OpenWorldPlan::open_world_slice01_fixture();
    let template = OpenWorldSceneTemplate::small_island_from_plan(&plan);
    let mut adapter = BevyAdapter::default();
    let mut world = bevy::prelude::World::new();

    for stable_id in ["player", "reward_chest"] {
        let entity = template
            .entities
            .iter()
            .find(|entity| entity.stable_id == stable_id)
            .unwrap_or_else(|| panic!("expected template entity {stable_id}"));
        adapter
            .apply_engine_command(template_entity_to_create_command(entity), &mut world)
            .unwrap();
    }

    let mut runtime = OpenWorldRuntimeState::from_plan(&plan);
    runtime.enter_zone("player", "puzzle_zone").unwrap();
    runtime.interact("player", "puzzle_switch").unwrap();

    process_open_world_loot_interaction(
        &mut world,
        &mut runtime,
        "player",
        "reward_chest",
        "reward_item",
    )
    .unwrap();

    assert_eq!(
        runtime.loot_state("reward_chest").unwrap(),
        LootRuntimeState::LootClaimed
    );
    assert!(runtime.inventory_contains("player", "reward_item"));
    assert!(runtime
        .events
        .iter()
        .any(|event| event.label() == "player opened reward_chest"));
    assert!(runtime
        .events
        .iter()
        .any(|event| event.label() == "player collected reward_item"));

    let chest = find_open_world_entity(&mut world, "reward_chest");
    assert_eq!(
        world.get::<OpenWorldReplayLootState>(chest).unwrap().state,
        "LootClaimed"
    );
    assert_eq!(
        world
            .get::<OpenWorldReplayGameplayState>(chest)
            .expect("chest should have replay gameplay state"),
        &OpenWorldReplayGameplayState {
            visible: true,
            interaction_enabled: false,
        }
    );
}

#[test]
fn test_open_world_loot_interaction_respects_replay_disabled_state() {
    let plan = OpenWorldPlan::open_world_slice01_fixture();
    let template = OpenWorldSceneTemplate::small_island_from_plan(&plan);
    let mut adapter = BevyAdapter::default();
    let mut world = bevy::prelude::World::new();

    for stable_id in ["player", "reward_chest"] {
        let entity = template
            .entities
            .iter()
            .find(|entity| entity.stable_id == stable_id)
            .unwrap_or_else(|| panic!("expected template entity {stable_id}"));
        adapter
            .apply_engine_command(template_entity_to_create_command(entity), &mut world)
            .unwrap();
    }

    let chest = find_open_world_entity(&mut world, "reward_chest");
    world
        .entity_mut(chest)
        .insert(OpenWorldReplayGameplayState {
            visible: true,
            interaction_enabled: false,
        });

    let mut runtime = OpenWorldRuntimeState::from_plan(&plan);
    runtime.enter_zone("player", "puzzle_zone").unwrap();
    runtime.interact("player", "puzzle_switch").unwrap();

    let error = process_open_world_loot_interaction(
        &mut world,
        &mut runtime,
        "player",
        "reward_chest",
        "reward_item",
    )
    .unwrap_err();

    assert!(error
        .reason
        .contains("replay gameplay state has disabled interaction"));
    assert!(!runtime.inventory_contains("player", "reward_item"));
}

#[test]
fn test_open_world_combat_interaction_bridges_bevy_entities_to_core_runtime() {
    let plan = OpenWorldPlan::open_world_slice01_fixture();
    let template = OpenWorldSceneTemplate::small_island_from_plan(&plan);
    let mut adapter = BevyAdapter::default();
    let mut world = bevy::prelude::World::new();

    for stable_id in ["player", "camp_enemy_01"] {
        let entity = template
            .entities
            .iter()
            .find(|entity| entity.stable_id == stable_id)
            .unwrap_or_else(|| panic!("expected template entity {stable_id}"));
        adapter
            .apply_engine_command(template_entity_to_create_command(entity), &mut world)
            .unwrap();
    }

    let mut runtime = OpenWorldRuntimeState::from_plan(&plan);

    process_open_world_combat_interaction(&mut world, &mut runtime, "player", "camp_enemy_01")
        .unwrap();

    assert_eq!(
        runtime.enemy_state("camp_enemy_01").unwrap(),
        EnemyRuntimeState::Dead
    );
    assert!(runtime
        .events
        .iter()
        .any(|event| event.label() == "player aggroed camp_enemy_01"));
    assert!(runtime
        .events
        .iter()
        .any(|event| event.label() == "player defeated camp_enemy_01"));

    let enemy = find_open_world_entity(&mut world, "camp_enemy_01");
    assert_eq!(
        world.get::<OpenWorldReplayEnemyState>(enemy).unwrap().state,
        "Dead"
    );
    assert_eq!(
        world
            .get::<OpenWorldReplayGameplayState>(enemy)
            .expect("enemy should have replay gameplay state"),
        &OpenWorldReplayGameplayState {
            visible: false,
            interaction_enabled: false,
        }
    );
}

#[test]
fn test_open_world_combat_interaction_respects_replay_disabled_state() {
    let plan = OpenWorldPlan::open_world_slice01_fixture();
    let template = OpenWorldSceneTemplate::small_island_from_plan(&plan);
    let mut adapter = BevyAdapter::default();
    let mut world = bevy::prelude::World::new();

    for stable_id in ["player", "camp_enemy_01"] {
        let entity = template
            .entities
            .iter()
            .find(|entity| entity.stable_id == stable_id)
            .unwrap_or_else(|| panic!("expected template entity {stable_id}"));
        adapter
            .apply_engine_command(template_entity_to_create_command(entity), &mut world)
            .unwrap();
    }

    let enemy = find_open_world_entity(&mut world, "camp_enemy_01");
    world
        .entity_mut(enemy)
        .insert(OpenWorldReplayGameplayState {
            visible: false,
            interaction_enabled: false,
        });

    let mut runtime = OpenWorldRuntimeState::from_plan(&plan);
    let error =
        process_open_world_combat_interaction(&mut world, &mut runtime, "player", "camp_enemy_01")
            .unwrap_err();

    assert!(error
        .reason
        .contains("replay gameplay state has disabled interaction"));
    assert_eq!(
        runtime.enemy_state("camp_enemy_01").unwrap(),
        EnemyRuntimeState::Patrol
    );
}

fn find_open_world_entity(
    world: &mut bevy::prelude::World,
    object_id: &str,
) -> bevy::prelude::Entity {
    let mut query = world.query::<(bevy::prelude::Entity, &OpenWorldObject)>();
    query
        .iter(world)
        .find_map(|(entity, object)| (object.object_id == object_id).then_some(entity))
        .unwrap_or_else(|| panic!("expected OpenWorldObject entity {object_id}"))
}

fn template_entity_to_create_command(
    entity: &agent_core::OpenWorldTemplateEntity,
) -> EngineCommand {
    EngineCommand::CreateEntity {
        name: entity.name.clone(),
        components: entity
            .components
            .iter()
            .map(|component| ComponentPatch {
                type_name: component.type_name.clone(),
                value: serde_json::to_value(&component.properties).unwrap(),
            })
            .collect(),
    }
}

#[test]
fn test_engine_command_load_asset() {
    let cmd = EngineCommand::LoadAsset {
        path: "textures/player.png".into(),
        asset_type: AssetType::Image,
    };

    let json = serde_json::to_string(&cmd).unwrap();
    let deserialized: EngineCommand = serde_json::from_str(&json).unwrap();

    match deserialized {
        EngineCommand::LoadAsset { path, asset_type } => {
            assert_eq!(path, "textures/player.png");
            assert!(matches!(asset_type, AssetType::Image));
        }
        _ => panic!("Expected LoadAsset"),
    }
}

#[test]
fn test_engine_command_spawn_prefab() {
    let cmd = EngineCommand::SpawnPrefab {
        asset_handle: "prefabs/enemy.prefab".into(),
        transform: Some([100.0, 200.0, 0.0]),
    };

    let json = serde_json::to_string(&cmd).unwrap();
    let deserialized: EngineCommand = serde_json::from_str(&json).unwrap();

    match deserialized {
        EngineCommand::SpawnPrefab {
            asset_handle,
            transform,
        } => {
            assert_eq!(asset_handle, "prefabs/enemy.prefab");
            assert_eq!(transform, Some([100.0, 200.0, 0.0]));
        }
        _ => panic!("Expected SpawnPrefab"),
    }
}

// ============================================================================
// ComponentPatch tests
// ============================================================================

#[test]
fn test_component_patch_basic() {
    let patch = ComponentPatch {
        type_name: "Sprite".into(),
        value: json!({"color": [1.0, 0.0, 0.0, 1.0]}),
    };
    assert_eq!(patch.type_name, "Sprite");
    assert_eq!(patch.value, json!({"color": [1.0, 0.0, 0.0, 1.0]}));
}

#[test]
fn test_component_patch_serialization() {
    let patch = ComponentPatch {
        type_name: "Transform".into(),
        value: json!({"translation": [100.0, 200.0, 0.0]}),
    };

    let json = serde_json::to_string(&patch).unwrap();
    let deserialized: ComponentPatch = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.type_name, "Transform");
    assert_eq!(
        deserialized.value,
        json!({"translation": [100.0, 200.0, 0.0]})
    );
}

#[test]
fn test_component_patch_json_null_value() {
    let patch = ComponentPatch {
        type_name: "Collider".into(),
        value: serde_json::Value::Null,
    };

    let json = serde_json::to_string(&patch).unwrap();
    let deserialized: ComponentPatch = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.value, serde_json::Value::Null);
}

// ============================================================================
// EntitySnapshot tests
// ============================================================================

#[test]
fn test_entity_snapshot_basic() {
    let snapshot = EntitySnapshot {
        entity_id: 1,
        name: "Player".into(),
        translation: Some([0.0, 0.0, 0.0]),
        rotation: None,
        scale: Some([1.0, 1.0, 1.0]),
        sprite_color: Some([1.0, 0.0, 0.0, 1.0]),
    };

    assert_eq!(snapshot.entity_id, 1);
    assert_eq!(snapshot.name, "Player");
    assert_eq!(snapshot.translation, Some([0.0, 0.0, 0.0]));
}

#[test]
fn test_entity_snapshot_serialization() {
    let snapshot = EntitySnapshot {
        entity_id: 42,
        name: "Enemy".into(),
        translation: Some([10.0, 20.0, 0.0]),
        rotation: Some([0.0, 0.0, 90.0]),
        scale: Some([2.0, 2.0, 1.0]),
        sprite_color: Some([1.0, 0.0, 0.0, 1.0]),
    };

    let json = serde_json::to_string(&snapshot).unwrap();
    let deserialized: EntitySnapshot = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.entity_id, 42);
    assert_eq!(deserialized.name, "Enemy");
    assert_eq!(deserialized.translation, Some([10.0, 20.0, 0.0]));
    assert_eq!(deserialized.sprite_color, Some([1.0, 0.0, 0.0, 1.0]));
}

#[test]
fn test_entity_snapshot_all_none() {
    let snapshot = EntitySnapshot {
        entity_id: 0,
        name: "Empty".into(),
        translation: None,
        rotation: None,
        scale: None,
        sprite_color: None,
    };

    assert!(snapshot.translation.is_none());
    assert!(snapshot.rotation.is_none());
    assert!(snapshot.scale.is_none());
    assert!(snapshot.sprite_color.is_none());
}

// ============================================================================
// RollbackOperation tests
// ============================================================================

#[test]
fn test_rollback_operation_delete_entity() {
    let op = RollbackOperation::DeleteEntity { entity_id: 99 };

    let json = serde_json::to_string(&op).unwrap();
    let deserialized: RollbackOperation = serde_json::from_str(&json).unwrap();

    match deserialized {
        RollbackOperation::DeleteEntity { entity_id } => assert_eq!(entity_id, 99),
        _ => panic!("Expected DeleteEntity"),
    }
}

#[test]
fn test_rollback_operation_restore_transform() {
    let op = RollbackOperation::RestoreTransform {
        entity_id: 1,
        translation: [10.0, 20.0, 30.0],
    };

    let json = serde_json::to_string(&op).unwrap();
    let deserialized: RollbackOperation = serde_json::from_str(&json).unwrap();

    match deserialized {
        RollbackOperation::RestoreTransform {
            entity_id,
            translation,
        } => {
            assert_eq!(entity_id, 1);
            assert_eq!(translation, [10.0, 20.0, 30.0]);
        }
        _ => panic!("Expected RestoreTransform"),
    }
}

#[test]
fn test_rollback_operation_restore_sprite_color() {
    let op = RollbackOperation::RestoreSpriteColor {
        entity_id: 3,
        rgba: [1.0, 1.0, 1.0, 1.0],
    };

    let json = serde_json::to_string(&op).unwrap();
    let deserialized: RollbackOperation = serde_json::from_str(&json).unwrap();

    match deserialized {
        RollbackOperation::RestoreSpriteColor { entity_id, rgba } => {
            assert_eq!(entity_id, 3);
            assert_eq!(rgba, [1.0, 1.0, 1.0, 1.0]);
        }
        _ => panic!("Expected RestoreSpriteColor"),
    }
}

// ============================================================================
// CommandHistory tests
// ============================================================================

#[test]
fn test_command_history_default() {
    let history = CommandHistory::default();
    assert!(history.undo_stack.is_empty());
    assert!(history.redo_stack.is_empty());
}

#[test]
fn test_command_history_push_entry() {
    let mut history = CommandHistory::default();

    let forward = vec![EngineCommand::DeleteEntity { entity_id: 1 }];
    let reverse = vec![EngineCommand::CreateEntity {
        name: "Restored".into(),
        components: vec![],
    }];

    history.undo_stack.push((forward, reverse));
    history.redo_stack.clear();

    assert_eq!(history.undo_stack.len(), 1);
    assert!(history.redo_stack.is_empty());
}

#[test]
fn test_command_history_multiple_entries() {
    let mut history = CommandHistory::default();

    for i in 0..5 {
        let forward = vec![EngineCommand::SetVisibility {
            entity_id: i,
            visible: false,
        }];
        let reverse = vec![EngineCommand::SetVisibility {
            entity_id: i,
            visible: true,
        }];
        history.undo_stack.push((forward, reverse));
    }

    assert_eq!(history.undo_stack.len(), 5);
}

// ============================================================================
// RuntimeAgentComponent tests
// ============================================================================

#[test]
fn test_runtime_agent_component_new() {
    let component = RuntimeAgentComponent::new("agent_1", "profile_default");

    assert_eq!(component.id.0, "agent_1");
    assert_eq!(component.profile_id.0, "profile_default");
    assert!(matches!(
        component.control_mode,
        RuntimeAgentControlMode::Autonomous
    ));
    assert!(matches!(component.status, RuntimeAgentStatus::Idle));
    assert!(component.tick_enabled);
    assert!(component.pending_actions.is_empty());
}

#[test]
fn test_runtime_agent_component_control_mode_manual() {
    let mut component = RuntimeAgentComponent::new("agent_2", "profile_manual");
    component.control_mode = RuntimeAgentControlMode::Manual;

    assert!(matches!(
        component.control_mode,
        RuntimeAgentControlMode::Manual
    ));
}

#[test]
fn test_runtime_agent_component_status_transitions() {
    let mut component = RuntimeAgentComponent::new("agent_3", "profile_test");
    component.status = RuntimeAgentStatus::Acting;

    assert!(matches!(component.status, RuntimeAgentStatus::Acting));
}

#[test]
fn test_runtime_agent_component_is_active() {
    let component = RuntimeAgentComponent::new("agent_4", "profile_test");
    assert!(component.is_active());
    assert!(component.can_act());
}

#[test]
fn test_runtime_agent_component_with_behavior() {
    let behavior = RuntimeBehaviorSpec::Scripted {
        script_name: "patrol".to_string(),
    };

    let component = RuntimeAgentComponent::new("agent_5", "profile_patrol").with_behavior(behavior);

    match &component.behavior {
        RuntimeBehaviorSpec::Scripted { script_name } => {
            assert_eq!(script_name, "patrol");
        }
        _ => panic!("Expected Scripted behavior"),
    }
}

#[test]
fn test_runtime_agent_component_with_control_mode() {
    let component = RuntimeAgentComponent::new("agent_6", "profile_test")
        .with_control_mode(RuntimeAgentControlMode::Manual);

    assert!(matches!(
        component.control_mode,
        RuntimeAgentControlMode::Manual
    ));
}

// ============================================================================
// RuntimeAgentId, RuntimeTarget tests
// ============================================================================

#[test]
fn test_runtime_agent_id() {
    let id = RuntimeAgentId("my_agent".to_string());
    assert_eq!(id.0, "my_agent");
}

#[test]
fn test_runtime_agent_profile_id() {
    let pid = RuntimeAgentProfileId("warrior_profile".to_string());
    assert_eq!(pid.0, "warrior_profile");
}

#[test]
fn test_runtime_target_self_entity() {
    let target = RuntimeTarget::SelfEntity;
    assert!(matches!(target, RuntimeTarget::SelfEntity));
}

#[test]
fn test_runtime_target_position() {
    let target = RuntimeTarget::Position([10.0, 20.0, 0.0]);
    match target {
        RuntimeTarget::Position(pos) => assert_eq!(pos, [10.0, 20.0, 0.0]),
        _ => panic!("Expected Position target"),
    }
}

#[test]
fn test_runtime_target_blackboard_key() {
    let target = RuntimeTarget::BlackboardKey("player_position".into());
    match target {
        RuntimeTarget::BlackboardKey(key) => assert_eq!(key, "player_position"),
        _ => panic!("Expected BlackboardKey target"),
    }
}

// ============================================================================
// RuntimeAgentAction tests
// ============================================================================

#[test]
fn test_runtime_agent_action_noop() {
    let action = RuntimeAgentAction::Noop;
    assert!(matches!(action, RuntimeAgentAction::Noop));
}

#[test]
fn test_runtime_agent_action_move_to() {
    let action = RuntimeAgentAction::MoveTo {
        target: RuntimeTarget::Position([5.0, 0.0, 0.0]),
    };

    match &action {
        RuntimeAgentAction::MoveTo { target } => match target {
            RuntimeTarget::Position(pos) => assert_eq!(*pos, [5.0, 0.0, 0.0]),
            _ => panic!("Expected Position target"),
        },
        _ => panic!("Expected MoveTo action"),
    }
}

#[test]
fn test_runtime_agent_action_play_animation() {
    let action = RuntimeAgentAction::PlayAnimation {
        name: "walk".to_string(),
    };

    match &action {
        RuntimeAgentAction::PlayAnimation { name } => assert_eq!(name, "walk"),
        _ => panic!("Expected PlayAnimation action"),
    }
}

// ============================================================================
// AssetReference and AssetType tests
// ============================================================================

#[test]
fn test_asset_type_variants() {
    assert!(matches!(AssetType::Image, AssetType::Image));
    assert!(matches!(AssetType::SpriteSheet, AssetType::SpriteSheet));
    assert!(matches!(AssetType::Scene, AssetType::Scene));
    assert!(matches!(AssetType::Mesh, AssetType::Mesh));
    assert!(matches!(AssetType::Audio, AssetType::Audio));

    let custom = AssetType::Custom("Shader".into());
    match custom {
        AssetType::Custom(s) => assert_eq!(s, "Shader"),
        _ => panic!("Expected Custom"),
    }
}

#[test]
fn test_asset_reference_basic() {
    let asset = AssetReference {
        handle: "hnd_123".into(),
        asset_type: AssetType::Image,
        path: "textures/player.png".into(),
    };

    assert_eq!(asset.handle, "hnd_123");
    assert_eq!(asset.path, "textures/player.png");
    assert!(matches!(asset.asset_type, AssetType::Image));
}

#[test]
fn test_asset_reference_serialization() {
    let asset = AssetReference {
        handle: "hnd_abc".into(),
        asset_type: AssetType::Scene,
        path: "scenes/level1.scene".into(),
    };

    let json = serde_json::to_string(&asset).unwrap();
    let deserialized: AssetReference = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.handle, "hnd_abc");
    assert_eq!(deserialized.path, "scenes/level1.scene");
}

// ============================================================================
// EngineCommandResult tests
// ============================================================================

#[test]
fn test_engine_command_result_success() {
    let result = EngineCommandResult {
        success: true,
        message: "Entity created".into(),
        entity_id: Some(42),
    };

    assert!(result.success);
    assert_eq!(result.message, "Entity created");
    assert_eq!(result.entity_id, Some(42));
}

#[test]
fn test_engine_command_result_failure() {
    let result = EngineCommandResult {
        success: false,
        message: "Entity not found".into(),
        entity_id: None,
    };

    assert!(!result.success);
    assert!(result.entity_id.is_none());
}

// ============================================================================
// CommandProcessor undo/redo tests
// ============================================================================

#[test]
fn test_spawn_prefab_reverse_is_delete() {
    use bevy::prelude::*;
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(bevy::asset::AssetPlugin::default());
    app.init_resource::<BevyAdapter>();
    app.init_resource::<CommandHistory>();
    app.init_resource::<PendingCommands>();
    app.add_plugins(CommandProcessorPlugin);

    app.world_mut()
        .resource_mut::<PendingCommands>()
        .commands
        .push(EngineCommand::SpawnPrefab {
            asset_handle: "prefabs/enemy".into(),
            transform: Some([5.0, 0.0, 0.0]),
        });
    app.update();

    let history = app.world().resource::<CommandHistory>();
    assert_eq!(
        history.undo_stack.len(),
        1,
        "SpawnPrefab should produce one undo entry"
    );
    let (_forward, reverse) = &history.undo_stack[0];
    assert!(matches!(
        reverse.as_slice(),
        [EngineCommand::DeleteEntity { .. }]
    ));
}

#[test]
fn test_set_sprite_texture_reverse_removes_added_sprite() {
    use bevy::prelude::*;
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(bevy::asset::AssetPlugin::default());
    app.init_asset::<bevy::image::Image>();
    app.init_resource::<BevyAdapter>();
    app.init_resource::<CommandHistory>();
    app.init_resource::<PendingCommands>();
    app.add_plugins(CommandProcessorPlugin);

    let e = app.world_mut().spawn(Name::new("Target")).id();
    let aid = app
        .world_mut()
        .resource_mut::<BevyAdapter>()
        .register_entity(e)
        .0;

    app.world_mut()
        .resource_mut::<PendingCommands>()
        .commands
        .push(EngineCommand::SetSpriteTexture {
            entity_id: aid,
            asset_handle: "textures/p.png".into(),
        });
    app.update();

    let history = app.world().resource::<CommandHistory>();
    assert_eq!(history.undo_stack.len(), 1);
    let (_f, reverse) = &history.undo_stack[0];
    assert!(matches!(
        reverse.as_slice(),
        [EngineCommand::RemoveComponent { component_type, .. }] if component_type == "Sprite"
    ));
}

#[test]
fn test_director_red_enemy_request_mutates_bevy_world_and_undoes() {
    use bevy::prelude::*;
    use bevy_adapter::integration::{
        SceneIndexCache, SceneIndexIncrementalPlugin, SceneIndexSceneBridge,
    };

    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(bevy::asset::AssetPlugin::default());
    app.init_resource::<BevyAdapter>();
    app.init_resource::<SceneIndexCache>();
    app.add_plugins(CommandProcessorPlugin);
    app.add_plugins(SceneIndexIncrementalPlugin::new(10_000, 10_000));

    app.update();
    let cache = app.world().resource::<SceneIndexCache>().clone();

    let mut director = DirectorRuntime::new();
    director.set_scene_bridge(Box::new(SceneIndexSceneBridge::from_cache(&cache)));

    let events = director.handle_user_request("创建一个红色敌人放在右边");
    assert!(
        events.iter().any(|event| matches!(
            event,
            agent_core::director::EditorEvent::DirectExecutionCompleted { success: true, .. }
        )),
        "request should complete through the direct execution path: {:?}",
        events
    );

    let commands: Vec<EngineCommand> = director
        .drain_bridge_commands()
        .into_iter()
        .map(|value| serde_json::from_value(value).expect("valid EngineCommand"))
        .collect();
    assert!(
        commands.iter().any(
            |command| matches!(command, EngineCommand::CreateEntity { name, .. } if name == "敌人")
        ),
        "expected create command for 敌人, got {:?}",
        commands
    );
    assert!(
        commands
            .iter()
            .any(|command| matches!(command, EngineCommand::SetSpriteColor { rgba, .. } if rgba[0] == 1.0 && rgba[1] == 0.0 && rgba[2] == 0.0)),
        "expected red sprite command, got {:?}",
        commands
    );
    assert!(
        commands
            .iter()
            .any(|command| matches!(command, EngineCommand::SetTransform { translation: Some([x, _, _]), .. } if *x > 0.0)),
        "expected right-side transform command, got {:?}",
        commands
    );

    app.world_mut()
        .resource_mut::<PendingCommands>()
        .commands
        .extend(commands);
    app.update();
    app.update();

    let mut query = app
        .world_mut()
        .query::<(&Name, &Transform, Option<&Sprite>, &AgentEntityId)>();
    let created = query
        .iter(app.world())
        .find(|(name, _, _, _)| name.as_str() == "敌人")
        .map(|(_, transform, sprite, agent_id)| {
            let agent_entity_id = agent_id.0;
            (
                transform.translation.x,
                sprite.map(|s| s.color.to_linear()),
                agent_entity_id.0,
            )
        });
    let (x, color, agent_id) = created.expect("red enemy should exist in Bevy World");
    assert!(x > 0.0, "enemy should be placed on the right, x={}", x);
    let color = color.expect("enemy should have a Sprite");
    assert_eq!(
        [color.red, color.green, color.blue, color.alpha],
        [1.0, 0.0, 0.0, 1.0]
    );

    let cache = app.world().resource::<SceneIndexCache>();
    assert!(
        cache.find_by_name("敌人").is_some(),
        "SceneIndex should observe the created enemy"
    );

    let reverse = {
        let mut history = app.world_mut().resource_mut::<CommandHistory>();
        let (_forward, reverse) = history.undo_stack.pop().expect("undo entry");
        reverse
    };
    app.world_mut()
        .resource_mut::<PendingCommands>()
        .commands
        .extend(reverse);
    app.update();
    app.update();

    assert!(
        app.world()
            .resource::<BevyAdapter>()
            .get_bevy_entity(agent_core::EntityId(agent_id))
            .is_none(),
        "undo should remove the created enemy from the adapter map"
    );
}

// ============================================================================
// Edge cases and boundary tests
// ============================================================================

#[test]
fn test_engine_command_empty_components() {
    let cmd = EngineCommand::CreateEntity {
        name: "Empty".into(),
        components: vec![],
    };

    let json = serde_json::to_string(&cmd).unwrap();
    let deserialized: EngineCommand = serde_json::from_str(&json).unwrap();

    match deserialized {
        EngineCommand::CreateEntity { name, components } => {
            assert_eq!(name, "Empty");
            assert!(components.is_empty());
        }
        _ => panic!("Expected CreateEntity"),
    }
}

#[test]
fn test_engine_command_roundtrip_all_variants() {
    let commands = vec![
        EngineCommand::CreateEntity {
            name: "A".into(),
            components: vec![],
        },
        EngineCommand::DeleteEntity { entity_id: 1 },
        EngineCommand::SetTransform {
            entity_id: 1,
            translation: None,
            rotation: None,
            scale: None,
        },
        EngineCommand::SetSpriteColor {
            entity_id: 1,
            rgba: [0.0; 4],
        },
        EngineCommand::SetVisibility {
            entity_id: 1,
            visible: true,
        },
        EngineCommand::AddComponent {
            entity_id: 1,
            component: ComponentPatch {
                type_name: "Test".into(),
                value: json!(null),
            },
        },
        EngineCommand::RemoveComponent {
            entity_id: 1,
            component_type: "Test".into(),
        },
        EngineCommand::ModifyComponent {
            entity_id: 1,
            component_type: "Test".into(),
            property: "p".into(),
            value: json!(null),
        },
        EngineCommand::SetParent {
            child_entity_id: 1,
            parent_entity_id: 2,
        },
        EngineCommand::RemoveFromParent { entity_id: 1 },
        EngineCommand::ReparentChildren {
            source_parent_id: 1,
            target_parent_id: 2,
        },
        EngineCommand::LoadAsset {
            path: "a.png".into(),
            asset_type: AssetType::Image,
        },
        EngineCommand::SetSpriteTexture {
            entity_id: 1,
            asset_handle: "h1".into(),
        },
        EngineCommand::SpawnPrefab {
            asset_handle: "p1".into(),
            transform: None,
        },
    ];

    for (i, cmd) in commands.into_iter().enumerate() {
        let json = serde_json::to_string(&cmd)
            .unwrap_or_else(|e| panic!("Failed to serialize variant {}: {:?}", i, e));
        let _: EngineCommand = serde_json::from_str(&json)
            .unwrap_or_else(|e| panic!("Failed to deserialize variant {}: {:?}", i, e));
    }
}

// ============================================================================
// T13: Multi-undo chain integration test
// ============================================================================

/// Push two forward commands in one frame, verify the undo stack records both
/// with the correct reverse commands.
#[test]
fn test_multi_undo_chain() {
    use bevy::prelude::*;
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(bevy::asset::AssetPlugin::default());
    app.init_resource::<BevyAdapter>();
    app.init_resource::<CommandHistory>();
    app.init_resource::<PendingCommands>();
    app.add_plugins(CommandProcessorPlugin);

    // Create an entity and register it
    let e = app
        .world_mut()
        .spawn((
            Name::new("Target1"),
            bevy::sprite::Sprite::from_color(
                bevy::color::Color::WHITE,
                bevy::prelude::Vec2::new(32.0, 32.0),
            ),
        ))
        .id();
    let aid = app
        .world_mut()
        .resource_mut::<BevyAdapter>()
        .register_entity(e)
        .0;

    // Push two different commands
    {
        let mut pc = app.world_mut().resource_mut::<PendingCommands>();
        pc.commands.push(EngineCommand::SetSpriteColor {
            entity_id: aid,
            rgba: [1.0, 0.0, 0.0, 1.0],
        });
        pc.commands.push(EngineCommand::SetTransform {
            entity_id: aid,
            translation: Some([100.0, 0.0, 0.0]),
            rotation: None,
            scale: None,
        });
    }

    app.update();

    // Both commands are batched into a single undo entry
    let history = app.world().resource::<CommandHistory>();
    assert_eq!(
        history.undo_stack.len(),
        1,
        "Multiple commands per frame = one undo entry"
    );

    let (forward, reverse) = &history.undo_stack[0];
    assert_eq!(forward.len(), 2, "Forward batch should have 2 commands");
    assert_eq!(reverse.len(), 2, "Reverse batch should have 2 commands");

    assert!(
        matches!(&reverse[0], EngineCommand::SetSpriteColor { .. }),
        "First reverse should be SetSpriteColor: got {:?}",
        reverse[0]
    );
    assert!(
        matches!(&reverse[1], EngineCommand::SetTransform { .. }),
        "Second reverse should be SetTransform: got {:?}",
        reverse[1]
    );
}

// ============================================================================
// T13: SceneIndex rebuild with entity deletion integration test
// ============================================================================

/// Use SceneIndex API directly (without Bevy ECS) to verify that entity
/// addition followed by removal produces a clean index.
#[test]
fn test_scene_index_add_then_remove_returns_clean_state() {
    let mut idx = scene_index::SceneIndex::new();
    assert_eq!(idx.entities_by_name.len(), 0);

    idx.add_entity("Player".into(), 1, vec![]);
    idx.add_entity("Enemy".into(), 2, vec![]);
    idx.add_entity("Npc".into(), 3, vec![]);
    assert_eq!(idx.entities_by_name.len(), 3);

    // Remove one entity
    idx.remove_entity(2);
    assert_eq!(
        idx.entities_by_name.len(),
        2,
        "Remove should drop count to 2"
    );
    assert!(idx.entities_by_name.contains_key("Player"));
    assert!(!idx.entities_by_name.contains_key("Enemy"));

    // Remove the rest
    idx.remove_entity(1);
    idx.remove_entity(3);
    assert_eq!(idx.entities_by_name.len(), 0, "All entities removed");
}

// ============================================================================
// T14: Team/Squad/HR integration tests
// ============================================================================

/// Verify that SquadAgent can be instantiated and registered with the
/// agent registry, and can route create-squad commands.
#[tokio::test]
async fn test_squad_agent_routes_to_create_squad() {
    use agent_core::registry::{Agent, AgentId, AgentRequest};
    use agent_core::squad::{SquadId, SquadRegistry};
    use agent_core::squad_agent::SquadAgent;

    let registry = SquadRegistry::new();
    let mut agent = SquadAgent::new(AgentId(100), "SquadManager".into(), SquadId(0), registry);

    let req = AgentRequest {
        task_id: Some("test_squad".into()),
        instruction: "create a squad for map editing".into(),
        context: serde_json::json!({
            "name": "MapEditingSquad",
            "leader_id": 1,
            "policy": "skill_based"
        }),
    };

    let result = agent.handle(req).await.unwrap();
    match result.result {
        agent_core::registry::AgentResultKind::Success { summary, .. } => {
            assert!(summary.contains("Squad"), "summary: {}", summary);
        }
        agent_core::registry::AgentResultKind::NeedUserInput { question } => {
            assert!(
                question.contains("squad") || question.contains("Squad"),
                "question: {}",
                question
            );
        }
        _ => {}
    }
}

/// Verify that HrAgent add/remove routes correctly through the agent
/// interface and produces the expected confirmation prompts.
#[tokio::test]
async fn test_hr_agent_add_remove_flow() {
    use agent_core::hr_agent::HrAgent;
    use agent_core::registry::{Agent, AgentId, AgentRequest};
    use agent_core::team_structure::TeamRoster;

    let roster = TeamRoster::new();
    let mut agent = HrAgent::new(AgentId(200), roster);

    let add_req = AgentRequest {
        task_id: Some("h2".into()),
        instruction: "add executor to the team".into(),
        context: serde_json::json!({"role": "executor", "name": "Executor"}),
    };

    let result = agent.handle(add_req).await.unwrap();
    assert!(
        matches!(
            result.result,
            agent_core::registry::AgentResultKind::NeedUserInput { .. }
        ),
        "Expected NeedUserInput, got: {:?}",
        result.result
    );

    let remove_req = AgentRequest {
        task_id: Some("h3".into()),
        instruction: "remove Alice from the team".into(),
        context: serde_json::json!({"agent_id": 0}),
    };

    let result = agent.handle(remove_req).await.unwrap();
    assert!(
        matches!(
            result.result,
            agent_core::registry::AgentResultKind::Failed { .. }
        ),
        "Expected Failed for unknown agent, got: {:?}",
        result.result
    );
}

/// Verify HR agent roster management: add, confirm, list, reject flow.
#[tokio::test]
async fn test_hr_agent_full_roster_lifecycle() {
    use agent_core::hr_agent::HrAgent;
    use agent_core::registry::{Agent, AgentId, AgentRequest};
    use agent_core::team_structure::TeamRoster;

    let roster = TeamRoster::new();
    let mut agent = HrAgent::new(AgentId(200), roster);

    // Step 1: Add agent
    let add_req = AgentRequest {
        task_id: Some("h10".into()),
        instruction: "add Alice to the team".into(),
        context: serde_json::json!({"role": "executor", "name": "Alice"}),
    };
    let result = agent.handle(add_req).await.unwrap();
    assert!(matches!(
        result.result,
        agent_core::registry::AgentResultKind::NeedUserInput { .. }
    ));

    // Step 2: Confirm add
    let applied = agent.confirm_pending().unwrap();
    assert!(matches!(
        applied.result,
        agent_core::registry::AgentResultKind::Success { .. }
    ));
    assert!(agent.pending_action().is_none());
    assert_eq!(agent.team_size(), 1);

    // Step 3: List team
    let list_req = AgentRequest {
        task_id: Some("h11".into()),
        instruction: "list team".into(),
        context: serde_json::json!({}),
    };
    let result = agent.handle(list_req).await.unwrap();
    assert!(matches!(
        result.result,
        agent_core::registry::AgentResultKind::Success { .. }
    ));

    // Step 4: Reject pending (no pending action)
    let rejected = agent.reject_pending();
    assert!(!rejected, "no pending action to reject");
}

/// Verify that DirectorRuntime can be instantiated and handles a basic
/// request through the dispatch pipeline.
#[test]
fn test_director_routes_add_request() {
    use agent_core::director::DirectorRuntime;

    let mut director = DirectorRuntime::new();
    let events = director.handle_user_request("add executor to the team");
    assert!(!events.is_empty(), "should produce events for add request");
}

/// Verify Squad lifecycle: create, add member, submit task, get status.
#[test]
fn test_squad_lifecycle() {
    use agent_core::registry::AgentId;
    use agent_core::squad::{RoutingPolicy, Squad, SquadId, SquadTask, TaskId};

    let mut squad = Squad::new(
        SquadId(0),
        "editing_squad".into(),
        AgentId(0),
        RoutingPolicy::default(),
    );
    assert_eq!(squad.name, "editing_squad");
    assert!(squad.members.is_empty());
    assert!(squad.task_queue.is_empty());

    squad.add_member(AgentId(1));
    squad.add_member(AgentId(2));
    assert_eq!(squad.members.len(), 2);

    let task = SquadTask::new(
        TaskId(0),
        "create_entity".into(),
        "Create an enemy entity".into(),
        vec![],
    );
    squad.submit_task(task);
    assert_eq!(squad.task_queue.len(), 1);

    assert_eq!(squad.active_tasks.len(), 0);
    assert_eq!(squad.active_task_count(), 0);
}
