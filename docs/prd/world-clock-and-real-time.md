# PRD: WorldClock 与现实时间系统

> 日期：2026-07-05  
> 状态：初版 Rust 类型已落地到 `crates/agent-core/src/world_clock.rs`；已接入 playable scenario、runtime events 和 verification bundle  
> 上游路线：`docs/windwave-ai-playable-world-editor-roadmap.md`  
> 关联切片：`docs/prd/open-world-vertical-slice-prd.md`  
> 目标版本：WindWave v0.3 Phase 2 起步

## 1. 背景

WindWave 已经在 `OpenWorldSlice01` 中建立了 `OpenWorldPlan`、`GameplayPrimitive`、`PlayableScenario`、`OpenWorldRuntimeState` 和 `VerificationBundle`。这些能力证明了“计划 -> 运行时状态 -> 自动 playtest -> QA evidence”的 core 层路径。

下一步要让 WindWave 成为 AI 可玩世界编辑器，必须把“时间”从普通日志字段升级为一等领域模型。AI 居民、世界事件、离线结算、回放、长期记忆和真实窗口 QA 都需要回答同一个问题：现在是世界里的什么时候，对应现实里的什么时候，过了多久，哪些行为因此可发生或必须禁止。

当前仓库已有多种局部时间使用方式，例如 Bevy `Time`、`chrono::Utc`、`timestamp` 字段和测试 fixture 时间，但它们没有统一语义。本 PRD 定义 `WorldClock` 系统，作为 Rust 类型、测试和后续接线的边界。2026-07-05 已完成第一层 core 类型与 integration tests。

## 2. 一句话目标

WindWave 提供一个可序列化、可冻结、可推进、可回放的 `WorldClock`，同时记录现实时间、模拟时间和 tick，让 3D 世界、AI 代理、playtest、离线结算和 QA report 都能使用同一套时间语义。

## 3. 非目标

- 不做完整 MMO 服务器时间服务。
- 不做分布式时钟同步、跨机房一致性或反作弊时间校验。
- 不在第一版实现复杂季节、农历、天文、时区转换 UI。
- 不让现实时间直接绕过 `PlayableScenario` 或 `VerificationBundle` 的验证链路。
- 不允许 AI 代理通过自然语言臆造时间跳跃；所有时间推进必须来自 `WorldClock` API 或测试 fixture。

## 4. 核心术语

| 术语 | 定义 |
|---|---|
| `wall_time` | 现实世界时间，第一版使用 UTC 存储，UI 可显示本地时间 |
| `sim_time` | 世界模拟时间，从场景或存档开始计算的逻辑时间 |
| `tick` | 单调递增的模拟步数，用于确定性 playtest 和回放 |
| `time_scale` | 现实时间到模拟时间的倍率 |
| `clock_mode` | 时间模式：冻结、手动推进、实时推进、回放 |
| `CalendarEvent` | 带开始/结束时间窗口的世界事件 |
| `AgentSchedule` | AI 代理在现实时间或模拟时间窗口内的可行动规则 |
| `OfflineProgression` | 玩家离线后按真实经过时间计算的低风险世界变化 |
| `ReplayLedger` | 带 `wall_time`、`sim_time`、`tick` 的事件账本 |

## 5. 用户价值

- 用户可以在编辑器里看到 3D 世界当前的现实时间、模拟时间和事件窗口。
- AI 代理可以按营业时间、巡逻时间、夜晚事件等规则行动。
- 自动 playtest 可以冻结时间，稳定复现失败。
- QA report 可以说明失败发生在真实时间、模拟时间和 tick 的哪一刻。
- 后续持续世界可以支持安全的离线变化，而不是只在窗口打开时运行。

## 6. 成功定义

第一版完成时必须满足：

- `WorldClock` 可 JSON round-trip。
- 测试可以冻结 `wall_time` 和 `sim_time`，并确定性推进 tick。
- `TimePolicy` 可以表达暂停、实时推进、加速推进和手动推进。
- `CalendarEvent` 能判断当前时间是否处于事件窗口内。
- `AgentSchedule` 能根据当前时钟判断代理是否可执行某个日程动作。
- `OfflineProgression` 可以计算离线经过时间，但只产生低风险、可解释事件。
- `ReplayLedger` 中每条记录都同时包含 `wall_time`、`sim_time` 和 `tick`。
- `VerificationBundle` 可接入时间 evidence，不改变既有 playtest 主链路。

## 7. Rust API 边界

