//! Structured plans for AI-generated open-world vertical slices.
//!
//! `OpenWorldPlan` is intentionally separate from `EditPlan`: it captures the
//! domain-level playable slice that an AI wants to build before lower-level
//! scene commands, Bevy components, and playtests are generated.

use crate::gameplay_primitive::{
    GameplayPrimitiveCatalog, GameplayPrimitiveKind, GameplayPrimitiveValidationError,
};
use crate::permission::OperationRisk;
use crate::world_clock::AgentSchedule;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenWorldPlan {
    pub id: String,
    pub goal: String,
    pub source_prompt: String,
    pub world: WorldSpec,
    pub object_manifest: Vec<OpenWorldObjectSpec>,
    pub gameplay_primitives: Vec<GameplayPrimitiveKind>,
    pub agent_schedules: Vec<AgentSchedule>,
    pub quest_flow: QuestFlowSpec,
    pub task_graph: OpenWorldTaskGraph,
    pub verification_goals: Vec<OpenWorldVerificationGoal>,
    pub risk: OpenWorldPlanRisk,
}

impl OpenWorldPlan {
    pub fn validate(
        &self,
        catalog: &GameplayPrimitiveCatalog,
    ) -> Result<(), OpenWorldPlanValidationError> {
        let mut problems = Vec::new();

        if let Err(error) = catalog.validate_references(&self.referenced_primitives()) {
            problems.push(OpenWorldPlanValidationProblem::MissingPrimitive {
                missing: error.missing,
            });
        }

        push_duplicate_id_problems(
            "world.zones",
            self.world.zones.iter().map(|zone| &zone.id),
            &mut problems,
        );
        push_duplicate_id_problems(
            "object_manifest",
            self.object_manifest.iter().map(|object| &object.id),
            &mut problems,
        );
        push_duplicate_id_problems(
            "task_graph.tasks",
            self.task_graph.tasks.iter().map(|task| &task.id),
            &mut problems,
        );
        push_duplicate_id_problems(
            "verification_goals",
            self.verification_goals.iter().map(|goal| &goal.id),
            &mut problems,
        );

        let zone_ids: BTreeSet<&str> = self
            .world
            .zones
            .iter()
            .map(|zone| zone.id.as_str())
            .collect();
        for object in &self.object_manifest {
            if !zone_ids.contains(object.zone_id.as_str()) {
                problems.push(OpenWorldPlanValidationProblem::MissingZone {
                    object_id: object.id.clone(),
                    zone_id: object.zone_id.clone(),
                });
            }
        }

        if let Err(problem) = self.task_graph.execution_order_ids() {
            problems.push(problem);
        }

        if problems.is_empty() {
            Ok(())
        } else {
            Err(OpenWorldPlanValidationError { problems })
        }
    }

    pub fn referenced_primitives(&self) -> Vec<GameplayPrimitiveKind> {
        let mut primitives: BTreeSet<GameplayPrimitiveKind> =
            self.gameplay_primitives.iter().copied().collect();

        for object in &self.object_manifest {
            primitives.extend(object.required_primitives.iter().copied());
        }
        for task in &self.task_graph.tasks {
            primitives.extend(task.required_primitives.iter().copied());
        }

        primitives.into_iter().collect()
    }

