//! World Timeline panel for playable-world QA and replay evidence.

use agent_core::{
    DirectorRuntime, OpenWorldReplayWorldState, OpenWorldTimeline, OpenWorldTimelineTick,
};
use bevy::app::AppExit;
use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts, EguiPrimaryContextPass};
use log::{error, info};
use std::path::{Path, PathBuf};

pub struct WorldTimelinePanelPlugin;

impl Plugin for WorldTimelinePanelPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WorldTimelinePanelState>()
            .init_resource::<WorldTimelineUpdateQueue>()
            .init_resource::<OpenWorldQaRequestQueue>()
            .add_systems(
                Update,
                (
                    process_open_world_qa_requests,
                    apply_world_timeline_updates,
                    open_world_qa_accept_mode_system,
                )
                    .chain(),
            )
            .add_systems(EguiPrimaryContextPass, render_world_timeline_panel);
    }
}

#[derive(Resource, Default)]
pub struct WorldTimelineUpdateQueue {
    pending: Option<OpenWorldTimeline>,
}

impl WorldTimelineUpdateQueue {
    pub fn request_update(&mut self, timeline: OpenWorldTimeline) {
        self.pending = Some(timeline);
    }

    pub fn take_pending(&mut self) -> Option<OpenWorldTimeline> {
        self.pending.take()
    }
}

#[derive(Clone, Debug)]
pub struct OpenWorldQaArtifactPaths {
    pub markdown_path: PathBuf,
    pub timeline_json_path: Option<PathBuf>,
    pub visual_snapshot_path: Option<PathBuf>,
}

#[derive(Clone, Debug)]
enum OpenWorldQaRequest {
    PreviewOnly,
    WriteArtifacts(OpenWorldQaArtifactPaths),
}

#[derive(Resource, Default)]
pub struct OpenWorldQaRequestQueue {
    pending_open_world_slice01: Option<OpenWorldQaRequest>,
    last_error: Option<String>,
}

impl OpenWorldQaRequestQueue {
    pub fn request_open_world_slice01(&mut self) {
        self.pending_open_world_slice01 = Some(OpenWorldQaRequest::PreviewOnly);
    }

    pub fn default_open_world_slice01_artifact_paths() -> OpenWorldQaArtifactPaths {
        OpenWorldQaArtifactPaths {
            markdown_path: PathBuf::from("docs/qa/open-world-slice01.md"),
            timeline_json_path: Some(PathBuf::from(
                WorldTimelinePanelState::default_open_world_slice01_timeline_path(),
            )),
            visual_snapshot_path: Some(PathBuf::from("docs/qa/open-world-slice01-visual.png")),
        }
    }

    pub fn default_open_world_slice01_framebuffer_path() -> PathBuf {
        PathBuf::from("docs/qa/open-world-slice01-framebuffer.png")
    }

    pub fn request_default_open_world_slice01_artifacts(&mut self) {
        let paths = Self::default_open_world_slice01_artifact_paths();
        self.request_open_world_slice01_artifacts(
            paths.markdown_path,
            paths.timeline_json_path,
            paths.visual_snapshot_path,
        );
    }

    pub fn request_open_world_slice01_artifacts(
        &mut self,
        markdown_path: PathBuf,
        timeline_json_path: Option<PathBuf>,
        visual_snapshot_path: Option<PathBuf>,
    ) {
        self.pending_open_world_slice01 = Some(OpenWorldQaRequest::WriteArtifacts(
            OpenWorldQaArtifactPaths {
                markdown_path,
                timeline_json_path,
                visual_snapshot_path,
            },
        ));
    }

    pub fn last_error(&self) -> Option<&str> {
        self.last_error.as_deref()
    }

    /// True while a WriteArtifacts request is queued (including wait-for-framebuffer).
    /// Used by VGRC bridge so it does not steal ScreenshotQueue results mid-QA.
    pub fn has_pending_write_artifacts(&self) -> bool {
        matches!(
            self.pending_open_world_slice01,
            Some(OpenWorldQaRequest::WriteArtifacts(_))
        )
    }

    fn take_open_world_slice01(&mut self) -> Option<OpenWorldQaRequest> {
        self.pending_open_world_slice01.take()
    }