建议新增公开模块：

```rust
agent_core::world_clock
```

建议第一版公开类型：

- `WorldClock`
- `WorldClockMode`
- `WorldTimestamp`
- `WorldDuration`
- `TimePolicy`
- `TimeAdvance`
- `CalendarEvent`
- `CalendarEventState`
- `AgentSchedule`
- `ScheduleWindow`
- `OfflineProgressionPolicy`
- `OfflineProgressionReport`
- `ReplayLedger`
- `ReplayLedgerEntry`
- `WorldClockError`

`agent-core/src/lib.rs` 后续应 re-export 最小稳定类型，供 `PlayableScenario`、`OpenWorldRuntimeState`、`DirectorRuntime` 和 UI 查询。

## 8. 数据模型

### 8.1 `WorldClock`

建议字段：

| 字段 | 类型方向 | 说明 |
|---|---|---|
| `wall_time` | UTC timestamp | 当前现实时间 |
| `sim_time_ms` | `u64` 或 typed duration | 当前模拟时间 |
| `tick` | `u64` | 单调 tick |
| `time_scale` | `f32` | 现实时间到模拟时间倍率 |
| `mode` | `WorldClockMode` | 冻结、手动、实时、回放 |
| `last_advanced_wall_time` | UTC timestamp | 上次推进依据 |

第一版要求 `tick` 只增不减。回放模式可以读取历史 tick，但不能修改主世界 tick。

### 8.2 `WorldClockMode`

| 模式 | 行为 |
|---|---|
| `Frozen` | 时间不自动推进，用于可复现测试 |
| `Manual` | 只能通过显式 `advance()` 推进 |
| `Realtime` | 根据现实经过时间和 `time_scale` 推进模拟时间 |
| `Replay` | 从 `ReplayLedger` 读取时间，不写入主世界 |

### 8.3 `TimePolicy`

`TimePolicy` 定义某个世界或切片如何解释现实时间：

| 字段 | 说明 |
|---|---|
| `allow_offline_progression` | 是否允许离线结算 |
| `max_offline_duration_ms` | 单次离线最多结算多久 |
| `default_time_scale` | 默认倍率 |
| `calendar_origin_wall_time` | 世界日历起点 |
| `day_length_sim_ms` | 模拟一天长度 |
| `high_risk_requires_approval` | 离线期间高风险动作是否必须阻止 |

第一版推荐默认关闭高风险离线动作，只允许补货冷却、巡逻位置摘要、事件过期、低价值资源刷新等可解释变化。

### 8.4 `CalendarEvent`

建议字段：

| 字段 | 说明 |
|---|---|
| `id` | 稳定事件 ID |
| `label` | UI 名称 |
| `starts_at` | 现实时间或模拟时间窗口起点 |
| `ends_at` | 窗口终点 |
| `scope` | 全局、区域、对象或代理 |
| `risk` | 低、中、高 |
| `allowed_actions` | 事件期间允许的动作模板 |

事件状态：

```text
Scheduled -> Active -> Expired
```

过期事件不能被 AI 代理继续当作当前事实使用。

### 8.5 `AgentSchedule`

建议字段：

| 字段 | 说明 |
|---|---|
| `agent_id` | 代理 ID |
| `windows` | 日程窗口列表 |
| `fallback_behavior` | 无匹配窗口时的行为 |
| `allowed_actions` | 日程内允许动作模板 |
| `requires_world_state` | 进入日程前必须满足的世界状态 |

第一版可支持三类窗口：

- 按现实时间每日重复，例如 09:00-21:00 营业。
- 按模拟日内时间，例如夜晚巡逻。
- 一次性事件窗口，例如集市只持续 30 分钟。

### 8.6 `ReplayLedger`

每条 `ReplayLedgerEntry` 必须包含：

| 字段 | 说明 |
|---|---|
| `entry_id` | 稳定 ID |
| `wall_time` | 现实时间 |
| `sim_time_ms` | 模拟时间 |
| `tick` | 模拟 tick |
| `actor` | 玩家、AI 代理、系统或测试 runner |
| `event_kind` | 时间推进、动作、裁决、失败、修正 |
| `summary` | 可读摘要 |
| `evidence_refs` | SceneIndex、截图、QA、trace 引用 |

这使“失败回放”可以按两种方式定位：用户看到的现实时间，以及可复现的模拟 tick。

## 9. 行为规则

### 9.1 时间推进

`WorldClock` 只允许通过明确 API 推进：

