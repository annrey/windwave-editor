//! Bevy bridge for AiResidentSlice01 — visible merchant open/closed and guard patrol.
//!
//! Does not replace OpenWorldSlice01 main-path playtest; residents are additive
//! entities driven by a frozen WorldClock snapshot resource.

use agent_core::{
    ActionTemplateId, AdjudicationOutcome, AiResidentSandbox, ResidentIntent, ResidentRole,
    WorldClock,
};
use bevy::prelude::*;
use serde_json::Value;
use std::collections::HashMap;

use crate::scene_index::ComponentSummary;

/// Frozen clock snapshot used by resident behavior systems.
#[derive(Resource, Debug, Clone)]
pub struct AiResidentClock {
    pub clock: WorldClock,
}

impl Default for AiResidentClock {
    fn default() -> Self {
        Self {
            clock: AiResidentSandbox::slice01_fixture().clock,
        }
    }
}

#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub struct ResidentAgent {
    pub agent_id: String,
    pub role: String,
    pub zone_id: String,
}

#[derive(Component, Debug, Clone, PartialEq)]
pub struct ResidentBehaviorState {
    pub template: String,
    pub schedule_active: bool,
    pub window_id: Option<String>,
    /// Merchant shop-open signal (visible in SceneIndex).
    pub shop_open: bool,
    /// Guard patrol progress 0..1 between waypoints.
    pub patrol_t: f32,
}

#[derive(Component, Debug, Clone, PartialEq)]
pub struct GuardPatrolPath {
    pub waypoint_a: Vec3,
    pub waypoint_b: Vec3,
}

pub struct AiResidentPlugin;

impl Plugin for AiResidentPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AiResidentClock>()
            .add_systems(Update, (sync_resident_behavior_from_clock, animate_guard_patrol));
    }
}

/// Spawn merchant_01 + guard_01 for AiResidentSlice01 on the OpenWorld island.
pub fn spawn_ai_resident_slice01(world: &mut World) {
    // Dock / spawn-side merchant
    world.spawn((
        Name::new("merchant_01"),
        Transform::from_xyz(-4.0, 0.5, 0.0),
        Visibility::Visible,
        ResidentAgent {
            agent_id: "merchant_01".into(),
            role: ResidentRole::Merchant.as_str().into(),
            zone_id: "spawn_zone".into(),
        },
        ResidentBehaviorState {
            template: ActionTemplateId::IdleOffHours.as_str().into(),
            schedule_active: false,
            window_id: None,
            shop_open: false,
            patrol_t: 0.0,
        },
    ));

    // Camp-side guard with a short patrol segment
    world.spawn((
        Name::new("guard_01"),
        Transform::from_xyz(4.0, 0.5, -1.0),
        Visibility::Visible,
        ResidentAgent {
            agent_id: "guard_01".into(),
            role: ResidentRole::Guard.as_str().into(),
            zone_id: "camp_zone".into(),
        },
        ResidentBehaviorState {
            template: ActionTemplateId::IdleOffDuty.as_str().into(),
            schedule_active: false,
            window_id: None,
            shop_open: false,
            patrol_t: 0.0,
        },
        GuardPatrolPath {
            waypoint_a: Vec3::new(4.0, 0.5, -1.0),
            waypoint_b: Vec3::new(6.0, 0.5, 1.0),
        },
    ));
}

/// Apply schedule-driven adjudication into Bevy components (headless-friendly).
pub fn apply_sandbox_to_world(world: &mut World, sandbox: &AiResidentSandbox) {
    world.insert_resource(AiResidentClock {
        clock: sandbox.clock.clone(),
    });

    let mut query = world.query::<(&ResidentAgent, &mut ResidentBehaviorState)>();
    let observations: Vec<_> = query
        .iter(world)
        .map(|(agent, _)| agent.agent_id.clone())
        .collect();

    for agent_id in observations {
        let Ok(obs) = sandbox.observe(&agent_id) else {
            continue;
        };
        let template = sandbox
            .active_templates
            .get(&agent_id)
            .copied()
            .or_else(|| {
                sandbox
                    .schedule_driven_intent(&agent_id)
                    .ok()
                    .map(|intent| intent.template)
            })
            .unwrap_or(match obs.role {
                ResidentRole::Merchant => ActionTemplateId::IdleOffHours,
                ResidentRole::Guard => ActionTemplateId::IdleOffDuty,
            });

        for (agent, mut behavior) in query.iter_mut(world) {
            if agent.agent_id != agent_id {
                continue;
            }
            behavior.template = template.as_str().into();
            behavior.schedule_active = obs.schedule.is_active;
            behavior.window_id = obs.schedule.window_id.clone();
            behavior.shop_open = matches!(
                (obs.role, template),
                (
                    ResidentRole::Merchant,
                    ActionTemplateId::QuotePrice | ActionTemplateId::RestockLowRiskItem
                )
            );
        }
    }
}

