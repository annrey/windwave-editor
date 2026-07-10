# VerificationBundle

> 日期：2026-06-07  
> 状态：OWS-7 初版已落地到 `crates/agent-core/src/open_world_verification.rs`，并已覆盖应用 small-island template 后的 SceneBridge 验证；2026-07-05 已接入 `PlayableScenario` 时间证据  
> 关联 PRD：`docs/prd/open-world-vertical-slice-prd.md`

## 用途

`OpenWorldVerificationBundle` 把开放世界垂直切片的一次自动验收合并成一份可序列化 QA artifact：

```text
OpenWorldPlan.verification_goals
  + PlayableScenarioReport
  + OpenWorldRuntimeEvent
  + WorldClock / time evidence
  + SceneBridge / SceneIndex query evidence
  + optional screenshots / Director events / Engine events
  -> VerificationBundle
  -> Markdown QA report
```

它解决的问题是：Agent 不能只说“playtest passed”。后续每次对话生成游戏切片，都应该产出一份能说明通过/失败原因、目标状态、最近事件和修复建议的 bundle。

## Rust API

公开模块：

```rust
agent_core::open_world_verification
```

主要类型：

- `OpenWorldVerificationBundle`
- `OpenWorldVerificationResult`
- `VerificationBundleStatus`
- `VerificationGoalStatus`
- `VerificationEvidence`
- `PlaytestVerificationSummary`

主路径调用：

```rust
let plan = OpenWorldPlan::open_world_slice01_fixture();
let scenario = PlayableScenario::open_world_slice01_main_path(&plan);
let mut state = PlayableScenarioState::from_plan(&plan);
let report = scenario.run(&mut state);

let bundle = OpenWorldVerificationBundle::from_playtest(
    &plan,
    &scenario,
    &state,
    &report,
);

let markdown = bundle.to_markdown();
```

接入真实 `SceneBridge`：

```rust
let bundle = OpenWorldVerificationBundle::from_playtest_with_scene_bridge(
    &plan,
    &scenario,
    &state,
    &report,
    scene_bridge,
);
```

Director 入口：

```rust
let mut director = DirectorRuntime::new();
let bundle = director.verify_open_world_slice01();
let markdown = director.verify_open_world_slice01_markdown();
```

## 当前支持的验证表达式

第一版只支持 PRD fixture 中已经使用的表达式，避免过早做一套完整 DSL：

| 表达式 | 示例 | 数据来源 |
|---|---|---|
| `exists("id")` | `exists("player")` | 优先 `SceneBridge::query_entities`；无 bridge 时回退 runtime object manifest |
| `has_component("id", "Component")` | `has_component("player", "PlayerController")` | 优先 `SceneBridge::query_entities`；无 bridge 时回退 runtime primitives |
| `state("id") == "State"` | `state("puzzle_switch") == "Solved"` | puzzle / loot / enemy / quest states |
| `inventory_contains("actor", "item")` | `inventory_contains("player", "reward_item")` | runtime inventory |
| `quest_state("id") == "State"` | `quest_state("main_quest") == "Completed"` | runtime quest state |

不支持的表达式会标记为 `Unsupported`，并让 bundle 进入失败状态，防止 Agent 把无法验证的目标当作已通过。

## Bundle 内容

`OpenWorldVerificationBundle` 包含：

- `plan_id`
- `scenario_id`
- `status`
- `playtest`
- `goals`
- `evidence`

`VerificationEvidence` 当前包含：

- `runtime_events`
- `playtest_events`
- `time_evidence`
- `schedule_decisions`
- `performance_evidence`
- `scene_index_observations`
- `screenshot_paths`
- `visual_check_evidence`
- `director_events`
- `engine_events`

其中 `screenshot_paths` 已可由 `DirectorRuntime::verify_open_world_slice01_with_visual_snapshot()` 填入 SceneIndex proxy PNG 路径；`visual_check_evidence` 已由 Director 写入当前 SceneIndex proxy 视觉检查结果，并在生成 PNG 时标记 `screenshot_capture=scene_index_proxy_png`。2026-07-05 已新增 `DirectorRuntime::write_open_world_slice01_qa_artifacts_with_engine_screenshot(...)`，可把 Bevy 已保存的 framebuffer screenshot 路径写入 bundle / timeline，并标记 `screenshot_capture=bevy_framebuffer` 与截图尺寸。UI QA 请求消费 `ScreenshotQueue::Failure` 时会回退到 SceneIndex proxy PNG，同时在 `performance_evidence` 记录 `bevy_framebuffer_screenshot_error=...`，避免截图失败在 QA artifact 中消失。2026-07-06 起，`bevy-adapter::ScreenshotQueue::runtime_readback_evidence()` 可输出 `bevy_screenshot_requested`、请求总数、成功/失败总数、队列结果数和最后一次 readback 摘要；2026-07-07 已由 `agent-ui` QA artifact writer 汇总进 bundle / timeline / Markdown 的 `performance_evidence`，用于诊断运行时截图链路是否真的被请求、完成或失败；这仍是 runtime diagnostic evidence，不替代完整 app 主窗口视觉验收。Director events、engine events 已可由 `DirectorRuntime::verify_open_world_slice01()` 填充。

