use crate::adapter;
use crate::scene_index::ComponentSummary;
use agent_core::{ports::scene, OpenWorldRuntimeError, OpenWorldRuntimeState};
use bevy::ecs::system::EntityCommands;
use bevy::prelude::*;
use serde_json::Value;
use std::collections::{BTreeSet, HashMap};

#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub struct OpenWorldObject {
    pub object_id: String,
    pub kind: String,
    pub zone_id: String,
}

#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub struct TemplateObject {
    pub template_id: String,
}

#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub struct ZoneMarker {
    pub zone_id: String,
    pub role: Option<String>,
}

#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub struct CampMarker {
    pub encounter_id: String,
}

#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub struct PuzzleAnchor {
    pub puzzle_id: String,
}

#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub struct OpenWorldReplayActorState {
    pub zone_id: String,
}

#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub struct OpenWorldReplayPuzzleState {
    pub state: String,
}

#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub struct OpenWorldReplayLootState {
    pub state: String,
}

#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub struct OpenWorldReplayEnemyState {
    pub state: String,
}

#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub struct OpenWorldReplayQuestState {
    pub state: String,
}

#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub struct OpenWorldReplayInventoryState {
    pub items: BTreeSet<String>,
}

#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub struct OpenWorldReplayGameplayState {
    pub visible: bool,
    pub interaction_enabled: bool,
}

macro_rules! marker_component {
    ($name:ident) => {
        #[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
        pub struct $name;
    };
}

marker_component!(PlayerController);
marker_component!(FollowCamera);
marker_component!(Interactable);
marker_component!(InteractionZone);
marker_component!(Quest);
marker_component!(QuestObjective);
marker_component!(PuzzleSwitch);
marker_component!(LootContainer);
marker_component!(Inventory);
marker_component!(Combatant);
marker_component!(Attack);
marker_component!(EnemyBrain);
marker_component!(WorldSurface);

pub fn apply_open_world_replay_state_to_world(
    world: &mut World,
    replay_state: &agent_core::OpenWorldReplayWorldState,
) {
    let mut query = world.query::<(Entity, &OpenWorldObject)>();
    let entities = query
        .iter(world)
        .map(|(entity, object)| (entity, object.object_id.clone()))
        .collect::<Vec<_>>();

    for (entity, object_id) in entities {
        let mut entity = world.entity_mut(entity);

        if let Some(zone_id) = replay_state.actor_zones.get(&object_id) {
            entity.insert(OpenWorldReplayActorState {
                zone_id: zone_id.clone(),
            });
        }

        if let Some(state) = replay_state.puzzle_states.get(&object_id) {
            entity.insert(OpenWorldReplayPuzzleState {
                state: state.clone(),
            });
        }

        if let Some(state) = replay_state.loot_states.get(&object_id) {
            entity.insert(OpenWorldReplayLootState {
                state: state.clone(),
            });
        }

        if let Some(state) = replay_state.enemy_states.get(&object_id) {
            entity.insert(OpenWorldReplayEnemyState {
                state: state.clone(),
            });
        }

        if let Some(state) = replay_state.quest_states.get(&object_id) {
            entity.insert(OpenWorldReplayQuestState {
                state: state.clone(),
            });
        }

        if let Some(items) = replay_state.inventory.get(&object_id) {
            entity.insert(OpenWorldReplayInventoryState {
                items: items.clone(),
            });
        }
    }
}

pub fn sync_open_world_replay_gameplay_state(world: &mut World) {
    let mut query = world.query::<(
        Entity,
        Option<&OpenWorldReplayEnemyState>,
        Option<&OpenWorldReplayLootState>,
    )>();
    let updates = query
        .iter(world)
        .filter_map(|(entity, enemy_state, loot_state)| {
            let gameplay_state = replay_gameplay_state(enemy_state, loot_state)?;
            Some((entity, gameplay_state))
        })
        .collect::<Vec<_>>();

    for (entity, gameplay_state) in updates {
        let mut entity = world.entity_mut(entity);
        entity.insert(if gameplay_state.visible {
            Visibility::Visible
        } else {
            Visibility::Hidden
        });
        entity.insert(gameplay_state);
    }
}

