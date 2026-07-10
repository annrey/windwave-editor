# WindWave 版本规划、目标与待办记录

> 生成日期：2026-06-07  
> 范围：根目录 Rust workspace、核心文档、`src/` 与 `crates/` 下主要模块  
> 目的：把项目当前状态、版本目标、优先级待办和验收标准收敛到一份可执行记录中。

## 结论摘要

WindWave 当前代码主体是 `Cargo.toml` 中的 `agent-edit` v0.1.0：一个基于 Rust、Bevy 0.17、bevy_egui 的 AI Agent 驱动游戏编辑器。根 `README.md` 已重新对齐到 WindWave；旧的 Understand Anything 根 README 和过期规划文档已经归档到 `docs/deprecated/`。根仓库仍保留 Understand Anything 插件、Node workspace 和大量历史/外部项目内容，因此后续仍要明确 Rust 主项目与混居资料的边界。

当前可作为基线的版本是 `v0.1.0`。它已经具备 Director、Planner、SceneBridge、SceneIndex、Memory、EventStream、权限/回滚/审计、agent-ui、bevy-adapter、multica-bridge 等主体模块。2026-06-07 已在原 iCloud 路径实跑通过 `cargo check`、`cargo test --workspace`、`cargo clippy --workspace -- -D warnings`，并确认 `cargo run --bin agent-edit` 可启动。历史文档中测试数量曾出现 880、876、861、1031 等多组口径，后续以实跑 Cargo 输出为准。

下一阶段不应继续扩散新功能。优先目标是把“能规划”收敛为“能闭环执行”：用户输入经过 Director/Agent 的 think -> act -> observe -> revise -> verify 路径，能真实改动 Bevy 场景、生成可撤销命令、用 SceneIndex/Vision 校验结果，并在失败时自动修正。

## 依据与可信度

| 来源 | 当前用途 | 可信度 | 备注 |
|---|---|---:|---|
| [`Cargo.toml`](../Cargo.toml) | 项目真实身份、Rust workspace、crate 列表 | 高 | `agent-edit` v0.1.0；根 README 已对齐 |
| [`CONTEXT-MAP.md`](../CONTEXT-MAP.md) | 领域上下文、上下文关系、标准术语 | 高 | 明确 WindWave 是 AI Agent 游戏编辑器 |
| [`PROJECT_STATUS.md`](../PROJECT_STATUS.md) | 当前总体状态、Sprint 完成度、质量结论 | 高 | 局部测试数量有历史残留 |
| [`CURRENT_CAPABILITIES.md`](../CURRENT_CAPABILITIES.md) | Multica/WindWave 当前能力 | 中高 | 适合描述桥接层能力 |
| [`design/plans/remaining-tasks-roadmap.md`](../design/plans/remaining-tasks-roadmap.md) | 可执行的下一批待办 | 中高 | 部分任务已被代码推进，需要逐项核验 |
| [`docs/deprecated/spec-and-todo.md`](deprecated/spec-and-todo.md) | 历史需求池 | 低 | 已废弃，不再作为当前计划来源 |
| [`docs/deprecated/implementation-roadmap.md`](deprecated/implementation-roadmap.md) | 历史路线图与阶段意图 | 低 | 已废弃，与当前代码和 `PROJECT_STATUS.md` 有冲突 |
| [`docs/deprecated/pending-implementation.md`](deprecated/pending-implementation.md) | 历史待实现清单 | 低 | 已废弃，含部分有效待办但状态混杂 |

## 当前架构快照

### Workspace 与入口

- 根入口：[`src/main.rs`](../src/main.rs)，启动 Bevy App，注册 `AgentCorePlugin`、`AgentUiPlugin`、`BevyAdapterPlugin`、`BevySceneBridgePlugin`、`RuntimeAgentPlugin`、`LlmRuntimeAgentPlugin`、`SceneIndexRebuildPlugin`、`SceneIndexIncrementalPlugin`、`IntegrationPlugin`、`VisionPlugin`、`ScreenshotPlugin`、`CommandProcessorPlugin`。
- 核心 crate：`agent-core`、`agent-ui`、`bevy-adapter`、`multica-bridge`、`ai-frameworks`、`game-simulator`。
- 主应用启动时创建 Player、Enemy_01、Enemy_02，并将 Bevy Entity 注册为 `AgentEntityId`。
- 用户输入路径：`handle_agent_input` 读取 chat user message，注入 `SceneIndexSceneBridge`，优先走 `DirectorRuntime::execute_with_llm()`，否则走 `DirectorRuntime::handle_user_request()` 的规则回退路径。
- 命令落地：Director 通过 SceneBridge 产出 JSON EngineCommand，主应用放入 `PendingCommands`，`bevy-adapter::CommandProcessorPlugin` 下一帧应用到 Bevy World 并记录 undo/redo。

