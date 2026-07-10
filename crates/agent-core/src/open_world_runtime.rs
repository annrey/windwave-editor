//! Deterministic runtime state for open-world vertical-slice gameplay.
//!
//! This module is the core-layer behavior source for quest, interaction, loot,
//! inventory, and the existing scripted enemy state transitions. It does not
//! simulate Bevy input, physics, or rendering.

use crate::gameplay_primitive::GameplayPrimitiveKind;
use crate::open_world_plan::{OpenWorldObjectKind, OpenWorldPlan, QuestObjectiveKind};
use crate::world_clock::{AgentSchedule, ScheduleDecision, WorldClock, WorldTimestamp};
use chrono::{TimeZone, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OpenWorldRuntimeState {
    pub plan_id: String,
    pub main_quest_id: Option<String>,
    pub objects: BTreeMap<String, OpenWorldRuntimeObject>,
    pub zones: BTreeSet<String>,
    pub actor_zones: BTreeMap<String, String>,
    pub quest_states: BTreeMap<String, QuestRuntimeState>,
    pub quest_objectives: BTreeMap<String, QuestObjectiveRuntime>,
    pub puzzle_states: BTreeMap<String, PuzzleRuntimeState>,
    pub loot_states: BTreeMap<String, LootRuntimeState>,
    pub enemy_states: BTreeMap<String, EnemyRuntimeState>,
    pub combatants: BTreeMap<String, CombatantRuntime>,
    pub inventory: BTreeMap<String, BTreeSet<String>>,
    pub events: Vec<OpenWorldRuntimeEvent>,
    pub clock: WorldClock,
    pub agent_schedules: BTreeMap<String, AgentSchedule>,
}

impl OpenWorldRuntimeState {
    pub fn from_plan(plan: &OpenWorldPlan) -> Self {
        let mut objects = BTreeMap::new();
        let mut actor_zones = BTreeMap::new();
        let mut puzzle_states = BTreeMap::new();
        let mut loot_states = BTreeMap::new();
        let mut enemy_states = BTreeMap::new();
        let mut combatants = BTreeMap::new();

        for object in &plan.object_manifest {
            let primitives: BTreeSet<GameplayPrimitiveKind> =
                object.required_primitives.iter().copied().collect();
            objects.insert(
                object.id.clone(),
                OpenWorldRuntimeObject {
                    id: object.id.clone(),
                    kind: object.kind,
                    zone_id: object.zone_id.clone(),
                    primitives: primitives.clone(),
                },
            );

            match object.kind {
                OpenWorldObjectKind::Player => {
                    actor_zones.insert(object.id.clone(), object.zone_id.clone());
                    if primitives.contains(&GameplayPrimitiveKind::Combatant) {
                        combatants.insert(object.id.clone(), CombatantRuntime::player(&object.id));
                    }
                }
                OpenWorldObjectKind::PuzzleSwitch => {
                    puzzle_states.insert(object.id.clone(), PuzzleRuntimeState::Locked);
                }
                OpenWorldObjectKind::LootContainer => {
                    loot_states.insert(object.id.clone(), LootRuntimeState::Locked);
                }
                OpenWorldObjectKind::Enemy => {
                    enemy_states.insert(object.id.clone(), EnemyRuntimeState::Patrol);
                    if primitives.contains(&GameplayPrimitiveKind::Combatant) {
                        combatants.insert(object.id.clone(), CombatantRuntime::enemy(&object.id));
                    }
                }
                _ => {}
            }
        }

        let zones = plan
            .world
            .zones
            .iter()
            .map(|zone| zone.id.clone())
            .collect();

        let mut quest_states = BTreeMap::new();
        quest_states.insert(plan.quest_flow.quest_id.clone(), QuestRuntimeState::Active);

        let quest_objectives = plan
            .quest_flow
            .objectives
            .iter()
            .map(|objective| {
                (
                    objective.id.clone(),
                    QuestObjectiveRuntime {
                        id: objective.id.clone(),
                        kind: objective.kind,
                        target_id: objective.target_id.clone(),
                        state: QuestObjectiveRuntimeState::Pending,
                    },
                )
            })
            .collect();

        let clock = WorldClock::frozen_at(Utc.with_ymd_and_hms(2026, 7, 5, 12, 0, 0).unwrap());
        let agent_schedules = plan
            .agent_schedules
            .iter()
            .map(|schedule| (schedule.agent_id.clone(), schedule.clone()))
            .collect();

        Self {
            plan_id: plan.id.clone(),
            main_quest_id: Some(plan.quest_flow.quest_id.clone()),
            objects,
            zones,
            actor_zones,
            quest_states,
            quest_objectives,
            puzzle_states,
            loot_states,
            enemy_states,
            combatants,
            inventory: BTreeMap::new(),
            events: vec![OpenWorldRuntimeEvent::RuntimeInitialized {
                plan_id: plan.id.clone(),
                timestamp: clock.timestamp(),
            }],
            clock,
            agent_schedules,
        }
    }

    pub fn remove_primitive(&mut self, object_id: &str, primitive: GameplayPrimitiveKind) {
        if let Some(object) = self.objects.get_mut(object_id) {
            object.primitives.remove(&primitive);
        }
    }

    pub fn enter_zone(
        &mut self,
        actor_id: &str,
        zone_id: &str,
    ) -> Result<(), OpenWorldRuntimeError> {
        self.require_object(actor_id)?;
        self.require_zone(zone_id)?;

        self.actor_zones
            .insert(actor_id.to_string(), zone_id.to_string());
        self.events.push(OpenWorldRuntimeEvent::ActorEnteredZone {
            actor_id: actor_id.to_string(),
            zone_id: zone_id.to_string(),
            timestamp: self.clock.timestamp(),
        });

        self.complete_objectives(QuestObjectiveKind::ReachZone, zone_id);
        self.advance_main_quest(QuestRuntimeState::Active);

        for puzzle_id in self.object_ids_by_kind_in_zone(OpenWorldObjectKind::PuzzleSwitch, zone_id)
        {
            if self.puzzle_states.get(puzzle_id.as_str()) == Some(&PuzzleRuntimeState::Locked) {
                self.puzzle_states
                    .insert(puzzle_id.clone(), PuzzleRuntimeState::AwaitingInteraction);
                self.events
                    .push(OpenWorldRuntimeEvent::PuzzleAwaitingInteraction {
                        puzzle_id: puzzle_id.clone(),
                        timestamp: self.clock.timestamp(),
                    });
            }
        }

        self.maybe_complete_quest();
        Ok(())
    }

    pub fn interact(
        &mut self,
        actor_id: &str,
        target_id: &str,
    ) -> Result<(), OpenWorldRuntimeError> {
        self.require_object(actor_id)?;
        self.require_component(target_id, GameplayPrimitiveKind::Interactable)?;
        self.require_component(target_id, GameplayPrimitiveKind::InteractionZone)?;
        self.require_actor_in_target_zone(actor_id, target_id)?;

        let target = self
            .objects
            .get(target_id)
            .ok_or_else(|| OpenWorldRuntimeError::missing_object(target_id))?;

        if target.kind == OpenWorldObjectKind::PuzzleSwitch {
            self.puzzle_states
                .insert(target_id.to_string(), PuzzleRuntimeState::Solved);
            self.events.push(OpenWorldRuntimeEvent::PuzzleSolved {
                puzzle_id: target_id.to_string(),
                timestamp: self.clock.timestamp(),
            });
            self.complete_objectives(QuestObjectiveKind::SolvePuzzle, target_id);
            self.advance_main_quest(QuestRuntimeState::PuzzleSolved);
            self.unlock_locked_loot_containers();
            self.maybe_complete_quest();
        }

        Ok(())
    }

    pub fn engage_enemy(
        &mut self,
        actor_id: &str,
        enemy_id: &str,
    ) -> Result<(), OpenWorldRuntimeError> {
        self.require_component(actor_id, GameplayPrimitiveKind::Combatant)?;
        self.require_component(enemy_id, GameplayPrimitiveKind::Combatant)?;
        self.require_component(enemy_id, GameplayPrimitiveKind::EnemyBrain)?;

        self.enemy_states
            .insert(enemy_id.to_string(), EnemyRuntimeState::Aggro);
        self.events.push(OpenWorldRuntimeEvent::EnemyEngaged {
            actor_id: actor_id.to_string(),
            enemy_id: enemy_id.to_string(),
            timestamp: self.clock.timestamp(),
        });
        Ok(())
    }

    pub fn attack_enemy(
        &mut self,
        actor_id: &str,
        enemy_id: &str,
    ) -> Result<CombatHitReport, OpenWorldRuntimeError> {
        self.require_component(actor_id, GameplayPrimitiveKind::Attack)?;
        self.require_component(actor_id, GameplayPrimitiveKind::Combatant)?;
        self.require_component(enemy_id, GameplayPrimitiveKind::Combatant)?;
        self.require_component(enemy_id, GameplayPrimitiveKind::EnemyBrain)?;

        match self.enemy_state(enemy_id)? {
            EnemyRuntimeState::Aggro | EnemyRuntimeState::Attacking => {}
            EnemyRuntimeState::Dead => {
                return Err(OpenWorldRuntimeError::blocked(
                    enemy_id,
                    "enemy is already Dead",
                    "do not attack an enemy after its defeat objective is complete",
                ));
            }
            actual => {
                return Err(OpenWorldRuntimeError::blocked(
                    enemy_id,
                    format!(
                        "enemy must be Aggro or Attacking before attack, got {:?}",
                        actual
                    ),
                    format!("engage {} before attacking", enemy_id),
                ));
            }
        }

        let damage = self
            .combatants
            .get(actor_id)
            .ok_or_else(|| OpenWorldRuntimeError::missing_object(actor_id))?
            .attack_damage;
        let target = self
            .combatants
            .get_mut(enemy_id)
            .ok_or_else(|| OpenWorldRuntimeError::missing_object(enemy_id))?;
        target.apply_damage(damage);

        let report = CombatHitReport {
            attacker_id: actor_id.to_string(),
            target_id: enemy_id.to_string(),
            damage,
            target_health: target.current_health,
            defeated: target.is_defeated(),
        };

        self.events.push(OpenWorldRuntimeEvent::CombatHit {
            actor_id: actor_id.to_string(),
            target_id: enemy_id.to_string(),
            damage,
            remaining_health: target.current_health,
            timestamp: self.clock.timestamp(),
        });

        if target.is_defeated() {
            self.enemy_states
                .insert(enemy_id.to_string(), EnemyRuntimeState::Dead);
            self.events.push(OpenWorldRuntimeEvent::EnemyDefeated {
                actor_id: actor_id.to_string(),
                enemy_id: enemy_id.to_string(),
                timestamp: self.clock.timestamp(),
            });
            self.complete_objectives(QuestObjectiveKind::DefeatEnemy, enemy_id);
            self.advance_main_quest(QuestRuntimeState::EnemyDefeated);
            self.maybe_complete_quest();
        } else {
            self.enemy_states
                .insert(enemy_id.to_string(), EnemyRuntimeState::Attacking);
        }

        Ok(report)
    }

    pub fn attack_until_defeated(
        &mut self,
        actor_id: &str,
        enemy_id: &str,
    ) -> Result<Vec<CombatHitReport>, OpenWorldRuntimeError> {
        let mut hits = Vec::new();
        for _ in 0..32 {
            if self.enemy_state(enemy_id)? == EnemyRuntimeState::Dead {
                return Ok(hits);
            }
            hits.push(self.attack_enemy(actor_id, enemy_id)?);
        }

        Err(OpenWorldRuntimeError::blocked(
            enemy_id,
            "enemy did not reach Dead after 32 scripted attacks",
            "increase attack damage or reduce enemy health for the vertical-slice fixture",
        ))
    }

    pub fn defeat_enemy(
        &mut self,
        actor_id: &str,
        enemy_id: &str,
    ) -> Result<(), OpenWorldRuntimeError> {
        self.attack_until_defeated(actor_id, enemy_id)?;
        Ok(())
    }

    pub fn open_loot(
        &mut self,
        actor_id: &str,
        container_id: &str,
    ) -> Result<(), OpenWorldRuntimeError> {
        self.require_object(actor_id)?;
        self.require_component(container_id, GameplayPrimitiveKind::LootContainer)?;

        match self.loot_state(container_id)? {
            LootRuntimeState::Unlocked => {
                self.loot_states
                    .insert(container_id.to_string(), LootRuntimeState::Opened);
                self.events.push(OpenWorldRuntimeEvent::LootOpened {
                    actor_id: actor_id.to_string(),
                    container_id: container_id.to_string(),
                    timestamp: self.clock.timestamp(),
                });
                Ok(())
            }
            actual => Err(OpenWorldRuntimeError::blocked(
                container_id,
                format!(
                    "loot container must be Unlocked before opening, got {:?}",
                    actual
                ),
                "solve the puzzle before opening the reward chest",
            )),
        }
    }

    pub fn collect_reward(
        &mut self,
        actor_id: &str,
        container_id: &str,
        reward_id: &str,
    ) -> Result<(), OpenWorldRuntimeError> {
        self.require_component(actor_id, GameplayPrimitiveKind::Inventory)?;
        self.require_component(container_id, GameplayPrimitiveKind::LootContainer)?;
        self.require_object(reward_id)?;

        if self.inventory_contains(actor_id, reward_id) {
            return Err(OpenWorldRuntimeError::blocked(
                reward_id,
                format!("{} has already been collected by {}", reward_id, actor_id),
                "do not collect the same reward twice",
            ));
        }

        match self.loot_state(container_id)? {
            LootRuntimeState::Opened => {
                self.inventory
                    .entry(actor_id.to_string())
                    .or_default()
                    .insert(reward_id.to_string());
                self.loot_states
                    .insert(container_id.to_string(), LootRuntimeState::LootClaimed);
                self.events.push(OpenWorldRuntimeEvent::RewardCollected {
                    actor_id: actor_id.to_string(),
                    container_id: container_id.to_string(),
                    reward_id: reward_id.to_string(),
                    timestamp: self.clock.timestamp(),
                });
                self.complete_objectives(QuestObjectiveKind::CollectReward, reward_id);
                self.advance_main_quest(QuestRuntimeState::RewardCollected);
                self.maybe_complete_quest();
                Ok(())
            }
            actual => Err(OpenWorldRuntimeError::blocked(
                container_id,
                format!(
                    "loot container must be Opened before collection, got {:?}",
                    actual
                ),
                "open the reward chest before collecting reward_item",
            )),
        }
    }

    pub fn quest_state(&self, quest_id: &str) -> Result<QuestRuntimeState, OpenWorldRuntimeError> {
        self.quest_states
            .get(quest_id)
            .copied()
            .ok_or_else(|| OpenWorldRuntimeError::missing_object(quest_id))
    }

    pub fn puzzle_state(
        &self,
        puzzle_id: &str,
    ) -> Result<PuzzleRuntimeState, OpenWorldRuntimeError> {
        self.puzzle_states
            .get(puzzle_id)
            .copied()
            .ok_or_else(|| OpenWorldRuntimeError::missing_object(puzzle_id))
    }

    pub fn loot_state(
        &self,
        container_id: &str,
    ) -> Result<LootRuntimeState, OpenWorldRuntimeError> {
        self.loot_states
            .get(container_id)
            .copied()
            .ok_or_else(|| OpenWorldRuntimeError::missing_object(container_id))
    }

    pub fn enemy_state(&self, enemy_id: &str) -> Result<EnemyRuntimeState, OpenWorldRuntimeError> {
        self.enemy_states
            .get(enemy_id)
            .copied()
            .ok_or_else(|| OpenWorldRuntimeError::missing_object(enemy_id))
    }

    pub fn combatant(&self, actor_id: &str) -> Result<&CombatantRuntime, OpenWorldRuntimeError> {
        self.combatants
            .get(actor_id)
            .ok_or_else(|| OpenWorldRuntimeError::missing_object(actor_id))
    }

    pub fn inventory_contains(&self, actor_id: &str, item_id: &str) -> bool {
        self.inventory
            .get(actor_id)
            .map(|items| items.contains(item_id))
            .unwrap_or(false)
    }

    pub fn objective_state(
        &self,
        objective_id: &str,
    ) -> Result<QuestObjectiveRuntimeState, OpenWorldRuntimeError> {
        self.quest_objectives
            .get(objective_id)
            .map(|objective| objective.state)
            .ok_or_else(|| OpenWorldRuntimeError::missing_object(objective_id))
    }

    pub fn schedule_decision(
        &self,
        agent_id: &str,
    ) -> Result<ScheduleDecision, OpenWorldRuntimeError> {
        let schedule = self
            .agent_schedules
            .get(agent_id)
            .ok_or_else(|| OpenWorldRuntimeError::missing_object(agent_id))?;
        Ok(schedule.decision_at(&self.clock.timestamp()))
    }

    fn require_object(&self, object_id: &str) -> Result<(), OpenWorldRuntimeError> {
        if self.objects.contains_key(object_id) {
            Ok(())
        } else {
            Err(OpenWorldRuntimeError::missing_object(object_id))
        }
    }

    fn require_zone(&self, zone_id: &str) -> Result<(), OpenWorldRuntimeError> {
        if self.zones.contains(zone_id) {
            Ok(())
        } else {
            Err(OpenWorldRuntimeError::blocked(
                zone_id,
                format!("zone '{}' does not exist", zone_id),
                "add the zone to OpenWorldPlan.world.zones",
            ))
        }
    }

    fn require_component(
        &self,
        object_id: &str,
        primitive: GameplayPrimitiveKind,
    ) -> Result<(), OpenWorldRuntimeError> {
        let object = self
            .objects
            .get(object_id)
            .ok_or_else(|| OpenWorldRuntimeError::missing_object(object_id))?;

        if object.primitives.contains(&primitive) {
            Ok(())
        } else {
            Err(OpenWorldRuntimeError::missing_component(
                object_id, primitive,
            ))
        }
    }

    fn require_actor_in_target_zone(
        &self,
        actor_id: &str,
        target_id: &str,
    ) -> Result<(), OpenWorldRuntimeError> {
        let actor_zone = self
            .actor_zones
            .get(actor_id)
            .ok_or_else(|| OpenWorldRuntimeError::missing_object(actor_id))?;
        let target = self
            .objects
            .get(target_id)
            .ok_or_else(|| OpenWorldRuntimeError::missing_object(target_id))?;

        if actor_zone == &target.zone_id {
            Ok(())
        } else {
            Err(OpenWorldRuntimeError::blocked(
                target_id,
                format!(
                    "{} is in {}, but {} is in {}",
                    actor_id, actor_zone, target_id, target.zone_id
                ),
                format!("move {} to {} before interacting", actor_id, target.zone_id),
            ))
        }
    }

    fn object_ids_by_kind_in_zone(&self, kind: OpenWorldObjectKind, zone_id: &str) -> Vec<String> {
        self.objects
            .values()
            .filter(|object| object.kind == kind && object.zone_id == zone_id)
            .map(|object| object.id.clone())
            .collect()
    }

    fn object_ids_by_kind(&self, kind: OpenWorldObjectKind) -> Vec<String> {
        self.objects
            .values()
            .filter(|object| object.kind == kind)
            .map(|object| object.id.clone())
            .collect()
    }

    fn unlock_locked_loot_containers(&mut self) {
        for container_id in self.object_ids_by_kind(OpenWorldObjectKind::LootContainer) {
            if self.loot_states.get(container_id.as_str()) == Some(&LootRuntimeState::Locked) {
                self.loot_states
                    .insert(container_id.clone(), LootRuntimeState::Unlocked);
                self.events.push(OpenWorldRuntimeEvent::LootUnlocked {
                    container_id: container_id.clone(),
                    timestamp: self.clock.timestamp(),
                });
            }
        }
    }

    fn complete_objectives(&mut self, kind: QuestObjectiveKind, target_id: &str) {
        let objective_ids: Vec<String> = self
            .quest_objectives
            .values()
            .filter(|objective| objective.kind == kind && objective.target_id == target_id)
            .map(|objective| objective.id.clone())
            .collect();

        for objective_id in objective_ids {
            if let Some(objective) = self.quest_objectives.get_mut(objective_id.as_str()) {
                if objective.state == QuestObjectiveRuntimeState::Pending {
                    objective.state = QuestObjectiveRuntimeState::Completed;
                    self.events
                        .push(OpenWorldRuntimeEvent::QuestObjectiveCompleted {
                            objective_id: objective.id.clone(),
                            timestamp: self.clock.timestamp(),
                        });
                }
            }
        }
    }

    fn advance_main_quest(&mut self, state: QuestRuntimeState) {
        let Some(quest_id) = self.main_quest_id.clone() else {
            return;
        };
        let current = self
            .quest_states
            .get(quest_id.as_str())
            .copied()
            .unwrap_or(QuestRuntimeState::NotStarted);

        if state.rank() >= current.rank() && current != state {
            self.quest_states.insert(quest_id.clone(), state);
            self.events.push(OpenWorldRuntimeEvent::QuestStateChanged {
                quest_id,
                state,
                timestamp: self.clock.timestamp(),
            });
        }
    }

    fn maybe_complete_quest(&mut self) {
        if self.quest_objectives.is_empty() {
            return;
        }

        let all_completed = self
            .quest_objectives
            .values()
            .all(|objective| objective.state == QuestObjectiveRuntimeState::Completed);

        if all_completed {
            self.advance_main_quest(QuestRuntimeState::Completed);
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenWorldRuntimeObject {
    pub id: String,
    pub kind: OpenWorldObjectKind,
    pub zone_id: String,
    pub primitives: BTreeSet<GameplayPrimitiveKind>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CombatantRuntime {
    pub id: String,
    pub faction: CombatFaction,
    pub max_health: i32,
    pub current_health: i32,
    pub attack_damage: i32,
}

impl CombatantRuntime {
    pub fn player(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            faction: CombatFaction::Player,
            max_health: 100,
            current_health: 100,
            attack_damage: 10,
        }
    }

    pub fn enemy(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            faction: CombatFaction::Enemy,
            max_health: 30,
            current_health: 30,
            attack_damage: 6,
        }
    }

    pub fn is_defeated(&self) -> bool {
        self.current_health == 0
    }

    fn apply_damage(&mut self, damage: i32) {
        self.current_health = (self.current_health - damage).max(0);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CombatFaction {
    Player,
    Enemy,
    Neutral,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CombatHitReport {
    pub attacker_id: String,
    pub target_id: String,
    pub damage: i32,
    pub target_health: i32,
    pub defeated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuestObjectiveRuntime {
    pub id: String,
    pub kind: QuestObjectiveKind,
    pub target_id: String,
    pub state: QuestObjectiveRuntimeState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum QuestObjectiveRuntimeState {
    Pending,
    Completed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum QuestRuntimeState {
    NotStarted,
    Active,
    PuzzleSolved,
    EnemyDefeated,
    RewardCollected,
    Completed,
    Failed,
}

impl QuestRuntimeState {
    fn rank(self) -> u8 {
        match self {
            Self::NotStarted => 0,
            Self::Active => 1,
            Self::PuzzleSolved => 2,
            Self::EnemyDefeated => 3,
            Self::RewardCollected => 4,
            Self::Completed => 5,
            Self::Failed => 6,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PuzzleRuntimeState {
    Locked,
    AwaitingInteraction,
    Solved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LootRuntimeState {
    Locked,
    Unlocked,
    Opened,
    LootClaimed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EnemyRuntimeState {
    Idle,
    Patrol,
    Aggro,
    Attacking,
    Dead,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OpenWorldRuntimeEvent {
    RuntimeInitialized {
        plan_id: String,
        timestamp: WorldTimestamp,
    },
    ActorEnteredZone {
        actor_id: String,
        zone_id: String,
        timestamp: WorldTimestamp,
    },
    PuzzleAwaitingInteraction {
        puzzle_id: String,
        timestamp: WorldTimestamp,
    },
    PuzzleSolved {
        puzzle_id: String,
        timestamp: WorldTimestamp,
    },
    LootUnlocked {
        container_id: String,
        timestamp: WorldTimestamp,
    },
    LootOpened {
        actor_id: String,
        container_id: String,
        timestamp: WorldTimestamp,
    },
    RewardCollected {
        actor_id: String,
        container_id: String,
        reward_id: String,
        timestamp: WorldTimestamp,
    },
    EnemyEngaged {
        actor_id: String,
        enemy_id: String,
        timestamp: WorldTimestamp,
    },
    CombatHit {
        actor_id: String,
        target_id: String,
        damage: i32,
        remaining_health: i32,
        timestamp: WorldTimestamp,
    },
    EnemyDefeated {
        actor_id: String,
        enemy_id: String,
        timestamp: WorldTimestamp,
    },
    QuestObjectiveCompleted {
        objective_id: String,
        timestamp: WorldTimestamp,
    },
    QuestStateChanged {
        quest_id: String,
        state: QuestRuntimeState,
        timestamp: WorldTimestamp,
    },
}

impl OpenWorldRuntimeEvent {
    pub fn timestamp(&self) -> &WorldTimestamp {
        match self {
            Self::RuntimeInitialized { timestamp, .. }
            | Self::ActorEnteredZone { timestamp, .. }
            | Self::PuzzleAwaitingInteraction { timestamp, .. }
            | Self::PuzzleSolved { timestamp, .. }
            | Self::LootUnlocked { timestamp, .. }
            | Self::LootOpened { timestamp, .. }
            | Self::RewardCollected { timestamp, .. }
            | Self::EnemyEngaged { timestamp, .. }
            | Self::CombatHit { timestamp, .. }
            | Self::EnemyDefeated { timestamp, .. }
            | Self::QuestObjectiveCompleted { timestamp, .. }
            | Self::QuestStateChanged { timestamp, .. } => timestamp,
        }
    }

    pub fn label(&self) -> String {
        match self {
            Self::RuntimeInitialized { plan_id, .. } => {
                format!("runtime state initialized from {}", plan_id)
            }
            Self::ActorEnteredZone {
                actor_id, zone_id, ..
            } => {
                format!("{} is in {}", actor_id, zone_id)
            }
            Self::PuzzleAwaitingInteraction { puzzle_id, .. } => {
                format!("{} awaiting interaction", puzzle_id)
            }
            Self::PuzzleSolved { puzzle_id, .. } => format!("{} solved", puzzle_id),
            Self::LootUnlocked { container_id, .. } => format!("{} unlocked", container_id),
            Self::LootOpened {
                actor_id,
                container_id,
                ..
            } => {
                format!("{} opened {}", actor_id, container_id)
            }
            Self::RewardCollected {
                actor_id,
                reward_id,
                ..
            } => {
                format!("{} collected {}", actor_id, reward_id)
            }
            Self::EnemyEngaged {
                actor_id, enemy_id, ..
            } => {
                format!("{} aggroed {}", actor_id, enemy_id)
            }
            Self::CombatHit {
                actor_id,
                target_id,
                damage,
                remaining_health,
                ..
            } => {
                format!(
                    "{} hit {} for {} damage, {} hp remaining",
                    actor_id, target_id, damage, remaining_health
                )
            }
            Self::EnemyDefeated {
                actor_id, enemy_id, ..
            } => {
                format!("{} defeated {}", actor_id, enemy_id)
            }
            Self::QuestObjectiveCompleted { objective_id, .. } => {
                format!("objective {} completed", objective_id)
            }
            Self::QuestStateChanged {
                quest_id, state, ..
            } => {
                format!("{} advanced to {:?}", quest_id, state)
            }
        }
    }

    pub fn scene_index_observation(&self) -> Option<String> {
        match self {
            Self::RuntimeInitialized { .. } => None,
            Self::ActorEnteredZone {
                actor_id, zone_id, ..
            } => Some(format!("{} is in {}", actor_id, zone_id)),
            Self::PuzzleAwaitingInteraction { puzzle_id, .. } => {
                Some(format!("{} awaiting interaction", puzzle_id))
            }
            Self::PuzzleSolved { puzzle_id, .. } => Some(format!("{} solved", puzzle_id)),
            Self::LootUnlocked { container_id, .. } => Some(format!("{} unlocked", container_id)),
            Self::LootOpened {
                actor_id,
                container_id,
                ..
            } => Some(format!("{} opened {}", actor_id, container_id)),
            Self::RewardCollected {
                actor_id,
                reward_id,
                ..
            } => Some(format!("{} collected {}", actor_id, reward_id)),
            Self::EnemyEngaged {
                actor_id, enemy_id, ..
            } => Some(format!("{} aggroed {}", actor_id, enemy_id)),
            Self::CombatHit {
                actor_id,
                target_id,
                damage,
                remaining_health,
                ..
            } => Some(format!(
                "{} hit {} damage {} hp {}",
                actor_id, target_id, damage, remaining_health
            )),
            Self::EnemyDefeated {
                actor_id, enemy_id, ..
            } => Some(format!("{} defeated {}", actor_id, enemy_id)),
            Self::QuestObjectiveCompleted { objective_id, .. } => {
                Some(format!("objective {} completed", objective_id))
            }
            Self::QuestStateChanged {
                quest_id, state, ..
            } => Some(format!("{} state {:?}", quest_id, state)),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenWorldRuntimeError {
    pub target_id: String,
    pub reason: String,
    pub suggested_fix: String,
}

impl OpenWorldRuntimeError {
    fn missing_object(object_id: &str) -> Self {
        Self {
            target_id: object_id.to_string(),
            reason: format!("object '{}' does not exist", object_id),
            suggested_fix: format!("add '{}' to OpenWorldPlan.object_manifest", object_id),
        }
    }

    fn missing_component(object_id: &str, primitive: GameplayPrimitiveKind) -> Self {
        Self {
            target_id: object_id.to_string(),
            reason: format!("{} exists but is missing {}", object_id, primitive.id()),
            suggested_fix: format!("add {} to {}", primitive.id(), object_id),
        }
    }

    fn blocked(
        target_id: &str,
        reason: impl Into<String>,
        suggested_fix: impl Into<String>,
    ) -> Self {
        Self {
            target_id: target_id.to_string(),
            reason: format!("{} blocked: {}", target_id, reason.into()),
            suggested_fix: suggested_fix.into(),
        }
    }
}

impl std::fmt::Display for OpenWorldRuntimeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.reason)
    }
}

impl std::error::Error for OpenWorldRuntimeError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::open_world_plan::{OpenWorldPlan, QuestObjectiveKind};

    #[test]
    fn runtime_solves_puzzle_unlocks_chest_and_collects_reward() {
        let plan = quest_interaction_loot_plan();
        let mut runtime = OpenWorldRuntimeState::from_plan(&plan);

        runtime.enter_zone("player", "puzzle_zone").unwrap();
        assert_eq!(
            runtime.puzzle_state("puzzle_switch").unwrap(),
            PuzzleRuntimeState::AwaitingInteraction
        );

        runtime.interact("player", "puzzle_switch").unwrap();
        assert_eq!(
            runtime.puzzle_state("puzzle_switch").unwrap(),
            PuzzleRuntimeState::Solved
        );
        assert_eq!(
            runtime.loot_state("reward_chest").unwrap(),
            LootRuntimeState::Unlocked
        );

        runtime.open_loot("player", "reward_chest").unwrap();
        runtime
            .collect_reward("player", "reward_chest", "reward_item")
            .unwrap();

        assert_eq!(
            runtime.loot_state("reward_chest").unwrap(),
            LootRuntimeState::LootClaimed
        );
        assert!(runtime.inventory_contains("player", "reward_item"));
        assert_eq!(
            runtime.quest_state("main_quest").unwrap(),
            QuestRuntimeState::Completed
        );
        assert_eq!(
            runtime.objective_state("solve_puzzle_switch").unwrap(),
            QuestObjectiveRuntimeState::Completed
        );
        assert_eq!(
            runtime.objective_state("collect_reward_item").unwrap(),
            QuestObjectiveRuntimeState::Completed
        );
    }

    #[test]
    fn runtime_events_carry_world_timestamps() {
        let plan = quest_interaction_loot_plan();
        let mut runtime = OpenWorldRuntimeState::from_plan(&plan);

        assert_eq!(runtime.events[0].timestamp().tick, 0);

        runtime.clock.advance_one_tick().unwrap();
        runtime.enter_zone("player", "puzzle_zone").unwrap();

        assert_eq!(runtime.events[1].timestamp().tick, 1);
        assert_eq!(
            runtime.events[1].timestamp().wall_time,
            runtime.clock.wall_time
        );
    }

    #[test]
    fn runtime_evaluates_merchant_schedule_from_world_clock() {
        let plan = OpenWorldPlan::open_world_slice01_fixture();
        let runtime = OpenWorldRuntimeState::from_plan(&plan);

        let decision = runtime.schedule_decision("merchant_01").unwrap();

        assert!(decision.is_active);
        assert!(decision.allows("quote_price"));
        assert!(decision.allows("restock_low_risk_item"));
        assert!(!decision.allows("transfer_high_value_asset"));
    }

    #[test]
    fn runtime_rejects_opening_locked_chest() {
        let plan = quest_interaction_loot_plan();
        let mut runtime = OpenWorldRuntimeState::from_plan(&plan);

        let error = runtime.open_loot("player", "reward_chest").unwrap_err();

        assert_eq!(
            error.reason,
            "reward_chest blocked: loot container must be Unlocked before opening, got Locked"
        );
        assert_eq!(
            error.suggested_fix,
            "solve the puzzle before opening the reward chest"
        );
    }

    #[test]
    fn runtime_prevents_duplicate_reward_claims() {
        let plan = quest_interaction_loot_plan();
        let mut runtime = OpenWorldRuntimeState::from_plan(&plan);

        runtime.enter_zone("player", "puzzle_zone").unwrap();
        runtime.interact("player", "puzzle_switch").unwrap();
        runtime.open_loot("player", "reward_chest").unwrap();
        runtime
            .collect_reward("player", "reward_chest", "reward_item")
            .unwrap();

        let error = runtime
            .collect_reward("player", "reward_chest", "reward_item")
            .unwrap_err();

        assert_eq!(
            error.reason,
            "reward_item blocked: reward_item has already been collected by player"
        );
        assert_eq!(error.suggested_fix, "do not collect the same reward twice");
    }

    #[test]
    fn runtime_reports_missing_inventory_before_reward_collection() {
        let plan = quest_interaction_loot_plan();
        let mut runtime = OpenWorldRuntimeState::from_plan(&plan);
        runtime.remove_primitive("player", GameplayPrimitiveKind::Inventory);

        runtime.enter_zone("player", "puzzle_zone").unwrap();
        runtime.interact("player", "puzzle_switch").unwrap();
        runtime.open_loot("player", "reward_chest").unwrap();
        let error = runtime
            .collect_reward("player", "reward_chest", "reward_item")
            .unwrap_err();

        assert_eq!(error.reason, "player exists but is missing Inventory");
        assert_eq!(error.suggested_fix, "add Inventory to player");
    }

    #[test]
    fn runtime_attacks_until_enemy_defeated_and_completes_objective() {
        let plan = OpenWorldPlan::open_world_slice01_fixture();
        let mut runtime = OpenWorldRuntimeState::from_plan(&plan);

        assert_eq!(
            runtime.combatant("camp_enemy_01").unwrap().current_health,
            30
        );

        runtime.engage_enemy("player", "camp_enemy_01").unwrap();
        let first_hit = runtime.attack_enemy("player", "camp_enemy_01").unwrap();

        assert_eq!(first_hit.damage, 10);
        assert_eq!(first_hit.target_health, 20);
        assert!(!first_hit.defeated);
        assert_eq!(
            runtime.enemy_state("camp_enemy_01").unwrap(),
            EnemyRuntimeState::Attacking
        );
        assert_eq!(
            runtime.objective_state("defeat_camp_enemy").unwrap(),
            QuestObjectiveRuntimeState::Pending
        );

        let remaining_hits = runtime
            .attack_until_defeated("player", "camp_enemy_01")
            .unwrap();

        assert_eq!(remaining_hits.len(), 2);
        assert_eq!(
            runtime.combatant("camp_enemy_01").unwrap().current_health,
            0
        );
        assert_eq!(
            runtime.enemy_state("camp_enemy_01").unwrap(),
            EnemyRuntimeState::Dead
        );
        assert_eq!(
            runtime.objective_state("defeat_camp_enemy").unwrap(),
            QuestObjectiveRuntimeState::Completed
        );
        assert_eq!(
            runtime.quest_state("main_quest").unwrap(),
            QuestRuntimeState::EnemyDefeated
        );
    }

    #[test]
    fn runtime_rejects_attack_before_enemy_engagement() {
        let plan = OpenWorldPlan::open_world_slice01_fixture();
        let mut runtime = OpenWorldRuntimeState::from_plan(&plan);

        let error = runtime.attack_enemy("player", "camp_enemy_01").unwrap_err();

        assert_eq!(
            error.reason,
            "camp_enemy_01 blocked: enemy must be Aggro or Attacking before attack, got Patrol"
        );
        assert_eq!(error.suggested_fix, "engage camp_enemy_01 before attacking");
    }

    #[test]
    fn runtime_reports_missing_attack_primitive_before_damage() {
        let plan = OpenWorldPlan::open_world_slice01_fixture();
        let mut runtime = OpenWorldRuntimeState::from_plan(&plan);
        runtime.remove_primitive("player", GameplayPrimitiveKind::Attack);

        runtime.engage_enemy("player", "camp_enemy_01").unwrap();
        let error = runtime.attack_enemy("player", "camp_enemy_01").unwrap_err();

        assert_eq!(error.reason, "player exists but is missing Attack");
        assert_eq!(error.suggested_fix, "add Attack to player");
    }

    fn quest_interaction_loot_plan() -> OpenWorldPlan {
        let mut plan = OpenWorldPlan::open_world_slice01_fixture();
        plan.quest_flow
            .objectives
            .retain(|objective| objective.kind != QuestObjectiveKind::DefeatEnemy);
        plan
    }
}