pub fn process_open_world_loot_interaction(
    world: &mut World,
    runtime: &mut OpenWorldRuntimeState,
    actor_id: &str,
    container_id: &str,
    reward_id: &str,
) -> Result<(), OpenWorldRuntimeError> {
    let container_entity = find_open_world_object_entity(world, container_id).ok_or_else(|| {
        adapter_runtime_error(container_id, "missing Bevy OpenWorldObject entity")
    })?;

    if world.get::<LootContainer>(container_entity).is_none() {
        return Err(adapter_runtime_error(
            container_id,
            "Bevy entity is missing LootContainer",
        ));
    }

    if world
        .get::<OpenWorldReplayGameplayState>(container_entity)
        .is_some_and(|state| !state.interaction_enabled)
    {
        return Err(adapter_runtime_error(
            container_id,
            "replay gameplay state has disabled interaction",
        ));
    }

    runtime.open_loot(actor_id, container_id)?;
    runtime.collect_reward(actor_id, container_id, reward_id)?;
    sync_loot_runtime_result_to_world(world, runtime, actor_id, container_id)?;
    sync_open_world_replay_gameplay_state(world);
    Ok(())
}

pub fn process_open_world_combat_interaction(
    world: &mut World,
    runtime: &mut OpenWorldRuntimeState,
    actor_id: &str,
    enemy_id: &str,
) -> Result<(), OpenWorldRuntimeError> {
    let actor_entity = find_open_world_object_entity(world, actor_id)
        .ok_or_else(|| adapter_runtime_error(actor_id, "missing Bevy actor entity"))?;
    let enemy_entity = find_open_world_object_entity(world, enemy_id)
        .ok_or_else(|| adapter_runtime_error(enemy_id, "missing Bevy enemy entity"))?;

    if world.get::<Combatant>(actor_entity).is_none() {
        return Err(adapter_runtime_error(
            actor_id,
            "Bevy actor entity is missing Combatant",
        ));
    }
    if world.get::<Combatant>(enemy_entity).is_none() {
        return Err(adapter_runtime_error(
            enemy_id,
            "Bevy enemy entity is missing Combatant",
        ));
    }
    if world.get::<EnemyBrain>(enemy_entity).is_none() {
        return Err(adapter_runtime_error(
            enemy_id,
            "Bevy enemy entity is missing EnemyBrain",
        ));
    }
    if world
        .get::<OpenWorldReplayGameplayState>(enemy_entity)
        .is_some_and(|state| !state.interaction_enabled)
    {
        return Err(adapter_runtime_error(
            enemy_id,
            "replay gameplay state has disabled interaction",
        ));
    }

    runtime.engage_enemy(actor_id, enemy_id)?;
    runtime.attack_until_defeated(actor_id, enemy_id)?;
    sync_combat_runtime_result_to_world(world, runtime, enemy_id)?;
    sync_open_world_replay_gameplay_state(world);
    Ok(())
}

fn replay_gameplay_state(
    enemy_state: Option<&OpenWorldReplayEnemyState>,
    loot_state: Option<&OpenWorldReplayLootState>,
) -> Option<OpenWorldReplayGameplayState> {
    if enemy_state.is_some_and(|state| state.state == "Dead") {
        return Some(OpenWorldReplayGameplayState {
            visible: false,
            interaction_enabled: false,
        });
    }

    if loot_state.is_some_and(|state| matches!(state.state.as_str(), "Opened" | "LootClaimed")) {
        return Some(OpenWorldReplayGameplayState {
            visible: true,
            interaction_enabled: false,
        });
    }

    None
}

fn sync_combat_runtime_result_to_world(
    world: &mut World,
    runtime: &OpenWorldRuntimeState,
    enemy_id: &str,
) -> Result<(), OpenWorldRuntimeError> {
    let enemy_entity = find_open_world_object_entity(world, enemy_id)
        .ok_or_else(|| adapter_runtime_error(enemy_id, "missing Bevy enemy entity"))?;

    let enemy_state = runtime.enemy_state(enemy_id)?;
    world
        .entity_mut(enemy_entity)
        .insert(OpenWorldReplayEnemyState {
            state: format!("{enemy_state:?}"),
        });

    Ok(())
}

