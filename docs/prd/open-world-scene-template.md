# OpenWorld Scene Template

> 日期：2026-06-07  
> 状态：OWS-4 初版已落地到 `crates/agent-core/src/open_world_template.rs`，并已推进到 gameplay object manifest 写入 SceneBridge / SceneIndex  
> 关联 PRD：`docs/prd/open-world-vertical-slice-prd.md`

## 用途

`OpenWorldSceneTemplate` 把 `OpenWorldPlan` 中的世界布局转成可写入场景的模板实体。它是从“对话生成可玩计划”走向“对话生成真实场景对象”的第一层落地。

当前模板仍保持 Bevy-free：core 层输出 `SceneBridge` component patches；`bevy-adapter` 测试负责证明这些模板实体能进入 `SceneIndex` 的名称和组件索引。

## Rust API

公开模块：

```rust
agent_core::open_world_template
```

主要类型：

- `OpenWorldSceneTemplate`
- `OpenWorldTemplateEntity`
- `OpenWorldTemplateApplyReport`
- `OpenWorldTemplateCreatedEntity`

从计划生成小岛模板：

```rust
let plan = OpenWorldPlan::open_world_slice01_fixture();
let template = OpenWorldSceneTemplate::small_island_from_plan(&plan);
```

写入 SceneBridge：

```rust
let report = template.apply_to_bridge(&mut bridge)?;
```

回滚模板创建的实体：

```rust
report.revert_from_bridge(&mut bridge)?;
```

## 当前模板实体

小岛模板包含基础布局对象：

- `island_ground`
- `spawn_zone`
- `puzzle_zone`
- `camp_zone`
- `reward_zone`
- `boundary_zone`
- `enemy_camp_marker`
- `puzzle_anchor`

并会从 `OpenWorldPlan.object_manifest` 生成或补齐 gameplay 对象：

- `player`
- `follow_camera`
- `island_boundary`
- `puzzle_switch`
- `reward_chest`
- `camp_enemy_01`
- `main_quest`
- `reward_item`

必查区域：

```text
spawn_zone
puzzle_zone
camp_zone
reward_zone
```

每个区域对象都有 `ZoneMarker` 组件标记，供 SceneIndex 和后续 playtest 引用。每个 manifest 对象都有 `OpenWorldObject` 组件，并带有对应的 primitive marker，例如 `PlayerController`、`PuzzleSwitch`、`LootContainer`、`EnemyBrain`。

## 当前测试

core 窄测试：

```bash
cargo test -p agent-core open_world_template
```

已覆盖：

- small island template 包含 required zones。
- small island template 包含 required gameplay objects。
- 模板能应用到 `MockSceneBridge`。
- `MockSceneBridge` 可查询 `player`、`reward_chest`、`camp_enemy_01` 等 gameplay 对象及其 primitive marker。
- apply report 能反向删除模板创建的实体。
- 模板可 JSON round-trip。

bevy-adapter 窄测试：

```bash
cargo test -p bevy-adapter test_open_world_small_island_template_indexes_required_zones
```

已覆盖：

- 模板实体可转换成 `SceneIndex` entry。
- `spawn_zone`、`puzzle_zone`、`camp_zone`、`reward_zone` 可按名称查询。
- `ZoneMarker` 可按组件查询。
- `player`、`puzzle_switch`、`reward_chest`、`camp_enemy_01` 可按名称查询，并带有对应 gameplay 组件 marker。

## 与用户对话生成游戏的关系

当前链路推进为：

```text
User prompt
  -> OpenWorldPlan
  -> GameplayPrimitive validation
  -> PlayableScenario
  -> OpenWorldRuntimeState
  -> OpenWorldSceneTemplate
  -> SceneBridge / SceneIndex
```

这意味着 Agent 不只是能说“这个小岛应该有出生点和敌人营地”，而是已经能产出一组带稳定 ID 和语义组件的模板实体。`bevy-adapter` 已能把这些 component patch 翻译成真实 Bevy ECS marker components，并由 `SceneIndex` 读回。

## 后续接线

OWS-5 已把 `Quest` / `QuestObjective` / `PuzzleSwitch` / `LootContainer` / `Inventory` 推进到最小 runtime 状态。下一步应继续做：

1. 让 SceneIndex 反映任务、谜题、宝箱和奖励的运行时状态。
2. 为这些 marker component 接入真实 Bevy gameplay 行为系统。
3. 将真实窗口截图和输入模拟接入 verification bundle。