    pub fn open_world_slice01_fixture() -> Self {
        use GameplayPrimitiveKind::*;

        let prompt = "创建一个开放世界小岛：玩家可以第三人称移动，右侧有敌人营地，中央有一个机关谜题，完成谜题后打开宝箱，击败敌人并拿到奖励，最后在任务面板显示完成。";

        Self {
            id: "open_world_slice01".into(),
            goal: "Build a playable open-world island vertical slice.".into(),
            source_prompt: prompt.into(),
            world: WorldSpec {
                scene_id: "OpenWorldSlice01".into(),
                name: "OpenWorldSlice01".into(),
                layout: WorldLayout::SmallIsland,
                zones: vec![
                    WorldZoneSpec::new(
                        "spawn_zone",
                        "Spawn Zone",
                        WorldZoneRole::Spawn,
                        ZoneLocation::Left,
                    ),
                    WorldZoneSpec::new(
                        "puzzle_zone",
                        "Puzzle Zone",
                        WorldZoneRole::Puzzle,
                        ZoneLocation::Center,
                    ),
                    WorldZoneSpec::new(
                        "camp_zone",
                        "Enemy Camp",
                        WorldZoneRole::Encounter,
                        ZoneLocation::Right,
                    ),
                    WorldZoneSpec::new(
                        "reward_zone",
                        "Reward Zone",
                        WorldZoneRole::Reward,
                        ZoneLocation::Behind,
                    ),
                    WorldZoneSpec::new(
                        "boundary_zone",
                        "Island Boundary",
                        WorldZoneRole::Boundary,
                        ZoneLocation::Outer,
                    ),
                ],
                constraints: WorldConstraints {
                    max_logical_zones: 5,
                    max_key_entities: 20,
                    max_total_entities: 200,
                    max_enemies: 3,
                    max_main_quests: 1,
                    max_puzzles: 1,
                    max_playtest_seconds: 30,
                },
            },
            object_manifest: vec![
                OpenWorldObjectSpec::new(
                    "player",
                    OpenWorldObjectKind::Player,
                    "spawn_zone",
                    vec![PlayerController, Combatant, Attack, Inventory],
                ),
                OpenWorldObjectSpec::new(
                    "follow_camera",
                    OpenWorldObjectKind::Camera,
                    "spawn_zone",
                    vec![FollowCamera],
                ),
                OpenWorldObjectSpec::new(
                    "island_ground",
                    OpenWorldObjectKind::Terrain,
                    "boundary_zone",
                    vec![WorldSurface],
                ),
                OpenWorldObjectSpec::new(
                    "island_boundary",
                    OpenWorldObjectKind::Boundary,
                    "boundary_zone",
                    vec![ZoneMarker],
                ),
                OpenWorldObjectSpec::new(
                    "puzzle_switch",
                    OpenWorldObjectKind::PuzzleSwitch,
                    "puzzle_zone",
                    vec![Interactable, InteractionZone, PuzzleSwitch],
                ),
                OpenWorldObjectSpec::new(
                    "reward_chest",
                    OpenWorldObjectKind::LootContainer,
                    "reward_zone",
                    vec![LootContainer, Interactable, InteractionZone],
                ),
                OpenWorldObjectSpec::new(
                    "enemy_camp_marker",
                    OpenWorldObjectKind::ZoneMarker,
                    "camp_zone",
                    vec![ZoneMarker],
                ),
                OpenWorldObjectSpec::new(
                    "camp_enemy_01",
                    OpenWorldObjectKind::Enemy,
                    "camp_zone",
                    vec![EnemyBrain, Combatant, Attack],
                ),
                OpenWorldObjectSpec::new(
                    "main_quest",
                    OpenWorldObjectKind::Quest,
                    "spawn_zone",
                    vec![Quest, QuestObjective],
                ),
                OpenWorldObjectSpec::new(
                    "reward_item",
                    OpenWorldObjectKind::Reward,
                    "reward_zone",
                    vec![Inventory],
                ),
            ],
            gameplay_primitives: vec![
                PlayerController,
                FollowCamera,
                Interactable,
                InteractionZone,
                Quest,
                QuestObjective,
                PuzzleSwitch,
                LootContainer,
                Inventory,
                Combatant,
                Attack,
                EnemyBrain,
                WorldSurface,
                ZoneMarker,
            ],
            agent_schedules: vec![AgentSchedule::new("merchant_01").with_window(
                crate::ScheduleWindow::wall_time_daily(
                    "shop_hours",
                    9,
                    0,
                    21,
                    0,
                    ["quote_price", "restock_low_risk_item"],
                ),
            )],
            quest_flow: QuestFlowSpec {
                quest_id: "main_quest".into(),
                title: "Complete the island trial".into(),
                states: vec![
                    QuestStateSpec::new("NotStarted"),
                    QuestStateSpec::new("Active"),
                    QuestStateSpec::new("PuzzleSolved"),
                    QuestStateSpec::new("EnemyDefeated"),
                    QuestStateSpec::new("RewardCollected"),
                    QuestStateSpec::new("Completed"),
                ],
                objectives: vec![
                    QuestObjectiveSpec::new(
                        "reach_puzzle_zone",
                        QuestObjectiveKind::ReachZone,
                        "puzzle_zone",
                    ),
                    QuestObjectiveSpec::new(
                        "solve_puzzle_switch",
                        QuestObjectiveKind::SolvePuzzle,
                        "puzzle_switch",
                    ),
                    QuestObjectiveSpec::new(
                        "defeat_camp_enemy",
                        QuestObjectiveKind::DefeatEnemy,
                        "camp_enemy_01",
                    ),
                    QuestObjectiveSpec::new(
                        "collect_reward_item",
                        QuestObjectiveKind::CollectReward,
                        "reward_item",
                    ),
                ],
                completion_state: "Completed".into(),
            },
            task_graph: OpenWorldTaskGraph {
                tasks: vec![
                    OpenWorldTask::new(
                        "build_island_layout",
                        "Build island zones and terrain",
                        OpenWorldTaskTarget::Scene,
                        vec![WorldSurface, ZoneMarker],
                        vec![],
                    ),
                    OpenWorldTask::new(
                        "spawn_player_and_camera",
                        "Spawn player and follow camera",
                        OpenWorldTaskTarget::Gameplay,
                        vec![PlayerController, FollowCamera, Combatant, Inventory],
                        vec!["build_island_layout"],
                    ),
                    OpenWorldTask::new(
                        "add_puzzle_and_chest",
                        "Add puzzle switch and locked reward chest",
                        OpenWorldTaskTarget::Gameplay,
                        vec![Interactable, InteractionZone, PuzzleSwitch, LootContainer],
                        vec!["build_island_layout"],
                    ),
                    OpenWorldTask::new(
                        "add_enemy_camp",
                        "Add enemy camp marker and first enemy",
                        OpenWorldTaskTarget::Gameplay,
                        vec![EnemyBrain, Combatant, Attack, ZoneMarker],
                        vec!["build_island_layout"],
                    ),
                    OpenWorldTask::new(
                        "wire_main_quest",
                        "Wire objectives across puzzle, enemy, and reward",
                        OpenWorldTaskTarget::Quest,
                        vec![Quest, QuestObjective, Inventory],
                        vec![
                            "spawn_player_and_camera",
                            "add_puzzle_and_chest",
                            "add_enemy_camp",
                        ],
                    ),
                    OpenWorldTask::new(
                        "define_verification_goals",
                        "Define SceneIndex and playtest verification goals",
                        OpenWorldTaskTarget::Verification,
                        vec![Quest, QuestObjective],
                        vec!["wire_main_quest"],
                    ),
                    OpenWorldTask::new(
                        "run_main_path_playtest",
                        "Run the scripted main-path playtest",
                        OpenWorldTaskTarget::Verification,
                        vec![PlayerController, Quest, Combatant, LootContainer],
                        vec!["define_verification_goals"],
                    ),
                ],
            },
            verification_goals: vec![
                OpenWorldVerificationGoal::new(
                    "player_exists",
                    VerificationGoalKind::SceneIndex,
                    r#"exists("player")"#,
                ),
                OpenWorldVerificationGoal::new(
                    "puzzle_is_solved",
                    VerificationGoalKind::SceneIndex,
                    r#"state("puzzle_switch") == "Solved""#,
                ),
                OpenWorldVerificationGoal::new(
                    "enemy_defeated",
                    VerificationGoalKind::SceneIndex,
                    r#"state("camp_enemy_01") == "Dead""#,
                ),
                OpenWorldVerificationGoal::new(
                    "reward_collected",
                    VerificationGoalKind::SceneIndex,
                    r#"inventory_contains("player", "reward_item")"#,
                ),
                OpenWorldVerificationGoal::new(
                    "main_quest_completed",
                    VerificationGoalKind::Playtest,
                    r#"quest_state("main_quest") == "Completed""#,
                ),
            ],
            risk: OpenWorldPlanRisk {
                level: OperationRisk::MediumRisk,
                requires_approval: true,
                reasons: vec![
                    "Creates multiple scene objects".into(),
                    "Adds runtime gameplay state".into(),
                    "Defines scripted verification flow".into(),
                ],
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorldSpec {
    pub scene_id: String,
    pub name: String,
    pub layout: WorldLayout,
    pub zones: Vec<WorldZoneSpec>,
    pub constraints: WorldConstraints,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorldLayout {
    SmallIsland,
    LinearArena,
    HubAndSpoke,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorldZoneSpec {
    pub id: String,
    pub name: String,
    pub role: WorldZoneRole,
    pub location: ZoneLocation,
}

impl WorldZoneSpec {
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        role: WorldZoneRole,
        location: ZoneLocation,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            role,
            location,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorldZoneRole {
    Spawn,
    Puzzle,
    Encounter,
    Reward,
    Boundary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ZoneLocation {
    Left,
    Center,
    Right,
    Behind,
    Outer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorldConstraints {
    pub max_logical_zones: u32,
    pub max_key_entities: u32,
    pub max_total_entities: u32,
    pub max_enemies: u32,
    pub max_main_quests: u32,
    pub max_puzzles: u32,
    pub max_playtest_seconds: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenWorldObjectSpec {
    pub id: String,
    pub kind: OpenWorldObjectKind,
    pub zone_id: String,
    pub required_primitives: Vec<GameplayPrimitiveKind>,
    pub tags: Vec<String>,
}

impl OpenWorldObjectSpec {
    pub fn new(
        id: impl Into<String>,
        kind: OpenWorldObjectKind,
        zone_id: impl Into<String>,
        required_primitives: Vec<GameplayPrimitiveKind>,
    ) -> Self {
        Self {
            id: id.into(),
            kind,
            zone_id: zone_id.into(),
            required_primitives,
            tags: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OpenWorldObjectKind {
    Player,
    Camera,
    Terrain,
    Boundary,
    PuzzleSwitch,
    LootContainer,
    ZoneMarker,
    Enemy,
    Quest,
    Reward,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuestFlowSpec {
    pub quest_id: String,
    pub title: String,
    pub states: Vec<QuestStateSpec>,
    pub objectives: Vec<QuestObjectiveSpec>,
    pub completion_state: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuestStateSpec {
    pub id: String,
}

impl QuestStateSpec {
    pub fn new(id: impl Into<String>) -> Self {
        Self { id: id.into() }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuestObjectiveSpec {
    pub id: String,
    pub kind: QuestObjectiveKind,
    pub target_id: String,
}

impl QuestObjectiveSpec {
    pub fn new(
        id: impl Into<String>,
        kind: QuestObjectiveKind,
        target_id: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            kind,
            target_id: target_id.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum QuestObjectiveKind {
    ReachZone,
    SolvePuzzle,
    DefeatEnemy,
    CollectReward,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenWorldTaskGraph {
    pub tasks: Vec<OpenWorldTask>,
}

impl OpenWorldTaskGraph {
    pub fn execution_order_ids(&self) -> Result<Vec<String>, OpenWorldPlanValidationProblem> {
        let tasks_by_id: BTreeMap<&str, &OpenWorldTask> = self
            .tasks
            .iter()
            .map(|task| (task.id.as_str(), task))
            .collect();
        let mut visiting = BTreeSet::new();
        let mut visited = BTreeSet::new();
        let mut ordered = Vec::new();

        for task in &self.tasks {
            visit_task(
                task.id.as_str(),
                &tasks_by_id,
                &mut visiting,
                &mut visited,
                &mut ordered,
            )?;
        }

        Ok(ordered)
    }
}

fn visit_task(
    task_id: &str,
    tasks_by_id: &BTreeMap<&str, &OpenWorldTask>,
    visiting: &mut BTreeSet<String>,
    visited: &mut BTreeSet<String>,
    ordered: &mut Vec<String>,
) -> Result<(), OpenWorldPlanValidationProblem> {
    if visited.contains(task_id) {
        return Ok(());
    }
    if !visiting.insert(task_id.to_string()) {
        return Err(OpenWorldPlanValidationProblem::CyclicTaskDependency {
            task_id: task_id.to_string(),
        });
    }

    let task =
        tasks_by_id
            .get(task_id)
            .ok_or_else(|| OpenWorldPlanValidationProblem::MissingTask {
                task_id: task_id.to_string(),
            })?;

    for dependency in &task.depends_on {
        if !tasks_by_id.contains_key(dependency.as_str()) {
            return Err(OpenWorldPlanValidationProblem::MissingTaskDependency {
                task_id: task.id.clone(),
                dependency_id: dependency.clone(),
            });
        }
        visit_task(dependency.as_str(), tasks_by_id, visiting, visited, ordered)?;
    }

    visiting.remove(task_id);
    visited.insert(task_id.to_string());
    ordered.push(task_id.to_string());
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenWorldTask {
    pub id: String,
    pub title: String,
    pub target: OpenWorldTaskTarget,
    pub required_primitives: Vec<GameplayPrimitiveKind>,
    pub depends_on: Vec<String>,
    pub risk: OperationRisk,
}

impl OpenWorldTask {
    pub fn new(
        id: impl Into<String>,
        title: impl Into<String>,
        target: OpenWorldTaskTarget,
        required_primitives: Vec<GameplayPrimitiveKind>,
        depends_on: Vec<&str>,
    ) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            target,
            required_primitives,
            depends_on: depends_on.into_iter().map(str::to_string).collect(),
            risk: OperationRisk::LowRisk,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OpenWorldTaskTarget {
    Scene,
    Gameplay,
    Quest,
    Verification,
    Asset,
    Ui,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenWorldVerificationGoal {
    pub id: String,
    pub kind: VerificationGoalKind,
    pub expression: String,
}

impl OpenWorldVerificationGoal {
    pub fn new(
        id: impl Into<String>,
        kind: VerificationGoalKind,
        expression: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            kind,
            expression: expression.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VerificationGoalKind {
    SceneIndex,
    Screenshot,
    Playtest,
    Performance,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenWorldPlanRisk {
    pub level: OperationRisk,
    pub requires_approval: bool,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenWorldPlanValidationError {
    pub problems: Vec<OpenWorldPlanValidationProblem>,
}

impl std::fmt::Display for OpenWorldPlanValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let messages: Vec<String> = self.problems.iter().map(ToString::to_string).collect();
        write!(f, "invalid open-world plan: {}", messages.join("; "))
    }
}

impl std::error::Error for OpenWorldPlanValidationError {}

impl From<GameplayPrimitiveValidationError> for OpenWorldPlanValidationError {
    fn from(error: GameplayPrimitiveValidationError) -> Self {
        Self {
            problems: vec![OpenWorldPlanValidationProblem::MissingPrimitive {
                missing: error.missing,
            }],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OpenWorldPlanValidationProblem {
    MissingPrimitive {
        missing: Vec<GameplayPrimitiveKind>,
    },
    DuplicateId {
        collection: String,
        id: String,
    },
    MissingZone {
        object_id: String,
        zone_id: String,
    },
    MissingTask {
        task_id: String,
    },
    MissingTaskDependency {
        task_id: String,
        dependency_id: String,
    },
    CyclicTaskDependency {
        task_id: String,
    },
}

impl std::fmt::Display for OpenWorldPlanValidationProblem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingPrimitive { missing } => {
                let ids: Vec<&str> = missing.iter().map(|kind| kind.id()).collect();
                write!(f, "missing gameplay primitives: {}", ids.join(", "))
            }
            Self::DuplicateId { collection, id } => {
                write!(f, "duplicate id '{}' in {}", id, collection)
            }
            Self::MissingZone { object_id, zone_id } => {
                write!(
                    f,
                    "object '{}' references missing zone '{}'",
                    object_id, zone_id
                )
            }
            Self::MissingTask { task_id } => write!(f, "missing task '{}'", task_id),
            Self::MissingTaskDependency {
                task_id,
                dependency_id,
            } => write!(
                f,
                "task '{}' references missing dependency '{}'",
                task_id, dependency_id
            ),
            Self::CyclicTaskDependency { task_id } => {
                write!(f, "cyclic task dependency at '{}'", task_id)
            }
        }
    }
}

fn push_duplicate_id_problems<'a>(
    collection: &str,
    ids: impl Iterator<Item = &'a String>,
    problems: &mut Vec<OpenWorldPlanValidationProblem>,
) {
    let mut seen = BTreeSet::new();
    for id in ids {
        if !seen.insert(id.as_str()) {
            problems.push(OpenWorldPlanValidationProblem::DuplicateId {
                collection: collection.to_string(),
                id: id.clone(),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixture_validates_against_default_catalog() {
        let plan = OpenWorldPlan::open_world_slice01_fixture();
        let catalog = GameplayPrimitiveCatalog::open_world_slice_defaults();

        plan.validate(&catalog).unwrap();
    }

    #[test]
    fn fixture_round_trips_through_json() {
        let plan = OpenWorldPlan::open_world_slice01_fixture();
        let json = serde_json::to_string_pretty(&plan).unwrap();
        let decoded: OpenWorldPlan = serde_json::from_str(&json).unwrap();

        assert_eq!(decoded, plan);
    }

    #[test]
    fn task_graph_returns_stable_dependency_order() {
        let plan = OpenWorldPlan::open_world_slice01_fixture();

        assert_eq!(
            plan.task_graph.execution_order_ids().unwrap(),
            vec![
                "build_island_layout",
                "spawn_player_and_camera",
                "add_puzzle_and_chest",
                "add_enemy_camp",
                "wire_main_quest",
                "define_verification_goals",
                "run_main_path_playtest",
            ]
        );
    }

    #[test]
    fn validation_rejects_missing_primitive_references() {
        let plan = OpenWorldPlan::open_world_slice01_fixture();
        let catalog = GameplayPrimitiveCatalog::new();
        let error = plan.validate(&catalog).unwrap_err();

        assert!(error.problems.iter().any(|problem| matches!(
            problem,
            OpenWorldPlanValidationProblem::MissingPrimitive { missing }
                if missing.contains(&GameplayPrimitiveKind::PlayerController)
        )));
    }

    #[test]
    fn validation_rejects_missing_zone_references() {
        let mut plan = OpenWorldPlan::open_world_slice01_fixture();
        plan.object_manifest[0].zone_id = "missing_zone".into();
        let catalog = GameplayPrimitiveCatalog::open_world_slice_defaults();
        let error = plan.validate(&catalog).unwrap_err();

        assert!(error.problems.iter().any(|problem| matches!(
            problem,
            OpenWorldPlanValidationProblem::MissingZone { object_id, zone_id }
                if object_id == "player" && zone_id == "missing_zone"
        )));
    }

    #[test]
    fn validation_rejects_missing_task_dependency() {
        let mut plan = OpenWorldPlan::open_world_slice01_fixture();
        plan.task_graph.tasks[1].depends_on = vec!["missing_task".into()];
        let catalog = GameplayPrimitiveCatalog::open_world_slice_defaults();
        let error = plan.validate(&catalog).unwrap_err();

        assert!(error.problems.iter().any(|problem| matches!(
            problem,
            OpenWorldPlanValidationProblem::MissingTaskDependency { task_id, dependency_id }
                if task_id == "spawn_player_and_camera" && dependency_id == "missing_task"
        )));
    }
}