### 核心上下文

- `agent-core`：Director、Planner、Memory、Skill、Tool、Permission、Rollback、Event、Reflection、Dynamic Planner、Hybrid Controller、Vision 抽象、Team/Squad/HR Agent 等编排核心。
- `bevy-adapter`：将 `EngineCommand` 应用到 Bevy ECS，维护 `SceneIndexCache`，提供 SceneBridge 实现、截图、Vision provider、运行时 Agent 组件。
- `agent-ui`：Director Desk、Chat、Inspector、Hierarchy、Runtime Agent Panel、Task Panel、Visual Understanding、Gizmo、Prefab Browser、布局与快捷键。
- `multica-bridge`：任务同步、WebSocket client、agent proxy、skill adapter、scene context、scene event bus、memory/scene integration、daemon/test server。

## 已完成能力

### v0.1.0 基线能力

- Director/Planner/Skill/Tool 主链路已存在，内置技能包括创建实体、修改 Transform、查询场景、导入资源。
- `BaseAgent::run()` 已实现基础 think -> act -> observe 循环、最大步数、stuck 检测和 conversation memory 写入。
- `DirectorRuntime` 已支持 rule-based fallback、LLM 模式选择、内置 AgentRegistry、SceneBridge 注入、事件 drain、命令 drain、pending approval 同步。
- `KeywordMatcher` 已抽取到独立模块，并被 `router.rs` 使用。
- `SceneIndexCache`、周期全量 rebuild、基于 Bevy `Changed<T>` 的增量更新已经实现。
- `CommandProcessorPlugin` 已集中处理 `PendingCommands`，并为多种命令生成 undo 反向命令。
- `TeamAgentContext` 已完成命名，旧路线图中的 `team_context::AgentContext` 命名冲突项已经完成。
- HR Agent 已实现 add/list/remove；add 与 remove/fire 都会返回 `NeedUserInput`，并通过 `confirm_pending()` / `reject_pending()` 显式应用或取消 roster 变更。
- Visual Understanding UI 状态与 VGRC 截图桥接已经接入主应用，当前每 30 帧请求截图并更新 UI 状态。
- Multica bridge 已具备本地协议、任务同步、代理、技能适配、本地测试服务器、WebSocket 客户端与 daemon 能力。

## 文档与代码冲突

| 冲突 | 当前判断 | 处理建议 |
|---|---|---|
| 根 `README.md` 曾描述 Understand Anything，不描述 WindWave | 已修复：根 README 改为 WindWave 入口，旧 README 归档到 `docs/deprecated/` | 后续只需继续收敛混居 Node workspace 的边界说明 |
| `package.json` 名称为 `understand-anything`，`Cargo.toml` 名称为 `agent-edit` | Node workspace 与 Rust workspace 混居 | 已新增 `docs/repository-boundaries.md`，并在 README/package.json 说明边界 |
| 旧 `design/*.md` 路线图把很多已存在模块标为未开始 | 已修复：过期规划文档移入 `docs/deprecated/` | 当前计划以本文件和 `PROJECT_STATUS.md` 为准 |
| `PROJECT_STATUS.md` 测试数量不一致 | 已收敛为 2026-06-07 实跑 `cargo test --workspace` 通过，不再强调单一历史数字 | 后续以真实 Cargo/CI 输出为准 |
| Remaining roadmap 中部分 E 项已完成 | 路线图需要逐项更新 | 在本文件中按“完成/仍有效/过期”重新归类 |

## 版本规划

### v0.1.0：当前基线

目标：保留并稳定现有 AI Agent 游戏编辑器骨架。

范围：Director/Planner/Skill/Tool、SceneBridge、Bevy Adapter、agent-ui、Memory、EventStream、Multica bridge、权限/回滚/审计、基础 Vision/SceneIndex。

出货标准：
- `cargo build` 通过。
- `cargo test --workspace` 通过。
- `cargo clippy --workspace -- -D warnings` 通过。
- 文档明确说明当前主项目身份是 WindWave/agent-edit，Understand Anything 是混居历史内容或外部插件内容。

### v0.2.0：闭环执行与正确性修复

目标：从“能规划/能展示”推进到“能稳定执行并可回滚”。

