# OpenWorldPlan Schema

> 日期：2026-06-07  
> 状态：OWS-2 初版已落地到 `crates/agent-core/src/open_world_plan.rs`  
> 关联 PRD：`docs/prd/open-world-vertical-slice-prd.md`

## 用途

`OpenWorldPlan` 是 AI 生成开放世界垂直切片时的结构化计划。它位于自然语言 prompt 和低层 `EngineCommand` / Bevy 组件之间，用于把“创建一个开放世界小岛”拆成可验证的世界、对象、玩法、任务、验证目标和任务依赖图。

它不是执行器。执行器、Bevy 组件映射、playtest runner 会在后续 OWS-3/OWS-4/OWS-5/OWS-6 中接入。

## Rust API

公开模块：

```rust
agent_core::open_world_plan
```

主要类型：

- `OpenWorldPlan`
- `WorldSpec`
- `WorldZoneSpec`
- `OpenWorldObjectSpec`
- `QuestFlowSpec`
- `OpenWorldTaskGraph`
- `OpenWorldTask`
- `OpenWorldVerificationGoal`
- `OpenWorldPlanRisk`
- `OpenWorldPlanValidationError`

Fixture：

```rust
let plan = OpenWorldPlan::open_world_slice01_fixture();
```

校验：

```rust
let catalog = GameplayPrimitiveCatalog::open_world_slice_defaults();
plan.validate(&catalog)?;
```

## Schema 顶层字段

| 字段 | 说明 |
|---|---|
| `id` | 计划稳定 ID |
| `goal` | 计划目标 |
| `source_prompt` | 原始用户 prompt |
| `world` | 世界布局、区域和规模约束 |
| `object_manifest` | 必须创建或绑定的对象清单 |
| `gameplay_primitives` | 计划显式引用的玩法积木 |
| `quest_flow` | 主任务状态和目标 |
| `task_graph` | 可验证任务依赖图 |
| `verification_goals` | SceneIndex / playtest / screenshot / performance 验证目标 |
| `risk` | 风险等级、是否需要人工审批、原因 |

## 第一版验证规则

`OpenWorldPlan::validate()` 当前覆盖：

- 计划引用的所有 `GameplayPrimitiveKind` 必须存在于 catalog。
- 世界区域 ID 不能重复。
- 对象 ID 不能重复。
- 任务 ID 不能重复。
- 验证目标 ID 不能重复。
- 对象引用的 `zone_id` 必须存在。
- 任务依赖必须存在。
- 任务依赖图不能成环。

## Fixture 内容

`OpenWorldPlan::open_world_slice01_fixture()` 对应 PRD 固定 prompt：

```text
创建一个开放世界小岛：玩家可以第三人称移动，右侧有敌人营地，中央有一个机关谜题，完成谜题后打开宝箱，击败敌人并拿到奖励，最后在任务面板显示完成。
```

包含 5 个区域：

- `spawn_zone`
- `puzzle_zone`
- `camp_zone`
- `reward_zone`
- `boundary_zone`

包含关键对象：

- `player`
- `follow_camera`
- `island_ground`
- `island_boundary`
- `puzzle_switch`
- `reward_chest`
- `enemy_camp_marker`
- `camp_enemy_01`
- `main_quest`
- `reward_item`

任务依赖顺序：

```text
build_island_layout
spawn_player_and_camera
add_puzzle_and_chest
add_enemy_camp
wire_main_quest
define_verification_goals
run_main_path_playtest
```

## 当前测试

窄测试：

```bash
cargo test -p agent-core open_world_plan
```

已覆盖：

- fixture 可通过默认 GameplayPrimitive catalog 校验。
- fixture 可 JSON round-trip。
- task graph 依赖顺序稳定。
- 缺失 primitive 会被拒绝。
- 对象引用缺失 zone 会被拒绝。
- 任务引用缺失 dependency 会被拒绝。

## 后续接线

下一步 OWS-3 应做 `PlayableScenario` harness：

1. 使用 `OpenWorldPlan::open_world_slice01_fixture()` 作为输入 fixture。
2. 将 `task_graph` 转为 scripted playtest 准备步骤。
3. 成功路径输出 playtest report。
4. 常见失败路径必须指出缺失对象、组件或 primitive。
