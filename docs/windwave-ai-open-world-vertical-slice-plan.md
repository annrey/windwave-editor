# WindWave AI 开放世界垂直切片执行计划

> 日期：2026-06-07  
> 状态：执行规划  
> 目标：让 WindWave 从“AI 可改动一个 Bevy 场景”推进到“AI 可持续生成、验证、修正一个开放世界动作冒险垂直切片”。  
> 边界：本计划不复刻任何现有 IP、角色、美术或关卡；“原神/塞尔达那样”只作为复杂度和品类参照。
> 上层路线：`docs/windwave-ai-playable-world-editor-roadmap.md` 将本切片纳入“AI 可玩世界编辑器”的长期方向，新增 3D 世界、现实时间、代理治理和回放要求。

## 一句话目标

用户给出一句高层需求后，WindWave 能让 AI 规划、创建、运行、验证并迭代一个可玩的 3D 开放世界小区域，包含玩家、相机、探索、战斗、任务、交互、奖励、保存、视觉校验、性能门禁和可回滚编辑历史。

## 当前基线

当前 WindWave 已经具备以下基础：

- Rust + Bevy 0.17 workspace 是真实主项目入口。
- `DirectorRuntime -> SceneBridge -> EngineCommand -> Bevy World -> SceneIndex -> undo` 自动测试已覆盖“创建红色敌人”闭环。
- 失败后修正闭环已有测试，失败不会静默结束。
- `cargo check`、`cargo test --workspace`、`cargo clippy --workspace -- -D warnings` 已能作为质量门禁。
- HR approval、undo reverse contract、SceneIndex 删除残留、关键词路由等 P0/P1 正确性问题已基本收敛到代码侧通过。
- 仍缺真实窗口手动验收、Visual feedback loop、MemoryInjector 主路径稳定性、Multica 真实环境、关键模块覆盖和性能压测。

这意味着当前不是“从零做游戏引擎”，而是要把已有 Agent 编辑器链路升级成能生产可玩内容的系统。

## 非目标

- 不做完整商业级开放世界游戏。
- 不承诺自动生成高质量美术资产。
- 不一次性接入 Blender、Godot、Unity、Figma、VS Code、MCP 全家桶。
- 不绕过人工确认去执行破坏性文件操作、批量删除或外部发布。
- 不用“能生成代码”代替“能运行、能玩、能验证”。

## 最小垂直切片定义

第一版目标场景命名为 `OpenWorldSlice01`。

### 用户输入

```text
创建一个开放世界小岛：玩家可以第三人称移动，右侧有敌人营地，中央有一个机关谜题，完成谜题后打开宝箱，击败敌人并拿到奖励，最后在任务面板显示完成。
```

### 必须生成的可玩内容

| 模块 | 最低要求 |
|---|---|
| 世界 | 一个可导航 3D 小岛或地块，有边界、地面、障碍、兴趣点 |
| 玩家 | 第三人称移动、相机跟随、跳跃或冲刺之一 |
| 交互 | 可与机关、宝箱、NPC 或标记点交互 |
| 战斗 | 至少一种玩家攻击、一个敌人、敌人血量、失败/死亡或重置规则 |
| 任务 | 任务状态：未开始、进行中、完成 |
| 谜题 | 一个简单状态机：触发机关 -> 打开路径或宝箱 |
| 奖励 | 宝箱或掉落物进入 inventory/resource 状态 |
| 保存 | 可保存并恢复任务/奖励/玩家位置的最小状态 |
| 验证 | 自动 playtest 能证明主线从开始到完成 |
| 回滚 | AI 编辑产生的关键命令可 undo/redo 或有明确不可逆说明 |

### 通过标准

垂直切片完成时，必须满足：

- `cargo check` 通过。
- `cargo test --workspace` 通过。
- `cargo clippy --workspace -- -D warnings` 通过。
- 自动 playtest 能完成主线流程。
- 场景截图或视觉检查能确认关键对象存在：玩家、敌人、机关、宝箱、任务 UI。
- 性能基准有记录：实体数、帧时间、SceneIndex 更新耗时、命令处理耗时。
- 用户可以在真实窗口中玩通一次主线。

## 核心原则

