# PRD: OpenWorldSlice01 开放世界垂直切片

> 日期：2026-06-07  
> 状态：v0.1 草案，作为后续代码 issue 的边界文件  
> 上游计划：`docs/windwave-ai-open-world-vertical-slice-plan.md`  
> 目标版本：WindWave v0.3 起步，先完成可玩垂直切片，不追求完整开放世界规模

## 1. 背景

WindWave 当前已经能通过 `DirectorRuntime -> SceneBridge -> EngineCommand -> Bevy World -> SceneIndex -> undo` 证明 AI 可以改动场景，并已有“创建红色敌人”的自动闭环测试。下一步不能直接扩张到大世界或多工具生产，而要先证明 AI 能生成一个可玩、可验证、可回滚的小型开放世界动作冒险切片。

本 PRD 将 `OpenWorldSlice01` 写成工程可执行规格，后续 `GameplayPrimitive catalog`、`OpenWorldPlan schema`、`PlayableScenario`、场景模板和任务链路都以此为第一目标。

## 2. 一句话目标

用户输入一句高层需求后，WindWave 能让 AI 生成一个 3D 小岛场景：玩家从出生点出发，完成机关谜题，打开宝箱，击败敌人营地，获得奖励，并在任务面板显示完成；全过程可被自动 playtest 和 SceneIndex/截图验证。

## 3. 非目标

- 不做完整商业级开放世界。
- 不复刻任何现有游戏的角色、美术、地图、剧情或 UI。
- 不要求高质量模型、动画、音乐或 VFX。
- 不一次性接入 Blender/Godot/Unity/Figma/VS Code/MCP。
- 不实现完整物理、完整 NavMesh、完整开放世界流式加载。
- 不让 LLM 自由散写 gameplay 代码作为主要实现方式；优先组合稳定 primitives。
- 不把“文档生成完成”视为功能完成；最终必须能跑测试和真实窗口 QA。

## 4. 用户输入

第一版固定验收 prompt：

```text
创建一个开放世界小岛：玩家可以第三人称移动，右侧有敌人营地，中央有一个机关谜题，完成谜题后打开宝箱，击败敌人并拿到奖励，最后在任务面板显示完成。
```

## 5. 用户价值

- 证明 WindWave 的 AI 编辑能力从“创建单个对象”升级为“生成可玩任务链”。
- 为后续 AI 生成关卡、任务、敌人、资产和测试建立稳定模板。
- 给每个后续功能一个硬验收：能否让 `OpenWorldSlice01` 更完整、更可靠、更可验证。

## 6. 成功定义

`OpenWorldSlice01` 完成时必须满足：

- 用户能在真实窗口中玩通一次。
- 自动 playtest 能在无人工输入下跑通主线。
- `SceneIndex` 能查询到关键对象和任务状态。
- 截图或视觉验证能确认关键对象在预期区域。
- 关键 AI 编辑步骤可 undo/redo，或明确声明不可逆。
- `cargo check`、`cargo test --workspace`、`cargo clippy --workspace -- -D warnings` 全部通过。

## 7. 场景概念

### 7.1 场景名称

`OpenWorldSlice01`

### 7.2 地图布局

小岛按 5 个逻辑区域组织，第一版不要求真实 world streaming。

| 区域 ID | 名称 | 位置 | 作用 |
|---|---|---|---|
| `spawn_zone` | 出生点 | 左侧或中心偏左 | 玩家开始位置、相机初始化 |
| `puzzle_zone` | 机关区 | 中央 | 触发机关，解锁宝箱或路径 |
| `camp_zone` | 敌人营地 | 右侧 | 放置敌人和战斗目标 |
| `reward_zone` | 宝箱区 | 机关区附近或营地后方 | 领取奖励 |
| `boundary_zone` | 小岛边界 | 外圈 | 防止玩家离开测试区域 |

### 7.3 美术要求

第一版允许使用 primitive/占位物：

