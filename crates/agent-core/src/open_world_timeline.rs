//! UI-facing timeline DTOs for open-world verification artifacts.
//!
//! The timeline keeps the core layer independent from any frontend framework
//! while still giving UI/replay panels a stable, serializable shape.

use crate::open_world_verification::OpenWorldVerificationBundle;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenWorldTimeline {
    pub plan_id: String,
    pub scenario_id: String,
    pub ticks: Vec<OpenWorldTimelineTick>,
    pub time_evidence: Vec<String>,
    pub schedule_decisions: Vec<String>,
    pub performance_evidence: Vec<String>,
    pub screenshot_paths: Vec<String>,
    pub visual_check_evidence: Vec<String>,
}

impl OpenWorldTimeline {
    pub fn from_verification_bundle(bundle: &OpenWorldVerificationBundle) -> Self {
        let mut ticks: BTreeMap<u64, OpenWorldTimelineTick> = BTreeMap::new();
        let mut world_state = OpenWorldReplayWorldState::default();

        for event in &bundle.evidence.runtime_events {
            if let Some((tick, label)) = parse_tick_event(event) {
                world_state.apply_runtime_event(label);
                ticks
                    .entry(tick)
                    .or_insert_with(|| OpenWorldTimelineTick {
                        tick,
                        runtime_events: Vec::new(),
                        world_state: OpenWorldReplayWorldState::default(),
                    })
                    .runtime_events
                    .push(label.to_string());
                if let Some(entry) = ticks.get_mut(&tick) {
                    entry.world_state = world_state.clone();
                }
            }
        }

        Self {
            plan_id: bundle.plan_id.clone(),
            scenario_id: bundle.scenario_id.clone(),
            ticks: ticks.into_values().collect(),
            time_evidence: bundle.evidence.time_evidence.clone(),
            schedule_decisions: bundle.evidence.schedule_decisions.clone(),
            performance_evidence: bundle.evidence.performance_evidence.clone(),
            screenshot_paths: bundle.evidence.screenshot_paths.clone(),
            visual_check_evidence: bundle.evidence.visual_check_evidence.clone(),
        }
    }

    pub fn tick(&self, tick: u64) -> Option<&OpenWorldTimelineTick> {
        self.ticks.iter().find(|entry| entry.tick == tick)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenWorldTimelineTick {
    pub tick: u64,
    pub runtime_events: Vec<String>,
    pub world_state: OpenWorldReplayWorldState,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenWorldReplayWorldState {
    pub actor_zones: BTreeMap<String, String>,
    pub puzzle_states: BTreeMap<String, String>,
    pub loot_states: BTreeMap<String, String>,
    pub enemy_states: BTreeMap<String, String>,
    pub combat_health: BTreeMap<String, i32>,
    pub quest_objectives: BTreeMap<String, String>,
    pub quest_states: BTreeMap<String, String>,
    pub inventory: BTreeMap<String, BTreeSet<String>>,
}

impl OpenWorldReplayWorldState {
    fn apply_runtime_event(&mut self, label: &str) {
        if let Some((actor_id, zone_id)) = label.split_once(" is in ") {
            self.actor_zones
                .insert(actor_id.to_string(), zone_id.to_string());
            return;
        }

        if let Some(puzzle_id) = label.strip_suffix(" awaiting interaction") {
            self.puzzle_states
                .insert(puzzle_id.to_string(), "AwaitingInteraction".to_string());
            return;
        }

        if let Some(puzzle_id) = label.strip_suffix(" solved") {
            self.puzzle_states
                .insert(puzzle_id.to_string(), "Solved".to_string());
            return;
        }

        if let Some(container_id) = label.strip_suffix(" unlocked") {
            self.loot_states
                .insert(container_id.to_string(), "Unlocked".to_string());
            return;
        }

        if let Some((actor_id, container_id)) = label.split_once(" opened ") {
            self.loot_states
                .insert(container_id.to_string(), "Opened".to_string());
            self.inventory.entry(actor_id.to_string()).or_default();
            return;
        }

        if let Some((actor_id, reward_id)) = label.split_once(" collected ") {
            self.inventory
                .entry(actor_id.to_string())
                .or_default()
                .insert(reward_id.to_string());
            return;
        }

        if let Some((_, enemy_id)) = label.split_once(" aggroed ") {
            self.enemy_states
                .insert(enemy_id.to_string(), "Aggro".to_string());
            return;
        }

        if let Some((_, hit_rest)) = label.split_once(" hit ") {
            if let Some((target_id, health_rest)) = hit_rest.split_once(" damage, ") {
                if let Some((target_id, _)) = target_id.rsplit_once(" for ") {
                    if let Some((health, _)) = health_rest.split_once(" hp remaining") {
                        if let Ok(health) = health.parse::<i32>() {
                            self.combat_health.insert(target_id.to_string(), health);
                        }
                    }
                }
            }
            return;
        }

        if let Some((_, enemy_id)) = label.split_once(" defeated ") {
            self.enemy_states
                .insert(enemy_id.to_string(), "Dead".to_string());
            self.combat_health.insert(enemy_id.to_string(), 0);
            return;
        }

        if let Some(objective_id) = label
            .strip_prefix("objective ")
            .and_then(|label| label.strip_suffix(" completed"))
        {
            self.quest_objectives
                .insert(objective_id.to_string(), "Completed".to_string());
            return;
        }

        if let Some((quest_id, state)) = label.split_once(" advanced to ") {
            self.quest_states
                .insert(quest_id.to_string(), state.to_string());
        }
    }
}

fn parse_tick_event(event: &str) -> Option<(u64, &str)> {
    let rest = event.strip_prefix("tick=")?;
    let (tick, label) = rest.split_once(' ')?;
    Some((tick.parse().ok()?, label))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_tick_event_strips_tick_prefix() {
        assert_eq!(
            parse_tick_event("tick=8 player defeated camp_enemy_01"),
            Some((8, "player defeated camp_enemy_01"))
        );
        assert_eq!(parse_tick_event("player defeated camp_enemy_01"), None);
    }
}
