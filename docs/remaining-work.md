# WindWave 剩余任务与待解决问题

> 更新日期：2026-06-07  
> 范围：当前 Rust workspace、核心文档、v0.2 闭环目标、近期 P0/P1 修复  
> 用途：作为后续开发和派单入口。更细的代码级步骤见 `docs/windwave-execution-plan.md`。

## 总体判断

WindWave 当前不是“从零补模块”的阶段，而是“把已有模块接成可验证闭环”的阶段。剩余任务可以分成四类：

1. 必须先完成的 v0.2 闭环执行。
2. 已写代码但还没跑完整验证的正确性修复。
3. 需要人工、外部凭据或真实环境的验收项。
4. v0.3+ 的体验、记忆、视觉、生态扩展。

当前最重要的验收场景仍是：

```text
创建一个红色敌人放在右边
```

目标链路：

```text
user request -> plan -> act -> observe -> revise -> verify -> undo
```

## P0：必须先处理

| 编号 | 任务 | 当前状态 | 下一步 | 验收标准 |
|---|---|---|---|---|
| P0-1 | 打穿红色敌人闭环执行 | 自动端到端测试通过；编辑器可启动；Computer Use 未暴露 Bevy/winit 窗口，仍需真人窗口点击验收 | 由用户或可访问窗口的本机工具手动输入"创建一个红色敌人放在右边"，观察场景变化 | 输入"创建一个红色敌人放在右边"后，场景真实新增红色敌人，位于右侧 |
| P0-2 | 自动验收升级为真实端到端 | ✅ 已完成 (2026-06-03) | `test_director_red_enemy_request_mutates_bevy_world_and_undoes` 在 iCloud 路径通过 | 测试能证明实体创建、SceneIndex 可见、undo 后实体消失 |
| P0-3 | 失败后修正闭环 | ✅ 已完成 (2026-06-03) | `test_failed_internal_plan_emits_revision_review` 在 iCloud 路径通过 | 执行失败时不是静默结束，而是生成修订计划或可解释失败 |
| P0-4 | 质量门禁真实跑通 | ✅ 2026-06-07 `cargo check`、`cargo test --workspace`、`cargo clippy --workspace -- -D warnings` 全部在 iCloud 路径通过；`cargo run --bin agent-edit` 可启动 | 保持 CI 或本地复跑 | 三条命令完整返回且通过 |
| P0-5 | GitHub issue 远程发布 | 被凭据阻塞 | 修复 `gh auth` 后发布 `docs/issues/v0.2.0-closed-loop-execution.md` | GitHub 上存在 v0.2.0 闭环 issue |

参考文档：

- `docs/qa/red-enemy-closed-loop.md`
- `docs/issues/v0.2.0-closed-loop-execution.md`
- `docs/windwave-execution-plan.md`

## P1：下一批正确性与集成问题

| 编号 | 任务 | 当前状态 | 下一步 | 验收标准 |
|---|---|---|---|---|
| P1-1 | SceneIndex 删除实体残留测试确认 | ✅ 已完成 (2026-06-03) | `test_incremental_plugin_reconciles_deleted_entities_without_waiting_for_fallback` 在 iCloud 路径通过 | 测试通过；删除实体不会残留到 fallback interval |
| P1-2 | undo/redo full UI 验收 | 自动回归通过；真实窗口点击仍受 Computer Use 窗口发现限制 | `test_multi_undo_chain`、Prefab/SpriteTexture reverse tests 已通过；真人窗口验收仍需补 | Undo 后新实体消失，Redo 后恢复或行为符合设计 |
| P1-3 | HR add/remove/fire approval UI smoke | ✅ 自动 UI smoke 通过；真实窗口点击仍受 Computer Use 窗口发现限制 | `cargo test -p agent-edit ui_smoke_hr_request` 已覆盖 approve/reject 清 desk 与 roster 变化 | roster 按 approve/reject 正确变化，危险操作不绕过确认 |
| P1-4 | 关键词判断收敛 | ✅ 已完成 (2026-06-03) | agent_dispatch.rs 改用 KeywordMatcher 统一方法；rule_based/keywords.rs 添加文档区分话题路由 vs 实体解析 | 新增/修改关键词只需改统一策略或有明确局部理由 |
| P1-5 | 新增 EngineCommand reverse contract 守卫 | ✅ 代码侧完成 (2026-06-03) | 13 个写入型 EngineCommand 变体：12 个有完整反向命令，1 个 (LoadAsset) 有意跳过 | 每个写入型 `EngineCommand` 都能说明是否可撤销以及如何撤销 |
| P1-6 | 测试数字口径清理 | ✅ 已完成 | 统一口径：不硬编码总数，以 2026-06-07 `cargo test --workspace` 完整输出或 CI 输出为准 | 不再出现 861/876/880/1031 互相冲突的"当前数字" |

已确认事项：

