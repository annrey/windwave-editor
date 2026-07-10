//! Scripted playtest harness for playable vertical slices.
//!
//! This is a deterministic core-layer harness. It does not simulate Bevy input,
//! physics, or rendering; it proves that a conversation-generated
//! `OpenWorldPlan` has enough structured gameplay state to be played through.

use crate::gameplay_primitive::GameplayPrimitiveKind;
use crate::open_world_plan::OpenWorldPlan;
use crate::open_world_runtime::{
    EnemyRuntimeState, LootRuntimeState, OpenWorldRuntimeError, OpenWorldRuntimeObject,
    OpenWorldRuntimeState, PuzzleRuntimeState, QuestRuntimeState,
};
use crate::world_clock::{WorldClock, WorldClockMode, WorldTimestamp};
use chrono::{TimeZone, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlayableScenario {
    pub id: String,
    pub plan_id: String,
    pub steps: Vec<PlayableScenarioStep>,
}

impl PlayableScenario {
    pub fn open_world_slice01_main_path(plan: &OpenWorldPlan) -> Self {
        Self {
            id: "playtest_open_world_slice01_main_path".into(),
            plan_id: plan.id.clone(),
            steps: vec![
                PlayableScenarioStep::AssertQuestState {
                    quest_id: "main_quest".into(),
                    expected: QuestRuntimeState::Active,
                },
                PlayableScenarioStep::MoveActorToZone {
                    actor_id: "player".into(),
                    zone_id: "puzzle_zone".into(),
                },
                PlayableScenarioStep::Interact {
                    actor_id: "player".into(),
                    target_id: "puzzle_switch".into(),
                },
                PlayableScenarioStep::AssertPuzzleState {
                    puzzle_id: "puzzle_switch".into(),
                    expected: PuzzleRuntimeState::Solved,
                },
                PlayableScenarioStep::AssertLootState {
                    container_id: "reward_chest".into(),
                    expected: LootRuntimeState::Unlocked,
                },
                PlayableScenarioStep::MoveActorToZone {
                    actor_id: "player".into(),
                    zone_id: "camp_zone".into(),
                },
                PlayableScenarioStep::EngageEnemy {
                    actor_id: "player".into(),
                    enemy_id: "camp_enemy_01".into(),
                },
                PlayableScenarioStep::AttackUntilDefeated {
                    actor_id: "player".into(),
                    enemy_id: "camp_enemy_01".into(),
                },
                PlayableScenarioStep::AssertEnemyState {
                    enemy_id: "camp_enemy_01".into(),
                    expected: EnemyRuntimeState::Dead,
                },
                PlayableScenarioStep::OpenLootContainer {
                    actor_id: "player".into(),
                    container_id: "reward_chest".into(),
                },
                PlayableScenarioStep::CollectReward {
                    actor_id: "player".into(),
                    container_id: "reward_chest".into(),
                    reward_id: "reward_item".into(),
                },
                PlayableScenarioStep::AssertInventoryContains {
                    actor_id: "player".into(),
                    item_id: "reward_item".into(),
                },
                PlayableScenarioStep::AssertQuestState {
                    quest_id: "main_quest".into(),
                    expected: QuestRuntimeState::Completed,
                },
            ],
        }
    }

    pub fn run(&self, state: &mut PlayableScenarioState) -> PlayableScenarioReport {
        for (index, step) in self.steps.iter().enumerate() {
            if let Err(error) = state.advance_step_clock() {
                let failure = PlayableScenarioFailure::blocked(
                    "WorldClock",
                    error.to_string(),
                    "fix WorldClock mode or time policy before running this scenario",
                );
                return PlayableScenarioReport::failed(
                    self.id.clone(),
                    index,
                    step,
                    failure,
                    state,
                );
            }
            state.record_event(format!("step {}: {}", index + 1, step.label()));
            if let Err(failure) = step.execute(state) {
                return PlayableScenarioReport::failed(
                    self.id.clone(),
                    index,
                    step,
                    failure,
                    state,
                );
            }
        }

        PlayableScenarioReport::passed(self.id.clone(), self.steps.len(), state)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlayableScenarioStep {
    AssertQuestState {
        quest_id: String,
        expected: QuestRuntimeState,
    },
    MoveActorToZone {
        actor_id: String,
        zone_id: String,
    },
    Interact {
        actor_id: String,
        target_id: String,
    },
    AssertPuzzleState {
        puzzle_id: String,
        expected: PuzzleRuntimeState,
    },
    AssertLootState {
        container_id: String,
        expected: LootRuntimeState,
    },
    EngageEnemy {
        actor_id: String,
        enemy_id: String,
    },
    AttackUntilDefeated {
        actor_id: String,
        enemy_id: String,
    },
    AssertEnemyState {
        enemy_id: String,
        expected: EnemyRuntimeState,
    },
    OpenLootContainer {
        actor_id: String,
        container_id: String,
    },
    CollectReward {
        actor_id: String,
        container_id: String,
        reward_id: String,
    },
    AssertInventoryContains {
        actor_id: String,
        item_id: String,
    },
}

impl PlayableScenarioStep {
    pub fn label(&self) -> String {
        match self {
            Self::AssertQuestState { quest_id, expected } => {
                format!("assert quest '{}' is {:?}", quest_id, expected)
            }
            Self::MoveActorToZone { actor_id, zone_id } => {
                format!("move '{}' to zone '{}'", actor_id, zone_id)
            }
            Self::Interact {
                actor_id,
                target_id,
            } => {
                format!("'{}' interacts with '{}'", actor_id, target_id)
            }
            Self::AssertPuzzleState {
                puzzle_id,
                expected,
            } => {
                format!("assert puzzle '{}' is {:?}", puzzle_id, expected)
            }
            Self::AssertLootState {
                container_id,
                expected,
            } => {
                format!("assert loot '{}' is {:?}", container_id, expected)
            }
            Self::EngageEnemy { actor_id, enemy_id } => {
                format!("'{}' engages enemy '{}'", actor_id, enemy_id)
            }
            Self::AttackUntilDefeated { actor_id, enemy_id } => {
                format!("'{}' attacks '{}' until defeated", actor_id, enemy_id)
            }
            Self::AssertEnemyState { enemy_id, expected } => {
                format!("assert enemy '{}' is {:?}", enemy_id, expected)
            }
            Self::OpenLootContainer {
                actor_id,
                container_id,
            } => {
                format!("'{}' opens loot '{}'", actor_id, container_id)
            }
            Self::CollectReward {
                actor_id,
                container_id,
                reward_id,
            } => {
                format!(
                    "'{}' collects '{}' from '{}'",
                    actor_id, reward_id, container_id
                )
            }
            Self::AssertInventoryContains { actor_id, item_id } => {
                format!("assert '{}' inventory contains '{}'", actor_id, item_id)
            }
        }
    }

    fn execute(&self, state: &mut PlayableScenarioState) -> Result<(), PlayableScenarioFailure> {
        match self {
            Self::AssertQuestState { quest_id, expected } => {
                let actual = state.quest_state(quest_id)?;
                if actual == *expected {
                    Ok(())
                } else {
                    Err(PlayableScenarioFailure::state_mismatch(
                        quest_id,
                        format!("{:?}", expected),
                        format!("{:?}", actual),
                    ))
                }
            }
            Self::MoveActorToZone { actor_id, zone_id } => {
                let event_start = state.runtime.events.len();
                state.runtime.enter_zone(actor_id, zone_id)?;
                state.sync_runtime_events_since(event_start);
                Ok(())
            }
            Self::Interact {
                actor_id,
                target_id,
            } => {
                let event_start = state.runtime.events.len();
                state.runtime.interact(actor_id, target_id)?;
                state.sync_runtime_events_since(event_start);
                Ok(())
            }
            Self::AssertPuzzleState {
                puzzle_id,
                expected,
            } => {
                let actual = state.puzzle_state(puzzle_id)?;
                if actual == *expected {
                    Ok(())
                } else {
                    Err(PlayableScenarioFailure::state_mismatch(
                        puzzle_id,
                        format!("{:?}", expected),
                        format!("{:?}", actual),
                    ))
                }
            }
            Self::AssertLootState {
                container_id,
                expected,
            } => {
                let actual = state.loot_state(container_id)?;
                if actual == *expected {
                    Ok(())
                } else {
                    Err(PlayableScenarioFailure::state_mismatch(
                        container_id,
                        format!("{:?}", expected),
                        format!("{:?}", actual),
                    ))
                }
            }
            Self::EngageEnemy { actor_id, enemy_id } => {
                let event_start = state.runtime.events.len();
                state.runtime.engage_enemy(actor_id, enemy_id)?;
                state.sync_runtime_events_since(event_start);
                Ok(())
            }
            Self::AttackUntilDefeated { actor_id, enemy_id } => {
                let event_start = state.runtime.events.len();
                state.runtime.attack_until_defeated(actor_id, enemy_id)?;
                state.sync_runtime_events_since(event_start);
                Ok(())
            }
            Self::AssertEnemyState { enemy_id, expected } => {
                let actual = state.enemy_state(enemy_id)?;
                if actual == *expected {
                    Ok(())
                } else {
                    Err(PlayableScenarioFailure::state_mismatch(
                        enemy_id,
                        format!("{:?}", expected),
                        format!("{:?}", actual),
                    ))
                }
            }
            Self::OpenLootContainer {
                actor_id,
                container_id,
            } => {
                let event_start = state.runtime.events.len();
                state.runtime.open_loot(actor_id, container_id)?;
                state.sync_runtime_events_since(event_start);
                Ok(())
            }
            Self::CollectReward {
                actor_id,
                container_id,
                reward_id,
            } => {
                let event_start = state.runtime.events.len();
                state
                    .runtime
                    .collect_reward(actor_id, container_id, reward_id)?;
                state.sync_runtime_events_since(event_start);
                Ok(())
            }
            Self::AssertInventoryContains { actor_id, item_id } => {
                if state.inventory_contains(actor_id, item_id) {
                    Ok(())
                } else {
                    Err(PlayableScenarioFailure::blocked(
                        actor_id,
                        format!("inventory does not contain {}", item_id),
                        "collect reward_item before asserting inventory",
                    ))
                }
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlayableScenarioState {
    pub runtime: OpenWorldRuntimeState,
    pub clock: WorldClock,
    pub events: Vec<String>,
    pub scene_index_observations: Vec<String>,
}

impl PlayableScenarioState {
    pub fn from_plan(plan: &OpenWorldPlan) -> Self {
        Self {
            runtime: OpenWorldRuntimeState::from_plan(plan),
            clock: WorldClock::frozen_at(Utc.with_ymd_and_hms(2026, 7, 5, 12, 0, 0).unwrap()),
            events: vec!["scenario state initialized from OpenWorldPlan".into()],
            scene_index_observations: vec![
                "exists(player)".into(),
                "exists(puzzle_switch)".into(),
                "exists(reward_chest)".into(),
                "exists(camp_enemy_01)".into(),
            ],
        }
    }

    pub fn remove_primitive(&mut self, object_id: &str, primitive: GameplayPrimitiveKind) {
        self.runtime.remove_primitive(object_id, primitive);
    }

    fn quest_state(&self, quest_id: &str) -> Result<QuestRuntimeState, PlayableScenarioFailure> {
        self.runtime.quest_state(quest_id).map_err(Into::into)
    }

    fn puzzle_state(&self, puzzle_id: &str) -> Result<PuzzleRuntimeState, PlayableScenarioFailure> {
        self.runtime.puzzle_state(puzzle_id).map_err(Into::into)
    }

    fn loot_state(&self, container_id: &str) -> Result<LootRuntimeState, PlayableScenarioFailure> {
        self.runtime.loot_state(container_id).map_err(Into::into)
    }

    fn enemy_state(&self, enemy_id: &str) -> Result<EnemyRuntimeState, PlayableScenarioFailure> {
        self.runtime.enemy_state(enemy_id).map_err(Into::into)
    }

    fn inventory_contains(&self, actor_id: &str, item_id: &str) -> bool {
        self.runtime.inventory_contains(actor_id, item_id)
    }

    fn main_quest_state_label(&self) -> String {
        self.runtime
            .quest_states
            .get("main_quest")
            .map(|state| format!("{:?}", state))
            .unwrap_or_else(|| "Missing".into())
    }

    fn record_event(&mut self, event: String) {
        self.events.push(event);
    }

    fn advance_step_clock(&mut self) -> Result<(), crate::WorldClockError> {
        self.clock.advance_one_tick()?;
        self.runtime.clock = self.clock.clone();
        Ok(())
    }

    fn record_observation(&mut self, observation: String) {
        self.scene_index_observations.push(observation);
    }

    fn sync_runtime_events_since(&mut self, start: usize) {
        let events: Vec<_> = self.runtime.events[start..].to_vec();
        for event in events {
            self.record_event(event.label());
            if let Some(observation) = event.scene_index_observation() {
                self.record_observation(observation);
            }
        }
    }

    fn recent_events(&self, max: usize) -> Vec<String> {
        let start = self.events.len().saturating_sub(max);
        self.events[start..].to_vec()
    }
}

pub type PlayableObjectState = OpenWorldRuntimeObject;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlayableScenarioReport {
    pub scenario_id: String,
    pub passed: bool,
    pub completed_steps: usize,
    pub failure: Option<PlayableScenarioFailureReport>,
    pub time_evidence: PlayableScenarioTimeEvidence,
    pub final_quest_state: String,
    pub recent_events: Vec<String>,
    pub scene_index_observations: Vec<String>,
}

impl PlayableScenarioReport {
    fn passed(scenario_id: String, completed_steps: usize, state: &PlayableScenarioState) -> Self {
        Self {
            scenario_id,
            passed: true,
            completed_steps,
            failure: None,
            time_evidence: PlayableScenarioTimeEvidence::from_state(state),
            final_quest_state: state.main_quest_state_label(),
            recent_events: state.recent_events(10),
            scene_index_observations: state.scene_index_observations.clone(),
        }
    }

    fn failed(
        scenario_id: String,
        failed_step_index: usize,
        step: &PlayableScenarioStep,
        failure: PlayableScenarioFailure,
        state: &PlayableScenarioState,
    ) -> Self {
        Self {
            scenario_id,
            passed: false,
            completed_steps: failed_step_index,
            failure: Some(PlayableScenarioFailureReport {
                failed_step_index,
                step_label: step.label(),
                reason: failure.reason,
                quest_state: state.main_quest_state_label(),
                suggested_fix: failure.suggested_fix,
                time_evidence: PlayableScenarioTimeEvidence::from_state(state),
            }),
            time_evidence: PlayableScenarioTimeEvidence::from_state(state),
            final_quest_state: state.main_quest_state_label(),
            recent_events: state.recent_events(10),
            scene_index_observations: state.scene_index_observations.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlayableScenarioTimeEvidence {
    pub wall_time: String,
    pub sim_time_ms: u64,
    pub tick: u64,
    pub clock_mode: String,
}

impl PlayableScenarioTimeEvidence {
    fn from_state(state: &PlayableScenarioState) -> Self {
        Self::from_clock(&state.clock)
    }

    fn from_clock(clock: &WorldClock) -> Self {
        let WorldTimestamp {
            wall_time,
            sim_time_ms,
            tick,
        } = clock.timestamp();
        Self {
            wall_time: wall_time.to_rfc3339(),
            sim_time_ms,
            tick,
            clock_mode: match clock.mode {
                WorldClockMode::Frozen => "Frozen",
                WorldClockMode::Manual => "Manual",
                WorldClockMode::Realtime => "Realtime",
                WorldClockMode::Replay => "Replay",
            }
            .into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlayableScenarioFailureReport {
    pub failed_step_index: usize,
    pub step_label: String,
    pub reason: String,
    pub quest_state: String,
    pub suggested_fix: String,
    pub time_evidence: PlayableScenarioTimeEvidence,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PlayableScenarioFailure {
    reason: String,
    suggested_fix: String,
}

impl PlayableScenarioFailure {
    fn state_mismatch(object_id: &str, expected: String, actual: String) -> Self {
        Self {
            reason: format!(
                "{} expected state {}, but current state is {}",
                object_id, expected, actual
            ),
            suggested_fix: format!(
                "advance {} to {} before this assertion",
                object_id, expected
            ),
        }
    }

    fn blocked(object_id: &str, reason: String, suggested_fix: impl Into<String>) -> Self {
        Self {
            reason: format!("{} blocked: {}", object_id, reason),
            suggested_fix: suggested_fix.into(),
        }
    }
}

impl From<OpenWorldRuntimeError> for PlayableScenarioFailure {
    fn from(error: OpenWorldRuntimeError) -> Self {
        Self {
            reason: error.reason,
            suggested_fix: error.suggested_fix,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::open_world_plan::OpenWorldPlan;

    #[test]
    fn open_world_slice01_playtest_passes_main_path() {
        let plan = OpenWorldPlan::open_world_slice01_fixture();
        let scenario = PlayableScenario::open_world_slice01_main_path(&plan);
        let mut state = PlayableScenarioState::from_plan(&plan);

        let report = scenario.run(&mut state);

        assert!(report.passed, "{report:?}");
        assert_eq!(report.completed_steps, scenario.steps.len());
        assert_eq!(state.clock.timestamp().tick, scenario.steps.len() as u64);
        assert_eq!(report.time_evidence.tick, scenario.steps.len() as u64);
        assert_eq!(report.time_evidence.clock_mode, "Frozen");
        assert!(state
            .runtime
            .events
            .iter()
            .any(|event| event.label() == "puzzle_switch solved" && event.timestamp().tick == 3));
        assert_eq!(report.final_quest_state, "Completed");
        assert!(state.inventory_contains("player", "reward_item"));
        assert!(report
            .scene_index_observations
            .iter()
            .any(|observation| observation == "reward_chest unlocked"));
    }

    #[test]
    fn playtest_reports_missing_interactable_on_puzzle_switch() {
        let plan = OpenWorldPlan::open_world_slice01_fixture();
        let scenario = PlayableScenario::open_world_slice01_main_path(&plan);
        let mut state = PlayableScenarioState::from_plan(&plan);
        state.remove_primitive("puzzle_switch", GameplayPrimitiveKind::Interactable);

        let report = scenario.run(&mut state);
        let failure = report.failure.expect("expected failure report");

        assert!(!report.passed);
        assert_eq!(failure.failed_step_index, 2);
        assert_eq!(failure.time_evidence.tick, 3);
        assert_eq!(report.time_evidence.tick, 3);
        assert_eq!(failure.quest_state, "Active");
        assert_eq!(
            failure.reason,
            "puzzle_switch exists but is missing Interactable"
        );
        assert_eq!(failure.suggested_fix, "add Interactable to puzzle_switch");
        assert!(report
            .recent_events
            .iter()
            .any(|event| event.contains("interacts with 'puzzle_switch'")));
    }

    #[test]
    fn playtest_reports_missing_enemy_combatant() {
        let plan = OpenWorldPlan::open_world_slice01_fixture();
        let scenario = PlayableScenario::open_world_slice01_main_path(&plan);
        let mut state = PlayableScenarioState::from_plan(&plan);
        state.remove_primitive("camp_enemy_01", GameplayPrimitiveKind::Combatant);

        let report = scenario.run(&mut state);
        let failure = report.failure.expect("expected failure report");

        assert!(!report.passed);
        assert_eq!(
            failure.reason,
            "camp_enemy_01 exists but is missing Combatant"
        );
        assert_eq!(failure.suggested_fix, "add Combatant to camp_enemy_01");
    }

    #[test]
    fn scenario_round_trips_through_json() {
        let plan = OpenWorldPlan::open_world_slice01_fixture();
        let scenario = PlayableScenario::open_world_slice01_main_path(&plan);
        let json = serde_json::to_string_pretty(&scenario).unwrap();
        let decoded: PlayableScenario = serde_json::from_str(&json).unwrap();

        assert_eq!(decoded, scenario);
    }
}