```text
advance_by_sim_duration(duration)
advance_by_wall_duration(duration)
advance_to_wall_time(timestamp)
advance_one_tick()
```

所有推进必须产生可记录事件。失败条件包括：

- 时间倒退。
- tick 倒退。
- 超过 `TimePolicy.max_offline_duration_ms`。
- `Frozen` 模式下尝试自动推进。
- `Replay` 模式下尝试写入主世界。

### 9.2 AI 代理读取时间

AI 代理只能读取以下摘要：

- 当前 `WorldTimestamp`。
- 当前有效 `CalendarEvent`。
- 自身 `AgentSchedule` 的可执行窗口。
- 与自己权限相关的离线结算摘要。

AI 代理不能读取系统真实本地隐私信息、用户日历、外部账号时间线或未授权事件。

### 9.3 离线结算

离线结算第一版只允许低风险动作：

- 事件过期。
- 商人库存按上限补少量货。
- 巡逻代理生成位置摘要，不逐 tick 模拟。
- 冷却时间减少。
- 任务计时器进入超时或可领取状态。

明确禁止：

- 离线期间发动高影响攻击。
- 离线期间转移玩家高价值资产。
- 离线期间让 AI 通过投票、交易或治理改变长期规则。
- 离线期间执行不可回滚编辑命令。

## 10. 与现有系统的关系

| 系统 | 接入方式 |
|---|---|
| `OpenWorldPlan` | 后续可增加 `real_world_time` 或 `time_policy_ref` 字段 |
| `OpenWorldRuntimeState` | 持有或引用 `WorldClock`，runtime events 带时间戳 |
| `PlayableScenario` | 默认使用 `Frozen` 或 `Manual`，保证可复现 |
| `VerificationBundle` | 已增加 time evidence、bundle 构造耗时、SceneIndex 观测耗时、SceneIndex proxy 视觉检查 evidence、Bevy framebuffer screenshot result 的路径/尺寸 evidence，以及 UI QA artifact writer 汇总的 `ScreenshotQueue::runtime_readback_evidence()` 截图请求/成功/失败诊断 evidence，不改变原有 goal 验证表达式 |
| `DirectorRuntime` | 在执行计划、审批、失败修正时写入 `ReplayLedger` |
| `SceneIndex` | `VerificationBundle` 已将 `world_clock` 和 `agent_schedule` 摘要写入 SceneIndex observations；真实 Bevy SceneIndex runtime cache 后续可继续暴露更多事件状态 |
| `agent-ui` / `bevy-adapter` | 已新增 core World Timeline DTO、QA JSON 数据入口、每 tick `world_state` 快照、最小 `WorldTimelinePanel`、`OpenWorldQuestPanel`、QA JSON 文件加载、Director 状态注入 API、Bevy resource 热更新、最小 Replay tick 控制、World Timeline 截图/视觉检查 evidence 展示、`bevy-adapter` frame delta evidence rows、`agent-ui` frame metrics 注入入口、World Timeline 面板生成 QA 按钮、UI QA 请求队列到统一 artifact writer 的可配置落盘接线、UI QA 请求等待并消费 `bevy-adapter::ScreenshotQueue` framebuffer result、`ScreenshotQueue::runtime_readback_evidence()` 汇总进 UI QA `performance_evidence` 以诊断截图请求/成功/失败状态；`agent-ui`/`bevy-adapter` 依赖 `agent-core` 时已关闭默认 HTTP/LLM features，避免 UI/adapter 验证链路拉入无关网络客户端；以及 `bevy-adapter` replay state 组件应用、`Visibility`/交互可用性驱动、loot/combat interaction bridge、真实交互事件命令队列和 SceneIndex 暴露 |

## 11. 第一版验收场景

### 场景 A：冻结时间 playtest

输入：

```text
WorldClock(mode=Frozen, wall_time=2026-07-05T12:00:00Z, sim_time_ms=0, tick=0)
```

期望：

- `PlayableScenario` 每步只通过 `advance_one_tick()` 推进。
- 相同 fixture 的 tick 序列稳定。
- QA report 能显示失败 step 对应 tick。

### 场景 B：按现实时间营业的商人

输入：

```text
Merchant schedule: 09:00-21:00 local display time
WorldClock wall_time inside window
```

期望：

- schedule 判断为 open。
- 允许动作包含 `quote_price`、`restock_low_risk_item`。
- 不允许动作包含高风险资产转移。

