# WindWave AI 可玩世界编辑器路线总纲

> 日期：2026-07-05（North Star / 三项决策锁定：2026-07-11）  
> 状态：方向已确认（2026-07）  
> 上游参考：`/Users/chengyongwei/Downloads/deep-research-report-3.md`、`docs/windwave-ai-open-world-vertical-slice-plan.md`、`docs/prd/open-world-vertical-slice-prd.md`  
> 派单入口：`docs/remaining-work.md`  
> 技术基线：Rust workspace + Bevy runtime/editor adapter，不转向 Unity 或 Unreal 作为主引擎。

## North Star

长期愿景：WindWave 是一个 **AI 可创造、也可自己进入游玩的 3D 世界编辑器与运行时**——AI 利用各类 3D/2D 工具生成内容，也能作为玩家或居民在世界中行动。世界模型是 **每个人都是自己的服务器**：可邀请或加入他人的世界，需要 **AI↔AI 交流通讯**，且 AI 要能 **理解一个 3D 世界**。

分层口径：

| 层 | 含义 |
|---|---|
| North Star | AI 自创造（多工具）+ AI 自玩编辑器 + 个人服务器世界（邀请/加入、AI↔AI、3D 世界理解） |
| Near-term | `主窗口 framebuffer 验收` ✅（2026-07-11）→ `AiResidentSlice01` ✅（2026-07-11）→（工具中枢更后）；治理/回放并行加深 Timeline，不抢主线 |
| 不做（近程） | 不做完整商业级多人 MMO；不把外部工具中枢置于可玩证据与沙盘之前 |

「3D MMO」含义（**已确认，2026-07**）：是体验隐喻，不是近期商业级多人 MMO。等价于个人服务器 + 邀请/加入 + AI↔AI 通讯 + AI 理解 3D 世界。邀请/加入作为中期能力柱，依赖沙盘与世界理解之后再排。

## 一句话定位

WindWave 的主线方向收敛为：**面向 AI 可玩世界的 3D 游戏编辑器**。它不只生成内容，还要证明内容能玩、AI 能守规矩、失败能回放和修正。

这不是立刻制作完整商业级 3D MMO，也不是通用聊天式游戏编辑器。愿景指向个人服务器式共同世界与多工具创作；近程仍是先成为一个能生产、运行、观测、验证和治理 AI-driven gameplay 的 Rust 编辑器平台。

## 决策结论

当前选择第三条路线：

| 方向 | 结论 | 原因 |
|---|---|---|
| 通用 AI 游戏编辑器 | 不作为主叙事 | 容易退化成传统编辑器加聊天面板，差异化不足 |
| 多工具 Agent Command Deck | 保留为后续能力 | 工具中枢有价值，但不能早于可玩证据与沙盘 |
| AI 可玩世界编辑器 | 主线采用 | 与现有 `OpenWorldPlan`、`GameplayPrimitive`、`PlayableScenario`、`VerificationBundle` 最匹配 |

**2026-07 已确认（锁定）**：

1. 近程仍坚持「先可玩证据，再工具中枢」。
2. 「3D MMO」= 个人服务器 + 邀请/加入 + AI↔AI + AI 理解 3D；非近期商业多人 MMO。
3. 主窗口验收后下一刀优先 AI 居民沙盘；治理/回放并行加深，不抢主线。

WindWave 的短期目标不是“大规模自治 AI 世界”，而是先证明一个小型 3D 世界切片中，AI 生成的玩法、世界对象、代理行为和验证证据可以闭环。

## 硬约束

1. **必须有 3D 世界概念**  
   世界不是纯表格、纯剧情图或纯状态机。所有核心对象最终都要能落到 3D 空间、区域、导航、可见对象和 SceneIndex 查询上。

2. **必须有现实世界时间概念**  
   系统不仅有游戏内 tick，也要理解真实日历时间、真实经过时间、离线期间变化、日程触发、周期性事件和长线代理记忆。

3. **主引擎继续选择 Rust**  
   Rust 是核心工程语言；Bevy 继续作为当前 3D runtime/editor substrate。外部工具只能通过 `ToolAdapter` 服务于资产、验证或辅助编辑，不反客为主。

4. **先可玩、可验、可回放，再扩规模**  
   不能用“未来可扩展”替代当前证据。每个阶段都要有 SceneIndex、playtest、截图、事件日志或 QA report 之一作为验收证据。

