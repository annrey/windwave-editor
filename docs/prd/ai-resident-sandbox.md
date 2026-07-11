# PRD: AI 居民沙盘

> 日期：2026-07-11  
> 状态：**短 PRD 已批准；`AiResidentSlice01` 第一刀已实现（core harness + Bevy 桥接 + QA）**  
> 上游路线：`docs/windwave-ai-playable-world-editor-roadmap.md` Phase 2  
> 派单入口：`docs/remaining-work.md`（P0-AI-1）  
> 前置：`OpenWorldSlice01` 主窗口 framebuffer 验收已通过（`docs/qa/open-world-slice01.md`）  
> 关联：`docs/prd/world-clock-and-real-time.md`、`docs/prd/verification-bundle.md`、`docs/prd/open-world-vertical-slice-prd.md`  
> QA：`docs/qa/ai-resident-slice01.md`

## 1. 问题 / 目标

OpenWorldSlice01 已证明「计划 → Bevy 世界 → SceneIndex → playtest → VerificationBundle → 主窗口 framebuffer」可玩证据链。缺口是：**少量 AI 居民能否在同一 3D 切片里按规则行动，且每一步可观测、可裁决、可回放**。

一句话目标：

> 让 5–20 量级受限 AI 居民在可玩 3D 切片中走「观察 → 意图 → 动作模板 → 权限裁决 → 世界写入 → 证据」，通向「AI 自己去玩」，而不是自由改世界的 LLM。

长期隐喻（已锁定，本 PRD 不交付）：每人自己的服务器；邀请/加入；AI↔AI；AI 理解 3D。近程只交付沙盘可玩证据。

## 2. 范围（In）

| 项 | 要求 |
|---|---|
| 代理规模 | 5–20 个受限代理；第一刀先 **2 类**：`Merchant`、`Guard` |
| Merchant | 在现有 `AgentSchedule` 商人营业 fixture 上扩展：营业窗口内 `quote_price` / `restock_low_risk_item`；窗外拒绝并记证据 |
| Guard | 日程窗口内巡逻路径 / 站岗；可复用 `EnemyBrain` 巡逻雏形，但**不**把战斗做成沙盘第一刀主验收 |
| 决策环 | 固定管线：`SceneIndex` 观察 → 受限意图 → **动作模板**（非任意自然语言写世界）→ `PermissionEngine` / 日程窗口裁决 → 允许则写 ECS / runtime → `ReplayLedger` + `VerificationBundle` |
| 权限边界 | 代理只能执行自身 `allowed_actions`；越权必须拒绝、进审计/账本，不得静默成功 |
| 证据链 | 与 `WorldClock`、`AgentSchedule`、`SceneIndex`、`VerificationBundle`、既有 Timeline/QA 落盘对齐；失败能指出时间 / 权限 / 日程 / 世界状态哪一环 |
| 场景底座 | 复用 `OpenWorldSlice01` 小岛；不新开大地图 |

## 3. 范围（Out）

- 完整工具中枢（Blender/Godot ToolAdapter、资产管线中枢）
- 真多人服务器、邀请/加入会话、商业级 MMO
- 完整 Governance Desk（本阶段只消费已有权限骨架 + Timeline；深化不抢主线）
- 任意 LLM 自由改世界 / 自由发 `EngineCommand`
- Phase 2 全量 3–5 类代理（Quest NPC、Explorer 等留给后续 slice）
- AI↔AI 完整通讯协议（沙盘深化后再开；本刀最多预留「受限消息」类型位，不实现）
- 真 Vision 闭环、长期记忆压缩、离线高风险演化

## 4. 与现有能力衔接

| 已有 | 沙盘怎么用 |
|---|---|
| `OpenWorldSlice01` | 场景、playtest、主窗口 QA 回归底座 |
| `WorldClock` / `TimePolicy` | 冻结/手动推进驱动营业与巡逻窗口 |
| `AgentSchedule` + merchant fixture | Merchant 第一刀直接扩展；Guard 新增日程窗口 |
| `EnemyBrain` / combat primitives | Guard 巡逻行为参考；战斗不作为本刀硬验收 |
| `PermissionEngine` + 审计骨架 | 动作模板风险裁决；越权 → deny + evidence |
| `SceneIndex` | 代理观察入口；QA 断言代理状态/位置/日程决策 |
| `ReplayLedger` / World Timeline | 记录 actor、tick、裁决、失败；并行加深 UI，不阻塞沙盘行为落地 |
| `VerificationBundle` | 输出 schedule decisions、越权拒绝、agent 状态摘要 |
| `PlayableScenario` harness | 自动 playtest：推进时钟 → 期望营业/巡逻/拒绝 |

## 5. 成功标准（可测）