注意：bundle 连接 `SceneBridge` 后会如实使用 bridge 查询结果；如果 bridge 中没有 `player`，`player_exists` 会失败。当前 small-island template 已能把 `OpenWorldPlan.object_manifest` 中的 `player`、`puzzle_switch`、`reward_chest`、`camp_enemy_01` 等对象写入 `SceneBridge`，因此“应用模板后的 mock bridge”验收路径可以通过。

`bevy-adapter` 侧已新增开放世界 marker components，可把 template component patch 写入真实 Bevy World，并由 `BevyAdapter::build_scene_index()` 读回 `PlayerController`、`PuzzleSwitch`、`LootContainer`、`EnemyBrain` 等组件 marker。

2026-07-05 后，bundle 会从 `PlayableScenarioReport.time_evidence` 汇总一条确定性时间证据：

```text
wall_time=2026-07-05T12:00:00+00:00 sim_time_ms=0 tick=13 clock_mode=Frozen
```

Markdown report 会输出 `## Time Evidence` 段落，用于把 playtest 结果、失败 step 和后续 replay/debug 面板对齐到同一套 `WorldClock` 语义。

2026-07-07 后，bundle 也会把同一套 `WorldClock` 摘要追加到 `scene_index_observations`，例如：

```text
world_clock wall_time=2026-07-05T12:00:00+00:00 sim_time_ms=0 tick=13 clock_mode=Frozen
```

这让 SceneIndex-style observations 不只描述对象和事件，也能说明观察发生时的现实时间、模拟时间和 tick。

同日，`runtime_events` evidence 也开始保留 runtime event tick 前缀，例如：

```text
tick=7 player defeated camp_enemy_01
```

`agent_schedules` 接入后，bundle 还会输出 `## Schedule Decisions`，用于证明 AI 代理在当前 `WorldClock` 下是否处于允许行动窗口。例如 `OpenWorldSlice01` 的商人 fixture：

```text
merchant_01 active window=shop_hours allowed=quote_price,restock_low_risk_item
```

同一 schedule decision 也会以 `agent_schedule ...` 行追加到 `scene_index_observations`，方便 World Timeline / Replay 调试时在同一 observation 面板里看到 AI 行动窗口。

性能/规模 evidence 会输出稳定计数指标，并记录 bundle 构造与 SceneIndex 观测耗时。耗时值只作为可解析观测指标，不作为固定数值断言，避免 CI 和本地机器差异污染测试：

```text
plan_objects=10
runtime_events=21
scene_index_observations=24
verification_goals=5
scenario_steps=13
verification_duration_us=38
scene_index_observation_duration_us=1
```

`bevy-adapter::integration::IntegrationState` 还会采集 Bevy `Time` 的 frame delta，并可输出以下外部 engine performance evidence rows；core 通过 `OpenWorldVerificationBundle::with_performance_evidence(...)` 追加这些行，避免 `agent-core` 直接依赖 Bevy。`agent-ui` 的 `WorldTimelinePanelState::load_open_world_slice01_from_director_with_performance_evidence(...)` 已提供 UI/app 侧注入入口：

```text
bevy_frame_count=2
bevy_frame_time_samples=2
bevy_frame_time_avg_ms=16.667
bevy_frame_time_max_ms=20.000
```

## 当前测试

窄测试：

```bash
cargo test -p agent-core open_world_verification
```

已覆盖：