1. **先垂直切片，后开放世界规模化**  
   先证明一个小区域可玩，再做世界分块、LOD、生态和大地图。

2. **所有 AI 操作必须可观察**  
   AI 不能只说“已完成”。必须能在 SceneIndex、UI、截图、测试或日志里看到结果。

3. **所有写入必须可回滚或声明不可逆**  
   新增 `EngineCommand` 前必须定义 reverse contract。

4. **玩法用 primitives 组合，不靠自由散写代码**  
   AI 应优先组合 `PlayerController`、`QuestState`、`InteractionZone`、`Combatant`、`LootContainer` 等稳定模块。

5. **验证优先级高于功能数量**  
   一个可自动玩通的小任务，比十个未验证系统更有价值。

6. **外部工具先通过 ToolAdapter 抽象接入**  
   Blender/Godot/Terminal/Git 不能直接散落在 Director 逻辑里。

## 能力阶梯

| 等级 | 能力 | 当前状态 | 升级目标 |
|---|---|---|---|
| L0 | AI 改单个场景对象 | 基本具备 | 真实窗口验收补齐 |
| L1 | AI 稳定编辑场景 | 部分具备 | Prefab、Hierarchy、selection、transform、undo UI 完整 |
| L2 | AI 组合可玩 gameplay primitives | 缺失 | 建立 gameplay starter kit |
| L3 | AI 生成小型任务链 | 缺失 | Quest/Interaction/Combat/Loot 状态机 |
| L4 | AI 自检和自动 playtest | 初步缺失 | Screenshot + SceneIndex + playtest harness |
| L5 | AI 使用资产生产管线 | 规划中 | Blender/asset/prefab/material adapter |
| L6 | AI 多 Agent 协作生产 | 规划中 | Task graph、Multica、GitHub issue、角色分工 |
| L7 | AI 规模化开放世界生产 | 未开始 | streaming、LOD、world partition、performance budgets |

## 目标架构

```text
User Goal
  -> DirectorRuntime
  -> OpenWorldPlan
  -> TaskGraph
  -> Tool Orchestrator
  -> Bevy Editor / Asset Pipeline / Tests
  -> Observe
  -> Visual + SceneIndex + Playtest Verify
  -> Revise or Accept
  -> Persist / Issue / Report
```

### 新增核心概念

| 概念 | 说明 |
|---|---|
| `OpenWorldPlan` | AI 生成开放世界垂直切片时使用的结构化计划，不是自由文本 |
| `GameplayPrimitive` | 可组合玩法单元，如玩家控制器、敌人、任务、交互、宝箱 |
| `WorldChunk` | 世界分块单元，第一版可以只是逻辑区域，不必立即流式加载 |
| `PlayableScenario` | 可自动 playtest 的任务流程定义 |
| `VerificationBundle` | SceneIndex 查询、截图、日志、playtest、性能指标的合并结果 |
| `ToolAdapter` | 外部工具统一协议，用于 Blender、Terminal、Git、未来 Godot/VS Code |

## 阶段路线图

### Phase 0：当前闭环收尾

目标：确认现有编辑器真的可用，避免在未验收基础上继续扩张。

| ID | 任务 | 交付物 | 验收 |
|---|---|---|---|
| P0.1 | 红色敌人真实窗口验收 | `docs/qa/red-enemy-closed-loop.md` 更新验收记录 | 用户输入后真实场景新增红色敌人，undo 后消失 |
| P0.2 | undo/redo UI 验收 | QA 记录 | history panel、Undo、Redo 行为一致 |
| P0.3 | HR approval UI 验收 | QA 记录 | approve/reject 后 roster 与 Director Desk 一致 |
| P0.4 | GitHub issue 发布 | GitHub issue 链接 | `docs/issues/v0.2.0-closed-loop-execution.md` 发布成功 |

优先级：最高。  
预计工作量：0.5-1 天，主要受真实窗口工具和 `gh auth` 影响。

### Phase 1：编辑器操作底座升级

目标：让 AI 能稳定搭建小场景，而不是只能生成一个实体。