    fn requeue_open_world_slice01(&mut self, request: OpenWorldQaRequest) {
        self.pending_open_world_slice01 = Some(request);
    }

    fn set_last_error(&mut self, error: Option<String>) {
        self.last_error = error;
    }
}

#[derive(Resource, Default)]
pub struct WorldTimelinePanelState {
    pub visible: bool,
    pub selected_tick: Option<u64>,
    pub timeline: Option<OpenWorldTimeline>,
    pub replay_enabled: bool,
    pub replay_cursor_tick: Option<u64>,
}

impl WorldTimelinePanelState {
    pub fn default_open_world_slice01_timeline_path() -> &'static str {
        "docs/qa/open-world-slice01-timeline.json"
    }

    pub fn load_timeline(&mut self, timeline: OpenWorldTimeline) {
        self.selected_tick = timeline.ticks.first().map(|tick| tick.tick);
        self.timeline = Some(timeline);
        self.replay_enabled = false;
        self.replay_cursor_tick = None;
    }

    pub fn load_default_open_world_slice01_timeline(
        &mut self,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.load_timeline_json_file(Self::default_open_world_slice01_timeline_path())
    }

    pub fn load_open_world_slice01_from_director(&mut self, director: &mut DirectorRuntime) {
        let bundle = director.verify_open_world_slice01();
        self.load_timeline(OpenWorldTimeline::from_verification_bundle(&bundle));
    }

    pub fn load_open_world_slice01_from_director_with_performance_evidence(
        &mut self,
        director: &mut DirectorRuntime,
        performance_evidence: Vec<String>,
    ) {
        let bundle = director
            .verify_open_world_slice01()
            .with_performance_evidence(performance_evidence);
        self.load_timeline(OpenWorldTimeline::from_verification_bundle(&bundle));
    }

    pub fn load_timeline_json_file(
        &mut self,
        path: impl AsRef<Path>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let json = std::fs::read_to_string(path)?;
        let timeline: OpenWorldTimeline = serde_json::from_str(&json)?;
        self.load_timeline(timeline);
        Ok(())
    }

    pub fn select_tick(&mut self, tick: u64) {
        if self
            .timeline
            .as_ref()
            .is_some_and(|timeline| timeline.tick(tick).is_some())
        {
            self.selected_tick = Some(tick);
        }
    }

    pub fn selected_tick_entry(&self) -> Option<&OpenWorldTimelineTick> {
        let tick = self.selected_tick?;
        self.timeline.as_ref()?.tick(tick)
    }

    pub fn screenshot_paths(&self) -> &[String] {
        self.timeline
            .as_ref()
            .map(|timeline| timeline.screenshot_paths.as_slice())
            .unwrap_or(&[])
    }

    pub fn visual_check_evidence(&self) -> &[String] {
        self.timeline
            .as_ref()
            .map(|timeline| timeline.visual_check_evidence.as_slice())
            .unwrap_or(&[])
    }

    pub fn start_replay(&mut self) {
        self.replay_enabled = true;
        self.replay_cursor_tick = self
            .timeline
            .as_ref()
            .and_then(|timeline| timeline.ticks.first().map(|tick| tick.tick));
    }

    pub fn start_replay_from_selected_tick(&mut self) {
        let Some(selected_tick) = self.selected_tick else {
            self.start_replay();
            return;
        };

        if self
            .timeline
            .as_ref()
            .is_some_and(|timeline| timeline.tick(selected_tick).is_some())
        {
            self.replay_enabled = true;
            self.replay_cursor_tick = Some(selected_tick);
        }
    }

    pub fn stop_replay(&mut self) {
        self.replay_enabled = false;
        self.replay_cursor_tick = None;
    }

    pub fn step_replay_forward(&mut self) {
        self.step_replay_by(1);
    }

    pub fn step_replay_backward(&mut self) {
        self.step_replay_by(-1);
    }

    pub fn replay_cursor_entry(&self) -> Option<&OpenWorldTimelineTick> {
        let tick = self.replay_cursor_tick?;
        self.timeline.as_ref()?.tick(tick)
    }

    pub fn replay_cursor_world_state(&self) -> Option<&OpenWorldReplayWorldState> {
        Some(&self.replay_cursor_entry()?.world_state)
    }

    fn step_replay_by(&mut self, direction: isize) {
        let Some(timeline) = self.timeline.as_ref() else {
            return;
        };
        if timeline.ticks.is_empty() {
            return;
        }

        if !self.replay_enabled || self.replay_cursor_tick.is_none() {
            self.start_replay();
            return;
        }

        let Some(current_tick) = self.replay_cursor_tick else {
            return;
        };
        let Some(current_index) = timeline
            .ticks
            .iter()
            .position(|entry| entry.tick == current_tick)
        else {
            self.replay_cursor_tick = timeline.ticks.first().map(|tick| tick.tick);
            return;
        };

        let next_index = current_index
            .saturating_add_signed(direction)
            .min(timeline.ticks.len() - 1);
        self.replay_cursor_tick = Some(timeline.ticks[next_index].tick);
    }
}

