use agent_core::{
    OpenWorldPlan, OpenWorldTimeline, OpenWorldVerificationBundle, PlayableScenario,
    PlayableScenarioState,
};

#[test]
fn timeline_groups_runtime_events_by_tick_and_keeps_summary_panels() {
    let plan = OpenWorldPlan::open_world_slice01_fixture();
    let scenario = PlayableScenario::open_world_slice01_main_path(&plan);
    let mut state = PlayableScenarioState::from_plan(&plan);
    let report = scenario.run(&mut state);
    let bundle = OpenWorldVerificationBundle::from_playtest(&plan, &scenario, &state, &report);

    let timeline = OpenWorldTimeline::from_verification_bundle(&bundle);

    assert_eq!(timeline.plan_id, "open_world_slice01");
    assert_eq!(
        timeline.scenario_id,
        "playtest_open_world_slice01_main_path"
    );
    assert!(timeline.time_evidence[0].contains("tick=13"));
    assert!(timeline.schedule_decisions.contains(
        &"merchant_01 active window=shop_hours allowed=quote_price,restock_low_risk_item"
            .to_string()
    ));
    assert!(timeline
        .performance_evidence
        .contains(&"runtime_events=21".to_string()));

    let bundle_with_visual = bundle
        .with_screenshot_paths(vec!["docs/qa/open-world-slice01.png".to_string()])
        .with_visual_check_evidence(vec!["visual_check=pass visible_objects=4".to_string()]);
    let timeline_with_visual = OpenWorldTimeline::from_verification_bundle(&bundle_with_visual);
    assert_eq!(
        timeline_with_visual.screenshot_paths,
        vec!["docs/qa/open-world-slice01.png"]
    );
    assert_eq!(
        timeline_with_visual.visual_check_evidence,
        vec!["visual_check=pass visible_objects=4"]
    );

    let tick_8 = timeline.tick(8).expect("tick 8 should exist");
    assert!(tick_8
        .runtime_events
        .iter()
        .any(|event| event == "player defeated camp_enemy_01"));

    let json = serde_json::to_string(&timeline).unwrap();
    let restored: OpenWorldTimeline = serde_json::from_str(&json).unwrap();
    assert_eq!(
        restored.tick(8).unwrap().runtime_events,
        tick_8.runtime_events
    );
}

#[test]
fn timeline_builds_replay_world_state_snapshots() {
    let plan = OpenWorldPlan::open_world_slice01_fixture();
    let scenario = PlayableScenario::open_world_slice01_main_path(&plan);
    let mut state = PlayableScenarioState::from_plan(&plan);
    let report = scenario.run(&mut state);
    let bundle = OpenWorldVerificationBundle::from_playtest(&plan, &scenario, &state, &report);

    let timeline = OpenWorldTimeline::from_verification_bundle(&bundle);

    let tick_8 = timeline.tick(8).expect("tick 8 should exist");
    assert_eq!(
        tick_8.world_state.enemy_states.get("camp_enemy_01"),
        Some(&"Dead".to_string())
    );

    let final_tick = timeline.ticks.last().expect("timeline should not be empty");
    assert_eq!(
        final_tick.world_state.actor_zones.get("player"),
        Some(&"camp_zone".to_string())
    );
    assert_eq!(
        final_tick.world_state.quest_states.get("main_quest"),
        Some(&"Completed".to_string())
    );
    assert!(final_tick
        .world_state
        .inventory
        .get("player")
        .is_some_and(|items| items.contains(&"reward_item".to_string())));
}
