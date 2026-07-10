//! Scene templates for open-world vertical slices.
//!
//! Templates bridge the gap between `OpenWorldPlan` and concrete scene writes.
//! They stay Bevy-free by emitting `SceneBridge` component patches; the
//! bevy-adapter can later translate the same entities into SceneIndex entries or
//! EngineCommand batches.

use crate::gameplay_primitive::GameplayPrimitiveKind;
use crate::open_world_plan::{
    OpenWorldObjectKind, OpenWorldObjectSpec, OpenWorldPlan, WorldZoneRole, WorldZoneSpec,
    ZoneLocation,
};
use crate::scene_bridge::{ComponentPatch, SceneBridge};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenWorldSceneTemplate {
    pub id: String,
    pub plan_id: String,
    pub entities: Vec<OpenWorldTemplateEntity>,
}

impl OpenWorldSceneTemplate {
    pub fn small_island_from_plan(plan: &OpenWorldPlan) -> Self {
        let mut entities = Vec::new();

        entities.push(OpenWorldTemplateEntity::new(
            "island_ground",
            "island_ground",
            "boundary_zone",
            [0.0, 0.0],
            vec![
                component("WorldSurface", vec![("walkable", serde_json::json!(true))]),
                component(
                    "TemplateObject",
                    vec![("template_id", serde_json::json!("open_world_small_island"))],
                ),
            ],
        ));

        for zone in &plan.world.zones {
            entities.push(OpenWorldTemplateEntity::new(
                zone.id.clone(),
                zone.id.clone(),
                zone.id.clone(),
                zone_location_position(zone.location),
                vec![
                    component(
                        "ZoneMarker",
                        vec![
                            ("zone_id", serde_json::json!(zone.id)),
                            ("role", serde_json::json!(format!("{:?}", zone.role))),
                        ],
                    ),
                    component(
                        "TemplateObject",
                        vec![("template_id", serde_json::json!("open_world_small_island"))],
                    ),
                ],
            ));
        }

        entities.push(OpenWorldTemplateEntity::new(
            "enemy_camp_marker",
            "enemy_camp_marker",
            "camp_zone",
            [120.0, 0.0],
            vec![
                component(
                    "ZoneMarker",
                    vec![
                        ("zone_id", serde_json::json!("camp_zone")),
                        (
                            "role",
                            serde_json::json!(format!("{:?}", WorldZoneRole::Encounter)),
                        ),
                    ],
                ),
                component(
                    "CampMarker",
                    vec![("encounter_id", serde_json::json!("camp_enemy_01"))],
                ),
            ],
        ));

        entities.push(OpenWorldTemplateEntity::new(
            "puzzle_anchor",
            "puzzle_anchor",
            "puzzle_zone",
            [0.0, 0.0],
            vec![
                component(
                    "PuzzleAnchor",
                    vec![("puzzle_id", serde_json::json!("puzzle_switch"))],
                ),
                component(
                    "ZoneMarker",
                    vec![("zone_id", serde_json::json!("puzzle_zone"))],
                ),
            ],
        ));

        add_manifest_objects(plan, &mut entities);

        Self {
            id: "open_world_small_island".into(),
            plan_id: plan.id.clone(),
            entities,
        }
    }