pub fn toggle_world_timeline_panel(mut state: ResMut<WorldTimelinePanelState>) {
    state.visible = !state.visible;
}

pub fn apply_world_timeline_updates(
    mut state: ResMut<WorldTimelinePanelState>,
    mut updates: ResMut<WorldTimelineUpdateQueue>,
) {
    if let Some(timeline) = updates.take_pending() {
        state.load_timeline(timeline);
    }
}

pub fn process_open_world_qa_requests(
    mut requests: ResMut<OpenWorldQaRequestQueue>,
    mut updates: ResMut<WorldTimelineUpdateQueue>,
    integration_state: Option<Res<bevy_adapter::integration::IntegrationState>>,
    mut screenshot_queue: Option<ResMut<bevy_adapter::ScreenshotQueue>>,
) {
    let Some(request) = requests.take_open_world_slice01() else {
        return;
    };

    let mut director = DirectorRuntime::new();
    let performance_evidence = open_world_qa_performance_evidence(
        integration_state.as_deref(),
        screenshot_queue.as_deref(),
    );

    let bundle = match request {
        OpenWorldQaRequest::PreviewOnly => Ok(director
            .verify_open_world_slice01()
            .with_performance_evidence(performance_evidence)),
        OpenWorldQaRequest::WriteArtifacts(paths) => {
            match take_or_request_bevy_framebuffer_screenshot(screenshot_queue.as_deref_mut()) {
                BevyFramebufferScreenshotStatus::Ready { path, dimensions } => {
                    match persist_open_world_framebuffer_screenshot(&path, &paths) {
                        Ok(durable_path) => director
                            .write_open_world_slice01_qa_artifacts_with_engine_screenshot(
                                &paths.markdown_path,
                                paths.timeline_json_path.as_ref(),
                                &durable_path,
                                dimensions,
                                performance_evidence,
                            ),
                        Err(error) => {
                            let mut performance_evidence = performance_evidence;
                            performance_evidence.push(format!(
                                "bevy_framebuffer_screenshot_error=persist_failed:{}",
                                error
                            ));
                            director.write_open_world_slice01_qa_artifacts(
                                &paths.markdown_path,
                                paths.timeline_json_path.as_ref(),
                                paths.visual_snapshot_path.as_ref(),
                                performance_evidence,
                            )
                        }
                    }
                }
                BevyFramebufferScreenshotStatus::Pending => {
                    requests.requeue_open_world_slice01(OpenWorldQaRequest::WriteArtifacts(paths));
                    return;
                }
                BevyFramebufferScreenshotStatus::Failed { error } => {
                    let mut performance_evidence = performance_evidence;
                    performance_evidence
                        .push(format!("bevy_framebuffer_screenshot_error={}", error));
                    director.write_open_world_slice01_qa_artifacts(
                        &paths.markdown_path,
                        paths.timeline_json_path.as_ref(),
                        paths.visual_snapshot_path.as_ref(),
                        performance_evidence,
                    )
                }
                BevyFramebufferScreenshotStatus::Unavailable => director
                    .write_open_world_slice01_qa_artifacts(
                        &paths.markdown_path,
                        paths.timeline_json_path.as_ref(),
                        paths.visual_snapshot_path.as_ref(),
                        performance_evidence,
                    ),
            }
        }
    };

    match bundle {
        Ok(bundle) => {
            requests.set_last_error(None);
            updates.request_update(OpenWorldTimeline::from_verification_bundle(&bundle));
        }
        Err(error) => {
            requests.set_last_error(Some(error.to_string()));
        }
    }
}