| ID | 任务 | 说明 | 验收 |
|---|---|---|---|
| E1.1 | 扩展 `EngineCommand` 场景编辑能力 | 地面、障碍、区域标记、触发区、光照、相机锚点 | 每个写入型命令有 reverse contract |
| E1.2 | Prefab / Hierarchy / Transform 操作闭环 | 创建、复制、组合、重命名、父子关系 | UI 和 SceneIndex 状态一致 |
| E1.3 | Selection 与多选工具接入 UI | 支持批量移动、对齐、分组 | 多选操作可 undo/redo |
| E1.4 | Scene template 机制 | 从模板创建小岛、营地、谜题房间 | AI 可选择模板并修改 |
| E1.5 | Command diff preview | 高风险操作先展示 diff | 用户批准后才执行 |

阶段完成定义：

- AI 能从空场景生成一个可浏览的小岛布局。
- 关键对象有稳定 ID、名称、标签和 SceneIndex 查询结果。
- 任何失败命令都会生成可解释错误和修正计划。

### Phase 2：Gameplay Starter Kit

目标：建立 AI 可组合的最小玩法库。

| ID | 任务 | Gameplay primitive | 验收 |
|---|---|---|---|
| G2.1 | 第三人称玩家控制器 | `PlayerController`, `FollowCamera` | 玩家能移动，相机稳定跟随 |
| G2.2 | 交互系统 | `Interactable`, `InteractionZone`, `InteractionPrompt` | 玩家靠近并触发交互 |
| G2.3 | 任务系统 | `Quest`, `QuestObjective`, `QuestProgress` | UI 显示任务状态变化 |
| G2.4 | 战斗系统 | `Combatant`, `Health`, `Damage`, `Attack` | 玩家能击败敌人 |
| G2.5 | 敌人基础 AI | `EnemyBrain`, `Patrol`, `Aggro` | 敌人巡逻、发现玩家、攻击 |
| G2.6 | 奖励和背包 | `LootContainer`, `Inventory`, `Reward` | 宝箱/掉落可进入状态 |
| G2.7 | 谜题状态机 | `PuzzleState`, `Switch`, `Gate` | 机关能改变世界状态 |
| G2.8 | 保存/读取 | `SaveGame`, `SaveSlot` | 任务、奖励、位置可恢复 |

阶段完成定义：

- 不依赖 LLM 的手写测试能跑通一条任务链。
- AI 可以通过结构化计划组合这些 primitive。
- 主线任务失败时能定位失败目标，例如“敌人不存在”“宝箱未打开”“任务状态未完成”。

### Phase 3：AI 任务规划与生产协议

目标：让 AI 从自然语言生成可执行 `OpenWorldPlan`，并把它拆成可验证 TaskGraph。

| ID | 任务 | 交付物 | 验收 |
|---|---|---|---|
| A3.1 | 定义 `OpenWorldPlan` schema | Rust 类型 + serde 测试 | 计划能 round-trip 序列化 |
| A3.2 | 定义 `GameplayPrimitive` catalog | capability registry | AI 只能引用已登记能力或提出缺口 |
| A3.3 | TaskGraph 执行器 | ordered + dependency-aware steps | 失败时停止或走 revise |
| A3.4 | Plan diff 和人工审批 | Director Desk plan preview | 高风险步骤必须等待 approve |
| A3.5 | MemoryInjector 接主路径 | project profile、style rules、failure patterns | prompt 注入在预算内，能召回类似失败 |
| A3.6 | Agent 角色规范 | Director、World Builder、Gameplay Engineer、QA、Asset Artist | 每个角色有输入/输出边界 |

`OpenWorldPlan` 最小字段：

```rust
pub struct OpenWorldPlan {
    pub goal: String,
    pub world: WorldSpec,
    pub player: PlayerSpec,
    pub quests: Vec<QuestSpec>,
    pub encounters: Vec<EncounterSpec>,
    pub puzzles: Vec<PuzzleSpec>,
    pub rewards: Vec<RewardSpec>,
    pub verification: Vec<VerificationGoal>,
    pub risk: PlanRisk,
}
```

阶段完成定义：

- 给定垂直切片 prompt，AI 能产出结构化计划。
- 计划能被拆成任务并逐步应用。
- 每步都有 observe 和 verify。