    pub fn required_zone_ids() -> &'static [&'static str] {
        &["spawn_zone", "puzzle_zone", "camp_zone", "reward_zone"]
    }

    pub fn required_gameplay_object_ids() -> &'static [&'static str] {
        &[
            "player",
            "follow_camera",
            "puzzle_switch",
            "reward_chest",
            "camp_enemy_01",
            "main_quest",
            "reward_item",
        ]
    }

    pub fn contains_required_zones(&self) -> bool {
        Self::required_zone_ids().iter().all(|zone_id| {
            self.entities
                .iter()
                .any(|entity| entity.stable_id == *zone_id)
        })
    }

    pub fn contains_required_gameplay_objects(&self) -> bool {
        Self::required_gameplay_object_ids()
            .iter()
            .all(|object_id| {
                self.entities
                    .iter()
                    .any(|entity| entity.stable_id == *object_id)
            })
    }

    pub fn apply_to_bridge(
        &self,
        bridge: &mut dyn SceneBridge,
    ) -> Result<OpenWorldTemplateApplyReport, String> {
        let mut created = Vec::new();
        for entity in &self.entities {
            let id = bridge.create_entity(
                &entity.name,
                Some(entity.position),
                entity.components.as_slice(),
            )?;
            created.push(OpenWorldTemplateCreatedEntity {
                stable_id: entity.stable_id.clone(),
                scene_entity_id: id,
            });
        }

        Ok(OpenWorldTemplateApplyReport {
            template_id: self.id.clone(),
            created,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenWorldTemplateEntity {
    pub stable_id: String,
    pub name: String,
    pub zone_id: String,
    pub position: [f64; 2],
    pub components: Vec<ComponentPatch>,
}

impl OpenWorldTemplateEntity {
    pub fn new(
        stable_id: impl Into<String>,
        name: impl Into<String>,
        zone_id: impl Into<String>,
        position: [f64; 2],
        components: Vec<ComponentPatch>,
    ) -> Self {
        Self {
            stable_id: stable_id.into(),
            name: name.into(),
            zone_id: zone_id.into(),
            position,
            components,
        }
    }

    pub fn has_primitive_marker(&self, primitive: GameplayPrimitiveKind) -> bool {
        self.components
            .iter()
            .any(|component| component.type_name == primitive.id())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenWorldTemplateApplyReport {
    pub template_id: String,
    pub created: Vec<OpenWorldTemplateCreatedEntity>,
}

impl OpenWorldTemplateApplyReport {
    pub fn revert_from_bridge(&self, bridge: &mut dyn SceneBridge) -> Result<(), String> {
        for entity in self.created.iter().rev() {
            bridge.delete_entity(entity.scene_entity_id)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenWorldTemplateCreatedEntity {
    pub stable_id: String,
    pub scene_entity_id: u64,
}

fn zone_location_position(location: ZoneLocation) -> [f64; 2] {
    match location {
        ZoneLocation::Left => [-120.0, 0.0],
        ZoneLocation::Center => [0.0, 0.0],
        ZoneLocation::Right => [120.0, 0.0],
        ZoneLocation::Behind => [0.0, -90.0],
        ZoneLocation::Outer => [0.0, 0.0],
    }
}

fn add_manifest_objects(plan: &OpenWorldPlan, entities: &mut Vec<OpenWorldTemplateEntity>) {
    for object in &plan.object_manifest {
        let patches = manifest_object_components(object);
        if let Some(existing) = entities
            .iter_mut()
            .find(|entity| entity.stable_id == object.id)
        {
            existing.zone_id = object.zone_id.clone();
            for patch in patches {
                push_missing_component(&mut existing.components, patch);
            }
            continue;
        }

        entities.push(OpenWorldTemplateEntity::new(
            object.id.clone(),
            object.id.clone(),
            object.zone_id.clone(),
            manifest_object_position(object, &plan.world.zones),
            patches,
        ));
    }
}

fn manifest_object_components(object: &OpenWorldObjectSpec) -> Vec<ComponentPatch> {
    let mut components = vec![
        component(
            "OpenWorldObject",
            vec![
                ("object_id", serde_json::json!(object.id)),
                ("kind", serde_json::json!(format!("{:?}", object.kind))),
                ("zone_id", serde_json::json!(object.zone_id)),
            ],
        ),
        component(
            "TemplateObject",
            vec![("template_id", serde_json::json!("open_world_small_island"))],
        ),
    ];

    for primitive in &object.required_primitives {
        components.push(component(primitive.id(), Vec::new()));
    }

    components
}

fn push_missing_component(components: &mut Vec<ComponentPatch>, patch: ComponentPatch) {
    if !components
        .iter()
        .any(|component| component.type_name == patch.type_name)
    {
        components.push(patch);
    }
}

fn manifest_object_position(object: &OpenWorldObjectSpec, zones: &[WorldZoneSpec]) -> [f64; 2] {
    let base = zones
        .iter()
        .find(|zone| zone.id == object.zone_id)
        .map(|zone| zone_location_position(zone.location))
        .unwrap_or([0.0, 0.0]);
    let offset = match object.kind {
        OpenWorldObjectKind::Player => [0.0, 0.0],
        OpenWorldObjectKind::Camera => [-16.0, 18.0],
        OpenWorldObjectKind::Terrain => [0.0, 0.0],
        OpenWorldObjectKind::Boundary => [0.0, 0.0],
        OpenWorldObjectKind::PuzzleSwitch => [0.0, 14.0],
        OpenWorldObjectKind::LootContainer => [0.0, -14.0],
        OpenWorldObjectKind::ZoneMarker => [0.0, 0.0],
        OpenWorldObjectKind::Enemy => [14.0, 0.0],
        OpenWorldObjectKind::Quest => [-10.0, -10.0],
        OpenWorldObjectKind::Reward => [10.0, -10.0],
    };

    [base[0] + offset[0], base[1] + offset[1]]
}

fn component(
    type_name: impl Into<String>,
    properties: Vec<(&str, serde_json::Value)>,
) -> ComponentPatch {
    ComponentPatch {
        type_name: type_name.into(),
        properties: properties
            .into_iter()
            .map(|(key, value)| (key.to_string(), value))
            .collect::<HashMap<_, _>>(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::open_world_plan::OpenWorldPlan;
    use crate::scene_bridge::{MockSceneBridge, SceneBridge};

    #[test]
    fn small_island_template_contains_required_zones() {
        let plan = OpenWorldPlan::open_world_slice01_fixture();
        let template = OpenWorldSceneTemplate::small_island_from_plan(&plan);

        assert!(template.contains_required_zones());
        assert!(template
            .entities
            .iter()
            .any(|entity| entity.stable_id == "spawn_zone"
                && entity.has_primitive_marker(GameplayPrimitiveKind::ZoneMarker)));
        assert!(template
            .entities
            .iter()
            .any(|entity| entity.stable_id == "puzzle_anchor"));
        assert!(template
            .entities
            .iter()
            .any(|entity| entity.stable_id == "enemy_camp_marker"));
    }

    #[test]
    fn small_island_template_contains_required_gameplay_objects() {
        let plan = OpenWorldPlan::open_world_slice01_fixture();
        let template = OpenWorldSceneTemplate::small_island_from_plan(&plan);

        assert!(template.contains_required_gameplay_objects());
        assert_entity_has_primitives(
            &template,
            "player",
            &[
                GameplayPrimitiveKind::PlayerController,
                GameplayPrimitiveKind::Combatant,
                GameplayPrimitiveKind::Attack,
                GameplayPrimitiveKind::Inventory,
            ],
        );
        assert_entity_has_primitives(
            &template,
            "puzzle_switch",
            &[
                GameplayPrimitiveKind::Interactable,
                GameplayPrimitiveKind::InteractionZone,
                GameplayPrimitiveKind::PuzzleSwitch,
            ],
        );
        assert_entity_has_primitives(
            &template,
            "reward_chest",
            &[
                GameplayPrimitiveKind::LootContainer,
                GameplayPrimitiveKind::Interactable,
            ],
        );
        assert_entity_has_primitives(
            &template,
            "camp_enemy_01",
            &[
                GameplayPrimitiveKind::EnemyBrain,
                GameplayPrimitiveKind::Combatant,
                GameplayPrimitiveKind::Attack,
            ],
        );
    }

    #[test]
    fn small_island_template_applies_to_scene_bridge() {
        let plan = OpenWorldPlan::open_world_slice01_fixture();
        let template = OpenWorldSceneTemplate::small_island_from_plan(&plan);
        let mut bridge = MockSceneBridge::new();

        let report = template.apply_to_bridge(&mut bridge).unwrap();

        assert_eq!(report.created.len(), template.entities.len());
        for zone_id in OpenWorldSceneTemplate::required_zone_ids() {
            let result = bridge.query_entities(Some(zone_id), None);
            assert_eq!(result.len(), 1, "expected query result for {zone_id}");
            assert!(result[0].components.contains(&"ZoneMarker".to_string()));
        }
    }

    #[test]
    fn small_island_template_applies_gameplay_objects_to_scene_bridge() {
        let plan = OpenWorldPlan::open_world_slice01_fixture();
        let template = OpenWorldSceneTemplate::small_island_from_plan(&plan);
        let mut bridge = MockSceneBridge::new();

        template.apply_to_bridge(&mut bridge).unwrap();

        for object_id in OpenWorldSceneTemplate::required_gameplay_object_ids() {
            let result = bridge.query_entities(Some(object_id), None);
            assert_eq!(result.len(), 1, "expected bridge entity for {object_id}");
            assert!(result[0]
                .components
                .contains(&"OpenWorldObject".to_string()));
        }

        let player = bridge.query_entities(Some("player"), Some("PlayerController"));
        assert_eq!(player.len(), 1);
        let chest = bridge.query_entities(Some("reward_chest"), Some("LootContainer"));
        assert_eq!(chest.len(), 1);
        let enemy = bridge.query_entities(Some("camp_enemy_01"), Some("EnemyBrain"));
        assert_eq!(enemy.len(), 1);
    }

    #[test]
    fn small_island_template_reverts_created_entities() {
        let plan = OpenWorldPlan::open_world_slice01_fixture();
        let template = OpenWorldSceneTemplate::small_island_from_plan(&plan);
        let mut bridge = MockSceneBridge::new();

        let report = template.apply_to_bridge(&mut bridge).unwrap();
        report.revert_from_bridge(&mut bridge).unwrap();

        for zone_id in OpenWorldSceneTemplate::required_zone_ids() {
            let result = bridge.query_entities(Some(zone_id), None);
            assert!(result.is_empty(), "expected {zone_id} to be removed");
        }
    }

    #[test]
    fn template_round_trips_through_json() {
        let plan = OpenWorldPlan::open_world_slice01_fixture();
        let template = OpenWorldSceneTemplate::small_island_from_plan(&plan);
        let json = serde_json::to_string_pretty(&template).unwrap();
        let decoded: OpenWorldSceneTemplate = serde_json::from_str(&json).unwrap();

        assert_eq!(decoded.id, template.id);
        assert_eq!(decoded.entities.len(), template.entities.len());
        assert!(decoded.contains_required_zones());
        assert!(decoded.contains_required_gameplay_objects());
    }

    fn assert_entity_has_primitives(
        template: &OpenWorldSceneTemplate,
        stable_id: &str,
        primitives: &[GameplayPrimitiveKind],
    ) {
        let entity = template
            .entities
            .iter()
            .find(|entity| entity.stable_id == stable_id)
            .unwrap_or_else(|| panic!("expected template entity {stable_id}"));
        assert!(entity
            .components
            .iter()
            .any(|component| component.type_name == "OpenWorldObject"));
        for primitive in primitives {
            assert!(
                entity.has_primitive_marker(*primitive),
                "expected {stable_id} to include {}",
                primitive.id()
            );
        }
    }
}
