//! Open-world vertical-slice verification entrypoints for DirectorRuntime.

use super::types::{DirectorRuntime, DirectorTraceEntry, EditorEvent};
use crate::open_world_plan::OpenWorldPlan;
use crate::open_world_timeline::OpenWorldTimeline;
use crate::open_world_verification::{OpenWorldVerificationBundle, VerificationBundleStatus};
use crate::playable_scenario::{PlayableScenario, PlayableScenarioState};
use crate::types::now_millis;
use std::path::Path;

impl DirectorRuntime {
    pub fn verify_open_world_slice01(&mut self) -> OpenWorldVerificationBundle {
        let plan = OpenWorldPlan::open_world_slice01_fixture();
        let scenario = PlayableScenario::open_world_slice01_main_path(&plan);
        let mut state = PlayableScenarioState::from_plan(&plan);
        let report = scenario.run(&mut state);

        let mut bundle = match self.scene_bridge.as_deref() {
            Some(scene_bridge) => OpenWorldVerificationBundle::from_playtest_with_scene_bridge(
                &plan,
                &scenario,
                &state,
                &report,
                scene_bridge,
            ),
            None => OpenWorldVerificationBundle::from_playtest(&plan, &scenario, &state, &report),
        };

        let passed = bundle.status == VerificationBundleStatus::Passed;
        let summary = format!(
            "OpenWorldSlice01 verification {:?}: {} goals, playtest {}",
            bundle.status,
            bundle.goals.len(),
            if bundle.playtest.passed {
                "passed"
            } else {
                "failed"
            }
        );

        self.events.push(EditorEvent::GoalChecked {
            task_id: 1,
            all_matched: passed,
            summary: summary.clone(),
        });
        self.trace_entries.push(DirectorTraceEntry {
            timestamp_ms: now_millis(),
            actor: "OpenWorldVerifier".into(),
            summary,
        });

        bundle.evidence.director_events = self
            .recent_events(10)
            .iter()
            .map(format_editor_event)
            .collect();
        bundle.evidence.engine_events = vec![format!(
            "scene_bridge_connected={}",
            self.scene_bridge.is_some()
        )];
        bundle.evidence.visual_check_evidence = open_world_visual_check_evidence(&bundle, None);

        bundle
    }

    pub fn verify_open_world_slice01_with_visual_snapshot(
        &mut self,
        path: impl AsRef<Path>,
    ) -> std::io::Result<OpenWorldVerificationBundle> {
        let path = path.as_ref();
        let mut bundle = self.verify_open_world_slice01();
        crate::open_world_visual_snapshot::write_open_world_visual_snapshot_png(&bundle, path)?;
        let screenshot_path = path.to_string_lossy().to_string();
        bundle.evidence.screenshot_paths = vec![screenshot_path.clone()];
        bundle.evidence.visual_check_evidence =
            open_world_visual_check_evidence(&bundle, Some(&screenshot_path));
        Ok(bundle)
    }

    pub fn verify_open_world_slice01_markdown(&mut self) -> String {
        self.verify_open_world_slice01().to_markdown()
    }

    pub fn write_open_world_slice01_markdown_report(
        &mut self,
        path: impl AsRef<Path>,
    ) -> std::io::Result<()> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, self.verify_open_world_slice01_markdown())
    }

    pub fn write_open_world_slice01_qa_artifacts(
        &mut self,
        markdown_path: impl AsRef<Path>,
        timeline_json_path: Option<impl AsRef<Path>>,
        visual_snapshot_path: Option<impl AsRef<Path>>,
        performance_evidence: Vec<String>,
    ) -> std::io::Result<OpenWorldVerificationBundle> {
        let bundle = match visual_snapshot_path {
            Some(path) => self.verify_open_world_slice01_with_visual_snapshot(path)?,
            None => self.verify_open_world_slice01(),
        }
        .with_performance_evidence(performance_evidence);

        write_text_file(markdown_path.as_ref(), &bundle.to_markdown())?;

        if let Some(path) = timeline_json_path {
            let timeline = OpenWorldTimeline::from_verification_bundle(&bundle);
            let json = serde_json::to_string_pretty(&timeline)
                .expect("OpenWorldTimeline should serialize to JSON");
            write_text_file(path.as_ref(), &json)?;
        }

        Ok(bundle)
    }

    pub fn write_open_world_slice01_qa_artifacts_with_engine_screenshot(
        &mut self,
        markdown_path: impl AsRef<Path>,
        timeline_json_path: Option<impl AsRef<Path>>,
        engine_screenshot_path: impl AsRef<Path>,
        engine_screenshot_dimensions: (u32, u32),
        performance_evidence: Vec<String>,
    ) -> std::io::Result<OpenWorldVerificationBundle> {
        let screenshot_path = engine_screenshot_path
            .as_ref()
            .to_string_lossy()
            .to_string();
        let mut bundle = self
            .verify_open_world_slice01()
            .with_performance_evidence(performance_evidence);
        bundle.evidence.screenshot_paths = vec![screenshot_path.clone()];
        bundle.evidence.visual_check_evidence = open_world_engine_framebuffer_visual_check_evidence(
            &bundle,
            &screenshot_path,
            engine_screenshot_dimensions,
        );
        bundle.evidence.engine_events.push(format!(
            "bevy_framebuffer_screenshot path={} dimensions={}x{}",
            screenshot_path, engine_screenshot_dimensions.0, engine_screenshot_dimensions.1
        ));

        write_text_file(markdown_path.as_ref(), &bundle.to_markdown())?;

        if let Some(path) = timeline_json_path {
            let timeline = OpenWorldTimeline::from_verification_bundle(&bundle);
            let json = serde_json::to_string_pretty(&timeline)
                .expect("OpenWorldTimeline should serialize to JSON");
            write_text_file(path.as_ref(), &json)?;
        }

        Ok(bundle)
    }
}

