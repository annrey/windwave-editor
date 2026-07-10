# Quest / Interaction / Loot Runtime

> 日期：2026-06-07  
> 状态：OWS-5 初版已落地到 `crates/agent-core/src/open_world_runtime.rs`；2026-07-05 已接入结构化 `WorldTimestamp` runtime events  
> 关联 PRD：`docs/prd/open-world-vertical-slice-prd.md`

## 用途

`OpenWorldRuntimeState` 是开放世界垂直切片在 core 层的玩法状态真相源。它不模拟 Bevy 物理、输入或渲染，而是验证一份 `OpenWorldPlan` 是否能驱动最小任务链：

```text
进入机关区
  -> 与 puzzle_switch 交互
  -> reward_chest 解锁
  -> 打开宝箱
  -> reward_item 进入 player inventory
  -> main_quest 完成
```

2026-07-05 后，runtime state 持有 frozen `WorldClock`，每条 `OpenWorldRuntimeEvent` 都保存事件发生时的 `WorldTimestamp`。在 `PlayableScenario` 中，scenario step 推进 clock 后会同步给 runtime，因此同一 step 内触发的 runtime events 可以用同一个 tick 回放和定位。

## Rust API

公开模块：

```rust
agent_core::open_world_runtime
```

主要类型：

- `OpenWorldRuntimeState`
- `OpenWorldRuntimeObject`
- `QuestRuntimeState`
- `QuestObjectiveRuntime`
- `QuestObjectiveRuntimeState`
- `PuzzleRuntimeState`
- `LootRuntimeState`
- `EnemyRuntimeState`
- `OpenWorldRuntimeEvent`
- `OpenWorldRuntimeError`
- `WorldClock` / `WorldTimestamp`（通过 runtime state 和 event timestamp 暴露）

主路径调用：

```rust
let plan = OpenWorldPlan::open_world_slice01_fixture();
let mut runtime = OpenWorldRuntimeState::from_plan(&plan);

runtime.enter_zone("player", "puzzle_zone")?;
runtime.interact("player", "puzzle_switch")?;
runtime.open_loot("player", "reward_chest")?;
runtime.collect_reward("player", "reward_chest", "reward_item")?;
```

## 状态规则

### 任务

- `from_plan()` 会从 `QuestFlowSpec` 建立 `main_quest` 和 objective runtime。
- `enter_zone()` 完成 `ReachZone` objective。
- `interact()` 完成 `SolvePuzzle` objective。
- `defeat_enemy()` 完成 `DefeatEnemy` objective。
- `collect_reward()` 完成 `CollectReward` objective。
- 所有 objective 完成后，主任务进入 `Completed`。

### 机关

- 初始状态：`Locked`
- 玩家进入机关所在区域后：`AwaitingInteraction`
- 玩家与 `Interactable + InteractionZone + PuzzleSwitch` 对象交互后：`Solved`

### 宝箱和奖励

- 初始状态：`Locked`
- 机关解开后：`Unlocked`
- `open_loot()` 后：`Opened`
- `collect_reward()` 后：`LootClaimed`
- `Locked` 状态不能打开。
- `LootClaimed` 后不能重复领取同一奖励。
- 领取奖励要求 actor 拥有 `Inventory` primitive。

## PlayableScenario 接入

`PlayableScenarioState` 现在持有 `OpenWorldRuntimeState`，脚本 step 只负责描述动作序列：

```text
MoveActorToZone -> runtime.enter_zone
Interact -> runtime.interact
EngageEnemy -> runtime.engage_enemy
AttackUntilDefeated -> runtime.attack_until_defeated
OpenLootContainer -> runtime.open_loot
CollectReward -> runtime.collect_reward
```

这样后续 Director、QA 报告、SceneIndex 或 UI 任务面板接入时，可以复用同一套 core 状态机，而不是在不同测试里复制玩法规则。

## 当前测试

窄测试：

```bash
cargo test -p agent-core open_world_runtime
```

已覆盖：

- 解谜后宝箱解锁、打开、奖励进入 inventory，任务完成。
- `Locked` 宝箱不能打开，并给出修复建议。
- 奖励不能重复领取。
- 玩家缺少 `Inventory` 时，奖励领取失败并指出缺失 primitive。
- `OpenWorldRuntimeEvent` 携带结构化 `WorldTimestamp`。

回归测试：

```bash
cargo test -p agent-core playable_scenario
```

已确认：

- `OpenWorldSlice01` 主线 playtest 仍然通过。
- 缺失 `Interactable` / `Combatant` 的失败报告格式保持可用。
- runtime event tick 与 `PlayableScenario` step clock 对齐。

## 后续接线

1. 将 runtime event 映射到 SceneIndex 可查询状态。
2. 将 quest state 接入真实任务 UI。✅ 已完成最小 `OpenWorldQuestPanel`，可从 core runtime 和 World Timeline replay cursor 同步 quest/objective 状态
3. 将 `open_loot` / `collect_reward` 映射到 Bevy ECS 组件和交互输入。✅ 已完成 `bevy-adapter` loot interaction bridge，以及 `agent-ui` `InteractionCompleteEvent` -> open-world command queue/executor
4. 将 timestamped runtime events 展示到 World Timeline / Replay panel。✅ 已完成最小 World Timeline 展示、每 tick `world_state` 快照、Replay tick 控制、`OpenWorldQuestPanel` 对齐、`bevy-adapter` replay state 组件应用、`Visibility`/交互可用性驱动、loot/combat interaction bridge、真实交互事件命令队列和 SceneIndex 暴露
5. 在 `VerificationBundle` 中继续合并截图、视觉检查和性能 evidence。