第一刀 `AiResidentSlice01` 完成时必须同时满足：

1. **日程营业**：冻结时钟到营业窗口内，`merchant_01` 允许 `quote_price`（或等价模板）；窗外同一动作被拒。
2. **日程巡逻**：冻结到巡逻窗口，`guard_01` 处于允许巡逻/站岗模板；窗外拒绝或进入 `fallback_behavior`，且可在 evidence 中读到。
3. **越权可审计**：代理请求不在 `allowed_actions` 内的动作 → `PermissionEngine`/日程裁决拒绝；`ReplayLedger` 或 bundle 含 deny 记录（actor、tick、原因）。
4. **观察含 agent 状态**：`SceneIndex`（或 runtime 查询）能读到至少：`agent_id`、角色类型、当前 schedule 决策、关键世界位置/区域。
5. **证据链不断**：一次自动 playtest 产出 `VerificationBundle` Markdown/JSON，含 schedule decisions + 至少一条 agent 相关事件；不破坏既有 OpenWorldSlice01 主线 Passed。
6. **最小 QA 路径**：文档化一条命令或 `make` 目标（或明确挂到现有 accept/playtest），可复现上述 1–5；主窗口 framebuffer 回归仍用 `make accept-open-world-qa`，沙盘不要求新开真人窗口门禁。
7. **规模上限声明**：fixture/配置显式限制 ≤20 代理；超出配置应失败或被拒绝加载（测试覆盖一种边界即可）。

## 6. 切片建议（比 Phase 2 更窄）

| 字段 | 值 |
|---|---|
| 切片名 | **`AiResidentSlice01`** |
| 场景 | `OpenWorldSlice01` 小岛：码头商人点 + 营地/路径一处巡逻点 |
| 代理 | `merchant_01`（营业）、`guard_01`（巡逻）；可选 0–3 个静态旁观 NPC 占位，不计行为验收 |
| 时钟 | `Frozen` + 测试显式 `advance_*`；不依赖实时墙钟 |
| 动作模板（最小集） | Merchant：`quote_price`、`restock_low_risk_item`、`idle_off_hours`；Guard：`patrol_waypoint`、`hold_post`、`idle_off_duty` |
| 明确不做 | 交易经济闭环、战斗击杀验收、导航寻路完美性、LLM 选动作 |

Phase 2 全量（更多角色类型、白天/夜晚多事件、失败分类报告）在本切片通过后再开，不塞进第一刀。

## 7. 非目标与风险

**非目标**：把沙盘做成聊天 NPC 演示；用脚本假通过冒充「AI 在玩」却无 SceneIndex/账本证据；提前做邀请/加入网络。

| 风险 | 缓解 |
|---|---|
| 范围膨胀到 Phase 2 全量 | 硬锁 2 角色 + 上表动作模板；Quest/Explorer 不进本 PRD 验收 |
| LLM 绕过动作模板直接写世界 | 写入路径只接受模板 ID；自然语言只产生意图候选 |
| 与 Governance Desk 抢工 | Timeline/权限可视化并行加深，不阻塞 bevy 行为与 playtest |
| 行为不可复现 | 强制 Frozen 时钟 + tick 账本；playtest 不依赖墙钟 |
| 破坏 OpenWorld 主线 | 沙盘 fixture 附加在既有切片上；CI 保留 `OpenWorldSlice01` Passed |

## 8. 建议实现顺序

```text
1. 文档（本 PRD）✅ → remaining-work / roadmap 指针
2. 类型 / 权限：ResidentRole、ActionTemplateId、意图→模板映射；挂 PermissionEngine + AgentSchedule ✅
3. Core harness：AiResidentSlice01 fixture + schedule/越权单测 + VerificationBundle 字段 ✅
4. Bevy 行为：Merchant 可见营业态 / Guard 巡逻或站岗最小运动；SceneIndex 暴露 agent 状态 ✅
5. QA：自动 playtest + 短 QA 文档（docs/qa/ai-resident-slice01.md）；回归不破坏 accept-open-world-qa ✅
```

每步结束应能回答：谁在何时做了什么、是否被允许、证据在哪。

## 9. 完成定义（本 PRD）

- [x] 短 PRD 落盘，路径稳定为 `docs/prd/ai-resident-sandbox.md`
- [x] `AiResidentSlice01` 实现并通过第 5 节成功标准（自动：`make test-ai-resident` / `docs/qa/ai-resident-slice01.md`）
- [x] `docs/remaining-work.md` P0-AI-1 状态更新为进行中/完成（实现阶段再改）
- [x] 不启动工具中枢、真多人、完整 Governance Desk

---

**下一动作**：Phase 2 加深（更多角色类型 / 白天夜晚多事件）另开 slice；本刀 GUI 肉眼点验可选，不阻塞。