- 地面：平面、网格或简单高度变化。
- 玩家：胶囊体或简单模型。
- 敌人：红色或深色占位体。
- 机关：带明显颜色的开关、石柱或按钮。
- 宝箱：方块组合或 prefab 占位。
- 边界：低墙、隐形 collider 或明显边界线。

## 8. 核心玩法流程

玩家主线流程：

1. 玩家出生在 `spawn_zone`。
2. 任务面板显示 `探索小岛` 或 `完成营地试炼`。
3. 玩家移动到 `puzzle_zone`。
4. 玩家与机关交互。
5. 机关状态从 `Locked` 变为 `Solved`。
6. 宝箱从 `Locked` 变为 `Unlocked`。
7. 玩家移动到 `camp_zone`。
8. 敌人进入战斗状态。
9. 玩家击败敌人。
10. 玩家打开宝箱或拾取奖励。
11. 任务状态变为 `Completed`。
12. 任务面板显示完成。

允许的第一版简化：

- 谜题可以是单按钮交互，不要求复杂逻辑。
- 战斗可以是单一攻击键或测试脚本触发伤害。
- 敌人 AI 可以是简单巡逻/追击/受击状态，不要求高级行为树。
- 保存/读取可以先保存关键状态，不要求完整存档系统。

## 9. 对象清单

### 9.1 必须对象

| 对象 ID | 类型 | 必须组件/标签 | 验收 |
|---|---|---|---|
| `player` | 玩家 | `PlayerController`, `Combatant`, `Attack`, `Inventory` | 能移动，能触发交互，能造成伤害 |
| `follow_camera` | 相机 | `FollowCamera` | 跟随玩家，不丢目标 |
| `island_ground` | 地形 | `WorldSurface`, collider 或等价组件 | 玩家可站立，测试代理可导航 |
| `island_boundary` | 边界 | `BoundaryZone` | 玩家不能无限离开小岛 |
| `puzzle_switch` | 机关 | `Interactable`, `PuzzleSwitch` | 交互后改变谜题状态 |
| `reward_chest` | 宝箱 | `LootContainer`, `QuestTarget` | 解锁后可领取奖励 |
| `enemy_camp_marker` | 营地标记 | `ZoneMarker` | SceneIndex 可查询 |
| `camp_enemy_01` | 敌人 | `EnemyBrain`, `Combatant`, `QuestTarget` | 可被击败 |
| `main_quest` | 任务 | `Quest`, `QuestObjective` | 状态从 NotStarted 到 Completed |
| `reward_item` | 奖励 | `Reward`, `InventoryItem` | 进入 inventory 或资源状态 |

### 9.2 可选对象

| 对象 ID | 类型 | 作用 |
|---|---|---|
| `camp_fire` | 场景装饰 | 增强营地可读性 |
| `path_marker_*` | 路径点 | 自动 playtest 或敌人巡逻 |
| `gate_01` | 门/阻挡 | 谜题完成后打开 |
| `quest_beacon` | UI/场景提示 | 指引玩家目标 |

## 10. Gameplay primitives

后续代码应优先实现并登记这些 primitives。

| Primitive | 最低能力 | 第一版说明 |
|---|---|---|
| `PlayerController` | 移动、朝向、速度限制 | 第三人称或简化跟随相机均可 |
| `FollowCamera` | 跟随目标、偏移、稳定更新 | 不要求高级避障 |
| `Interactable` | 进入范围、触发交互、发事件 | 可先由测试脚本直接调用 |
| `InteractionZone` | 判断玩家是否可交互 | 可用距离或 collider |
| `Quest` | 状态、目标列表、完成判断 | 支持主线任务 |
| `QuestObjective` | 单个目标状态 | `ReachZone`, `SolvePuzzle`, `DefeatEnemy`, `CollectReward` |
| `PuzzleSwitch` | 机关状态机 | `Idle -> Activated -> Solved` |
| `LootContainer` | 锁定、解锁、领取 | 宝箱最小状态 |
| `Inventory` | 物品列表或资源数 | 不做完整 UI 背包 |
| `Combatant` | 生命值、阵营、死亡状态 | 玩家和敌人共用 |
| `Attack` | 伤害、冷却、目标 | 可先用测试脚本触发 |
| `EnemyBrain` | idle/patrol/aggro/dead | 不要求复杂行为树 |
| `WorldSurface` | 可站立区域 | 为导航和测试提供语义 |
| `ZoneMarker` | 逻辑区域标记 | 供 SceneIndex 和计划引用 |