### Phase 4：视觉反馈与自动 Playtest

目标：AI 不只相信命令返回，而是通过视觉、索引和玩法测试确认结果。

| ID | 任务 | 说明 | 验收 |
|---|---|---|---|
| V4.1 | Screenshot capture 稳定化 | 运行态截图、文件路径、metadata | 测试能读到最新截图 |
| V4.2 | Visual verification adapter | 用视觉模型判断对象/布局是否符合目标 | 失败产生修正建议 |
| V4.3 | SceneIndex verification DSL | `exists(entity)`, `has_component`, `near`, `quest_done` | 验证失败有具体原因 |
| V4.4 | Playtest harness | scripted input + deterministic scenario | 自动跑通任务链 |
| V4.5 | Golden snapshot | 截图/SceneIndex/事件日志合并 | 回归时能比较变化 |
| V4.6 | QA report generator | 自动生成 Markdown QA 报告 | 每次切片生成都有报告 |

阶段完成定义：

- 自动 playtest 可以从出生点完成任务。
- 截图检查能发现明显对象缺失或布局错误。
- AI 修正循环最多 N 次，超过后给出明确失败报告。

### Phase 5：资产生产管线

目标：AI 能创建和导入基础资产，不再只用 primitive 形状。

| ID | 任务 | 工具 | 验收 |
|---|---|---|---|
| AS5.1 | Asset manifest | 内置 | 所有资源有 ID、路径、类型、预算 |
| AS5.2 | Prefab authoring pipeline | Bevy | Prefab 可创建、实例化、回滚 |
| AS5.3 | Blender ToolAdapter POC | Blender Python/MCP | 生成简单模型并导入 |
| AS5.4 | Material/texture pipeline | Bevy asset | 材质可替换，有预算检查 |
| AS5.5 | Animation placeholder pipeline | Blender/Bevy | 简单 idle/attack 动画能挂载 |
| AS5.6 | Asset validation | 内置 QA | 面数、贴图大小、缺失依赖能报错 |

阶段完成定义：

- AI 能为敌人营地生成至少 3 类资产：地形装饰、敌人占位模型、宝箱/机关。
- 导入资产会进入 manifest，并能被 SceneIndex 与 prefab 系统引用。
- 资源超预算时阻止或要求人工确认。

### Phase 6：开放世界系统最小版

目标：从“一个关卡”升级为“可扩展的小开放区域”。

| ID | 任务 | 说明 | 验收 |
|---|---|---|---|
| W6.1 | WorldChunk 数据结构 | 区域、边界、入口、兴趣点 | AI 能创建多个 chunk |
| W6.2 | Terrain primitive | 高度场或网格地面 | 地形可保存和查询 |
| W6.3 | Navigation graph | 简化寻路，不必一开始做完整 NavMesh | 敌人/测试代理能到达目标 |
| W6.4 | Streaming stub | 先做启用/禁用 chunk，不做复杂异步加载 | 远处 chunk 可卸载/加载 |
| W6.5 | Day-night/weather stub | 状态变化影响光照或 UI | 时间推进可见 |
| W6.6 | Performance budgets | entity count、draw proxy、SceneIndex cost | 超预算有报告 |

阶段完成定义：

- 垂直切片区域可以拆成 3-5 个逻辑 chunk。
- 自动 playtest 能跨 chunk 完成任务。
- 大场景压测有稳定命令和报告。

### Phase 7：多 Agent 生产流

目标：让 AI 像小团队一样生产，而不是单 Agent 一口气乱改。

| 角色 | 责任 | 产物 |
|---|---|---|
| Director | 拆目标、定优先级、审批风险 | `OpenWorldPlan`, TaskGraph |
| World Builder | 地形、布局、兴趣点 | Scene edit commands |
| Gameplay Engineer | 玩家、战斗、任务、交互 | Gameplay primitives |
| Asset Artist | Prefab、材质、占位模型 | Asset manifest entries |
| QA | playtest、截图、性能、回归 | QA report |
| Producer | issue、里程碑、状态同步 | GitHub issues / docs |

任务：