fn sync_resident_behavior_from_clock(
    clock: Res<AiResidentClock>,
    mut query: Query<(&ResidentAgent, &mut ResidentBehaviorState)>,
) {
    let sandbox = AiResidentSandbox::slice01_fixture();
    // Rebuild a lightweight decision using the resource clock without mutating ledger.
    for (agent, mut behavior) in &mut query {
        let Some(schedule) = sandbox.schedules.get(&agent.agent_id) else {
            continue;
        };
        let decision = schedule.decision_at(&clock.clock.timestamp());
        behavior.schedule_active = decision.is_active;
        behavior.window_id = decision.window_id.clone();

        let template = if decision.is_active {
            decision
                .allowed_actions
                .first()
                .and_then(|a| ActionTemplateId::parse(a))
                .unwrap_or(ActionTemplateId::IdleOffHours)
        } else if agent.role == ResidentRole::Guard.as_str() {
            ActionTemplateId::IdleOffDuty
        } else {
            ActionTemplateId::IdleOffHours
        };

        behavior.template = template.as_str().into();
        behavior.shop_open = agent.role == ResidentRole::Merchant.as_str()
            && decision.is_active
            && matches!(
                template,
                ActionTemplateId::QuotePrice | ActionTemplateId::RestockLowRiskItem
            );
    }
}

fn animate_guard_patrol(
    time: Res<Time>,
    mut query: Query<(&ResidentBehaviorState, &GuardPatrolPath, &mut Transform)>,
) {
    for (behavior, path, mut transform) in &mut query {
        if behavior.template != ActionTemplateId::PatrolWaypoint.as_str() {
            // Hold post / idle: snap toward waypoint A
            transform.translation = transform.translation.lerp(path.waypoint_a, 0.2);
            continue;
        }

        let t = (time.elapsed_secs() * 0.25).sin().mul_add(0.5, 0.5);
        transform.translation = path.waypoint_a.lerp(path.waypoint_b, t);
    }
}

pub fn push_resident_scene_index_summaries(
    world: &World,
    entity: Entity,
    components: &mut Vec<ComponentSummary>,
) {
    if let Some(agent) = world.get::<ResidentAgent>(entity) {
        let mut props = HashMap::new();
        props.insert("agent_id".into(), Value::String(agent.agent_id.clone()));
        props.insert("role".into(), Value::String(agent.role.clone()));
        props.insert("zone_id".into(), Value::String(agent.zone_id.clone()));
        components.push(ComponentSummary {
            type_name: "ResidentAgent".into(),
            properties: props,
        });
    }

    if let Some(behavior) = world.get::<ResidentBehaviorState>(entity) {
        let mut props = HashMap::new();
        props.insert("template".into(), Value::String(behavior.template.clone()));
        props.insert("schedule_active".into(), Value::Bool(behavior.schedule_active));
        props.insert(
            "window_id".into(),
            behavior
                .window_id
                .clone()
                .map(Value::String)
                .unwrap_or(Value::Null),
        );
        props.insert("shop_open".into(), Value::Bool(behavior.shop_open));
        props.insert(
            "patrol_t".into(),
            Value::from(f64::from(behavior.patrol_t)),
        );
        components.push(ComponentSummary {
            type_name: "ResidentBehaviorState".into(),
            properties: props,
        });
    }
}