5. **AI 不能直接越权控制世界**  
   上层语言代理只能产生受限意图、计划或动作模板；真实写入必须经过权限、动作掩码、风险评估、服务器/编辑器裁决和审计记录。

## 产品边界

### WindWave 要做

- 生成和编辑 3D 可玩世界切片。
- 把自然语言目标转成结构化 `OpenWorldPlan`。
- 用 `GameplayPrimitive` catalog 限制 AI 只能组合已登记能力。
- 在 Rust/Bevy 中运行真实 gameplay 行为。
- 用 `PlayableScenario`、`SceneIndex`、截图和事件流证明切片能玩。
- 记录 AI 代理行为、权限裁决、失败原因和修正建议。
- 支持现实时间驱动的代理日程、世界事件和长期记忆。

### WindWave 暂不做（近程边界；不否定 North Star）

- 近程不做完整商业级 3D MMO；邀请/加入他人世界是中期能力，排在沙盘与世界理解之后。
- 不承诺自动生成最终美术质量。
- 不让 LLM 自由散写 gameplay 代码作为主路径。
- 近程不优先接入一整套外部工具生态；工具管线排在可玩证据与 AI 居民沙盘之后（Phase 4）。
- 不把纯模拟通过等同于真实窗口玩通。

## 目标架构

```text
User Goal
  -> DirectorRuntime
  -> OpenWorldPlan
  -> 3D WorldSpec + RealWorldTimeSpec
  -> GameplayPrimitive Catalog
  -> TaskGraph
  -> SceneBridge / BevyAdapter
  -> Bevy World + SceneIndex
  -> PlayableScenario + AgentSandbox
  -> VerificationBundle + ReplayLedger
  -> Revise / Accept / QA Report
```

## 3D 世界模型

3D 世界是产品主语之一，不能只作为渲染结果。第一版世界模型应包含：

| 概念 | 说明 | 当前承接点 |
|---|---|---|
| `WorldSpec` | 世界尺寸、主题、区域、边界、导航约束 | `OpenWorldPlan.world` |
| `WorldChunk` | 可扩展区域单元，第一版可只是逻辑分区 | `spawn_zone`、`puzzle_zone`、`camp_zone` 等 |
| `ZoneMarker` | 3D 区域语义标签，供计划和验证引用 | `GameplayPrimitive` |
| `SceneObjectManifest` | 关键对象稳定 ID、类型、组件、位置 | `OpenWorldPlan.object_manifest` |
| `SceneIndex` | 3D 世界的结构化观察入口 | `bevy-adapter` |
| `PlayableScenario` | 在世界中推进主线流程的可执行验收 | `agent-core` |

第一阶段不追求大地图流式加载，但必须保证玩家、相机、地面、边界、机关、敌人、宝箱、任务 UI 都能进入同一套 3D 可观察模型。

## 现实时间模型

现实时间要成为一等概念，而不是日志字段。建议新增或收敛以下概念：

| 概念 | 职责 | 验收方向 |
|---|---|---|
| `WorldClock` | 统一记录 `wall_time`、`sim_time`、`tick`、`time_scale` | 可序列化，可在测试中冻结 |
| `TimePolicy` | 定义现实时间如何影响游戏世界 | 支持暂停、加速、离线结算、按日刷新 |
| `AgentSchedule` | AI 代理日程，如巡逻、营业、休息、集会 | 真实时间变化后能触发不同意图 |
| `CalendarEvent` | 世界事件窗口，如夜晚袭击、集市、风暴 | 可被计划、UI 和验证查询 |
| `ReplayLedger` | 带时间戳的行为账本 | 可按真实时间和模拟 tick 回放 |
| `OfflineProgression` | 玩家离线期间世界变化规则 | 第一版只允许低风险、可解释变化 |

时间系统必须区分两条线：

- **模拟时间**：Bevy tick、物理更新、playtest 步骤、确定性重放。
- **现实时间**：北京时间/UTC 时间、日程触发、离线经过时长、长期运营事件。

两者要通过 `WorldClock` 显式转换。AI 代理不能直接假设“过了一天”，必须从 `WorldClock` 或 `CalendarEvent` 读取。

## AI 代理边界