## 11. 状态机

### 11.1 主任务状态

```text
NotStarted
  -> Active
  -> PuzzleSolved
  -> EnemyDefeated
  -> RewardCollected
  -> Completed
```

失败/重置：

```text
Active -> Failed -> Active
```

第一版失败条件：

- 玩家死亡。
- 自动 playtest 超时。
- 关键对象缺失。
- 任务目标不可达。

### 11.2 谜题状态

```text
Locked
  -> AwaitingInteraction
  -> Solved
```

触发规则：

- 玩家进入 `puzzle_zone` 后，`puzzle_switch` 可交互。
- 交互成功后，`reward_chest` 解锁，或 `gate_01` 打开。

### 11.3 宝箱状态

```text
Locked
  -> Unlocked
  -> Opened
  -> LootClaimed
```

约束：

- `Locked` 状态不能领取奖励。
- `Unlocked` 后可以打开。
- `LootClaimed` 后不能重复领取同一奖励。

### 11.4 敌人状态

```text
Idle
  -> Patrol
  -> Aggro
  -> Attacking
  -> Dead
```

第一版允许：

- 没有玩家时保持 `Patrol`。
- 自动 playtest 可以直接把玩家移动到敌人附近触发 `Aggro`。

### 11.5 玩家状态

```text
Alive
  -> InCombat
  -> Downed
  -> Respawned
```

第一版可简化为：

```text
Alive -> InCombat -> Alive
```

## 12. AI 计划输入输出

### 12.1 输入

AI 接收：

- 用户 prompt。
- 当前 SceneIndex 摘要。
- 已登记 `GameplayPrimitive` catalog。
- 已存在场景模板。
- 项目风格约束和非目标。
- 最近失败经验。

### 12.2 输出

AI 必须输出结构化计划，而不是只输出自然语言。

计划至少包含：

- `goal`
- `world_spec`
- `object_manifest`
- `gameplay_primitives`
- `quest_flow`
- `task_graph`
- `verification_goals`
- `risk_summary`

### 12.3 计划失败规则

如果 AI 需要未登记 primitive，必须输出缺口而不是假装能做：

```text
MissingPrimitive: NavigationGraph
Reason: Enemy patrol and scripted playtest need reachable path checks.
SuggestedIssue: Implement minimal NavigationGraph.
```

## 13. 自动 Playtest 规格

### 13.1 Playtest 名称

`playtest_open_world_slice01_main_path`

### 13.2 脚本流程

```text
Spawn player at spawn_zone
Assert quest main_quest is Active
Move player to puzzle_zone
Interact with puzzle_switch
Assert puzzle state is Solved
Assert reward_chest is Unlocked
Move player to camp_zone
Engage camp_enemy_01
Apply player attack until enemy Dead
Assert camp_enemy_01 is Dead
Open reward_chest
Collect reward_item
Assert inventory contains reward_item
Assert main_quest is Completed
```

### 13.3 失败输出

失败报告必须包含：

- 失败步骤。
- 缺失对象或组件。
- 当前任务状态。
- 最近 10 条 Director/Engine event。
- SceneIndex 查询结果。
- 建议修正方向。

示例：

```text
Failed at step: Interact with puzzle_switch
Reason: puzzle_switch exists but is missing Interactable
QuestState: Active
Suggested fix: add Interactable + InteractionZone to puzzle_switch
```

## 14. SceneIndex 验收查询

第一版至少支持这些验证目标：

