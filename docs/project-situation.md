# WindWave 项目情况整理

> 整理日期：2026-06-07  
> 范围：根目录 Rust workspace、`src/`、`crates/`、当前核心文档与近期会话日志  
> 目的：把项目现状、可信依据、正在收敛的问题和下一批要处理的内容集中到一个入口。

## 1. 一句话结论

WindWave（`agent-edit`）当前是一个基于 Rust、Bevy 0.17、bevy_egui 的 AI Agent 驱动游戏编辑器。项目主体骨架已经成型，具备 Director/Planner/Skill/Tool、SceneBridge、SceneIndex、Memory、EventStream、权限/回滚/审计、UI、Bevy adapter、Multica bridge 等模块。

下一阶段重点不是继续铺新模块，而是把已有能力从“能规划、能展示、能局部执行”收敛为“用户请求能稳定闭环执行、可观察、可验证、可撤销”。

## 2. 当前可信入口

| 文档/文件 | 用途 | 当前可信度 | 备注 |
|---|---|---:|---|
| `Cargo.toml` | Rust workspace、crate 列表、主二进制入口 | 高 | 主项目为 `agent-edit` v0.1.0 |
| `README.md` | 项目入口说明 | 高 | 已对齐为 WindWave，不再把根项目误写成 Understand Anything |
| `CONTEXT-MAP.md` | 领域上下文与模块关系 | 高 | 明确 agent-core、bevy-adapter、agent-ui 的边界 |
| `PROJECT_STATUS.md` | 项目总体状态与历史 Sprint 汇总 | 中高 | 内容较全，但仍有历史测试数量口径残留 |
| `docs/windwave-version-plan.md` | 版本目标、优先级与验收门禁 | 高 | 当前最适合作为路线图入口 |
| `docs/windwave-execution-plan.md` | v0.2 代码级执行清单 | 高 | 适合直接拆任务执行 |
| `docs/remaining-work.md` | 剩余任务与待解决问题 | 高 | 后续开发和派单入口 |
| `docs/repository-boundaries.md` | Rust/Node 混居边界 | 高 | 明确 `Cargo.toml` 与 `package.json` 的职责 |
| `docs/qa/red-enemy-closed-loop.md` | v0.2 P0 闭环 QA 场景 | 高 | 用于红色敌人验收 |
| `docs/issues/v0.2.0-closed-loop-execution.md` | v0.2 GitHub issue 草案 | 中高 | `gh` token 修复后可发布 |
| `CURRENT_CAPABILITIES.md` | Multica bridge 能力说明 | 中 | 偏桥接层总结，部分口径偏历史 |
| `docs/deprecated/` | 旧规划与旧 README | 低 | 仅作历史参考，不应作为当前计划来源 |

## 3. 项目结构现状

| 路径 | 当前职责 |
|---|---|
| `src/main.rs` | Bevy App 入口；注册 AgentCore、AgentUi、BevyAdapter、SceneIndex、Vision、CommandProcessor 等插件 |
| `crates/agent-core` | Director、Planner、Agent、Memory、Skill、Tool、Permission、Rollback、Event、Reflection、Dynamic Planner |
| `crates/bevy-adapter` | Bevy ECS 适配、EngineCommand、SceneBridge 实现、SceneIndex、截图、命令处理与 undo/redo |
| `crates/agent-ui` | egui UI、Director Desk、Chat、Hierarchy、Inspector、Runtime Agent Panel、Task Panel、Visual Understanding |
| `crates/multica-bridge` | Multica 协议桥接、任务同步、WebSocket、Agent proxy、技能适配、本地测试服务器 |
| `crates/ai-frameworks` | AI 框架集成实验层 |
| `crates/game-simulator` | 游戏逻辑仿真支持 |
| `docs/` | 当前项目文档、Multica 文档、ADR、执行计划 |
| `docs/deprecated/` | 已废弃历史路线图和旧根 README |

注意：根目录仍混有 Understand Anything / Node workspace 相关内容，例如 `package.json`、`pnpm-*`、`understand-anything-plugin/`、`docs/superpowers/`。WindWave 的构建与质量门禁以 Rust workspace 为准。详细边界见 `docs/repository-boundaries.md`。

## 4. 当前已完成能力

### 4.1 主应用与执行链路