AI 居民不是下一步的完整自治 NPC，而是先以可控沙盘方式进入 3D 世界。

| 代理类型 | 第一版目标 | 风险控制 |
|---|---|---|
| Guard / Enemy | 巡逻、发现玩家、进入战斗、失败后可回放 | 动作模板 + 战斗 primitive |
| Merchant | 固定时间营业、补货、报价、库存变化 | 配额、价格边界、审计 |
| Quest NPC | 发布任务、根据世界状态回应 | 对话模板 + 任务状态约束 |
| Explorer | 在 3D 区域中移动、标记发现物 | 导航边界、体力/风险限制 |
| Director QA Agent | 运行 playtest、收集失败证据 | 只读为主，高风险操作需确认 |

代理行为链路固定为：

```text
Observation
  -> Permission Filter
  -> Intent
  -> Action Template
  -> Risk / Rule Check
  -> Bevy World Mutation
  -> Event + Memory Write
  -> Verification / Replay
```

## 编辑器核心工作台

WindWave UI 需要从“面板集合”升级为可玩世界治理台。优先级如下：

| 工作台 | 用途 | 依赖 |
|---|---|---|
| 3D World View | 直接查看和编辑世界、区域、对象、代理 | Bevy viewport、SceneIndex |
| World Timeline | 查看现实时间、模拟 tick、事件、代理日程 | `WorldClock`、`ReplayLedger` |
| Agent Behavior Trace | 查看观察、意图、动作、裁决、结果 | event stream、audit |
| Verification Desk | 展示 playtest、截图、SceneIndex evidence | `VerificationBundle` |
| Governance Panel | 权限、风险、越权、经济/社交异常 | PermissionEngine、AuditLog |

这套 UI 的核心不是装饰风格，而是让用户能判断：AI 做了什么、为什么做、是否越权、失败在哪里、能否回滚。

## 分阶段路线

### Phase 1：3D 可玩切片补实

目标：让 `OpenWorldSlice01` 从 core fixture 走到真实 Bevy 行为。

交付：

- `PlayerController`、`FollowCamera` 接真实输入和相机。
- `PuzzleSwitch`、`LootContainer`、`Combatant`、`EnemyBrain` 接真实 ECS 行为。
- `SceneIndex` 能反映任务、谜题、宝箱、敌人运行时状态。
- `VerificationBundle` 写入截图路径、SceneIndex evidence 和 Markdown QA。
- 用户能在真实窗口玩通一次小岛主线。

### Phase 2：现实时间和代理沙盘

目标：在小型 3D 世界中证明 AI 居民能按现实时间和世界状态行动（主窗口验收后的主线下一刀；通向 AI↔AI 与 3D 世界理解）。

交付：

- `WorldClock` 和 `TimePolicy`。
- 3-5 类 AI 代理的受限日程。
- `AgentSchedule`、`CalendarEvent`、`ReplayLedger` 初版。
- 自动 playtest 覆盖“白天/夜晚”“营业/休息”“事件触发/过期”。
- 失败报告能说明是时间、导航、权限、primitive 还是世界状态问题。

### Phase 3：治理、审计和回放工作台

目标：让 WindWave 能管理 AI 行为风险，而不是只展示结果。与 Phase 2 沙盘并行加深已有 Timeline，但不抢沙盘主线。

交付：

- Agent 行为时间线。
- 权限裁决和高风险动作审批。
- 回放视图：按真实时间和模拟 tick 重放世界变化。
- 异常检测：越权动作、重复失败、经济/任务异常。
- QA report 自动落盘，并标记“已完成 / 已验证 / 仍待”。

### Phase 4：资产和外部工具管线

目标：外部工具服务于 3D 可玩世界，而不是替代主线。排在可玩证据与 AI 居民沙盘之后。

交付：

- Asset manifest 和 prefab authoring。
- Blender `ToolAdapter` POC 只用于生成/导入简单模型。
- Terminal/Git adapter 只用于构建、测试、版本证据。
- 资产预算、缺失依赖、导入失败进入 `VerificationBundle`。

### Phase 5：个人服务器与持续世界

目标：从单切片扩展到「每人自己的服务器」式小型持续世界；邀请/加入与 AI↔AI 在沙盘与世界理解就绪后进入。

交付：