| 验证 | 查询意图 |
|---|---|
| `exists("player")` | 玩家存在 |
| `exists("puzzle_switch")` | 机关存在 |
| `exists("reward_chest")` | 宝箱存在 |
| `exists("camp_enemy_01")` | 敌人存在 |
| `has_component("player", "PlayerController")` | 玩家可控 |
| `has_component("puzzle_switch", "Interactable")` | 机关可交互 |
| `has_component("reward_chest", "LootContainer")` | 宝箱可领取 |
| `has_component("camp_enemy_01", "Combatant")` | 敌人可战斗 |
| `quest_state("main_quest") == "Completed"` | 任务完成 |
| `inventory_contains("player", "reward_item")` | 奖励到账 |

## 15. 视觉验收

第一版视觉验收不要求精准美术判断，只要求防止明显错漏。

截图检查目标：

- 玩家可见。
- 敌人营地在玩家右侧或场景右半区。
- 中央区域存在机关。
- 宝箱可见或在可查询区域。
- 任务 UI 显示当前状态。

视觉失败必须进入 revise，而不是标记完成。

## 16. 保存/读取验收

第一版保存状态只要求覆盖：

- 玩家位置。
- `main_quest` 状态。
- 谜题状态。
- 宝箱状态。
- 敌人存活/死亡状态。
- 玩家 inventory 中是否有 `reward_item`。

保存测试：

```text
Reach after puzzle solved
Save
Reload
Assert puzzle Solved
Assert reward_chest Unlocked
Continue playtest to completion
```

## 17. 性能和规模约束

第一版预算：

| 指标 | 预算 |
|---|---:|
| 逻辑区域 | 5 个以内 |
| 关键实体 | 20 个以内 |
| 总实体 | 200 个以内 |
| 敌人 | 1-3 个 |
| 主线任务 | 1 条 |
| 支线任务 | 0 |
| 谜题 | 1 个 |
| 自动 playtest 时长 | 30 秒以内 |

需要记录但不先设硬门槛：

- SceneIndex rebuild/update 耗时。
- EngineCommand 批处理耗时。
- 平均帧时间。
- 截图保存耗时。

## 18. QA 矩阵

| 类型 | 测试 | 必须 |
|---|---|---|
| Unit | primitive 状态机测试 | 是 |
| Unit | quest 状态流转测试 | 是 |
| Unit | loot 不重复领取测试 | 是 |
| Integration | Scene template 生成后 SceneIndex 查询 | 是 |
| Integration | OpenWorldPlan serde round-trip | 是 |
| Integration | TaskGraph 顺序执行 | 是 |
| E2E | `playtest_open_world_slice01_main_path` | 是 |
| E2E | 失败后 revision report | 是 |
| Manual | 真实窗口玩通一次 | 是，除非明确豁免 |
| Manual | undo/redo 关键编辑步骤 | 是 |

## 19. 文档交付

完成第一版切片后必须产出：

- `docs/qa/open-world-slice01.md`
- `docs/session-logs/<date>-open-world-slice01.md`
- 如有代码变更，更新 `docs/remaining-work.md` 和 `docs/windwave-ai-open-world-vertical-slice-plan.md` 的对应状态。

## 20. 第一批工程 Issue

### Issue OWS-1: 定义 GameplayPrimitive catalog

状态：初版已落地，2026-06-07。已完成 `agent-core` 类型、默认 catalog、校验测试和 catalog 文档；Bevy 组件映射留给 OWS-4/OWS-5/OWS-6。

交付：

- primitive 类型定义。✅
- capability registry。✅
- catalog 文档。✅

验收：

- 可查询所有已登记 primitives。✅
- `OpenWorldPlan` 只能引用 catalog 中已有 primitive，或显式报告缺口。待 OWS-2 接入。

### Issue OWS-2: 定义 OpenWorldPlan schema

状态：初版已落地，2026-06-07。已完成 `agent-core` schema、`OpenWorldSlice01` fixture、JSON round-trip、GameplayPrimitive catalog 校验和 task graph 依赖校验；真实执行器留给后续 OWS-3+。

交付：

- Rust 类型。✅
- serde round-trip。✅
- fixture 计划。✅