fn write_text_file(path: &Path, content: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, content)
}

fn open_world_visual_check_evidence(
    bundle: &OpenWorldVerificationBundle,
    screenshot_path: Option<&str>,
) -> Vec<String> {
    let visible_targets = ["player", "puzzle_switch", "reward_chest", "camp_enemy_01"]
        .into_iter()
        .filter(|target| {
            bundle
                .evidence
                .scene_index_observations
                .iter()
                .any(|observation| observation.contains(target))
        })
        .collect::<Vec<_>>();

    let screenshot_capture = screenshot_path
        .map(|path| format!("scene_index_proxy_png path={}", path))
        .unwrap_or_else(|| "pending".to_string());

    vec![format!(
        "visual_check=scene_index_proxy visible_targets={}/4 targets=[{}] screenshot_capture={}",
        visible_targets.len(),
        visible_targets.join(","),
        screenshot_capture
    )]
}

fn open_world_engine_framebuffer_visual_check_evidence(
    bundle: &OpenWorldVerificationBundle,
    screenshot_path: &str,
    dimensions: (u32, u32),
) -> Vec<String> {
    let visible_targets = ["player", "puzzle_switch", "reward_chest", "camp_enemy_01"]
        .into_iter()
        .filter(|target| {
            bundle
                .evidence
                .scene_index_observations
                .iter()
                .any(|observation| observation.contains(target))
        })
        .collect::<Vec<_>>();

    vec![format!(
        "visual_check=engine_framebuffer visible_targets={}/4 targets=[{}] screenshot_capture=bevy_framebuffer path={} dimensions={}x{}",
        visible_targets.len(),
        visible_targets.join(","),
        screenshot_path,
        dimensions.0,
        dimensions.1
    )]
}