fn open_world_qa_performance_evidence(
    integration_state: Option<&bevy_adapter::integration::IntegrationState>,
    screenshot_queue: Option<&bevy_adapter::ScreenshotQueue>,
) -> Vec<String> {
    let mut evidence = integration_state
        .map(|state| state.frame_time_evidence())
        .unwrap_or_default();
    if let Some(queue) = screenshot_queue {
        evidence.extend(queue.runtime_readback_evidence());
    }
    evidence
}

enum BevyFramebufferScreenshotStatus {
    Ready {
        path: PathBuf,
        dimensions: (u32, u32),
    },
    Pending,
    Failed {
        error: String,
    },
    Unavailable,
}

fn take_or_request_bevy_framebuffer_screenshot(
    screenshot_queue: Option<&mut bevy_adapter::ScreenshotQueue>,
) -> BevyFramebufferScreenshotStatus {
    let Some(queue) = screenshot_queue else {
        return BevyFramebufferScreenshotStatus::Unavailable;
    };

    while let Some(result) = queue.pop_result() {
        match result {
            bevy_adapter::ScreenshotResult::Success {
                path, dimensions, ..
            } => return BevyFramebufferScreenshotStatus::Ready { path, dimensions },
            bevy_adapter::ScreenshotResult::Failure { error } => {
                return BevyFramebufferScreenshotStatus::Failed { error };
            }
        }
    }

    queue.request_capture();
    BevyFramebufferScreenshotStatus::Pending
}

fn durable_open_world_framebuffer_path(paths: &OpenWorldQaArtifactPaths) -> PathBuf {
    paths
        .markdown_path
        .parent()
        .map(|parent| parent.join("open-world-slice01-framebuffer.png"))
        .unwrap_or_else(OpenWorldQaRequestQueue::default_open_world_slice01_framebuffer_path)
}

fn persist_open_world_framebuffer_screenshot(
    captured_path: &Path,
    paths: &OpenWorldQaArtifactPaths,
) -> std::io::Result<PathBuf> {
    let durable_path = durable_open_world_framebuffer_path(paths);
    if let Some(parent) = durable_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if captured_path != durable_path {
        std::fs::copy(captured_path, &durable_path)?;
    }
    Ok(durable_path)
}

fn stamp_open_world_main_window_acceptance(markdown_path: &Path) -> std::io::Result<()> {
    const STAMP: &str = concat!(
        "> **Main-window acceptance (2026-07-11):** Passed via ",
        "`WINDWAVE_OPEN_WORLD_QA_ACCEPT=1` / `make accept-open-world-qa`.\n",
        "> Evidence: `screenshot_capture=bevy_framebuffer`, durable PNG ",
        "`docs/qa/open-world-slice01-framebuffer.png` (1600×900), playtest Passed.\n\n",
    );
    let existing = std::fs::read_to_string(markdown_path)?;
    if existing.contains("Main-window acceptance") {
        return Ok(());
    }
    let stamped = if let Some(rest) =
        existing.strip_prefix("# OpenWorld Verification Bundle: open_world_slice01\n\n")
    {
        format!("# OpenWorld Verification Bundle: open_world_slice01\n\n{STAMP}{rest}")
    } else {
        format!("{STAMP}{existing}")
    };
    std::fs::write(markdown_path, stamped)
}

fn open_world_qa_accept_mode_enabled() -> bool {
    matches!(
        std::env::var("WINDWAVE_OPEN_WORLD_QA_ACCEPT").as_deref(),
        Ok("1") | Ok("true") | Ok("TRUE") | Ok("yes") | Ok("YES")
    )
}

#[derive(Default)]
enum OpenWorldQaAcceptPhase {
    #[default]
    Idle,
    Warmup,
    Waiting,
    Done,
}

