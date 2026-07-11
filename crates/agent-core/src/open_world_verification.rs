//! Verification bundles for open-world vertical-slice playtests.
//!
//! The bundle joins plan verification goals, playtest results, runtime events,
//! SceneIndex-style observations, and optional external evidence into one
//! serializable QA artifact.

use crate::open_world_plan::{OpenWorldPlan, OpenWorldVerificationGoal, VerificationGoalKind};
use crate::playable_scenario::{PlayableScenario, PlayableScenarioReport, PlayableScenarioState};
use crate::scene_bridge::SceneBridge;
use serde::{Deserialize, Serialize};
use std::time::Instant;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenWorldVerificationBundle {
    pub id: String,
    pub plan_id: String,
    pub scenario_id: String,
    pub status: VerificationBundleStatus,
    pub playtest: PlaytestVerificationSummary,
    pub goals: Vec<OpenWorldVerificationResult>,
    pub evidence: VerificationEvidence,
}

impl OpenWorldVerificationBundle {
    pub fn from_playtest(
        plan: &OpenWorldPlan,
        scenario: &PlayableScenario,
        state: &PlayableScenarioState,
        report: &PlayableScenarioReport,
    ) -> Self {
        Self::from_playtest_with_optional_scene_bridge(plan, scenario, state, report, None)
    }

    pub fn from_playtest_with_scene_bridge(
        plan: &OpenWorldPlan,
        scenario: &PlayableScenario,
        state: &PlayableScenarioState,
        report: &PlayableScenarioReport,
        scene_bridge: &dyn SceneBridge,
    ) -> Self {
        Self::from_playtest_with_optional_scene_bridge(
            plan,
            scenario,
            state,
            report,
            Some(scene_bridge),
        )
    }

    fn from_playtest_with_optional_scene_bridge(
        plan: &OpenWorldPlan,
        scenario: &PlayableScenario,
        state: &PlayableScenarioState,
        report: &PlayableScenarioReport,
        scene_bridge: Option<&dyn SceneBridge>,
    ) -> Self {
        let verification_started = Instant::now();
        let goals = plan
            .verification_goals
            .iter()
            .map(|goal| evaluate_goal(goal, state, scene_bridge))
            .collect::<Vec<_>>();

        let status = if report.passed
            && goals
                .iter()
                .all(|goal| goal.status == VerificationGoalStatus::Passed)
        {
            VerificationBundleStatus::Passed
        } else {
            VerificationBundleStatus::Failed
        };

        let scene_index_observation_started = Instant::now();
        let mut scene_index_observations = scene_bridge
            .map(scene_bridge_observations)
            .unwrap_or_else(|| report.scene_index_observations.clone());
        scene_index_observations.extend(runtime_scene_index_context_observations(state));
        let scene_index_observation_duration_us =
            scene_index_observation_started.elapsed().as_micros();
        let verification_duration_us = verification_started.elapsed().as_micros();

        Self {
            id: format!("verification_{}_{}", plan.id, scenario.id),
            plan_id: plan.id.clone(),
            scenario_id: scenario.id.clone(),
            status,
            playtest: PlaytestVerificationSummary::from_report(report),
            goals,
            evidence: VerificationEvidence {
                runtime_events: state
                    .runtime
                    .events
                    .iter()
                    .map(|event| format!("tick={} {}", event.timestamp().tick, event.label()))
                    .collect(),
                playtest_events: report.recent_events.clone(),
                time_evidence: vec![format!(
                    "wall_time={} sim_time_ms={} tick={} clock_mode={}",
                    report.time_evidence.wall_time,
                    report.time_evidence.sim_time_ms,
                    report.time_evidence.tick,
                    report.time_evidence.clock_mode
                )],
                schedule_decisions: state
                    .runtime
                    .agent_schedules
                    .values()
                    .map(|schedule| {
                        let decision = schedule.decision_at(&state.runtime.clock.timestamp());
                        let window = decision.window_id.as_deref().unwrap_or("none");
                        let activity = if decision.is_active {
                            "active"
                        } else {
                            "inactive"
                        };
                        let allowed = if decision.allowed_actions.is_empty() {
                            "none".to_string()
                        } else {
                            decision.allowed_actions.join(",")
                        };
                        format!(
                            "{} {} window={} allowed={}",
                            decision.agent_id, activity, window, allowed
                        )
                    })
                    .collect(),
                performance_evidence: vec![
                    format!("plan_objects={}", plan.object_manifest.len()),
                    format!("runtime_objects={}", state.runtime.objects.len()),
                    format!("runtime_events={}", state.runtime.events.len()),
                    format!(
                        "scene_index_observations={}",
                        report.scene_index_observations.len()
                    ),
                    format!("verification_goals={}", plan.verification_goals.len()),
                    format!("scenario_steps={}", scenario.steps.len()),
                    format!("verification_duration_us={}", verification_duration_us),
                    format!(
                        "scene_index_observation_duration_us={}",
                        scene_index_observation_duration_us
                    ),
                ],
                scene_index_observations,
                screenshot_paths: Vec::new(),
                visual_check_evidence: Vec::new(),
                director_events: Vec::new(),
                engine_events: Vec::new(),
                agent_events: Vec::new(),
            },
        }
    }