| ID | 任务 | 验收 |
|---|---|---|
| M7.1 | 真实 Multica server smoke | 任务分发、进度回传、技能执行、场景状态同步 |
| M7.2 | GitHub issue workflow | 计划自动拆 issue，状态能回写 |
| M7.3 | Conflict arbitration | 多 Agent 修改同一资源时检测冲突 |
| M7.4 | Session handoff | 长任务可中断后恢复 |
| M7.5 | Release report | 自动生成版本验收报告 |

阶段完成定义：

- 一个垂直切片任务可拆给多个 Agent 顺序/并行执行。
- 失败任务可回滚、重试或交给人工确认。
- 文档、issue、QA 报告同步。

### Phase 8：从垂直切片到小型游戏

目标：在垂直切片稳定后扩大内容量。

| ID | 内容 | 标准 |
|---|---|---|
| S8.1 | 3 个区域 | 每个区域有独立任务和兴趣点 |
| S8.2 | 3 类敌人 | 行为不同，不只是换颜色 |
| S8.3 | 3 个谜题 | 状态机不同 |
| S8.4 | 1 条主线 + 2 条支线 | 任务可保存恢复 |
| S8.5 | 简单经济/奖励 | 奖励影响玩家能力或进度 |
| S8.6 | 每日构建报告 | build/test/playtest/perf 全记录 |

阶段完成定义：

- 用户可以玩 15-30 分钟。
- AI 可以继续扩展新区域，而不是每次破坏旧内容。
- 性能和测试趋势可追踪。

## 第一批可执行 Issue 拆分

### Issue 1：开放世界垂直切片 PRD

目标：把本计划收敛成工程 PRD。

交付：

- `docs/prd/open-world-vertical-slice-prd.md`（已创建，2026-06-07）
- 明确第一版场景、玩法、验收、非目标。

验收：

- PRD 能直接转成 issue。✅
- 不含“以后再说”的核心验收。✅

### Issue 2：GameplayPrimitive catalog

目标：定义 AI 可调用的玩法积木。

状态：初版已落地，2026-06-07。`agent-core` 已新增 `gameplay_primitive` 模块、默认 catalog、引用校验和 JSON round-trip 测试；Bevy 组件映射或占位仍在后续场景/任务/战斗 issue 中完成。

交付：

- `crates/agent-core` 中的 primitive/capability 类型。✅
- `crates/bevy-adapter` 中最小组件映射或占位。待后续 OWS-4/OWS-5/OWS-6
- catalog 文档。✅

验收：

- 测试覆盖 primitive 注册、查询、能力描述。✅
- AI 计划只能引用已登记 primitive。待 OWS-2 `OpenWorldPlan` schema 接入

### Issue 3：PlayableScenario 自动 playtest

目标：让垂直切片可被脚本玩通。

状态：初版已落地，2026-06-07。`agent-core` 已新增 `playable_scenario` 模块，能从 `OpenWorldPlan::open_world_slice01_fixture()` 生成主线 playtest，输出成功/失败报告；真实 Bevy ECS / 输入模拟留给后续接线。

交付：

- `PlayableScenario` 数据结构。✅
- scripted input runner。纯 core harness 已完成；真实输入模拟待后续
- 一个红色敌人或小任务 playtest。✅ `OpenWorldSlice01` 主线 playtest

验收：

- `cargo test -p agent-core playable_scenario` 通过。✅
- 失败输出能指出卡在哪个目标。✅

### Issue 4：OpenWorldPlan schema

目标：结构化 AI 计划。

状态：初版已落地，2026-06-07。`agent-core` 已新增 `open_world_plan` 模块、`OpenWorldSlice01` fixture、JSON round-trip、GameplayPrimitive catalog 校验和 task graph 依赖顺序校验；执行器留给后续 `PlayableScenario` / TaskGraph 执行任务。

交付：

- `OpenWorldPlan` Rust 类型。✅
- serde round-trip tests。✅
- plan -> TaskGraph 转换。已具备 task graph schema 与稳定依赖顺序校验；执行器待 OWS-3

验收：

- 给定 fixture prompt 可生成/加载计划 fixture。✅
- TaskGraph 顺序和依赖稳定。✅

### Issue 5：Scene template 和小岛布局生成

目标：AI 可从模板生成可导航区域。