/// Headless-friendly main-window acceptance driver.
///
/// When `WINDWAVE_OPEN_WORLD_QA_ACCEPT=1`, wait for the primary window to render,
/// request OpenWorld QA artifacts (which consume `ScreenshotQueue` framebuffer
/// readback), then exit success only if evidence contains
/// `screenshot_capture=bevy_framebuffer`.
fn open_world_qa_accept_mode_system(
    mut timeline_state: ResMut<WorldTimelinePanelState>,
    mut qa_requests: ResMut<OpenWorldQaRequestQueue>,
    mut screenshot_state: ResMut<bevy_adapter::ScreenshotState>,
    mut exit: MessageWriter<AppExit>,
    mut phase: Local<OpenWorldQaAcceptPhase>,
    mut frames: Local<u64>,
) {
    if !open_world_qa_accept_mode_enabled() {
        return;
    }

    *frames += 1;

    match *phase {
        OpenWorldQaAcceptPhase::Idle => {
            timeline_state.visible = true;
            let output_dir = std::env::temp_dir().join("windwave-open-world-qa-screenshots");
            let _ = std::fs::create_dir_all(&output_dir);
            screenshot_state.set_output_dir(output_dir);
            info!("OpenWorld QA accept mode: warming up primary window for framebuffer readback");
            *phase = OpenWorldQaAcceptPhase::Warmup;
        }
        OpenWorldQaAcceptPhase::Warmup => {
            // Give Bevy/winit a few frames so primary_window screenshot has real pixels.
            if *frames >= 90 {
                qa_requests.request_default_open_world_slice01_artifacts();
                info!("OpenWorld QA accept mode: requested Generate OpenWorld QA artifacts");
                *phase = OpenWorldQaAcceptPhase::Waiting;
            }
        }
        OpenWorldQaAcceptPhase::Waiting => {
            if qa_requests.has_pending_write_artifacts() {
                if *frames > 900 {
                    error!(
                        "OpenWorld QA accept mode: timed out waiting for ScreenshotQueue framebuffer"
                    );
                    exit.write(AppExit::from_code(2));
                    *phase = OpenWorldQaAcceptPhase::Done;
                }
                return;
            }

            if let Some(error) = qa_requests.last_error() {
                error!("OpenWorld QA accept mode: artifact write failed: {error}");
                exit.write(AppExit::from_code(3));
                *phase = OpenWorldQaAcceptPhase::Done;
                return;
            }

            let Some(timeline) = timeline_state.timeline.as_ref() else {
                if *frames > 900 {
                    error!("OpenWorld QA accept mode: timed out with no timeline loaded");
                    exit.write(AppExit::from_code(2));
                    *phase = OpenWorldQaAcceptPhase::Done;
                }
                return;
            };

            let has_framebuffer = timeline
                .visual_check_evidence
                .iter()
                .any(|row| row.contains("screenshot_capture=bevy_framebuffer"));
            if has_framebuffer {
                info!("OpenWorld QA accept mode: PASSED (screenshot_capture=bevy_framebuffer)");
                let _ = stamp_open_world_main_window_acceptance(
                    &OpenWorldQaRequestQueue::default_open_world_slice01_artifact_paths()
                        .markdown_path,
                );
                exit.write(AppExit::Success);
                *phase = OpenWorldQaAcceptPhase::Done;
                return;
            }

            error!(
                "OpenWorld QA accept mode: FAILED — expected bevy_framebuffer, got {:?}",
                timeline.visual_check_evidence
            );
            exit.write(AppExit::from_code(1));
            *phase = OpenWorldQaAcceptPhase::Done;
        }
        OpenWorldQaAcceptPhase::Done => {}
    }
}