    pub fn with_screenshot_paths(mut self, paths: Vec<String>) -> Self {
        self.evidence.screenshot_paths = paths;
        self
    }

    pub fn with_visual_check_evidence(mut self, evidence: Vec<String>) -> Self {
        self.evidence.visual_check_evidence = evidence;
        self
    }

    pub fn with_performance_evidence(mut self, evidence: Vec<String>) -> Self {
        self.evidence.performance_evidence.extend(evidence);
        self
    }

    pub fn with_director_events(mut self, events: Vec<String>) -> Self {
        self.evidence.director_events = events;
        self
    }

    pub fn with_engine_events(mut self, events: Vec<String>) -> Self {
        self.evidence.engine_events = events;
        self
    }

    pub fn to_markdown(&self) -> String {
        let mut markdown = String::new();
        markdown.push_str(&format!(
            "# OpenWorld Verification Bundle: {}\n\n",
            self.plan_id
        ));
        markdown.push_str(&format!("Status: {:?}\n", self.status));
        markdown.push_str(&format!("Scenario: `{}`\n", self.scenario_id));
        markdown.push_str(&format!(
            "Playtest: {} ({} steps, final quest: `{}`)\n\n",
            if self.playtest.passed {
                "passed"
            } else {
                "failed"
            },
            self.playtest.completed_steps,
            self.playtest.final_quest_state
        ));

        if let Some(reason) = &self.playtest.failure_reason {
            markdown.push_str("## Playtest Failure\n\n");
            markdown.push_str(&format!("- Reason: {}\n", reason));
            if let Some(step) = &self.playtest.failed_step_label {
                markdown.push_str(&format!("- Step: {}\n", step));
            }
            if let Some(fix) = &self.playtest.suggested_fix {
                markdown.push_str(&format!("- Suggested fix: {}\n", fix));
            }
            markdown.push('\n');
        }

        markdown.push_str("## Verification Goals\n\n");
        markdown.push_str("| ID | Kind | Status | Observed |\n");
        markdown.push_str("|---|---|---|---|\n");
        for goal in &self.goals {
            markdown.push_str(&format!(
                "| `{}` | {:?} | {:?} | {} |\n",
                escape_markdown_table_cell(&goal.goal_id),
                goal.kind,
                goal.status,
                escape_markdown_table_cell(&goal.observed)
            ));
        }

        markdown.push_str("\n## Runtime Events\n\n");
        push_lines(&mut markdown, &self.evidence.runtime_events);

        markdown.push_str("\n## Time Evidence\n\n");
        push_lines(&mut markdown, &self.evidence.time_evidence);

        if !self.evidence.schedule_decisions.is_empty() {
            markdown.push_str("\n## Schedule Decisions\n\n");
            push_lines(&mut markdown, &self.evidence.schedule_decisions);
        }

        if !self.evidence.agent_events.is_empty() {
            markdown.push_str("\n## Agent Events\n\n");
            push_lines(&mut markdown, &self.evidence.agent_events);
        }

        if !self.evidence.performance_evidence.is_empty() {
            markdown.push_str("\n## Performance Evidence\n\n");
            push_lines(&mut markdown, &self.evidence.performance_evidence);
        }

        markdown.push_str("\n## SceneIndex Observations\n\n");
        push_lines(&mut markdown, &self.evidence.scene_index_observations);

        if !self.evidence.screenshot_paths.is_empty() {
            markdown.push_str("\n## Screenshots\n\n");
            push_lines(&mut markdown, &self.evidence.screenshot_paths);
        }

        if !self.evidence.visual_check_evidence.is_empty() {
            markdown.push_str("\n## Visual Check Evidence\n\n");
            push_lines(&mut markdown, &self.evidence.visual_check_evidence);
        }

        if !self.evidence.director_events.is_empty() {
            markdown.push_str("\n## Director Events\n\n");
            push_lines(&mut markdown, &self.evidence.director_events);
        }

        if !self.evidence.engine_events.is_empty() {
            markdown.push_str("\n## Engine Events\n\n");
            push_lines(&mut markdown, &self.evidence.engine_events);
        }

        markdown
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VerificationBundleStatus {
    Passed,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlaytestVerificationSummary {
    pub scenario_id: String,
    pub passed: bool,
    pub completed_steps: usize,
    pub final_quest_state: String,
    pub failed_step_index: Option<usize>,
    pub failed_step_label: Option<String>,
    pub failure_reason: Option<String>,
    pub suggested_fix: Option<String>,
}

impl PlaytestVerificationSummary {
    pub fn from_report(report: &PlayableScenarioReport) -> Self {
        let failure = report.failure.as_ref();
        Self {
            scenario_id: report.scenario_id.clone(),
            passed: report.passed,
            completed_steps: report.completed_steps,
            final_quest_state: report.final_quest_state.clone(),
            failed_step_index: failure.map(|failure| failure.failed_step_index),
            failed_step_label: failure.map(|failure| failure.step_label.clone()),
            failure_reason: failure.map(|failure| failure.reason.clone()),
            suggested_fix: failure.map(|failure| failure.suggested_fix.clone()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenWorldVerificationResult {
    pub goal_id: String,
    pub kind: VerificationGoalKind,
    pub expression: String,
    pub status: VerificationGoalStatus,
    pub observed: String,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VerificationGoalStatus {
    Passed,
    Failed,
    Unsupported,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerificationEvidence {
    pub runtime_events: Vec<String>,
    pub playtest_events: Vec<String>,
    pub time_evidence: Vec<String>,
    pub schedule_decisions: Vec<String>,
    pub performance_evidence: Vec<String>,
    pub scene_index_observations: Vec<String>,
    pub screenshot_paths: Vec<String>,
    pub visual_check_evidence: Vec<String>,
    pub director_events: Vec<String>,
    pub engine_events: Vec<String>,
    /// AI resident adjudication lines (allow / deny / fallback).
    #[serde(default)]
    pub agent_events: Vec<String>,
}

fn evaluate_goal(
    goal: &OpenWorldVerificationGoal,
    state: &PlayableScenarioState,
    scene_bridge: Option<&dyn SceneBridge>,
) -> OpenWorldVerificationResult {
    if let Some(object_id) = parse_single_arg(&goal.expression, "exists") {
        if let Some(scene_bridge) = scene_bridge {
            let matches = scene_bridge.query_entities(Some(&object_id), None);
            let exists = matches.iter().any(|entity| entity.name == object_id);
            return goal_result(
                goal,
                exists,
                format!(
                    "exists({}) via SceneBridge = {} ({} matches)",
                    object_id,
                    exists,
                    matches.len()
                ),
            );
        }

        let exists = state.runtime.objects.contains_key(&object_id);
        return goal_result(
            goal,
            exists,
            format!("exists({}) via runtime = {}", object_id, exists),
        );
    }

    if let Some((object_id, component)) = parse_two_args(&goal.expression, "has_component") {
        let has_component = if let Some(scene_bridge) = scene_bridge {
            scene_bridge
                .query_entities(Some(&object_id), Some(&component))
                .iter()
                .any(|entity| entity.name == object_id)
        } else {
            state
                .runtime
                .objects
                .get(&object_id)
                .map(|object| {
                    object
                        .primitives
                        .iter()
                        .any(|primitive| primitive.id() == component)
                })
                .unwrap_or(false)
        };

        return goal_result(
            goal,
            has_component,
            format!(
                "has_component({}, {}) = {}",
                object_id, component, has_component
            ),
        );
    }

    if let Some((object_id, expected)) = parse_string_equality(&goal.expression, "state") {
        let observed = object_state_label(state, &object_id).unwrap_or_else(|| "Missing".into());
        return goal_result(
            goal,
            observed == expected,
            format!("state({}) = {}", object_id, observed),
        );
    }

    if let Some((actor_id, item_id)) = parse_two_args(&goal.expression, "inventory_contains") {
        let contains = state.runtime.inventory_contains(&actor_id, &item_id);
        return goal_result(
            goal,
            contains,
            format!(
                "inventory_contains({}, {}) = {}",
                actor_id, item_id, contains
            ),
        );
    }

    if let Some((quest_id, expected)) = parse_string_equality(&goal.expression, "quest_state") {
        let observed = state
            .runtime
            .quest_state(&quest_id)
            .map(|state| format!("{:?}", state))
            .unwrap_or_else(|_| "Missing".into());
        return goal_result(
            goal,
            observed == expected,
            format!("quest_state({}) = {}", quest_id, observed),
        );
    }

    OpenWorldVerificationResult {
        goal_id: goal.id.clone(),
        kind: goal.kind,
        expression: goal.expression.clone(),
        status: VerificationGoalStatus::Unsupported,
        observed: "unsupported expression".into(),
        message: format!("unsupported verification expression: {}", goal.expression),
    }
}

fn runtime_scene_index_context_observations(state: &PlayableScenarioState) -> Vec<String> {
    let clock = &state.clock;
    let mut observations = vec![format!(
        "world_clock wall_time={} sim_time_ms={} tick={} clock_mode={:?}",
        clock.wall_time.to_rfc3339(),
        clock.sim_time_ms,
        clock.tick,
        clock.mode
    )];

    observations.extend(state.runtime.agent_schedules.values().map(|schedule| {
        let decision = schedule.decision_at(&state.clock.timestamp());
        let window = decision.window_id.as_deref().unwrap_or("none");
        let activity = if decision.is_active {
            "active"
        } else {
            "inactive"
        };
        let allowed = if decision.allowed_actions.is_empty() {
            "none".to_string()
        } else {
            decision.allowed_actions.join(",")
        };
        format!(
            "agent_schedule {} {} window={} allowed={}",
            decision.agent_id, activity, window, allowed
        )
    }));

    observations
}

fn scene_bridge_observations(scene_bridge: &dyn SceneBridge) -> Vec<String> {
    let entities = scene_bridge.query_entities(None, None);
    if entities.is_empty() {
        return vec!["SceneBridge query returned no entities".into()];
    }

    entities
        .into_iter()
        .map(|entity| {
            format!(
                "SceneBridge entity {}#{} components [{}]",
                entity.name,
                entity.id,
                entity.components.join(", ")
            )
        })
        .collect()
}

fn goal_result(
    goal: &OpenWorldVerificationGoal,
    passed: bool,
    observed: String,
) -> OpenWorldVerificationResult {
    OpenWorldVerificationResult {
        goal_id: goal.id.clone(),
        kind: goal.kind,
        expression: goal.expression.clone(),
        status: if passed {
            VerificationGoalStatus::Passed
        } else {
            VerificationGoalStatus::Failed
        },
        message: if passed {
            "goal passed".into()
        } else {
            "goal failed".into()
        },
        observed,
    }
}

fn object_state_label(state: &PlayableScenarioState, object_id: &str) -> Option<String> {
    if let Some(value) = state.runtime.puzzle_states.get(object_id) {
        return Some(format!("{:?}", value));
    }
    if let Some(value) = state.runtime.loot_states.get(object_id) {
        return Some(format!("{:?}", value));
    }
    if let Some(value) = state.runtime.enemy_states.get(object_id) {
        return Some(format!("{:?}", value));
    }
    if let Some(value) = state.runtime.quest_states.get(object_id) {
        return Some(format!("{:?}", value));
    }
    state
        .runtime
        .objects
        .contains_key(object_id)
        .then(|| "Exists".into())
}

fn parse_single_arg(expression: &str, function_name: &str) -> Option<String> {
    let inner = expression
        .trim()
        .strip_prefix(&format!("{}(", function_name))?
        .strip_suffix(')')?;
    Some(unquote(inner.trim())?.to_string())
}

fn parse_two_args(expression: &str, function_name: &str) -> Option<(String, String)> {
    let inner = expression
        .trim()
        .strip_prefix(&format!("{}(", function_name))?
        .strip_suffix(')')?;
    let parts = inner.split(',').map(str::trim).collect::<Vec<_>>();
    if parts.len() != 2 {
        return None;
    }
    Some((
        unquote(parts[0])?.to_string(),
        unquote(parts[1])?.to_string(),
    ))
}

fn parse_string_equality(expression: &str, function_name: &str) -> Option<(String, String)> {
    let prefix = format!("{}(", function_name);
    let rest = expression.trim().strip_prefix(&prefix)?;
    let (arg, rhs) = rest.split_once(')')?;
    let expected = rhs.trim().strip_prefix("==")?.trim();
    Some((
        unquote(arg.trim())?.to_string(),
        unquote(expected)?.to_string(),
    ))
}

fn unquote(value: &str) -> Option<&str> {
    value.strip_prefix('"')?.strip_suffix('"')
}

fn escape_markdown_table_cell(value: &str) -> String {
    value.replace('|', "\\|").replace('\n', " ")
}

fn push_lines(markdown: &mut String, lines: &[String]) {
    if lines.is_empty() {
        markdown.push_str("- none\n");
        return;
    }

    for line in lines {
        markdown.push_str(&format!("- {}\n", line));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gameplay_primitive::GameplayPrimitiveKind;
    use crate::open_world_plan::OpenWorldPlan;
    use crate::open_world_template::OpenWorldSceneTemplate;
    use crate::scene_bridge::{ComponentPatch, MockSceneBridge, SceneBridge};
    use std::collections::HashMap;

    #[test]
    fn verification_bundle_passes_open_world_slice01_main_path() {
        let plan = OpenWorldPlan::open_world_slice01_fixture();
        let scenario = PlayableScenario::open_world_slice01_main_path(&plan);
        let mut state = PlayableScenarioState::from_plan(&plan);
        let report = scenario.run(&mut state);

        let bundle = OpenWorldVerificationBundle::from_playtest(&plan, &scenario, &state, &report);

        assert_eq!(bundle.status, VerificationBundleStatus::Passed);
        assert!(bundle.playtest.passed);
        assert!(bundle
            .goals
            .iter()
            .all(|goal| goal.status == VerificationGoalStatus::Passed));
        assert!(bundle
            .evidence
            .runtime_events
            .iter()
            .any(|event| event.starts_with("tick=")
                && event.ends_with("player defeated camp_enemy_01")));
        assert!(bundle
            .evidence
            .scene_index_observations
            .iter()
            .any(|observation| observation == "reward_chest unlocked"));
        assert!(bundle
            .evidence
            .scene_index_observations
            .iter()
            .any(|observation| observation
                == "world_clock wall_time=2026-07-05T12:00:00+00:00 sim_time_ms=0 tick=13 clock_mode=Frozen"));
        assert!(bundle
            .evidence
            .scene_index_observations
            .iter()
            .any(|observation| observation
                == "agent_schedule merchant_01 active window=shop_hours allowed=quote_price,restock_low_risk_item"));
        assert_eq!(
            bundle.evidence.time_evidence,
            vec![
                "wall_time=2026-07-05T12:00:00+00:00 sim_time_ms=0 tick=13 clock_mode=Frozen"
                    .to_string()
            ]
        );
        assert_eq!(
            bundle.evidence.schedule_decisions,
            vec![
                "merchant_01 active window=shop_hours allowed=quote_price,restock_low_risk_item"
                    .to_string()
            ]
        );
        assert!(bundle
            .evidence
            .performance_evidence
            .contains(&"plan_objects=10".to_string()));
        assert!(bundle
            .evidence
            .performance_evidence
            .contains(&"verification_goals=5".to_string()));
        assert!(bundle
            .evidence
            .performance_evidence
            .contains(&"scenario_steps=13".to_string()));
        assert_metric_exists(
            &bundle.evidence.performance_evidence,
            "verification_duration_us",
        );
        assert_metric_exists(
            &bundle.evidence.performance_evidence,
            "scene_index_observation_duration_us",
        );

        let markdown = bundle.to_markdown();
        assert!(markdown.contains("Status: Passed"));
        assert!(markdown.contains("main_quest_completed"));
        assert!(markdown.contains("player defeated camp_enemy_01"));
        assert!(markdown.contains("## Time Evidence"));
        assert!(markdown.contains("tick=13"));
        assert!(markdown.contains("## Schedule Decisions"));
        assert!(markdown.contains("merchant_01 active"));
        assert!(markdown.contains("## Performance Evidence"));
        assert!(markdown.contains("plan_objects=10"));
    }

    #[test]
    fn verification_bundle_reports_failed_playtest_and_goal_failures() {
        let plan = OpenWorldPlan::open_world_slice01_fixture();
        let scenario = PlayableScenario::open_world_slice01_main_path(&plan);
        let mut state = PlayableScenarioState::from_plan(&plan);
        state.remove_primitive("puzzle_switch", GameplayPrimitiveKind::Interactable);
        let report = scenario.run(&mut state);

        let bundle = OpenWorldVerificationBundle::from_playtest(&plan, &scenario, &state, &report);

        assert_eq!(bundle.status, VerificationBundleStatus::Failed);
        assert!(!bundle.playtest.passed);
        assert_eq!(
            bundle.playtest.failure_reason.as_deref(),
            Some("puzzle_switch exists but is missing Interactable")
        );
        assert_eq!(
            bundle.playtest.suggested_fix.as_deref(),
            Some("add Interactable to puzzle_switch")
        );
        assert_goal_failed(&bundle, "puzzle_is_solved", "AwaitingInteraction");
        assert_goal_failed(&bundle, "enemy_defeated", "Patrol");
        assert_goal_failed(&bundle, "reward_collected", "false");
        assert_goal_failed(&bundle, "main_quest_completed", "Active");

        let markdown = bundle.to_markdown();
        assert!(markdown.contains("Status: Failed"));
        assert!(markdown.contains("Suggested fix: add Interactable to puzzle_switch"));
    }

    #[test]
    fn verification_bundle_round_trips_through_json() {
        let plan = OpenWorldPlan::open_world_slice01_fixture();
        let scenario = PlayableScenario::open_world_slice01_main_path(&plan);
        let mut state = PlayableScenarioState::from_plan(&plan);
        let report = scenario.run(&mut state);
        let bundle = OpenWorldVerificationBundle::from_playtest(&plan, &scenario, &state, &report)
            .with_screenshot_paths(vec!["/tmp/open-world-slice01.png".into()])
            .with_visual_check_evidence(vec![
                "camera sees player, puzzle switch, reward chest, and camp enemy".into(),
            ])
            .with_performance_evidence(vec![
                "bevy_frame_time_avg_ms=16.667".into(),
                "bevy_frame_time_max_ms=20.000".into(),
            ])
            .with_director_events(vec!["director accepted open-world plan".into()])
            .with_engine_events(vec!["engine applied scene template".into()]);

        let json = serde_json::to_string_pretty(&bundle).unwrap();
        let decoded: OpenWorldVerificationBundle = serde_json::from_str(&json).unwrap();

        assert_eq!(decoded, bundle);
        assert_eq!(
            decoded.evidence.screenshot_paths,
            vec!["/tmp/open-world-slice01.png"]
        );
        assert_eq!(
            decoded.evidence.visual_check_evidence,
            vec!["camera sees player, puzzle switch, reward chest, and camp enemy"]
        );
        assert!(decoded
            .evidence
            .performance_evidence
            .contains(&"bevy_frame_time_avg_ms=16.667".to_string()));

        let markdown = decoded.to_markdown();
        assert!(markdown.contains("## Visual Check Evidence"));
        assert!(markdown.contains("camera sees player"));
        assert!(markdown.contains("bevy_frame_time_avg_ms=16.667"));
    }

    #[test]
    fn verification_bundle_uses_scene_bridge_for_exists_goals() {
        let plan = OpenWorldPlan::open_world_slice01_fixture();
        let scenario = PlayableScenario::open_world_slice01_main_path(&plan);
        let mut state = PlayableScenarioState::from_plan(&plan);
        let report = scenario.run(&mut state);
        let mut bridge = MockSceneBridge::new();
        bridge
            .create_entity("player", None, &[component_patch("PlayerController")])
            .unwrap();

        let bundle = OpenWorldVerificationBundle::from_playtest_with_scene_bridge(
            &plan, &scenario, &state, &report, &bridge,
        );

        assert_eq!(bundle.status, VerificationBundleStatus::Passed);
        let player_goal = bundle
            .goals
            .iter()
            .find(|goal| goal.goal_id == "player_exists")
            .expect("player_exists should exist");
        assert_eq!(player_goal.status, VerificationGoalStatus::Passed);
        assert_eq!(
            player_goal.observed,
            "exists(player) via SceneBridge = true (1 matches)"
        );
        assert!(bundle
            .evidence
            .scene_index_observations
            .iter()
            .any(|observation| observation.contains("SceneBridge entity player#")));
    }

    #[test]
    fn verification_bundle_fails_when_scene_bridge_missing_required_entity() {
        let plan = OpenWorldPlan::open_world_slice01_fixture();
        let scenario = PlayableScenario::open_world_slice01_main_path(&plan);
        let mut state = PlayableScenarioState::from_plan(&plan);
        let report = scenario.run(&mut state);
        let bridge = MockSceneBridge::new();

        let bundle = OpenWorldVerificationBundle::from_playtest_with_scene_bridge(
            &plan, &scenario, &state, &report, &bridge,
        );

        assert_eq!(bundle.status, VerificationBundleStatus::Failed);
        assert!(bundle.playtest.passed);
        assert_goal_failed(&bundle, "player_exists", "false");
        assert!(bundle
            .evidence
            .scene_index_observations
            .contains(&"SceneBridge query returned no entities".to_string()));
        assert!(bundle
            .evidence
            .scene_index_observations
            .iter()
            .any(|observation| observation.starts_with("world_clock wall_time=")));
    }

    #[test]
    fn verification_bundle_passes_with_scene_template_applied_to_bridge() {
        let plan = OpenWorldPlan::open_world_slice01_fixture();
        let template = OpenWorldSceneTemplate::small_island_from_plan(&plan);
        let scenario = PlayableScenario::open_world_slice01_main_path(&plan);
        let mut state = PlayableScenarioState::from_plan(&plan);
        let report = scenario.run(&mut state);
        let mut bridge = MockSceneBridge::new();
        template.apply_to_bridge(&mut bridge).unwrap();

        let bundle = OpenWorldVerificationBundle::from_playtest_with_scene_bridge(
            &plan, &scenario, &state, &report, &bridge,
        );

        assert_eq!(bundle.status, VerificationBundleStatus::Passed);
        let player_goal = bundle
            .goals
            .iter()
            .find(|goal| goal.goal_id == "player_exists")
            .expect("player_exists should exist");
        assert_eq!(player_goal.status, VerificationGoalStatus::Passed);
        assert_eq!(
            player_goal.observed,
            "exists(player) via SceneBridge = true (1 matches)"
        );
        assert!(bundle
            .evidence
            .scene_index_observations
            .iter()
            .any(|observation| {
                observation.contains("SceneBridge entity reward_chest#")
                    && observation.contains("LootContainer")
            }));
    }

    fn assert_goal_failed(
        bundle: &OpenWorldVerificationBundle,
        goal_id: &str,
        observed_fragment: &str,
    ) {
        let goal = bundle
            .goals
            .iter()
            .find(|goal| goal.goal_id == goal_id)
            .expect("goal should exist");
        assert_eq!(goal.status, VerificationGoalStatus::Failed);
        assert!(
            goal.observed.contains(observed_fragment),
            "observed {} did not contain {}",
            goal.observed,
            observed_fragment
        );
    }

    fn assert_metric_exists(evidence: &[String], key: &str) {
        let metric = evidence
            .iter()
            .find_map(|line| line.strip_prefix(&format!("{key}=")))
            .unwrap_or_else(|| panic!("missing performance metric {key}"));
        metric
            .parse::<u128>()
            .unwrap_or_else(|_| panic!("performance metric {key} was not numeric: {metric}"));
    }

    fn component_patch(type_name: &str) -> ComponentPatch {
        ComponentPatch {
            type_name: type_name.into(),
            properties: HashMap::new(),
        }
    }
}