状态：初版已落地并继续推进，2026-06-07。`agent-core` 已新增 `open_world_template` 模块，可从 `OpenWorldPlan` 生成 small island template，应用到 `SceneBridge` 并通过 apply report 回滚；模板现在会从 `object_manifest` 写入 `player`、`puzzle_switch`、`reward_chest`、`camp_enemy_01`、`main_quest`、`reward_item` 等 gameplay 对象。`bevy-adapter` 已验证 required zones 和 gameplay objects 可进入 `SceneIndex` 名称/组件索引，并已把 template component patch 写入真实 Bevy ECS marker components。真实 Bevy gameplay 行为系统和可视对象生成待后续接线。

交付：

- 小岛模板。✅
- 敌人营地模板。✅ `enemy_camp_marker`
- 谜题点模板。✅ `puzzle_anchor`
- gameplay 对象模板。✅ `OpenWorldPlan.object_manifest`
- Bevy ECS marker components。✅ `bevy_adapter::open_world_components`

验收：

- SceneIndex 能查询模板关键区域和 gameplay 对象。✅
- Bevy World 写入后 SceneIndex 能读回 gameplay marker。✅
- undo 后模板对象清理完整。✅ core `SceneBridge` revert 已覆盖

### Issue 6：Quest/Interaction/Loot 最小链路

目标：完成任务主线最小玩法。

状态：初版已落地，2026-06-07。`agent-core` 已新增 `open_world_runtime` 模块，能从 `OpenWorldPlan` 建立 quest objective、puzzle、loot、inventory runtime，并跑通 `enter_zone -> interact -> open_loot -> collect_reward`；`PlayableScenario` 已复用该 runtime。真实任务 UI、Bevy ECS 交互输入和 SceneIndex 状态映射待后续接线。

交付：

- 交互区。✅ core runtime
- 宝箱/奖励。✅ core runtime
- 任务状态 UI。待后续 UI 接线

验收：

- 自动测试能触发交互、打开宝箱、完成任务。✅ `cargo test -p agent-core open_world_runtime`
- 既有 `OpenWorldSlice01` 主线 playtest 仍通过。✅ `cargo test -p agent-core playable_scenario`

### Issue 7：Combat 最小链路

目标：完成敌人营地最小战斗玩法。

状态：初版已落地，2026-06-07。`agent-core` 已在 `open_world_runtime` 中新增 `CombatantRuntime`、`CombatFaction`、`CombatHitReport`、`engage_enemy()`、`attack_enemy()` 和 `attack_until_defeated()`；`player` 通过多次确定性攻击把 `camp_enemy_01` 推进到 `Dead`，并完成 `DefeatEnemy` objective。真实 Bevy ECS 输入、动画、玩家受击和失败/重置规则待后续接线。

交付：

- `Combatant`。✅ core runtime
- `Attack`。✅ core runtime
- `EnemyBrain` 最小状态。✅ `Patrol -> Aggro -> Attacking -> Dead`

验收：

- 玩家或 playtest runner 能击败 `camp_enemy_01`。✅ `cargo test -p agent-core open_world_runtime`
- 敌人死亡后任务目标更新。✅ `defeat_camp_enemy -> Completed`
- 既有 `OpenWorldSlice01` 主线 playtest 仍通过。✅ `cargo test -p agent-core playable_scenario`

### Issue 8：Visual verification bundle

目标：把截图、SceneIndex、事件日志合成验证结果。

状态：初版已落地，2026-06-07；Director/SceneBridge 接入已推进。`agent-core` 已新增 `open_world_verification` 模块，可把 `OpenWorldPlan.verification_goals`、`PlayableScenarioReport`、runtime events 和 SceneBridge/SceneIndex 查询 evidence 合成 `OpenWorldVerificationBundle`，并输出 Markdown QA report。`DirectorRuntime::verify_open_world_slice01()` 已可生成 bundle/Markdown，并记录 Director event/trace evidence；应用 small-island template 后，bridge-aware bundle 可以通过，Bevy World marker components 也可进入 SceneIndex。QA 文件自动落盘、性能 evidence、SceneIndex proxy PNG 截图路径和真实 Bevy gameplay bridge 已继续推进；真实 Bevy framebuffer screenshot 和完整 gameplay 行为系统待后续接线。