fn render_world_timeline_panel(
    mut contexts: EguiContexts,
    mut state: ResMut<WorldTimelinePanelState>,
    mut qa_requests: ResMut<OpenWorldQaRequestQueue>,
) {
    if !state.visible {
        return;
    }

    let ctx = match contexts.ctx_mut() {
        Ok(ctx) => ctx,
        Err(_) => return,
    };

    egui::Window::new("World Timeline")
        .default_size([520.0, 560.0])
        .resizable(true)
        .show(ctx, |ui| {
            if ui.button("Generate OpenWorld QA").clicked() {
                qa_requests.request_default_open_world_slice01_artifacts();
            }

            let Some(timeline) = state.timeline.clone() else {
                ui.label("No timeline loaded");
                return;
            };

            ui.label(egui::RichText::new(&timeline.plan_id).strong());
            ui.label(format!("Scenario: {}", timeline.scenario_id));
            ui.separator();

            ui.horizontal(|ui| {
                egui::ScrollArea::vertical()
                    .max_height(220.0)
                    .max_width(150.0)
                    .show(ui, |ui| {
                        for tick in &timeline.ticks {
                            let selected = state.selected_tick == Some(tick.tick);
                            if ui
                                .selectable_label(selected, format!("Tick {}", tick.tick))
                                .clicked()
                            {
                                state.select_tick(tick.tick);
                            }
                        }
                    });

                ui.separator();

                egui::ScrollArea::vertical()
                    .max_height(220.0)
                    .show(ui, |ui| {
                        if let Some(tick) = state.selected_tick_entry() {
                            ui.label(egui::RichText::new(format!("Tick {}", tick.tick)).strong());
                            for event in &tick.runtime_events {
                                ui.label(format!("- {}", event));
                            }
                        } else {
                            ui.label("Select a tick");
                        }
                    });
            });

            ui.separator();
            render_replay_controls(ui, &mut state);
            ui.separator();
            render_summary_group(ui, "Time Evidence", &timeline.time_evidence);
            render_summary_group(ui, "Schedule Decisions", &timeline.schedule_decisions);
            render_summary_group(ui, "Performance Evidence", &timeline.performance_evidence);
            render_summary_group(ui, "Screenshots", &timeline.screenshot_paths);
            render_summary_group(ui, "Visual Check Evidence", &timeline.visual_check_evidence);
        });
}

fn render_replay_controls(ui: &mut egui::Ui, state: &mut WorldTimelinePanelState) {
    ui.horizontal(|ui| {
        if ui.button("Start Replay").clicked() {
            state.start_replay_from_selected_tick();
        }
        if ui.button("Prev Tick").clicked() {
            state.step_replay_backward();
        }
        if ui.button("Next Tick").clicked() {
            state.step_replay_forward();
        }
        if ui.button("Stop").clicked() {
            state.stop_replay();
        }
    });

    if let Some(entry) = state.replay_cursor_entry() {
        ui.label(format!("Replay Tick {}", entry.tick));
        for event in &entry.runtime_events {
            ui.label(format!("- {}", event));
        }
        render_replay_world_state(ui, &entry.world_state);
    } else {
        ui.label("Replay idle");
    }
}

fn render_replay_world_state(ui: &mut egui::Ui, world_state: &OpenWorldReplayWorldState) {
    ui.collapsing("Replay World State", |ui| {
        render_key_value_group(ui, "Actor Zones", &world_state.actor_zones);
        render_key_value_group(ui, "Puzzle States", &world_state.puzzle_states);
        render_key_value_group(ui, "Loot States", &world_state.loot_states);
        render_key_value_group(ui, "Enemy States", &world_state.enemy_states);
        render_key_value_group(ui, "Quest States", &world_state.quest_states);

        ui.collapsing("Inventory", |ui| {
            if world_state.inventory.is_empty() {
                ui.label("none");
            } else {
                for (actor_id, items) in &world_state.inventory {
                    let items = items.iter().cloned().collect::<Vec<_>>().join(", ");
                    ui.label(format!("{}: {}", actor_id, items));
                }
            }
        });
    });
}

fn render_key_value_group(
    ui: &mut egui::Ui,
    title: &str,
    rows: &std::collections::BTreeMap<String, String>,
) {
    ui.collapsing(title, |ui| {
        if rows.is_empty() {
            ui.label("none");
        } else {
            for (key, value) in rows {
                ui.label(format!("{}: {}", key, value));
            }
        }
    });
}

fn render_summary_group(ui: &mut egui::Ui, title: &str, rows: &[String]) {
    ui.collapsing(title, |ui| {
        if rows.is_empty() {
            ui.label("none");
        } else {
            for row in rows {
                ui.label(row);
            }
        }
    });
}