- `src/main.rs` 已启动 Bevy App，并注册 Agent、UI、Bevy adapter、SceneIndex、Vision、Screenshot、CommandProcessor。
- 启动时创建基础场景实体：`Player`、`Enemy_01`、`Enemy_02`。
- 用户输入由 `handle_agent_input` 处理，注入 `SceneIndexSceneBridge` 后分流：
  - HR/team 请求走 `dispatch_to_registered_agent`。
  - LLM 可用时走 `execute_with_llm`。
  - LLM 不可用时走 `handle_user_request` 规则回退。
- Director 产出的 JSON EngineCommand 会进入 `PendingCommands`，由 `CommandProcessorPlugin` 在后续帧应用到 Bevy World。
- UI 已能处理 approve/reject、undo/redo、delete selected、focus selected、LLM recheck 等操作。

### 4.2 agent-core

- Director、Planner、AgentRegistry、SkillRegistry、ToolRegistry 等主框架已存在。
- RuleBasedPlanner、LlmPlanner、DynamicPlanner、ReflectionEngine 已形成基础能力。
- 四层 Memory 命名已经统一为 Working、Episodic、Semantic、Procedural。
- 权限、回滚、审计、事件流、目标检查、Review、Hybrid Controller、Runtime Agent 等模块已存在。
- God 模块拆分已大幅推进，近期日志显示大文件已拆成多个子模块，维护性明显改善。

### 4.3 bevy-adapter

- `EngineCommand` DSL 已覆盖实体创建/删除、Transform、Sprite、Visibility、Prefab、Asset、Reparent 等编辑命令。
- `SceneBridge` 已由 Bevy adapter 实现，用于让 agent-core 间接操作 Bevy World。
- `SceneIndexCache`、全量 rebuild、增量更新、截图与 Vision provider 已接入。
- `CommandProcessorPlugin` 已集中处理命令批次，并为多种命令记录 undo 反向命令。

### 4.4 agent-ui

- Director Desk、Chat、Hierarchy、Inspector、Console、History、Runtime Agent Panel、Task Panel、Visual Understanding、Gizmo、Prefab Browser 等面板已存在。
- UI 能展示 Director pending approval，并能把 approve/reject 操作传回 DirectorRuntime。
- Visual Understanding 状态已经通过截图队列和 VGRC 桥接到 UI；2026-06-07 启用 Bevy `png` feature 后，运行态不再刷 PNG screenshot 保存错误。

### 4.5 multica-bridge

- 已具备协议类型、错误处理、WebSocket client、任务同步、Agent proxy、Skill adapter、message handler、daemon、本地 test server。
- 示例覆盖 `basic_usage`、`complete_task_flow`、`full_integration_test` 等本地流程。
- 当前主要缺口是“真实 Multica server 环境 smoke test”，不能只依赖 mock/test server。

## 5. 当前核心问题

### P0：项目边界与计划入口已建立

根 README 已修正为 WindWave，`package.json` 已增加说明，`docs/repository-boundaries.md` 已明确 Rust 主项目与 Understand Anything / Node workspace 的边界。后续仍要避免在新计划中引用过期文档。

已处理：
- 明确 `package.json` 与 `Cargo.toml` 的边界：见 `docs/repository-boundaries.md`。
- 将新入口加入 README 文档入口。
- `package.json` 增加说明，标明它是混居的 Understand Anything Node workspace。

后续维护：
- 后续所有 WindWave 计划以 `docs/windwave-version-plan.md` 和 `docs/windwave-execution-plan.md` 为准。
- 保持 `PROJECT_STATUS.md`、README、版本计划三者口径一致。

### P0：闭环执行还需要打穿

当前主链路已经能接收输入、生成计划或命令、推入 PendingCommands，但 v0.2 目标是更强的闭环：

`用户请求 -> plan -> act -> observe -> revise -> verify -> undo`

已处理：
- Direct 模式下 RuleBasedPlanner 生成的 plan 现在会先进入 `PlanManager`，再执行，避免直接执行时找不到 plan。
- “创建一个红色敌人放在右边”的规则解析已收敛到实体名 `敌人`、红色 Sprite、右侧 Transform 三个 Scene 步骤。
- SceneBridge 执行器已优先处理规则步骤，直接生成 create/color/position 命令。
- `CommandProcessor` 已支持同一批次内 `CreateEntity` 后继续对新实体执行颜色和 Transform 命令。
- 新增 `test_director_red_enemy_request_mutates_bevy_world_and_undoes`，目标覆盖 DirectorRuntime -> SceneBridge -> EngineCommand -> Bevy World -> SceneIndex -> undo。