/// Headless helper: spawn residents, drive sandbox adjudication, sync into world.
pub fn run_ai_resident_slice01_bevy_smoke(world: &mut World) -> Vec<AdjudicationOutcome> {
    spawn_ai_resident_slice01(world);

    let mut sandbox = AiResidentSandbox::slice01_fixture();
    // Fixture defaults to shop/patrol hours (12:00).
    let mut outcomes = Vec::new();
    outcomes.push(
        sandbox
            .adjudicate(&ResidentIntent {
                agent_id: "merchant_01".into(),
                template: ActionTemplateId::QuotePrice,
            })
            .expect("merchant"),
    );
    outcomes.push(
        sandbox
            .adjudicate(&ResidentIntent {
                agent_id: "guard_01".into(),
                template: ActionTemplateId::PatrolWaypoint,
            })
            .expect("guard"),
    );
    apply_sandbox_to_world(world, &sandbox);
    outcomes
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_core::AdjudicationOutcome;

    #[test]
    fn bevy_smoke_spawns_residents_and_syncs_shop_open() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<AiResidentClock>();

        let outcomes = run_ai_resident_slice01_bevy_smoke(app.world_mut());
        assert!(outcomes.iter().any(|o| matches!(
            o,
            AdjudicationOutcome::Allowed {
                template: ActionTemplateId::QuotePrice,
                ..
            }
        )));
        assert!(outcomes.iter().any(|o| matches!(
            o,
            AdjudicationOutcome::Allowed {
                template: ActionTemplateId::PatrolWaypoint,
                ..
            }
        )));

        let mut query = app
            .world_mut()
            .query::<(&ResidentAgent, &ResidentBehaviorState)>();
        let mut found_merchant = false;
        let mut found_guard = false;
        for (agent, behavior) in query.iter(app.world()) {
            if agent.agent_id == "merchant_01" {
                found_merchant = true;
                assert!(behavior.shop_open);
                assert_eq!(behavior.template, "quote_price");
                assert_eq!(agent.zone_id, "spawn_zone");
            }
            if agent.agent_id == "guard_01" {
                found_guard = true;
                assert_eq!(behavior.template, "patrol_waypoint");
                assert_eq!(agent.zone_id, "camp_zone");
            }
        }
        assert!(found_merchant && found_guard);
    }

    #[test]
    fn scene_index_summaries_expose_agent_state() {
        let mut world = World::new();
        let entity = world
            .spawn((
                ResidentAgent {
                    agent_id: "merchant_01".into(),
                    role: "Merchant".into(),
                    zone_id: "spawn_zone".into(),
                },
                ResidentBehaviorState {
                    template: "quote_price".into(),
                    schedule_active: true,
                    window_id: Some("shop_hours".into()),
                    shop_open: true,
                    patrol_t: 0.0,
                },
            ))
            .id();

        let mut components = Vec::new();
        push_resident_scene_index_summaries(&world, entity, &mut components);
        assert!(components.iter().any(|c| c.type_name == "ResidentAgent"));
        assert!(components
            .iter()
            .any(|c| c.type_name == "ResidentBehaviorState"
                && c.properties.get("shop_open") == Some(&Value::Bool(true))));
    }

    #[test]
    fn clock_sync_closes_shop_outside_hours() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        spawn_ai_resident_slice01(app.world_mut());

        let mut sandbox = AiResidentSandbox::slice01_fixture();
        sandbox.freeze_to_hour(23);
        app.insert_resource(AiResidentClock {
            clock: sandbox.clock.clone(),
        });
        app.add_systems(Update, sync_resident_behavior_from_clock);
        app.update();

        let mut query = app
            .world_mut()
            .query::<(&ResidentAgent, &ResidentBehaviorState)>();
        for (agent, behavior) in query.iter(app.world()) {
            if agent.agent_id == "merchant_01" {
                assert!(!behavior.shop_open);
                assert!(!behavior.schedule_active);
                assert_eq!(behavior.template, "idle_off_hours");
            }
            if agent.agent_id == "guard_01" {
                assert!(!behavior.schedule_active);
                assert_eq!(behavior.template, "idle_off_duty");
            }
        }

        assert_eq!(
            app.world()
                .resource::<AiResidentClock>()
                .clock
                .wall_time
                .format("%H")
                .to_string(),
            "23"
        );
    }
}
