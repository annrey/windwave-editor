# WindWave 剩余任务与待解决问题

> 更新日期：2026-07-11  
> 范围：当前 Rust workspace、产品方向对齐、OpenWorldSlice01 验收、近程里程碑  
> 用途：开发和派单入口。产品愿景与阶段路线见 `docs/windwave-ai-playable-world-editor-roadmap.md`。  
> 更细的 v0.2 代码级步骤见 `docs/windwave-execution-plan.md`（历史口径，仅作参考）。

## 总体判断

WindWave 的长期愿景是：**AI 能在编辑器里创造世界（含外部 3D/2D 工具），也能作为玩家/居民进入并游玩；世界模型是「每个人都是自己的服务器」，可邀请/加入他人世界，并支持 AI↔AI 通讯与 AI 对 3D 世界的理解。**

近程仍坚持「先可玩证据，再工具中枢」：不立刻做商业级多人 MMO，也不把 Blender/Godot 工具中枢提前。当前阶段是把已有模块接成**可验证的可玩证据链**，再开 AI 居民沙盘。

当前最重要的工程卡点：

```text
AI 居民沙盘深化（AiResidentSlice01 第一刀已落地；下一刀可选更多角色/事件）
```

OpenWorldSlice01 主窗口真实 framebuffer readback 已于 2026-07-11 通过（`screenshot_capture=bevy_framebuffer`，见 `docs/qa/open-world-slice01.md`）。`AiResidentSlice01` 自动验收见 `docs/qa/ai-resident-slice01.md` / `make test-ai-resident`。

## 已确认方向（2026-07）

| # | 决策 | 状态 |
|---|---|---|
| 1 | 近程仍坚持「先可玩证据，再工具中枢」 | **已确认** |
| 2 | 「3D MMO」= 体验隐喻（个人服务器 + 邀请/加入 + AI↔AI 通讯 + AI 理解 3D 世界）；**不是**近期完整商业级多人 MMO | **已确认** |
| 3 | 主窗口验收后下一刀优先「AI 居民沙盘」；治理/回放并行加深已有 Timeline，但不抢主线 | **已确认** |

近程锁定顺序：

```text
主窗口 framebuffer 验收 ✅ → AI 居民沙盘 →（工具中枢更后）
```

## North Star（长期愿景）

一句话：

> WindWave 是面向 AI 可玩世界的 3D 编辑器与运行时：AI 可创造、可游玩、可守规矩；世界以「个人服务器」为单元持续演化，可邀请/加入他人世界，并具备 AI↔AI 通讯与 AI 对 3D 世界的理解。

「3D MMO」含义（已确认，2026-07）：

| 是 | 不是 |
|---|---|
| 体验隐喻：每人自己的服务器 | 近期完整商业级多人在线 MMO |
| 可邀请或加入他人的世界（中期能力柱） | 近程就排真多人网络里程碑 |
| AI↔AI 交流通讯 | 仅「多 NPC 同屏」的空口号 |
| AI 能理解一个 3D 世界 | 纯表格/纯剧情图替代 3D 主语 |

能力柱与现状：

| 能力柱 | 已有 | 缺口 | 下一刀 |
|---|---|---|---|
| 世界可玩性 | OpenWorldSlice01 自动 playtest Passed；主窗口 framebuffer 验收 Passed（2026-07-11） | 真人玩法手感与 Vision 闭环加深 | 已完成 P0 验收；后续体验增强见 P2 |
| AI 创作管线 / 外部 3D·2D 工具 | Director → Plan → SceneBridge；规则/LLM 规划 | Blender 等 ToolAdapter；资产预算与导入失败进 VerificationBundle | Phase 4，可玩闭环与沙盘之后 |
| AI 作为玩家 / 居民 | **AiResidentSlice01**（Merchant+Guard；动作模板+权限+证据；`make test-ai-resident`） | 更多角色类型、白天/夜晚多事件、AI↔AI 受限消息 | Phase 2 加深；见 `docs/prd/ai-resident-sandbox.md` |
| 持续世界 / 个人服务器模型 | WorldClock 初版；单切片小岛 | 多 chunk、离线低风险演化、长期记忆；邀请/加入他人世界（依赖沙盘与世界理解之后） | Phase 5 / 中期；非近程商业 MMO |
| 治理与回放 | 权限/审计骨架；ReplayLedger / Timeline 面板雏形 | 行为时间线、越权告警、按时间回放修正 | 与沙盘并行加深，不抢主线 |

