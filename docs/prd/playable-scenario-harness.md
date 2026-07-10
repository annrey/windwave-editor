# PlayableScenario Harness

> 日期：2026-06-07  
> 状态：OWS-3 初版已落地到 `crates/agent-core/src/playable_scenario.rs`  
> 关联 PRD：`docs/prd/open-world-vertical-slice-prd.md`

## 用途

`PlayableScenario` 是 WindWave 从“对话生成计划”走向“对话生成可玩游戏”的第一条自动验收线。

它当前是纯 `agent-core` 的脚本化 playtest harness，不模拟 Bevy 输入、物理或渲染。它验证的是：`OpenWorldPlan` 是否包含足够结构化的对象、primitive、任务状态和验证目标，使主线流程可以被确定性跑通。

OWS-5 后，`PlayableScenarioState` 内部持有 `OpenWorldRuntimeState`，任务、机关、宝箱、奖励和敌人状态变更都委托给 `agent_core::open_world_runtime`。

OWS-6 后，`AttackUntilDefeated` 不再直接把敌人置为 `Dead`，而是调用 `attack_until_defeated()`，通过多次 `CombatHit` 扣血完成敌人 objective。

2026-07-05 后，`PlayableScenarioState` 还持有 frozen `WorldClock`。每个 scripted step 执行前推进一个 tick，`PlayableScenarioReport` 和失败报告都会带 `PlayableScenarioTimeEvidence`，使 core playtest 的失败可以定位到确定性 tick。

## Rust API

公开模块：

```rust
agent_core::playable_scenario
```

主要类型：

- `PlayableScenario`
- `PlayableScenarioStep`
- `PlayableScenarioState`
- `PlayableScenarioReport`
- `PlayableScenarioFailureReport`
- `PlayableScenarioTimeEvidence`
- `QuestRuntimeState`（来自 `open_world_runtime`）
- `PuzzleRuntimeState`（来自 `open_world_runtime`）
- `LootRuntimeState`（来自 `open_world_runtime`）
- `EnemyRuntimeState`（来自 `open_world_runtime`）

从计划创建主线 playtest：

```rust
let plan = OpenWorldPlan::open_world_slice01_fixture();
let scenario = PlayableScenario::open_world_slice01_main_path(&plan);
let mut state = PlayableScenarioState::from_plan(&plan);
let report = scenario.run(&mut state);
```

## 当前脚本流程

`playtest_open_world_slice01_main_path` 覆盖：

```text
Assert quest main_quest is Active
Move player to puzzle_zone
Interact with puzzle_switch
Assert puzzle_switch is Solved
Assert reward_chest is Unlocked
Move player to camp_zone
Engage camp_enemy_01
Attack camp_enemy_01 until defeated
Assert camp_enemy_01 is Dead
Open reward_chest
Collect reward_item
Assert player inventory contains reward_item
Assert main_quest is Completed
```

## 失败报告

失败报告包含：

- `failed_step_index`
- `step_label`
- `reason`
- `quest_state`
- `suggested_fix`
- `time_evidence`
- `recent_events`
- `scene_index_observations`

示例：

```text
reason: puzzle_switch exists but is missing Interactable
quest_state: Active
suggested_fix: add Interactable to puzzle_switch
time_evidence.tick: 3
```

## 当前测试

窄测试：

```bash
cargo test -p agent-core playable_scenario
```

已覆盖：

- `OpenWorldSlice01` 主线路径可跑通。
- 缺少 `Interactable` 会在 puzzle 交互步骤失败，并给出修复建议。
- 敌人缺少 `Combatant` 会在战斗步骤失败，并给出修复建议。
- `AttackUntilDefeated` 复用 combat runtime，多次攻击后才完成击败目标。
- `PlayableScenarioState` 的 frozen `WorldClock` 会随每个 step 推进 tick。
- 成功和失败 report 都包含 `time_evidence`。
- `PlayableScenario` 可 JSON round-trip。

## 与用户对话生成游戏的关系

目标不是让 Agent 只生成“看起来合理”的计划，而是让每个对话生成的游戏切片都落到以下链路：

```text
User prompt
  -> OpenWorldPlan
  -> GameplayPrimitive validation
  -> OpenWorldRuntimeState
  -> PlayableScenario
  -> PlayableScenarioReport
  -> OpenWorldVerificationBundle
  -> Revise / Accept
```

如果 playtest 失败，Agent 后续应根据 `PlayableScenarioFailureReport` 修改计划或补齐 primitive，而不是向用户报告“已完成”。

## 后续接线

下一步应把 harness 往真实编辑器推进：

1. 将 `OpenWorldVerificationBundle` 接入 Director/QA 报告输出。
2. 将 scenario steps 映射到 Bevy ECS 或 scripted input runner。
3. 将真实截图和真实 SceneIndex 查询写入 bundle。
4. 在 UI 中让用户看到“对话生成游戏”的 playtest 状态。