优先任务：
- P0：统一项目身份文档已完成：根 README 改为 WindWave 入口，旧 README 与旧路线图归档到 `docs/deprecated/`，并新增 `docs/repository-boundaries.md` 说明 package/Cargo 边界。是否物理拆分 Node workspace 作为后续仓库治理决策。
- P0：把 `BaseAgent::run()` 或 ReAct loop 接入实际 Director/Agent 执行路径，而不是只作为基础实现存在。
- P0：完成 Plan-and-Solve 失败后重规划，以及 Reflection 每轮 `reflect_and_revise()`。
- P1：修复 `SceneIndexCache` 增量路径删除实体后旧索引残留风险：2026-06-07 插件级回归测试和 workspace 测试均已通过。
- P1：HR Agent remove/fire 已改为像 add 一样走确认，HR 自身已有 `confirm_pending()` / `reject_pending()`，并已接入 DirectorRuntime 的 pending approval API；2026-06-07 自动 UI smoke 已通过，真实窗口点击验收仍需用户或可访问 Bevy/winit 窗口的工具完成。
- P1：`SpawnPrefab`、`SetSpriteTexture` 的 undo 覆盖已有回归测试；`LoadAsset` 当前按幂等低风险操作处理，如需热卸载再引入 `RemoveAssetReference`。
- P1：确认 `self_modifying_agent` 待办是否仍存在。当前 `crates/agent-core/src` 未发现对应文件，旧任务可能已过期或被重命名。
- P1：消除剩余重复关键词判断。虽然 `KeywordMatcher` 已存在，但 `director/mod.rs`、`specialized_agents.rs`、`scene_agent.rs`、`reflection_engine`、`planner/rule_based/keywords.rs` 等位置仍有散落 keyword 逻辑。

验收场景：用户输入“创建一个红色敌人放在右边”，系统能生成计划、申请必要权限、执行 Bevy 命令、在 SceneIndex 中观察到新实体、失败时重试或修正、最终可 undo。

### v0.3.0：记忆、上下文与提示词生产化

目标：让 Agent 具备跨会话上下文、项目画像和稳定回归能力。

优先任务：
- P1：MemoryInjector 注入 project profile、top concepts、key files、patterns，并控制在约 2000 token 内。
- P1：实现旧会话 LLM compression，用于长会话记忆压缩。
- P1：基于 entity/action fingerprint 做 episodic memory recall。
- P1：从用户修正、执行结果和失败案例自动学习。
- P1：完成 L0-L3 layered context 与压缩策略落地。
- P2：按请求类型注入 few-shot examples。
- P2：建立 AgentSnapshot 确定性回归测试，固定关键任务输入输出。

验收标准：相同测试 prompt 在无外部 LLM 波动条件下能生成稳定计划；记忆注入不会超过预算；失败案例能在后续类似请求中影响计划。

### v0.4.0：视觉反馈与混合编辑体验

目标：让系统不只相信命令返回，而是用截图/SceneIndex/Vision 校验真实场景结果。

优先任务：
- P2：VisualUnderstanding UI 增加明确的手动分析入口：截图 -> VisionClient -> Director Desk/Chat。
- P2：实现 visual feedback loop：Operation -> Screenshot -> Vision verify -> retry/revise。
- P2：完善 HybridEditorController 策略选择，明确何时用 LLM、规则、Vision、人工确认。
- P2：抽取并统一 KeywordMatcher 后，减少 rule/LLM/vision 选择中的分叉判断。
- P2：补齐 Runtime Agent Panel、Task Panel 与 Director events 的实际状态同步。

验收标准：执行可视化场景修改后，UI 能显示截图、目标检查结果和失败原因；Vision 判定失败时能触发可解释的修正步骤。

### v0.5.0：生态集成与扩展能力

目标：把 WindWave 从单机原型推进为可接外部系统和多引擎的编辑器平台。

优先任务：
- P2：完成真实 Multica server smoke test，不只依赖 mock/test server。
- P2：把 `team_structure`、`hr_agent`、`cli_agent`、`team_context` 等团队模块纳入真实 DirectorRuntime 流程。
- P2：CodeGraph + ImpactAnalyzer + CodeContextGenerator，服务于代码影响分析和编辑前评估。
- P3：Multi-selection tools 与 Transform Palette。
- P3：Prefab、asset、hierarchy 更完整的编辑命令与 UI 流程。
- P3：Godot/Unreal adapter 从规划推进到最小可运行适配层。
- P3：Narrative agents 与游戏设计专用 Agent 扩展。

验收标准：真实 Multica 环境能完成任务分发、进度回传、技能执行和场景状态同步；至少一个非 Bevy adapter 能跑通读取/命令/回滚最小链路。

## 当前待办清单

### P0：立即处理

- 继续收敛仓库边界：已新增 `docs/repository-boundaries.md` 并在 README/package.json 标明 Rust/Node 边界。
- 建立 v0.2.0 里程碑 issue：本地草案已建于 `docs/issues/v0.2.0-closed-loop-execution.md`；远程 GitHub 创建需先修复 `gh` token。
- 为“创建红色敌人并放到右侧”写端到端回归测试或手动 QA 脚本：手动 QA 已建于 `docs/qa/red-enemy-closed-loop.md`；现有自动验收测试仍需升级到真实 Bevy World 端到端。
- 跑一次真实质量门禁并更新 `PROJECT_STATUS.md` 的测试数字：2026-06-07 `cargo check`、`cargo test --workspace`、`cargo clippy --workspace -- -D warnings` 已在原 iCloud 路径通过。