与旧决策的调和：

- **仍成立**：不立刻做完整商业级 3D MMO；外部工具服务于可玩闭环，不反客为主；先证据后规模；Rust/Bevy 主引擎。
- **已锁定**：MMO 感 = 个人服务器 + 邀请/加入 + AI↔AI + AI 理解 3D；近程顺序 = 主窗口验收 → AI 居民沙盘 → 工具中枢更后。

## P0：当前必须先处理

| 编号 | 任务 | 当前状态 | 下一步 | 验收标准 |
|---|---|---|---|---|
| P0-OW-1 | OpenWorldSlice01 主窗口 framebuffer 验收 | **已完成（2026-07-11）**：`make accept-open-world-qa` / `WINDWAVE_OPEN_WORLD_QA_ACCEPT=1` 跑通；QA 含 `screenshot_capture=bevy_framebuffer` 与 `docs/qa/open-world-slice01-framebuffer.png`（1600×900） | 保持回归：`make accept-open-world-qa` | 真人/自动主窗口路径均可复现；QA 字段保持 bevy_framebuffer |
| P0-OW-2 | 主线玩通证据落盘 | **已完成（2026-07-11）**：`docs/qa/open-world-slice01.md` + timeline JSON 已写入主窗口 framebuffer 证据 | 无；回归时重跑 accept | 文档状态为「主窗口已验证」 |
| P0-1 | 红色敌人真实窗口验收 | 自动端到端已通过；真人窗口仍受 Computer Use 限制 | 可与日常主窗口回归一并点验，不阻塞沙盘 | 输入后场景新增红色敌人于右侧 |
| P0-5 | GitHub issue 远程发布 | 被凭据阻塞；开发/设计文档默认不推 GitHub | 仅在需要公开跟踪时再 `gh auth` | 可选，不阻塞主线 |
| P0-AI-1 | AI 居民沙盘 PRD + 最小代理 | **已完成第一刀（2026-07-11）**：`AiResidentSlice01` core harness + Bevy 桥接；QA：`docs/qa/ai-resident-slice01.md`；`make test-ai-resident` | 可选：主窗口肉眼点验；下一刀更多角色/事件 | PRD §5：日程营业/巡逻、越权进证据、SceneIndex 含 agent 状态、playtest/QA 可复现 ✅ |

参考文档：

- `docs/prd/ai-resident-sandbox.md` ← **AiResidentSlice01 已落地；深化另开 slice**
- `docs/qa/ai-resident-slice01.md`
- `docs/windwave-ai-playable-world-editor-roadmap.md`
- `docs/prd/open-world-vertical-slice-prd.md`
- `docs/qa/open-world-slice01.md`
- `docs/prd/world-clock-and-real-time.md`

## 已完成（相对 2026-06 口径的跃迁）

以下不再作为当前 P0 阻塞项（细节见路线总纲 PRD 状态段）：

- v0.2 红色敌人自动闭环、失败修正、质量门禁、SceneIndex 删除残留、EngineCommand reverse contract、HR approval 自动 UI smoke。
- OpenWorldSlice01：自动 playtest 主线 Passed；WorldClock / ReplayLedger 初版；最小 Merchant 日程；World Timeline / Quest 面板；SceneIndex proxy 视觉 evidence；UI QA → ScreenshotQueue 接线；**主窗口 framebuffer readback 运行时验收（2026-07-11）**。

## P1：验收通过后立刻开