- 多 `WorldChunk`；世界实例按个人服务器边界隔离。
- 现实时间驱动的事件循环。
- 离线期间低风险世界变化。
- AI 居民的长期记忆摘要；AI↔AI 受限通讯（经权限与审计）。
- 邀请/加入他人世界（中期；依赖 Phase 2 沙盘与 3D 世界理解）。
- 性能预算：实体数、帧时间、SceneIndex 更新时间、回放账本大小。

说明：本阶段仍不是商业级多人在线 MMO；网络与会话形态服务于个人服务器 + 邀请/加入，而非大规模公服。

## 第一批新增 PRD 候选

后续应拆出三份 PRD，而不是把所有细节堆进本总纲：

1. `docs/prd/world-clock-and-real-time.md`  
   状态：初版 Rust 类型已落地，2026-07-05。已新增 `agent_core::world_clock`、serde 测试、冻结/手动/实时推进、事件窗口、代理日程、离线上限和回放账本；已接入 `PlayableScenario`、`OpenWorldRuntimeEvent`、`VerificationBundle`、最小 `AgentSchedule` 商人营业 fixture、QA 落盘、core World Timeline DTO、每 tick `world_state` 快照、最小 agent-ui 时间线面板、`OpenWorldQuestPanel`、QA JSON 文件加载、Director 状态注入 API、Bevy resource 热更新、最小 Replay tick 控制、`bevy-adapter` replay state 组件应用、`Visibility`/交互可用性驱动、loot/combat interaction bridge、真实交互事件命令队列、SceneIndex 暴露、bundle 构造/SceneIndex 观测耗时 evidence、SceneIndex proxy 视觉检查 evidence、SceneIndex proxy PNG 写入 `screenshot_paths`、`bevy-adapter` frame delta evidence rows、`agent-ui` frame metrics 注入入口、World Timeline 面板生成 QA 按钮、UI QA 请求队列到统一 artifact writer 的可配置落盘接线，以及 UI QA 请求等待并消费 `bevy-adapter::ScreenshotQueue` framebuffer result。**2026-07-11：完整 app 主窗口 framebuffer readback 运行时验收已通过**（`WINDWAVE_OPEN_WORLD_QA_ACCEPT=1` / `make accept-open-world-qa`，`screenshot_capture=bevy_framebuffer`，证据见 `docs/qa/open-world-slice01.md`）。下一步主线为 AI 居民沙盘。

2. `docs/prd/ai-resident-sandbox.md`  
   状态：**AiResidentSlice01 第一刀已实现（2026-07-11）** — Merchant + Guard；观察→意图→动作模板→权限裁决→证据链；QA：`docs/qa/ai-resident-slice01.md` / `make test-ai-resident`。Quest NPC / Explorer 等留给后续加深。

3. `docs/prd/replay-and-governance-desk.md`  
   定义行为账本、回放、权限审计、异常检测、QA 报告和 UI 面板。

## 最近落地顺序

截至 2026-07：WorldClock、自动 playtest、SceneIndex evidence、proxy 截图、Timeline/QA 接线等已大幅推进。三项产品决策已锁定（见上文「决策结论」）。

近程锁定顺序：

1. **P0（已完成 2026-07-11）**：完整 app 主窗口 framebuffer readback 运行时验收；证据写入 `docs/qa/open-world-slice01.md`（`screenshot_capture=bevy_framebuffer`）。回归：`make accept-open-world-qa`。
2. **AiResidentSlice01（已完成 2026-07-11）**：`docs/prd/ai-resident-sandbox.md` + `make test-ai-resident`；通向 AI 自玩、AI↔AI 雏形、3D 世界理解。
3. **并行不抢线**：治理/回放加深已有 Timeline。
4. **更后**：外部 3D/2D 工具中枢（Phase 4）；个人服务器持续世界与邀请/加入（Phase 5）；沙盘角色/事件加深。

## 成功标准

本路线成立的标志不是文档完整，而是能持续回答五个问题：

1. 这个内容是否真的存在于 3D 世界中？
2. 玩家或脚本是否真的能玩通？
3. AI 代理是否只做了被允许的动作？
4. 失败时是否能定位到对象、时间、权限、导航或 primitive 缺口？
5. 用户是否能按时间线回放并修正？

只有这五个问题都有证据，WindWave 才算真正进入“AI 可玩世界编辑器”方向。