已验证：
- `cargo test -p bevy-adapter test_director_red_enemy_request_mutates_bevy_world_and_undoes` 已于 2026-06-07 在原 iCloud 路径通过，覆盖 DirectorRuntime -> SceneBridge -> EngineCommand -> Bevy World -> SceneIndex -> undo。
- `cargo test -p agent-core test_failed_internal_plan_emits_revision_review` 已于 2026-06-07 在原 iCloud 路径通过，证明失败路径会产出 `ReviewCompleted(needs_revision)` 并写入修订计划。
- `cargo test --workspace` 已于 2026-06-07 在原 iCloud 路径通过。

仍需处理：
- 真人窗口验收“创建一个红色敌人放在右边”的可见场景变化。当前 Computer Use 未暴露 Bevy/winit 窗口，Codex 不能直接点击运行中的 editor UI。
- 继续深化失败后自动再执行修正计划的策略；当前已能生成修订/替代动作和可解释失败事件。
- 保持 CI 或普通本地路径复跑，避免 iCloud Drive 偶发文件锁/读取问题再次影响验收。

### P1：SceneIndex 删除实体残留风险已补修复

`SceneIndex::remove_entity(id)` 与 `reconcile_deletions` 已存在。2026-06-01 补充了一处增量系统修复：在 `changed.is_empty() && !force` 的 early return 之前先对账 live ids，避免纯删除操作一直等到 fallback interval 才清理索引。新增回归测试 `test_incremental_plugin_reconciles_deleted_entities_without_waiting_for_fallback` 覆盖插件路径。

已验证：
- `cargo test -p bevy-adapter test_incremental_plugin_reconciles_deleted_entities_without_waiting_for_fallback` 已于 2026-06-07 在原 iCloud 路径通过。
- `cargo test --workspace` 已于 2026-06-07 通过，覆盖该修复。

### P1：undo/redo 契约需要继续守住

`CommandProcessorPlugin` 已支持多种反向命令。2026-06-01 核对发现 `SpawnPrefab`、`SetSpriteTexture` 和 multi-command undo 已有专门回归测试；`LoadAsset` 当前按幂等低风险操作处理，不生成反向命令。剩余风险是 full QA 场景里要确认 UI undo/redo 与命令历史一致。

已确认：
- `SpawnPrefab` 成功后反向命令为 `DeleteEntity`，测试：`test_spawn_prefab_reverse_is_delete`。
- `SetSpriteTexture` 在原实体无 Sprite 时反向命令为 `RemoveComponent(Sprite)`，测试：`test_set_sprite_texture_reverse_removes_added_sprite`。
- `LoadAsset` 有意不生成反向命令；如后续需要热卸载，再引入 `RemoveAssetReference`。

仍需处理：
- 每个新增写入型 `EngineCommand` 必须明确 reverse contract。
- 在红色敌人闭环 QA 中确认 UI undo/redo 行为与命令历史保持一致。

### P1：HR approval flow 自动 smoke 已通过，仍需真实窗口点击验收

HR add/remove/fire 已向需要确认的方向推进，DirectorRuntime 也有 pending approval API。2026-06-07 `cargo test -p agent-edit ui_smoke_hr_request` 已通过，覆盖主应用系统链上的 HR 请求、Director Desk pending approval、approve/reject、desk 清除和 roster 变化/不变化。

需要处理：
- 真人在运行中的 Director Desk 点击 approve/reject，确认视觉 UI 与自动 smoke 一致。
- 如果要由 Codex 自动点击，需要先解决 Computer Use 无法发现 Bevy/winit 窗口的问题。

### P1：关键词判断仍分散

`KeywordMatcher` 已存在，但仍有多个模块保留散落关键词逻辑。

需要处理：
- 逐步把 Director、specialized agents、scene agent、reflection、rule planner 中的重复关键词判断收敛到统一策略。
- 保留必要的领域局部判断，但要写清楚边界。

### P2：真实 Multica 集成未验证

本地 mock/test server 流程较完整，但真实 Multica server 尚需 smoke test。

需要处理：
- 准备真实服务启动说明。
- 跑通任务分发、进度回传、技能执行、场景状态同步。
- 记录真实连接失败模式和恢复策略。

## 6. 推荐处理顺序

