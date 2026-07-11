# OpenWorld Verification Bundle: open_world_slice01

> **Main-window acceptance (2026-07-11):** Passed via `WINDWAVE_OPEN_WORLD_QA_ACCEPT=1` / `make accept-open-world-qa`.
> Evidence: `screenshot_capture=bevy_framebuffer`, durable PNG `docs/qa/open-world-slice01-framebuffer.png` (1600×900), playtest Passed.

Status: Passed
Scenario: `playtest_open_world_slice01_main_path`
Playtest: passed (13 steps, final quest: `Completed`)

## Verification Goals

| ID | Kind | Status | Observed |
|---|---|---|---|
| `player_exists` | SceneIndex | Passed | exists(player) via runtime = true |
| `puzzle_is_solved` | SceneIndex | Passed | state(puzzle_switch) = Solved |
| `enemy_defeated` | SceneIndex | Passed | state(camp_enemy_01) = Dead |
| `reward_collected` | SceneIndex | Passed | inventory_contains(player, reward_item) = true |
| `main_quest_completed` | Playtest | Passed | quest_state(main_quest) = Completed |

## Runtime Events

- tick=0 runtime state initialized from open_world_slice01
- tick=2 player is in puzzle_zone
- tick=2 objective reach_puzzle_zone completed
- tick=2 puzzle_switch awaiting interaction
- tick=3 puzzle_switch solved
- tick=3 objective solve_puzzle_switch completed
- tick=3 main_quest advanced to PuzzleSolved
- tick=3 reward_chest unlocked
- tick=6 player is in camp_zone
- tick=7 player aggroed camp_enemy_01
- tick=8 player hit camp_enemy_01 for 10 damage, 20 hp remaining
- tick=8 player hit camp_enemy_01 for 10 damage, 10 hp remaining
- tick=8 player hit camp_enemy_01 for 10 damage, 0 hp remaining
- tick=8 player defeated camp_enemy_01
- tick=8 objective defeat_camp_enemy completed
- tick=8 main_quest advanced to EnemyDefeated
- tick=10 player opened reward_chest
- tick=11 player collected reward_item
- tick=11 objective collect_reward_item completed
- tick=11 main_quest advanced to RewardCollected
- tick=11 main_quest advanced to Completed

## Time Evidence

- wall_time=2026-07-05T12:00:00+00:00 sim_time_ms=0 tick=13 clock_mode=Frozen

## Schedule Decisions

- merchant_01 active window=shop_hours allowed=quote_price,restock_low_risk_item

## Performance Evidence

- plan_objects=10
- runtime_objects=10
- runtime_events=21
- scene_index_observations=24
- verification_goals=5
- scenario_steps=13
- verification_duration_us=50
- scene_index_observation_duration_us=21
- bevy_frame_count=94
- bevy_frame_time_samples=94
- bevy_frame_time_avg_ms=20.973
- bevy_frame_time_max_ms=250.000
- bevy_screenshot_requested=true
- bevy_screenshot_requests_total=6
- bevy_screenshot_success_total=3
- bevy_screenshot_failure_total=0
- bevy_screenshot_result_count=1
- bevy_screenshot_last_result=success path=docs/qa/screenshot_2.png dimensions=1600x900

## SceneIndex Observations

- exists(player)
- exists(puzzle_switch)
- exists(reward_chest)
- exists(camp_enemy_01)
- player is in puzzle_zone
- objective reach_puzzle_zone completed
- puzzle_switch awaiting interaction
- puzzle_switch solved
- objective solve_puzzle_switch completed
- main_quest state PuzzleSolved
- reward_chest unlocked
- player is in camp_zone
- player aggroed camp_enemy_01
- player hit camp_enemy_01 damage 10 hp 20
- player hit camp_enemy_01 damage 10 hp 10
- player hit camp_enemy_01 damage 10 hp 0
- player defeated camp_enemy_01
- objective defeat_camp_enemy completed
- main_quest state EnemyDefeated
- player opened reward_chest
- player collected reward_item
- objective collect_reward_item completed
- main_quest state RewardCollected
- main_quest state Completed
- world_clock wall_time=2026-07-05T12:00:00+00:00 sim_time_ms=0 tick=13 clock_mode=Frozen
- agent_schedule merchant_01 active window=shop_hours allowed=quote_price,restock_low_risk_item

## Screenshots

- docs/qa/open-world-slice01-framebuffer.png

## Visual Check Evidence

- visual_check=engine_framebuffer visible_targets=4/4 targets=[player,puzzle_switch,reward_chest,camp_enemy_01] screenshot_capture=bevy_framebuffer path=docs/qa/open-world-slice01-framebuffer.png dimensions=1600x900

## Director Events

- GoalChecked task=1 all_matched=true summary=OpenWorldSlice01 verification Passed: 5 goals, playtest passed

## Engine Events

- scene_bridge_connected=false
- bevy_framebuffer_screenshot path=docs/qa/open-world-slice01-framebuffer.png dimensions=1600x900