交付：

- `VerificationBundle`。✅ core bundle
- QA report Markdown 输出。✅ `OpenWorldVerificationBundle::to_markdown()`
- Director/QA 入口。✅ `DirectorRuntime::verify_open_world_slice01()`
- SceneBridge 查询 evidence。✅ `exists(...)` 优先走 `SceneBridge::query_entities`
- 应用 template 后的 bridge-aware bundle。✅

验收：

- 成功和失败路径各有 fixture 报告。✅ `cargo test -p agent-core open_world_verification`
- Director 入口能输出 bundle/Markdown，并能在 SceneBridge 缺少 `player` 时失败。✅ `cargo test -p agent-core open_world_ops`
- 应用 small-island template 后，SceneBridge 中存在关键 gameplay 对象并通过 bundle 验证。✅ `cargo test -p agent-core open_world_verification`
- Bevy World marker components 可被 SceneIndex 读回。✅ `cargo test -p bevy-adapter test_open_world_template_components_apply_to_bevy_world_and_scene_index`

### Issue 9：性能基准与大场景压测

目标：防止 AI 生成不可运行内容。

交付：

- benchmark 命令。
- 场景规模 fixture。
- 性能报告模板。

验收：

- 报告含实体数、命令耗时、SceneIndex 耗时、平均帧时间。

## 推荐执行顺序

1. 完成 Phase 0 真实窗口验收。
2. 做 Issue 1，冻结垂直切片 PRD。
3. 做 Issue 2 和 Issue 4，先定义 AI 能力边界和计划结构。
4. 做 Issue 3，建立自动 playtest。
5. 做 Issue 5 和 Issue 6，生成第一个可玩小任务。
6. 做 Issue 7，补战斗最小链路。
7. 做 Issue 8，补视觉/索引/日志合并验证。
8. 做 Issue 9，加入性能门禁。
8. 再启动资产管线、世界分块和多 Agent 生产流。

## 质量门禁

每个阶段必须至少跑：

```bash
cargo check
cargo test --workspace
cargo clippy --workspace -- -D warnings
```

新增 gameplay 或 editor command 时还必须补：

```bash
cargo test -p agent-edit
cargo test -p bevy-adapter
cargo test -p agent-core
```

涉及 UI 的变更必须有：

- 自动 smoke。
- 真实窗口 QA 记录，或明确说明为什么豁免。

涉及 AI 计划执行的变更必须有：

- 成功 fixture。
- 失败 fixture。
- revise/retry 或明确失败报告。

## 风险清单

| 风险 | 表现 | 应对 |
|---|---|---|
| AI 生成不可玩内容 | 场景对象存在但任务无法完成 | PlayableScenario 必须先行 |
| 功能扩散 | 一上来做大地图、多工具、复杂美术 | 先锁定 `OpenWorldSlice01` |
| 验证不足 | 文档说完成，真实窗口不可用 | 每阶段保留 QA 报告 |
| 资产质量不可控 | 模型、材质、动画超预算或丢依赖 | Asset manifest + budget validation |
| 外部工具接入污染核心 | Blender/Godot 逻辑散落 Director | 必须经过 ToolAdapter |
| 旧内容被新 AI 改坏 | 新任务破坏已有场景 | TaskGraph diff + undo + regression |
| 性能崩溃 | 大量实体或截图导致卡顿 | benchmark 和预算门禁 |

## 当前立即下一步

如果要真正开始执行，建议下一个会话只做一件事：

```text
创建 docs/prd/open-world-vertical-slice-prd.md，把 OpenWorldSlice01 的玩法、对象、状态机、验收测试和非目标写死。
```

PRD 完成后，再进入代码层：

1. `GameplayPrimitive` catalog。
2. `OpenWorldPlan` schema。
3. `PlayableScenario` 自动 playtest。
4. 小岛模板和 Quest/Interaction/Loot 最小链路。

这条路线能把“AI 开发原神/塞尔达那样的游戏”从愿景压成可验证工程任务：先做一个可玩、可测、可回滚、可扩展的开放世界垂直切片。