验收：

- 固定 prompt 对应的 fixture plan 可加载。✅
- plan 中对象、任务和验证目标完整。✅
- plan 引用的 primitives 必须通过 catalog 校验。✅

### Issue OWS-3: 建立 PlayableScenario harness

状态：初版已落地，2026-06-07。已完成纯 `agent-core` scripted playtest runner、`OpenWorldSlice01` 主线路径、成功报告、缺失 `Interactable` / `Combatant` 失败报告和 JSON round-trip；真实 Bevy ECS / 输入模拟留给后续接线。

交付：

- scripted playtest runner。✅
- `playtest_open_world_slice01_main_path` fixture。✅

验收：

- 成功路径通过。✅
- 缺少 `Interactable` 等常见失败能给出具体失败报告。✅

### Issue OWS-4: Scene template: small island

状态：初版已落地并继续推进，2026-06-07。已完成 `OpenWorldSceneTemplate::small_island_from_plan()`、SceneBridge apply/revert、required zone 查询；模板现在会从 `OpenWorldPlan.object_manifest` 写入 `player`、`puzzle_switch`、`reward_chest`、`camp_enemy_01`、`main_quest`、`reward_item` 等 gameplay 对象。`bevy-adapter` 已验证这些对象能进入 SceneIndex 名称和组件索引，并已把 template component patch 翻译成真实 Bevy ECS marker components。真实 Bevy gameplay 行为系统和可视对象生成留给后续 adapter 接线。

交付：

- 小岛布局模板。✅
- 关键区域对象。✅
- 关键 gameplay 对象。✅

验收：

- SceneIndex 可查询 `spawn_zone`、`puzzle_zone`、`camp_zone`、`reward_zone`。✅
- SceneIndex 可查询 `player`、`puzzle_switch`、`reward_chest`、`camp_enemy_01` 等 gameplay 对象。✅
- template component patch 可写入真实 Bevy World 并由 SceneIndex 读回 marker components。✅
- undo 后模板对象清理完整。✅ core `SceneBridge` revert 已覆盖

### Issue OWS-5: Quest/Interaction/Loot 最小链路

状态：初版已落地，2026-06-07。已完成 `agent-core` 的 `open_world_runtime` 模块，覆盖 `enter_zone -> interact -> open_loot -> collect_reward` 主链路；`PlayableScenarioState` 已改为复用该 runtime。真实 Bevy ECS 交互、任务 UI 和 SceneIndex 状态映射留给后续接线。

交付：

- `Quest` / `QuestObjective` runtime。✅
- `Interactable` / `PuzzleSwitch` runtime。✅
- `LootContainer` / `Inventory` runtime。✅
- OWS-5 文档：`docs/prd/quest-interaction-loot-runtime.md`。✅

验收：

- 自动测试能从解谜到奖励领取并完成任务。✅ `cargo test -p agent-core open_world_runtime`
- 旧 `PlayableScenario` 主线仍通过。✅ `cargo test -p agent-core playable_scenario`

### Issue OWS-6: Combat 最小链路

状态：初版已落地，2026-06-07。已完成 `agent-core` 的 combat runtime：`CombatantRuntime`、`CombatFaction`、`CombatHitReport`、`engage_enemy()`、`attack_enemy()`、`attack_until_defeated()`；`PlayableScenario` 的战斗步骤已通过 runtime 多次攻击完成敌人击败。真实 Bevy ECS 攻击输入、动画、玩家受击和失败/重置规则留给后续接线。

交付：

- `Combatant` runtime。✅
- `Attack` runtime。✅
- `EnemyBrain` 最小状态。✅
- OWS-6 文档：`docs/prd/combat-runtime.md`。✅

验收：

- 玩家或 playtest runner 能击败 `camp_enemy_01`。✅ `cargo test -p agent-core open_world_runtime`
- 敌人死亡后任务目标更新。✅ `defeat_camp_enemy -> Completed`

### Issue OWS-7: VerificationBundle 和 QA 报告