1. 先锁定 v0.2 最小闭环切片：`创建红色敌人放在右边 -> Bevy World 变化 -> SceneIndex 观察 -> undo`。
2. 同步修 SceneIndex 删除残留，因为它会直接污染 Agent 观察结果。
3. 逐项核验 undo/redo 反向命令，避免闭环执行后用户不能安全撤销。
4. 给 HR approval flow 补真人窗口验收记录，确认高风险操作在真实 UI 上可控。
5. 收敛散落关键词判断，减少规则、LLM、Vision 分流中的隐性分叉。
6. 再进入 Memory、Vision feedback loop、真实 Multica、多引擎 adapter 等扩展主题。

## 7. 待办清单

### 立即处理

- [x] 建立 v0.2.0 里程碑 issue 草案，范围只包含闭环执行与正确性修复：`docs/issues/v0.2.0-closed-loop-execution.md`。远程发布需先修复 `gh` token。
- [x] 为“创建红色敌人并放到右侧”补手动 QA 脚本：`docs/qa/red-enemy-closed-loop.md`。
- [x] 将 CI summary 中的硬编码测试数量改为引用真实 `cargo test --workspace` 输出，避免继续传播旧数字。
- [x] 明确根 `package.json` / `understand-anything-plugin/` 与 WindWave Rust workspace 的边界：`docs/repository-boundaries.md`。

### 下一批开发

- [ ] 将 ReAct 执行路径更直接地接入 `DirectorRuntime` 主请求流程。
- [ ] 完成失败后 plan revision 与 reflection 修正闭环。
- [x] 修复 SceneIndex 增量删除残留：代码已补 early-return 前删除对账，并新增插件级回归测试；2026-06-07 已在原 iCloud 路径复跑通过。
- [x] 补 HR add/remove/fire approval 自动 UI smoke；真人窗口点击验收仍待可访问窗口。
- [x] 继续验证 Prefab、SpriteTexture、Asset 相关 undo/redo：已有回归测试和 `LoadAsset` 幂等策略说明；仍需在 full QA 中验证 UI undo/redo。
- [ ] 抽干重复关键词判断，统一到 `KeywordMatcher` 或明确分层策略。

### 稳定性与体验

- [ ] 完善 MemoryInjector、episodic recall、layered context 和 token 预算控制。
- [ ] 增加 Visual feedback loop：operation -> screenshot -> vision verify -> revise。
- [ ] 统一 Runtime Agent、Task Panel、Director events 的状态同步。
- [ ] 跑真实 Multica server smoke test。
- [ ] 增加 edit_ops、layout、memory、event_stream、SceneIndex integration 的测试覆盖。

### 扩展能力

- [ ] CodeGraph、ImpactAnalyzer、CodeContextGenerator。
- [ ] Multi-selection tools、Transform Palette、Prefab 深化。
- [ ] Godot/Unreal adapter 最小可运行适配层。
- [ ] Narrative/game-design agents。
- [ ] 性能 benchmark 和大型场景压测。

## 8. 版本目标

| 版本 | 目标 | 关键验收 |
|---|---|---|
| v0.1.0 | 保留并稳定当前 AI Agent 游戏编辑器骨架 | build/test/clippy 通过，文档入口不误导 |
| v0.2.0 | 闭环执行与正确性修复 | 用户请求能真实改场景、SceneIndex 可观察、失败可修正、操作可 undo |
| v0.3.0 | 记忆、上下文与提示词生产化 | 记忆注入受预算约束，失败案例可影响后续计划 |
| v0.4.0 | 视觉反馈与混合编辑体验 | 截图/Vision 能验证结果并触发修正 |
| v0.5.0 | 生态集成与扩展能力 | 真实 Multica smoke、多 Agent 团队流、多引擎适配最小链路 |

## 9. 质量门禁

每个版本或里程碑结束至少运行：

```bash
cargo build
cargo test --workspace
cargo clippy --workspace -- -D warnings
```

功能门禁：

- 用户场景“创建一个红色敌人放在右边”必须产生可观察的 Bevy 场景变化。
- 所有写入型 `EngineCommand` 必须明确 undo/redo 行为。
- 高风险操作必须进入 permission/confirmation 流程。
- SceneIndex、UI、Director events 对同一次操作的状态描述必须一致。
- Multica 相关版本必须跑通 mock server 和真实 server 两条 smoke test。

## 10. 当前行动建议

本项目最值得马上推进的是 v0.2.0 的最小闭环，不建议先做更多新功能。闭环打穿后，Memory、Vision、Multica、多引擎 adapter 才会有稳定承载点。

建议下一步直接打开 `docs/windwave-execution-plan.md`，从 M1/M2 中挑一个小任务开始做；如果需要开 issue，则按本文件的 P0/P1 清单拆成独立、可验收的任务。