- 成功的 `OpenWorldSlice01` 主线能产出 `Passed` bundle。
- 失败 fixture 能携带 playtest failure、suggested fix 和目标失败详情。
- `SceneBridge` 存在时，`exists(...)` 目标由 bridge 查询决定。
- 应用 small-island template 后，SceneBridge 查询路径可让 bundle 通过。
- `DirectorRuntime::verify_open_world_slice01()` 可生成 bundle 和 Markdown。
- bundle 可 JSON round-trip。
- bundle 可输出 Markdown QA report。
- bundle 会携带 `time_evidence`，Markdown report 会输出 `## Time Evidence`。
- bundle 的 `runtime_events` evidence 会输出 runtime event tick。
- bundle 会携带 `schedule_decisions`，Markdown report 会输出 `## Schedule Decisions`。
- bundle 会携带 `performance_evidence`，Markdown report 会输出 `## Performance Evidence`。
- bundle 会携带 `visual_check_evidence`，Markdown report 会输出 `## Visual Check Evidence`。

回归测试：

```bash
cargo test -p agent-core open_world_runtime
cargo test -p agent-core playable_scenario
```

QA report 落盘入口：

```bash
cargo run -p agent-core --example write_open_world_slice01_qa -- docs/qa/open-world-slice01.md
```

同时生成 World Timeline JSON。JSON 中每个 tick 包含 runtime events 和累计后的 `world_state` 快照，用于 Replay panel 定位玩家区域、任务状态、敌人状态、战利品状态和背包：

```bash
cargo run -p agent-core --example write_open_world_slice01_qa -- docs/qa/open-world-slice01.md --timeline-json docs/qa/open-world-slice01-timeline.json
```

同时生成 SceneIndex proxy PNG，并把路径写入 bundle / timeline。CLI 已走 `DirectorRuntime::write_open_world_slice01_qa_artifacts(...)` 统一入口；UI/app 侧可通过 `OpenWorldQaRequestQueue::request_open_world_slice01_artifacts(...)` 传入 markdown、timeline JSON 和 proxy PNG 路径，由 World Timeline QA request system 调用同一 writer，并把结果同步回面板 timeline：

```bash
cargo run -p agent-core --example write_open_world_slice01_qa -- docs/qa/open-world-slice01.md --timeline-json docs/qa/open-world-slice01-timeline.json --visual-snapshot-png docs/qa/open-world-slice01-visual.png
```

## 后续接线

1. 将真实 Bevy gameplay 行为系统接到 `PlayerController`、`PuzzleSwitch`、`LootContainer`、`EnemyBrain` 等 marker components。✅ 已完成 `LootContainer` -> core runtime、`Combatant`/`EnemyBrain` -> core runtime 的 adapter bridge、`InteractionCompleteEvent` -> open-world command queue/executor，以及最小 `OpenWorldQuestPanel`
2. 将截图路径和视觉检查结果填入 bundle。✅ 已完成 `visual_check_evidence`、SceneIndex proxy PNG、`screenshot_paths` 与 timeline JSON 透传；UI QA 请求已能等待并消费 `bevy-adapter::ScreenshotQueue` 的 Bevy framebuffer screenshot result，写入 `screenshot_capture=bevy_framebuffer`、路径和尺寸；framebuffer 失败时回退 SceneIndex proxy PNG 并记录 `bevy_framebuffer_screenshot_error=...`；`ScreenshotQueue::runtime_readback_evidence()` 已提供截图请求/成功/失败/最后结果的 runtime diagnostic evidence，并由 UI QA artifact writer 写入 `performance_evidence`；完整 app 主窗口 readback 仍待运行时验收
3. 将 Markdown report 自动落盘到 `docs/qa/open-world-slice01.md`，并保留 `Time Evidence` 段。✅
4. 增加性能 evidence：实体数、命令耗时、SceneIndex 耗时、帧时间。✅ 已完成稳定计数指标、bundle 构造耗时、SceneIndex 观测耗时、`bevy-adapter` frame delta evidence rows、`agent-ui` 注入入口、World Timeline 面板 `Generate OpenWorld QA` 按钮触发、UI QA 请求队列到 `write_open_world_slice01_qa_artifacts(...)` 的可配置落盘接线，以及 UI QA 请求队列对 Bevy framebuffer screenshot result 的等待/消费；完整 app 主窗口 readback 仍待运行时验收
5. 将 timestamped runtime events 接到 World Timeline / Replay panel。✅ 已完成 core DTO、QA JSON、每 tick `world_state` 快照、最小 agent-ui 面板、`OpenWorldQuestPanel`、QA JSON 文件加载、Director 状态注入 API、Bevy resource 热更新、最小 Replay tick 控制，以及 `bevy-adapter` replay state 组件应用、`Visibility`/交互可用性驱动、loot/combat interaction bridge、真实交互事件命令队列和 SceneIndex 暴露