fn format_editor_event(event: &EditorEvent) -> String {
    match event {
        EditorEvent::EditPlanCreated {
            plan_id,
            title,
            risk,
            mode,
            steps_count,
        } => format!(
            "EditPlanCreated plan={} title={} risk={} mode={} steps={}",
            plan_id, title, risk, mode, steps_count
        ),
        EditorEvent::PermissionRequested {
            plan_id,
            risk,
            reason,
        } => format!(
            "PermissionRequested plan={} risk={} reason={}",
            plan_id, risk, reason
        ),
        EditorEvent::PermissionResolved {
            plan_id,
            approved,
            reason,
        } => format!(
            "PermissionResolved plan={} approved={} reason={}",
            plan_id,
            approved,
            reason.as_deref().unwrap_or("")
        ),
        EditorEvent::PlanExecutionStarted { plan_id } => {
            format!("PlanExecutionStarted plan={}", plan_id)
        }
        EditorEvent::StepStarted {
            plan_id,
            step_id,
            title,
        } => format!(
            "StepStarted plan={} step={} title={}",
            plan_id, step_id, title
        ),
        EditorEvent::StepCompleted {
            plan_id,
            step_id,
            title,
            result,
        } => format!(
            "StepCompleted plan={} step={} title={} result={}",
            plan_id, step_id, title, result
        ),
        EditorEvent::StepFailed {
            plan_id,
            step_id,
            title,
            error,
        } => format!(
            "StepFailed plan={} step={} title={} error={}",
            plan_id, step_id, title, error
        ),
        EditorEvent::TransactionStarted {
            transaction_id,
            step_id,
        } => format!(
            "TransactionStarted transaction={} step={}",
            transaction_id, step_id
        ),
        EditorEvent::TransactionCommitted { transaction_id } => {
            format!("TransactionCommitted transaction={}", transaction_id)
        }
        EditorEvent::TransactionRolledBack { transaction_id } => {
            format!("TransactionRolledBack transaction={}", transaction_id)
        }
        EditorEvent::GoalChecked {
            task_id,
            all_matched,
            summary,
        } => format!(
            "GoalChecked task={} all_matched={} summary={}",
            task_id, all_matched, summary
        ),
        EditorEvent::ReviewCompleted {
            task_id,
            decision,
            summary,
        } => format!(
            "ReviewCompleted task={} decision={} summary={}",
            task_id, decision, summary
        ),
        EditorEvent::ExecutionCompleted { plan_id, success } => {
            format!("ExecutionCompleted plan={} success={}", plan_id, success)
        }
        EditorEvent::ModeChanged { mode } => format!("ModeChanged mode={}", mode),
        EditorEvent::Error { message } => format!("Error message={}", message),
        EditorEvent::DirectExecutionStarted {
            request,
            mode,
            complexity_score,
        } => format!(
            "DirectExecutionStarted request={} mode={} complexity={}",
            request, mode, complexity_score
        ),
        EditorEvent::DirectExecutionCompleted { request, success } => format!(
            "DirectExecutionCompleted request={} success={}",
            request, success
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene_bridge::{ComponentPatch, MockSceneBridge, SceneBridge};
    use std::collections::HashMap;

    #[test]
    fn director_generates_open_world_verification_bundle_without_scene_bridge() {
        let mut director = DirectorRuntime::new();

        let bundle = director.verify_open_world_slice01();

        assert_eq!(bundle.status, VerificationBundleStatus::Passed);
        assert!(bundle
            .evidence
            .director_events
            .iter()
            .any(|event| event.contains("GoalChecked")));
        assert_eq!(
            bundle.evidence.engine_events,
            vec!["scene_bridge_connected=false"]
        );
        assert_eq!(
            bundle.evidence.visual_check_evidence,
            vec![
                "visual_check=scene_index_proxy visible_targets=4/4 targets=[player,puzzle_switch,reward_chest,camp_enemy_01] screenshot_capture=pending"
            ]
        );
        assert!(director
            .trace()
            .iter()
            .any(|entry| entry.actor == "OpenWorldVerifier"));
    }

    #[test]
    fn director_verification_uses_connected_scene_bridge() {
        let mut bridge = MockSceneBridge::new();
        bridge
            .create_entity("player", None, &[component_patch("PlayerController")])
            .unwrap();
        let mut director = DirectorRuntime::new();
        director.set_scene_bridge(Box::new(bridge));

        let bundle = director.verify_open_world_slice01();

        assert_eq!(bundle.status, VerificationBundleStatus::Passed);
        assert_eq!(
            bundle.evidence.engine_events,
            vec!["scene_bridge_connected=true"]
        );
        assert!(bundle
            .evidence
            .scene_index_observations
            .iter()
            .any(|observation| observation.contains("SceneBridge entity player#")));
    }

    #[test]
    fn director_verification_fails_when_scene_bridge_missing_required_player() {
        let mut director = DirectorRuntime::new();
        director.set_scene_bridge(Box::new(MockSceneBridge::new()));

        let bundle = director.verify_open_world_slice01();

        assert_eq!(bundle.status, VerificationBundleStatus::Failed);
        assert!(bundle.playtest.passed);
        assert!(bundle
            .goals
            .iter()
            .any(|goal| goal.goal_id == "player_exists" && goal.observed.contains("false")));
    }

    #[test]
    fn director_generates_open_world_markdown_report() {
        let mut director = DirectorRuntime::new();

        let markdown = director.verify_open_world_slice01_markdown();

        assert!(markdown.contains("# OpenWorld Verification Bundle"));
        assert!(markdown.contains("## Director Events"));
        assert!(markdown.contains("GoalChecked"));
    }

    #[test]
    fn director_writes_open_world_markdown_report_to_path() {
        let mut director = DirectorRuntime::new();
        let path = std::env::temp_dir().join(format!(
            "windwave-open-world-slice01-qa-{}.md",
            crate::types::now_millis()
        ));

        director
            .write_open_world_slice01_markdown_report(&path)
            .unwrap();

        let markdown = std::fs::read_to_string(&path).unwrap();
        assert!(markdown.contains("# OpenWorld Verification Bundle"));
        assert!(markdown.contains("## Time Evidence"));
        assert!(markdown.contains("## Schedule Decisions"));
        assert!(markdown.contains("## Visual Check Evidence"));
        assert!(markdown.contains("screenshot_capture=pending"));

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn director_verification_can_write_open_world_visual_snapshot_png() {
        let mut director = DirectorRuntime::new();
        let temp_file = tempfile::Builder::new()
            .prefix("windwave-open-world-slice01-visual-")
            .suffix(".png")
            .tempfile()
            .unwrap();
        let path = temp_file.path().to_path_buf();

        let bundle = director
            .verify_open_world_slice01_with_visual_snapshot(&path)
            .unwrap();

        assert_eq!(
            bundle.evidence.screenshot_paths,
            vec![path.to_string_lossy().to_string()]
        );
        assert!(bundle
            .evidence
            .visual_check_evidence
            .iter()
            .any(|evidence| evidence.contains("screenshot_capture=scene_index_proxy_png")));

        let bytes = std::fs::read(&path).unwrap();
        assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
    }

    #[test]
    fn director_writes_open_world_qa_artifacts() {
        let mut director = DirectorRuntime::new();
        let dir = tempfile::tempdir().unwrap();
        let markdown_path = dir.path().join("open-world-slice01.md");
        let timeline_path = dir.path().join("open-world-slice01-timeline.json");
        let visual_path = dir.path().join("open-world-slice01-visual.png");

        let bundle = director
            .write_open_world_slice01_qa_artifacts(
                &markdown_path,
                Some(&timeline_path),
                Some(&visual_path),
                vec!["bevy_frame_time_avg_ms=16.667".to_string()],
            )
            .unwrap();

        assert!(markdown_path.exists());
        assert!(timeline_path.exists());
        assert!(visual_path.exists());
        assert!(bundle
            .evidence
            .performance_evidence
            .contains(&"bevy_frame_time_avg_ms=16.667".to_string()));

        let markdown = std::fs::read_to_string(&markdown_path).unwrap();
        assert!(markdown.contains("bevy_frame_time_avg_ms=16.667"));
        assert!(markdown.contains("open-world-slice01-visual.png"));

        let timeline_json = std::fs::read_to_string(&timeline_path).unwrap();
        assert!(timeline_json.contains("bevy_frame_time_avg_ms=16.667"));
        assert!(timeline_json.contains("open-world-slice01-visual.png"));
    }

    #[test]
    fn director_writes_open_world_qa_artifacts_with_engine_screenshot() {
        let mut director = DirectorRuntime::new();
        let dir = tempfile::tempdir().unwrap();
        let markdown_path = dir.path().join("open-world-slice01.md");
        let timeline_path = dir.path().join("open-world-slice01-timeline.json");
        let engine_screenshot_path = dir.path().join("bevy-framebuffer.png");

        let bundle = director
            .write_open_world_slice01_qa_artifacts_with_engine_screenshot(
                &markdown_path,
                Some(&timeline_path),
                &engine_screenshot_path,
                (1280, 720),
                vec!["bevy_frame_time_avg_ms=16.667".to_string()],
            )
            .unwrap();

        assert_eq!(
            bundle.evidence.screenshot_paths,
            vec![engine_screenshot_path.to_string_lossy().to_string()]
        );
        assert!(bundle
            .evidence
            .visual_check_evidence
            .iter()
            .any(|evidence| evidence.contains("screenshot_capture=bevy_framebuffer")));
        assert!(bundle
            .evidence
            .visual_check_evidence
            .iter()
            .any(|evidence| evidence.contains("dimensions=1280x720")));

        let markdown = std::fs::read_to_string(&markdown_path).unwrap();
        assert!(markdown.contains("screenshot_capture=bevy_framebuffer"));
        assert!(markdown.contains("bevy-framebuffer.png"));

        let timeline_json = std::fs::read_to_string(&timeline_path).unwrap();
        assert!(timeline_json.contains("screenshot_capture=bevy_framebuffer"));
        assert!(timeline_json.contains("bevy-framebuffer.png"));
    }

    fn component_patch(type_name: &str) -> ComponentPatch {
        ComponentPatch {
            type_name: type_name.into(),
            properties: HashMap::new(),
        }
    }
}