状态：初版已落地，2026-06-07；Director/SceneBridge 接入已推进。已完成 `agent-core` 的 `open_world_verification` 模块，可从 `OpenWorldPlan`、`PlayableScenarioReport`、runtime events 和 SceneBridge/SceneIndex 查询 evidence 生成 `OpenWorldVerificationBundle`，支持成功/失败 fixture、JSON round-trip 和 Markdown QA report。`DirectorRuntime::verify_open_world_slice01()` 已可生成 bundle 并写入 Director event/trace evidence；应用 small-island template 后，SceneBridge 查询路径可让 bundle 通过，Bevy World marker components 也可进入 SceneIndex。2026-07-05 已新增 `write_open_world_slice01_qa` example，可自动落盘 `docs/qa/open-world-slice01.md`、World Timeline JSON 和 SceneIndex proxy PNG，并把 PNG 路径写入 `screenshot_paths`。同日已新增 `DirectorRuntime::write_open_world_slice01_qa_artifacts_with_engine_screenshot(...)` 与 UI QA 请求队列接线，可等待并消费 `bevy-adapter::ScreenshotQueue` 的 Bevy framebuffer screenshot result，写入 `screenshot_capture=bevy_framebuffer`、路径和尺寸；完整 app 主窗口 readback 仍待运行时验收。

交付：

- SceneIndex 查询结果。✅ 通过 `SceneBridge::query_entities` 接入；应用 template 后关键 gameplay 对象可查询
- 截图路径。✅ 已完成 SceneIndex proxy PNG 路径写入，以及 Bevy framebuffer screenshot result 到 bundle / timeline 的路径写入；完整 app 主窗口 readback 待验收
- Director/Engine event 摘要。✅ Director 入口已填充；真实 Engine event 继续扩展
- Playtest 结果。✅
- Markdown QA 报告。✅ `OpenWorldVerificationBundle::to_markdown()`
- Markdown QA 落盘。✅ `cargo run -p agent-core --example write_open_world_slice01_qa -- docs/qa/open-world-slice01.md`
- OWS-7 文档：`docs/prd/verification-bundle.md`。✅

验收：

- 成功和失败 fixture 都能输出报告。✅ `cargo test -p agent-core open_world_verification`
- Director 入口能输出 bundle/Markdown，并在连接 SceneBridge 时使用 bridge 查询。✅ `cargo test -p agent-core open_world_ops`
- 应用 small-island template 后，bridge-aware bundle 通过。✅ `cargo test -p agent-core open_world_verification`
- Bevy World marker components 可被 SceneIndex 读回。✅ `cargo test -p bevy-adapter test_open_world_template_components_apply_to_bevy_world_and_scene_index`

## 21. 打开问题

这些问题不阻塞第一版 PRD，但进入代码前要逐步回答：

1. `GameplayPrimitive` 放在 `agent-core` 还是 `bevy-adapter`？建议：能力定义在 `agent-core`，Bevy 组件映射在 `bevy-adapter`。
2. `Quest` 属于编辑器运行时还是游戏模拟层？建议：先作为可测试 runtime primitive，不直接承诺最终游戏架构。
3. 自动 playtest 用 Bevy ECS 直接推进，还是模拟输入？建议：第一版先 ECS 直接推进，第二版补输入模拟。
4. 视觉验证用本地截图规则还是外部视觉模型？建议：第一版先 SceneIndex + 截图存在性，外部视觉模型作为 Phase 4。
5. 保存系统是否写磁盘？建议：第一版可用内存/临时文件 fixture，后续接真实 save slot。

## 22. 下一步

本 PRD 完成后，下一步应进入代码落地：

1. 新增 `GameplayPrimitive` catalog 的类型和测试。
2. 新增 `OpenWorldPlan` schema 和 fixture。
3. 新增 `PlayableScenario` harness 的最小失败/成功测试。

建议优先顺序：

```text
OWS-1 -> OWS-2 -> OWS-3 -> OWS-4 -> OWS-5 -> OWS-6 -> OWS-7
```