fn sync_loot_runtime_result_to_world(
    world: &mut World,
    runtime: &OpenWorldRuntimeState,
    actor_id: &str,
    container_id: &str,
) -> Result<(), OpenWorldRuntimeError> {
    let container_entity = find_open_world_object_entity(world, container_id)
        .ok_or_else(|| adapter_runtime_error(container_id, "missing Bevy loot entity"))?;
    let actor_entity = find_open_world_object_entity(world, actor_id)
        .ok_or_else(|| adapter_runtime_error(actor_id, "missing Bevy actor entity"))?;

    let loot_state = runtime.loot_state(container_id)?;
    world
        .entity_mut(container_entity)
        .insert(OpenWorldReplayLootState {
            state: format!("{loot_state:?}"),
        });

    let items = runtime.inventory.get(actor_id).cloned().unwrap_or_default();
    world
        .entity_mut(actor_entity)
        .insert(OpenWorldReplayInventoryState { items });

    Ok(())
}

fn find_open_world_object_entity(world: &mut World, object_id: &str) -> Option<Entity> {
    let mut query = world.query::<(Entity, &OpenWorldObject)>();
    query
        .iter(world)
        .find_map(|(entity, object)| (object.object_id == object_id).then_some(entity))
}

fn adapter_runtime_error(target_id: &str, reason: &str) -> OpenWorldRuntimeError {
    OpenWorldRuntimeError {
        target_id: target_id.to_string(),
        reason: format!("{target_id} blocked: {reason}"),
        suggested_fix: "ensure Bevy OpenWorldObject components match OpenWorldRuntimeState".into(),
    }
}

pub fn insert_engine_component_patch(
    entity: &mut EntityWorldMut,
    patch: &adapter::ComponentPatch,
) -> bool {
    insert_component(entity, patch.type_name.as_str(), &patch.value)
}

pub fn insert_scene_component_patch(
    entity: &mut EntityCommands,
    patch: &scene::ComponentPatch,
) -> bool {
    match patch.type_name.as_str() {
        "OpenWorldObject" => {
            entity.insert(OpenWorldObject {
                object_id: string_prop(&patch.properties, "object_id"),
                kind: string_prop(&patch.properties, "kind"),
                zone_id: string_prop(&patch.properties, "zone_id"),
            });
            true
        }
        "TemplateObject" => {
            entity.insert(TemplateObject {
                template_id: string_prop(&patch.properties, "template_id"),
            });
            true
        }
        "ZoneMarker" => {
            entity.insert(ZoneMarker {
                zone_id: string_prop(&patch.properties, "zone_id"),
                role: optional_string_prop(&patch.properties, "role"),
            });
            true
        }
        "CampMarker" => {
            entity.insert(CampMarker {
                encounter_id: string_prop(&patch.properties, "encounter_id"),
            });
            true
        }
        "PuzzleAnchor" => {
            entity.insert(PuzzleAnchor {
                puzzle_id: string_prop(&patch.properties, "puzzle_id"),
            });
            true
        }
        name => insert_marker_component(entity, name),
    }
}

pub fn remove_component(entity: &mut EntityWorldMut, component_type: &str) -> bool {
    match component_type {
        "OpenWorldObject" => entity.remove::<OpenWorldObject>(),
        "TemplateObject" => entity.remove::<TemplateObject>(),
        "ZoneMarker" => entity.remove::<ZoneMarker>(),
        "CampMarker" => entity.remove::<CampMarker>(),
        "PuzzleAnchor" => entity.remove::<PuzzleAnchor>(),
        "PlayerController" => entity.remove::<PlayerController>(),
        "FollowCamera" => entity.remove::<FollowCamera>(),
        "Interactable" => entity.remove::<Interactable>(),
        "InteractionZone" => entity.remove::<InteractionZone>(),
        "Quest" => entity.remove::<Quest>(),
        "QuestObjective" => entity.remove::<QuestObjective>(),
        "PuzzleSwitch" => entity.remove::<PuzzleSwitch>(),
        "LootContainer" => entity.remove::<LootContainer>(),
        "Inventory" => entity.remove::<Inventory>(),
        "Combatant" => entity.remove::<Combatant>(),
        "Attack" => entity.remove::<Attack>(),
        "EnemyBrain" => entity.remove::<EnemyBrain>(),
        "WorldSurface" => entity.remove::<WorldSurface>(),
        _ => return false,
    };
    true
}