| 编号 | 任务 | 当前状态 | 下一步 | 验收标准 |
|---|---|---|---|---|
| P1-1 | AI 居民沙盘加深 | **AiResidentSlice01 第一刀已完成**；见 P0-AI-1 | 更多角色（Quest/Explorer）、多事件窗口 | 同 PRD Phase 2 全量口径 |
| P1-2 | 治理 / 回放工作台加深 | Timeline / Replay 雏形已有 | 并行加深行为时间线、权限裁决可视化、异常标记；**不抢沙盘主线** | 能回答「谁在何时做了什么、是否越权」 |
| P1-3 | undo/redo 与 HR 真人窗口补验 | 自动回归通过 | 主窗口可用后补点验 | QA 有记录或明确豁免 |

## P2：体验与证据增强

| 编号 | 任务 | 当前状态 | 下一步 | 验收标准 |
|---|---|---|---|---|
| P2-1 | Visual feedback loop（真 Vision） | proxy + 主窗口 framebuffer 有；Vision 闭环未串 | operation → screenshot → vision verify → revise | 视觉失败能触发修正或明确失败原因 |
| P2-2 | MemoryInjector / episodic recall | 未完成 | 注入预算、召回、压缩 | Prompt 在预算内；失败经验影响类似请求 |
| P2-3 | Runtime / Task / Director 状态一致 | 部分面板已有 | 对齐同一操作的状态来源 | UI 不互相矛盾 |
| P2-4 | 性能与大型场景预算 | 未完成 | 实体数、帧时间、SceneIndex、回放账本 | 有可记录基准 |

## P3：愿景层扩展（可玩闭环与沙盘之后）

| 编号 | 任务 | 说明 |
|---|---|---|
| P3-1 | 外部 3D/2D ToolAdapter | Blender 等 POC：生成/导入简单模型；失败进 VerificationBundle（工具中枢，排在沙盘之后） |
| P3-2 | 小规模持续世界 / 个人服务器 | 多 WorldChunk、离线低风险演化、长期记忆摘要；每人自己的世界实例 |
| P3-3 | 邀请/加入他人世界 | 中期能力柱：依赖 AI 居民沙盘与「AI 理解 3D 世界」之后再排；**不是**近期商业级多人 MMO |
| P3-4 | AI↔AI 通讯 | 与沙盘深化联动；居民间受限消息/意图交换，经权限与审计 |
| P3-5 | Godot / Unreal adapter、Narrative agents 等 | 保留为后续，不抢主线 |

## 外部阻塞

| 阻塞项 | 影响 | 解决方式 |
|---|---|---|
| Computer Use 未暴露 Bevy/winit 窗口 | Agent 无法代点编辑器 UI | 自动 accept 模式 + 人工点验；`make accept-open-world-qa` |
| `gh` token / 文档不默认推 GitHub | 远程 issue 与公开规划不同步 | 本地文档为准；需公开时再发布 |
| 真实 Multica 环境未就绪 | 只能 mock/test server | 需要时再准备真实 smoke |

## 建议执行顺序

1. **立刻**：保持 `make test-ai-resident` 与 `make accept-open-world-qa` 回归绿；并行加深 P1-2 Timeline/权限可视化。
2. **下一刀（可选）**：AiResident 加深 — 更多角色类型、白天/夜晚多事件（不塞回 Slice01）。
3. **再后**：P2 Vision/Memory/性能；P3 工具中枢、个人服务器持续世界、邀请/加入与 AI↔AI。

不要并行开大 MMO、工具中枢和沙盘三条线；沙盘优先于工具中枢。

## 完成定义（当前里程碑）

OpenWorldSlice01 近程里程碑可视为完成，至少需要：

- 自动 playtest 主线继续 Passed。 ✅
- **主窗口**真实玩通一次，framebuffer / 截图证据写入 QA。 ✅（2026-07-11，`screenshot_capture=bevy_framebuffer`）
- SceneIndex 关键目标与任务状态可查询。 ✅
- `cargo check`、`cargo test --workspace`、`cargo clippy --workspace -- -D warnings` 通过。（回归时再跑）
- 本文件与 `docs/windwave-ai-playable-world-editor-roadmap.md` 状态一致。

v0.2 红色敌人闭环的自动侧已完成；真人窗口补验可并入日常主窗口回归，不再单独阻塞 open-world / 沙盘主线。