- `SpawnPrefab` 反向命令已有回归测试：`test_spawn_prefab_reverse_is_delete` ✅ 2026-06-03 通过。
- `SetSpriteTexture` 在新增 Sprite 情况下已有回归测试：`test_set_sprite_texture_reverse_removes_added_sprite` ✅ 2026-06-03 通过。
- `LoadAsset` 当前按幂等低风险操作处理，不生成反向命令；如需热卸载，应新增 `RemoveAssetReference`。
- `SceneIndex` 增量删除残留已修复，`test_incremental_plugin_reconciles_deleted_entities_without_waiting_for_fallback` ✅ 2026-06-03 通过。
- 红色敌人闭环已补 `test_director_red_enemy_request_mutates_bevy_world_and_undoes`，覆盖 DirectorRuntime -> SceneBridge -> EngineCommand -> Bevy World -> SceneIndex -> undo；✅ 2026-06-07 在 iCloud 路径通过。
- 失败后修正闭环已补 `test_failed_internal_plan_emits_revision_review`，覆盖缺失实体失败后产生 `needs_revision` 复盘事件并改写计划；✅ 2026-06-07 在 iCloud 路径通过。
- HR approval 自动 UI smoke `ui_smoke_hr_request_approve_clears_desk_and_updates_roster` / `ui_smoke_hr_request_reject_clears_desk_without_updating_roster` ✅ 2026-06-07 通过。
- 质量门禁：`cargo check`、`cargo test --workspace`、`cargo clippy --workspace -- -D warnings` ✅ 2026-06-07 在 iCloud 路径通过；`cargo run --bin agent-edit` 可启动。启用 Bevy `png` feature 后，运行态不再刷 `Cannot save screenshot, IO error: The image format Png is not supported`。

## P2：稳定性、体验与真实集成

| 编号 | 任务 | 当前状态 | 下一步 | 验收标准 |
|---|---|---|---|---|
| P2-1 | MemoryInjector / episodic recall / layered context | 未完成 | 明确注入预算、召回依据、压缩策略 | Prompt 注入稳定在预算内，失败经验能影响类似请求 |
| P2-2 | Visual feedback loop | 未完成 | 串起 operation -> screenshot -> vision verify -> revise | 视觉验证失败能触发修正或给出明确失败原因 |
| P2-3 | Runtime Agent / Task Panel / Director events 状态一致 | 未完成 | 对齐同一操作在 UI 面板中的状态来源 | 用户看到的任务、事件、Agent 状态不互相矛盾 |
| P2-4 | 真实 Multica server smoke test | 未完成 | 准备真实服务启动与连接步骤 | 真实环境能任务分发、进度回传、技能执行、场景状态同步 |
| P2-5 | 关键模块测试覆盖补强 | 未完成 | 补 edit_ops、layout、memory、event_stream、SceneIndex integration 测试 | 关键路径回归测试覆盖写入/撤销/事件/索引 |
| P2-6 | 性能 benchmark 与大型场景压测 | 未完成 | 明确场景规模、指标、基准命令 | 大型场景下 SceneIndex、UI、命令处理有可记录性能数据 |

## P3：后续扩展能力

| 编号 | 任务 | 当前状态 | 下一步 | 验收标准 |
|---|---|---|---|---|
| P3-1 | CodeGraph / ImpactAnalyzer / CodeContextGenerator | 规划中 | 先定义最小输入输出 | 能用于编辑前影响分析 |
| P3-2 | Multi-selection tools / Transform Palette | 规划中 | 接入 UI 与命令历史 | 多选变换可 undo/redo |
| P3-3 | Prefab / Asset / Hierarchy 深化 | 规划中 | 明确常用编辑命令与 UI 流程 | 常见资源与层级操作可视化、可撤销 |
| P3-4 | Godot / Unreal adapter | 规划中 | 先做最小读取/命令/回滚链路 | 至少一个非 Bevy adapter 跑通最小链路 |
| P3-5 | Narrative / game-design agents | 规划中 | 明确与 DirectorRuntime 的协作边界 | 游戏设计 Agent 能参与真实任务流 |

## 外部阻塞

| 阻塞项 | 影响 | 解决方式 |
|---|---|---|
| `gh` token 无效 | 无法远程创建 GitHub issue | 重新执行 `gh auth login -h github.com`，再发布 issue 草案 |
| 真实 Multica 环境未就绪 | 只能验证 mock/test server | 准备真实 server smoke 环境 |
| Computer Use 未暴露 Bevy/winit 窗口 | 无法由 Codex 直接点击运行中的 editor UI | 当前以自动 UI smoke 和运行态日志作为代理验收；最终人工窗口验收仍需用户或可访问该窗口的工具完成 |

## 建议执行顺序

1. **P0 (代码侧已完成，外部/手动项剩余)**: 剩余 P0-1 的真人窗口验收和 P0-5 的 `gh auth` 凭据修复/远程 issue 发布。

2. **P1 (代码侧全部完成 2026-06-03)**:
   - EngineCommand 反向命令 13/13 覆盖 ✅
   - HR 审批链路 14 tests pass ✅
   - 关键词收敛：agent_dispatch 统一使用 KeywordMatcher ✅
   - 测试数字口径统一 ~1022 tests ✅
   - 剩余需要真人窗口验收：undo/redo UI (P1-2)、HR approval UI (P1-3)

3. **P2**:
   - Rust 黄色修复收尾（EventQueue/scheduler/PermissionRequested 告警）
   - 失败闭环阐述：agent-core 新增导演手册和 trace 文档
   - 自动验收 CI：GitHub Actions workflow 配 check + test + clippy
   - Agent 角色规范文档：明确各角色的职责边界
   - 关键模块测试覆盖补强
   - 性能 benchmark 与大型场景压测

4. 再做 Memory、Vision、Multica 真实 server 和平台扩展。

## 完成定义

v0.2.0 可视为完成，至少需要：

- 红色敌人闭环场景通过。
- SceneIndex 删除残留回归测试通过。
- Undo/redo 在 UI QA 中通过。
- HR approval 自动 smoke 通过；真实窗口点击验收有记录或明确豁免。
- `cargo build`、`cargo test --workspace`、`cargo clippy --workspace -- -D warnings` 完整通过。
- README、`PROJECT_STATUS.md`、`docs/project-situation.md`、`docs/windwave-version-plan.md`、本文件状态一致。