pub fn push_scene_index_summaries(
    world: &World,
    entity: Entity,
    components: &mut Vec<ComponentSummary>,
) {
    if let Some(component) = world.get::<OpenWorldObject>(entity) {
        components.push(summary(
            "OpenWorldObject",
            [
                ("object_id", Value::String(component.object_id.clone())),
                ("kind", Value::String(component.kind.clone())),
                ("zone_id", Value::String(component.zone_id.clone())),
            ],
        ));
    }
    if let Some(component) = world.get::<TemplateObject>(entity) {
        components.push(summary(
            "TemplateObject",
            [("template_id", Value::String(component.template_id.clone()))],
        ));
    }
    if let Some(component) = world.get::<ZoneMarker>(entity) {
        let mut properties = HashMap::new();
        properties.insert(
            "zone_id".to_string(),
            Value::String(component.zone_id.clone()),
        );
        if let Some(role) = &component.role {
            properties.insert("role".to_string(), Value::String(role.clone()));
        }
        components.push(ComponentSummary {
            type_name: "ZoneMarker".to_string(),
            properties,
        });
    }
    if let Some(component) = world.get::<CampMarker>(entity) {
        components.push(summary(
            "CampMarker",
            [(
                "encounter_id",
                Value::String(component.encounter_id.clone()),
            )],
        ));
    }
    if let Some(component) = world.get::<PuzzleAnchor>(entity) {
        components.push(summary(
            "PuzzleAnchor",
            [("puzzle_id", Value::String(component.puzzle_id.clone()))],
        ));
    }
    if let Some(component) = world.get::<OpenWorldReplayActorState>(entity) {
        components.push(summary(
            "OpenWorldReplayActorState",
            [("zone_id", Value::String(component.zone_id.clone()))],
        ));
    }
    if let Some(component) = world.get::<OpenWorldReplayPuzzleState>(entity) {
        components.push(summary(
            "OpenWorldReplayPuzzleState",
            [("state", Value::String(component.state.clone()))],
        ));
    }
    if let Some(component) = world.get::<OpenWorldReplayLootState>(entity) {
        components.push(summary(
            "OpenWorldReplayLootState",
            [("state", Value::String(component.state.clone()))],
        ));
    }
    if let Some(component) = world.get::<OpenWorldReplayEnemyState>(entity) {
        components.push(summary(
            "OpenWorldReplayEnemyState",
            [("state", Value::String(component.state.clone()))],
        ));
    }
    if let Some(component) = world.get::<OpenWorldReplayQuestState>(entity) {
        components.push(summary(
            "OpenWorldReplayQuestState",
            [("state", Value::String(component.state.clone()))],
        ));
    }
    if let Some(component) = world.get::<OpenWorldReplayInventoryState>(entity) {
        components.push(summary(
            "OpenWorldReplayInventoryState",
            [(
                "items",
                Value::Array(
                    component
                        .items
                        .iter()
                        .cloned()
                        .map(Value::String)
                        .collect::<Vec<_>>(),
                ),
            )],
        ));
    }
    if let Some(component) = world.get::<OpenWorldReplayGameplayState>(entity) {
        components.push(summary(
            "OpenWorldReplayGameplayState",
            [
                ("visible", Value::Bool(component.visible)),
                (
                    "interaction_enabled",
                    Value::Bool(component.interaction_enabled),
                ),
            ],
        ));
    }

    push_marker_summary::<PlayerController>(world, entity, components, "PlayerController");
    push_marker_summary::<FollowCamera>(world, entity, components, "FollowCamera");
    push_marker_summary::<Interactable>(world, entity, components, "Interactable");
    push_marker_summary::<InteractionZone>(world, entity, components, "InteractionZone");
    push_marker_summary::<Quest>(world, entity, components, "Quest");
    push_marker_summary::<QuestObjective>(world, entity, components, "QuestObjective");
    push_marker_summary::<PuzzleSwitch>(world, entity, components, "PuzzleSwitch");
    push_marker_summary::<LootContainer>(world, entity, components, "LootContainer");
    push_marker_summary::<Inventory>(world, entity, components, "Inventory");
    push_marker_summary::<Combatant>(world, entity, components, "Combatant");
    push_marker_summary::<Attack>(world, entity, components, "Attack");
    push_marker_summary::<EnemyBrain>(world, entity, components, "EnemyBrain");
    push_marker_summary::<WorldSurface>(world, entity, components, "WorldSurface");
}