### 场景 C：夜晚巡逻守卫

输入：

```text
Guard schedule: simulated night
TimePolicy day_length_sim_ms = 1_200_000
```

期望：

- 推进到夜晚后，守卫可进入 patrol intent。
- `ReplayLedger` 记录时间推进和 patrol intent。
- 如果 `ZoneMarker` 缺失，失败报告指出世界状态缺口，而不是时间缺口。

### 场景 D：离线结算上限

输入：

```text
last_seen = 2026-07-01T00:00:00Z
now = 2026-07-05T00:00:00Z
max_offline_duration = 24h
```

期望：

- 只结算 24h。
- report 标记被截断的真实经过时间。
- 高风险动作被阻止并进入 audit evidence。

### 场景 E：回放定位失败

输入：

```text
ReplayLedger contains failed puzzle interaction at tick 42
```

期望：

- 可以按 tick 42 查询失败摘要。
- 可以按 wall_time 查询用户看到的发生时间。
- 修正建议能引用 SceneIndex evidence 和最近 runtime events。

## 12. 当前测试

已新增测试文件：

```text
crates/agent-core/tests/world_clock_tests.rs
```

已跑命令：

```bash
cargo test -p agent-core --test world_clock_tests
```

已覆盖：

- `WorldClock` JSON round-trip。
- `Frozen` 模式不会自动推进。
- `Manual` 模式显式推进 tick。
- `Realtime` 模式按 wall duration 和 time scale 推进 sim time。
- 时间倒退被拒绝。
- `CalendarEvent` 状态判断。
- `AgentSchedule` 窗口判断。
- `OfflineProgressionReport` 标记上限截断。
- `ReplayLedgerEntry` 同时包含 wall/sim/tick。

后续接线后还应追加：

```bash
cargo test -p agent-core playable_scenario
cargo test -p agent-core open_world_verification
```

2026-07-05 已完成 `PlayableScenarioState` 接线，并通过：

```bash
cargo test -p agent-core playable_scenario
```

2026-07-05 已完成 `VerificationBundle` 接线，并通过：

```bash
cargo test -p agent-core open_world_verification
```

2026-07-05 已完成 `OpenWorldRuntimeEvent` 结构化时间戳接线，并通过：

```bash
cargo test -p agent-core open_world_runtime
```

## 13. 文档与 QA 输出

后续实现需要继续同步：

- 本 PRD 状态已从 `v0.1 规格稿` 改为 `初版 Rust 类型已落地`。
- `docs/windwave-ai-playable-world-editor-roadmap.md` 已标记 `WorldClock` PRD 和 Rust 类型完成状态。
- `docs/prd/verification-bundle.md` 已增加 time evidence 字段说明。
- `docs/prd/quest-interaction-loot-runtime.md` 已增加 runtime event `WorldTimestamp` 说明。
- `docs/qa/open-world-slice01.md` 在下一次 QA 中记录时间 evidence 是否接入。

QA Markdown 中建议新增：

```text
## Time Evidence

- wall_time: 2026-07-05T12:00:00Z
- sim_time_ms: 42000
- tick: 42
- clock_mode: Frozen
- active_events: night_patrol_window
- schedule_decisions: guard_01=patrol_allowed, merchant_01=closed
```

## 14. 落地顺序

1. 新增 `agent_core::world_clock` 类型和 serde 测试。✅
2. 给 `PlayableScenarioState` 增加 frozen/manual `WorldClock`。✅
3. 给 `OpenWorldRuntimeEvent` 增加结构化 `WorldTimestamp`。✅
4. 给 `VerificationBundle` 增加 time evidence。✅
5. 写一个最小 `AgentSchedule` fixture：夜晚巡逻守卫或现实时间营业商人。✅
6. 在 UI 之前先输出 Markdown QA，避免被面板设计阻塞。

## 15. 开放问题的当前取舍

| 问题 | 当前取舍 |
|---|---|
| 是否使用本地时区存储 | 不使用。本地时区只用于显示，持久化统一 UTC |
| `WorldClock` 放在哪个 crate | 先放 `agent-core`，因为 playtest、runtime、verification 都依赖它 |
| 是否接 Bevy `Time` | 后续由 `bevy-adapter` 映射，不让 core 依赖 Bevy |
| 离线结算是否逐 tick 模拟 | 第一版不逐 tick，只生成低风险摘要 |
| AI 能否修改时间 | 不能直接修改，只能提出需要审批的 time-control intent |