### P1：下一批开发

- 将 `BaseAgent::run()` 或 ReAct runner 接入 DirectorRuntime 的真实请求路径。
- 完成失败后 plan revision 与 reflection 修正闭环。
- 修复 SceneIndex 删除实体残留：代码已处理，待测试完成确认。
- 为 HR add/remove/fire 的 Director approval flow 补真人窗口验收；自动 UI smoke 已通过。
- 继续验证资产/Prefab/SpriteTexture 的 undo/redo 设计：Prefab 与 SpriteTexture 已有回归测试，`LoadAsset` 按幂等低风险操作处理；剩余是 full UI QA 中确认 undo/redo 体验。
- 继续抽干散落关键词判断，统一到 `KeywordMatcher` 或明确分层策略。

### P2：稳定性和体验

- MemoryInjector、episodic recall、自动学习与 layered context。
- Visual feedback loop 与 Vision UI 手动入口。
- Runtime Agent、Task Panel、Director events 的状态一致性。
- 真实 Multica server 集成测试。
- 增加 edit_ops、layout、memory、event_stream、SceneIndex integration 的测试覆盖。

### P3：扩展能力

- CodeGraph/ImpactAnalyzer/CodeContextGenerator。
- Multi-selection tools、Transform Palette、Prefab 深化。
- Godot/Unreal adapter。
- Narrative/game-design agents。
- 性能 benchmark 和大型场景压测。

## 关键风险

| 风险 | 用户影响 | 建议 |
|---|---|---|
| 仓库身份混乱 | 新开发者会从 README 进入错误项目 | v0.2.0 前先修文档入口 |
| 闭环执行未接入真实主路径 | Demo 看起来会规划，但用户看不到稳定场景结果 | 把 ReAct/BaseAgent loop 接入 DirectorRuntime |
| SceneIndex 删除残留 | Agent 会基于不存在的实体继续规划 | 2026-06-07 插件级回归测试和 workspace 测试均已通过 |
| undo/redo 覆盖不全 | 用户无法安全试错资产和 Prefab 操作 | Prefab/SpriteTexture 已有回归测试，LoadAsset 有意不可撤销；新增写入型 EngineCommand 必须定义 reverse contract |
| HR 确认后 apply 缺真实窗口验收 | DirectorRuntime 已通过 pending approval API 调用 HR `confirm_pending()` / `reject_pending()`；自动 UI smoke 已通过，但 Computer Use 当前无法发现 Bevy/winit 窗口 | 由用户或可访问窗口的工具做最终点击验收 |
| 旧文档持续误导 | 后续计划重复做已完成模块 | 已将主要旧路线图移入 `docs/deprecated/`，保留本文件为当前计划入口 |
| 真实 Multica 未验证 | 本地 mock 通过但实际协作链路失败 | v0.5 前增加真实 server smoke test |

## 验收门禁

每个版本出货前至少满足：

```bash
cargo build
cargo test --workspace
cargo clippy --workspace -- -D warnings
```

功能门禁：
- 用户场景“创建一个红色敌人放在右边”必须产生可观察的 Bevy 场景变化。
- 所有写入型 `EngineCommand` 必须明确 undo/redo 行为，不能默默跳过。
  - `LoadAsset` 有意不可撤销（幂等、低风险），如需热卸载再引入 `RemoveAssetReference`。
- 高风险操作必须进入 permission/confirmation 流程。
- SceneIndex、UI、Director events 三者对同一次操作的状态描述必须一致。
- Multica 相关版本必须跑通 mock server 和真实 server 两条 smoke test。

文档门禁：
- `README.md`、`PROJECT_STATUS.md`、本文件的项目身份和版本号一致。
- 历史路线图必须标注过期或迁移到当前待办。
- 新增能力必须在对应上下文文档或版本规划中留下入口。

## 建议执行顺序

> 详细、可逐条执行的落地计划见 [`docs/windwave-execution-plan.md`](windwave-execution-plan.md)（含 M1 正确性修复、M2 闭环执行的代码级步骤，以及 M3/M4+ 里程碑提纲）。

1. 修项目身份入口和测试数字，避免后续所有计划建立在错误 README 上。
2. 做 v0.2.0 闭环执行最小切片：单场景 prompt -> plan -> apply -> observe -> undo。
3. 修 SceneIndex 删除残留与剩余 undo/redo 缺口，并为 HR approval flow 补 UI smoke。
4. 再进入 memory/context/vision 深化，避免在执行链路未稳定前扩大系统复杂度。
5. 最后做 Multica 真实 server、CodeGraph、多引擎 adapter 等平台化扩展。