fn insert_component(entity: &mut EntityWorldMut, type_name: &str, value: &Value) -> bool {
    match type_name {
        "OpenWorldObject" => {
            entity.insert(OpenWorldObject {
                object_id: string_value_prop(value, "object_id"),
                kind: string_value_prop(value, "kind"),
                zone_id: string_value_prop(value, "zone_id"),
            });
            true
        }
        "TemplateObject" => {
            entity.insert(TemplateObject {
                template_id: string_value_prop(value, "template_id"),
            });
            true
        }
        "ZoneMarker" => {
            entity.insert(ZoneMarker {
                zone_id: string_value_prop(value, "zone_id"),
                role: optional_string_value_prop(value, "role"),
            });
            true
        }
        "CampMarker" => {
            entity.insert(CampMarker {
                encounter_id: string_value_prop(value, "encounter_id"),
            });
            true
        }
        "PuzzleAnchor" => {
            entity.insert(PuzzleAnchor {
                puzzle_id: string_value_prop(value, "puzzle_id"),
            });
            true
        }
        "PlayerController" => insert_marker::<PlayerController>(entity),
        "FollowCamera" => insert_marker::<FollowCamera>(entity),
        "Interactable" => insert_marker::<Interactable>(entity),
        "InteractionZone" => insert_marker::<InteractionZone>(entity),
        "Quest" => insert_marker::<Quest>(entity),
        "QuestObjective" => insert_marker::<QuestObjective>(entity),
        "PuzzleSwitch" => insert_marker::<PuzzleSwitch>(entity),
        "LootContainer" => insert_marker::<LootContainer>(entity),
        "Inventory" => insert_marker::<Inventory>(entity),
        "Combatant" => insert_marker::<Combatant>(entity),
        "Attack" => insert_marker::<Attack>(entity),
        "EnemyBrain" => insert_marker::<EnemyBrain>(entity),
        "WorldSurface" => insert_marker::<WorldSurface>(entity),
        _ => false,
    }
}

fn insert_marker<T: Component + Default>(entity: &mut EntityWorldMut) -> bool {
    entity.insert(T::default());
    true
}

fn insert_marker_component(entity: &mut EntityCommands, type_name: &str) -> bool {
    match type_name {
        "PlayerController" => insert_scene_marker::<PlayerController>(entity),
        "FollowCamera" => insert_scene_marker::<FollowCamera>(entity),
        "Interactable" => insert_scene_marker::<Interactable>(entity),
        "InteractionZone" => insert_scene_marker::<InteractionZone>(entity),
        "Quest" => insert_scene_marker::<Quest>(entity),
        "QuestObjective" => insert_scene_marker::<QuestObjective>(entity),
        "PuzzleSwitch" => insert_scene_marker::<PuzzleSwitch>(entity),
        "LootContainer" => insert_scene_marker::<LootContainer>(entity),
        "Inventory" => insert_scene_marker::<Inventory>(entity),
        "Combatant" => insert_scene_marker::<Combatant>(entity),
        "Attack" => insert_scene_marker::<Attack>(entity),
        "EnemyBrain" => insert_scene_marker::<EnemyBrain>(entity),
        "WorldSurface" => insert_scene_marker::<WorldSurface>(entity),
        _ => false,
    }
}

fn insert_scene_marker<T: Component + Default>(entity: &mut EntityCommands) -> bool {
    entity.insert(T::default());
    true
}

fn push_marker_summary<T: Component>(
    world: &World,
    entity: Entity,
    components: &mut Vec<ComponentSummary>,
    type_name: &str,
) {
    if world.get::<T>(entity).is_some() {
        components.push(ComponentSummary {
            type_name: type_name.to_string(),
            properties: HashMap::new(),
        });
    }
}

fn string_prop(properties: &HashMap<String, Value>, key: &str) -> String {
    optional_string_prop(properties, key).unwrap_or_default()
}

fn optional_string_prop(properties: &HashMap<String, Value>, key: &str) -> Option<String> {
    properties
        .get(key)
        .and_then(Value::as_str)
        .map(ToString::to_string)
}

fn string_value_prop(value: &Value, key: &str) -> String {
    optional_string_value_prop(value, key).unwrap_or_default()
}

fn optional_string_value_prop(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(ToString::to_string)
}

fn summary<const N: usize>(type_name: &str, properties: [(&str, Value); N]) -> ComponentSummary {
    ComponentSummary {
        type_name: type_name.to_string(),
        properties: properties
            .into_iter()
            .map(|(key, value)| (key.to_string(), value))
            .collect(),
    }
}
