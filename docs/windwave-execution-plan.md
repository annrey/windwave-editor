# WindWave 剩余待办落地执行计划

> **For agentic workers:** REQUIRED SUB-SKILL: 用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 按任务逐条执行。步骤使用 `- [ ]` 复选框跟踪。
> 生成日期：2026-05-31 · 基准代码核对：`agent-edit` v0.1.0（Rust + Bevy 0.17）

**Goal:** 把 WindWave 从“能规划/能展示”推进到“能稳定闭环执行并可回滚”，并补齐已知正确性缺口，再为记忆/视觉/生态扩展留出可执行里程碑。

**Architecture:** 真实执行链路是 `src/main.rs::handle_agent_input` → `DirectorRuntime` → `ReActAgent`(`strategy.rs`) / 规则计划器；命令经 `SceneBridge` 产出 JSON `EngineCommand`，放入 `PendingCommands`，由 `bevy-adapter::CommandProcessorPlugin` 下一帧应用到 Bevy World 并记录 undo/redo。本计划在该既有链路上“补缺口 + 修正确性 + 加测试”，不另起炉灶。

**Tech Stack:** Rust, Bevy 0.17, bevy_egui, tokio, async-trait, serde_json。

**配套文档：** 版本目标与优先级见 [`docs/windwave-version-plan.md`](windwave-version-plan.md)；本文件是其 v0.2 及之后的可执行落地版本。

---

## 0. 与旧路线图的关键差异（必须先读）

旧 [`design/plans/remaining-tasks-roadmap.md`](../design/plans/remaining-tasks-roadmap.md) 生成于 2026-05-10，多处已被代码推进或本就判断错误。基于 2026-05-31 对当前代码的逐文件核对：

| 旧条目 | 旧描述 | 当前真实情况（已核对） |
|---|---|---|
| A1 | “在 `BaseAgent::run()` 接入 LLM，让 specialized agents 委托给 run()” | **方向错误**。`BaseAgent`(`agent.rs:217`) 是独立状态机，不持有 LLM/工具，`think/act/observe` 是私有默认实现，无法被子类覆写。真实 ReAct 循环是 `ReActAgent`(`strategy.rs:87`) + `director/react_runner/`，已持有 `Arc<dyn LlmClient>` 和工具注册表。 |
| A2/A3 | “新增 Plan-and-Solve / Reflection” | **已部分存在**。`director/plan_revision.rs`、`dynamic_planner`、`reflection_engine` 已接入 `plan_ops.rs:107-180` 和 `plan_executor.rs:282-329`。需要的是“接进 ReAct 主循环 + 验证 + 测试”。 |
| E4 | “SceneIndex 增量更新清理已删除实体” | `SceneIndex::remove_entity(id)`(`scene_index.rs:171-203`) **已实现但从未被增量路径调用**。只需在增量路径里做删除对账。 |
| E5 | “rename `team_context::AgentContext`” | **已完成**，`TeamAgentContext` 已存在。 |
| E2 | “`self_modifying_agent` 路径沙盒” | `crates/agent-core/src` 当前**无该文件**。任务过期或被移除，本计划不含该项（除非后续重新引入）。 |
| B1 | “新增 MemoryInjector” | **已存在**（`ReActAgent` 有 `memory_injector` 字段，`director` 调 `memory_injector.inject`）。需“验证 + 完善预算控制 + 测试”，非从零实现。 |
| C1 | “新增 L0-L3 分层上下文” | `LayeredContext` 已存在（`ReActAgent::with_layered_context`）。需“验证 + 补齐压缩策略 + 测试”。 |
| F2 | “新增多选择工具” | `selection_tools.rs` 已存在于 `agent-core`。需“验证 + 接入 UI”。 |

**结论：** 真正“从零写”的极少。本计划以“修缺口 + 接线 + 测试 + 验证既有实现”为主。每个里程碑结束都必须过质量门禁。

---

## 1. 里程碑总览

| 里程碑 | 主题 | 任务 | 可否立即落地 |
|---|---|---|---|
| **M1** | 正确性修复 | T1 HR 删除确认、T2 SpawnPrefab undo、T3 SetSpriteTexture undo、T4 LoadAsset undo 决策、T5 SceneIndex 删除对账 | 是，代码级完整 |
| **M2** | 闭环执行接入 | T6 打通 `llm_client`/`has_llm` 缺口、T7 让主路径真正跑 ReAct、T8 给 ReAct 注册真实工具、T9 端到端“红色敌人”场景测试、T10 把修订/反思接进 ReAct 循环 | 是，含必要的“先读后改”步骤 |
| **M3** | 关键词收敛 + 回归测试 | T11 散落关键词迁移到 `KeywordMatcher`、T12 AgentSnapshot 确定性回归、T13 补集成测试 | 是 |
| **M4+** | 记忆/视觉/生态 | 里程碑级提纲（v0.3-v0.5），到达时各自拆成独立计划 | 提纲，非代码级 |

## 质量门禁（每个里程碑结束都必须跑）

```bash
cargo build
cargo test --workspace
cargo clippy --workspace -- -D warnings
```

要求：0 error、0 新增 warning、已通过测试数不减少、关键路径无新增裸 `unwrap()`/`expect()`。

---

## M1：正确性修复（可立即落地，代码级完整）

### Task 1：HR Agent remove/fire 改为需确认（旧 E3）

**问题：** `hr_agent.rs:46-53` 的 remove/fire 分支直接 `self.roster.remove(target)` 删除成员；而 add/hire 分支只返回 `NeedUserInput`（不改 roster）。高风险删除必须对齐 add 的确认行为。

**Files:**
- Modify: `crates/agent-core/src/hr_agent.rs:46-53`（remove 分支）
- Modify: `crates/agent-core/src/hr_agent.rs:88-97`（`test_hr_remove`）

- [ ] **Step 1：改写 remove 分支为需确认（不再直接删除）**

把 `hr_agent.rs` 第 46-53 行的整个 `else if` 分支体替换为（保留 `else if ins.contains(...)` 条件行）：

```rust
        } else if ins.contains("remove") || ins.contains("fire") || ins.contains("移除") {
            let target = request.context.get("agent_id").and_then(|v| v.as_u64()).unwrap_or(0);
            // 高风险：删除前必须确认，这里不直接改动 roster（对齐 add/hire 行为）。
            match self.roster.find(target) {
                Some(member) => Ok(AgentResponse {
                    agent_id: self.id,
                    agent_name: self.name.clone(),
                    result: AgentResultKind::NeedUserInput {
                        question: format!("Remove agent '{}' (id {})?", member.name, target),
                    },
                    events: vec![],
                }),
                None => Ok(AgentResponse {
                    agent_id: self.id,
                    agent_name: self.name.clone(),
                    result: AgentResultKind::Failed {
                        reason: format!("Agent with ID {} not found", target),
                    },
                    events: vec![],
                }),
            }
        }
```

- [ ] **Step 2：更新 `test_hr_remove`，断言不再立即删除**

把 `hr_agent.rs:88-97` 替换为：

```rust
    #[tokio::test]
    async fn test_hr_remove() {
        let mut roster = TeamRoster::new();
        let aid = roster.add("X", TeamRole::Executor, vec![]);
        let mut hr = HrAgent::new(AgentId(200), roster);
        let req = AgentRequest { task_id: Some("h3".into()), instruction: "remove".into(), context: json!({"agent_id": aid}) };
        let resp = hr.handle(req).await.unwrap();
        // 现在 remove 只请求确认，不应直接删除。
        assert!(matches!(resp.result, AgentResultKind::NeedUserInput { .. }));
        assert!(hr.team_size() == 1);
    }
```

- [ ] **Step 3：运行测试**

Run: `cargo test -p agent-core hr_agent`
Expected: `test_hr_add_needs_confirm`、`test_hr_remove`、`test_hr_list_team` 全部 PASS。

- [ ] **Step 4：提交**

```bash
git add crates/agent-core/src/hr_agent.rs
git commit -m "fix(hr-agent): require confirmation before remove/fire instead of immediate delete"
```

**范围说明（诚实声明）：** 本任务只把“立即破坏性删除”改为“请求确认”，与 add/hire 一致。**“用户确认后真正执行删除/添加”是另一个独立缺口**（add 路径同样没有 apply-on-confirm 的可见实现），应由 `DirectorRuntime` 的 pending-approval 处理统一补齐，记入 M2 后续或单独 issue，不在本任务内扩张。

---

### Task 2：SpawnPrefab 的 undo 反向命令（旧 E7 之一，最简单）

**问题：** `command_processor.rs:296-312` 的通用 `_` 分支只为 `ReparentChildren` 生成反向命令。`SpawnPrefab` 落入此分支、被应用但无反向命令，导致整批 undo 丢失。`cmd_spawn_prefab`(`commands.rs:559-590`) 在成功时返回 `entity_id: Some(agent_id.0)`，可直接镜像 `CreateEntity` 的反向逻辑。

**Files:**
- Modify: `crates/bevy-adapter/src/command_processor.rs`（在 `process_command_batch` 的 `match cmd` 中、通用 `_` 分支之前新增显式分支）
- Test: `crates/bevy-adapter/tests/adapter_core_tests.rs`

- [ ] **Step 1：新增 SpawnPrefab 显式分支**

在 `process_command_batch` 的 `match cmd { ... }` 中，于 `_ => { ... }`（约 296 行）**之前**插入（镜像 `CreateEntity` 分支 129-138）：

```rust
            EngineCommand::SpawnPrefab { .. } => {
                let result = world.resource_scope(|w, mut adapter: Mut<BevyAdapter>| {
                    adapter.apply_engine_command(cmd.clone(), w)
                });
                match result {
                    Ok(r) if r.success => {
                        applied += 1;
                        // 生成的实体 id 由 cmd_spawn_prefab 通过 entity_id 返回；反向即删除它。
                        if let Some(eid) = r.entity_id {
                            reverse_commands.push(EngineCommand::DeleteEntity { entity_id: eid });
                        }
                    }
                    _ => { failed += 1; }
                }
            }
```

- [ ] **Step 2：加单元测试（验证反向命令是 DeleteEntity）**

在 `crates/bevy-adapter/tests/adapter_core_tests.rs` 末尾新增：

```rust
#[test]
fn test_spawn_prefab_reverse_is_delete() {
    use bevy::prelude::*;
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(bevy::asset::AssetPlugin::default());
    app.init_resource::<bevy_adapter::BevyAdapter>();
    app.init_resource::<bevy_adapter::CommandHistory>();
    app.init_resource::<bevy_adapter::PendingCommands>();

    app.world_mut().resource_mut::<bevy_adapter::PendingCommands>().commands.push(
        EngineCommand::SpawnPrefab { asset_handle: "prefabs/enemy".into(), transform: Some([5.0, 0.0, 0.0]) },
    );
    // 推进一帧让 CommandProcessorPlugin 处理（若未加该插件，则直接调用 process 系统）。
    app.add_plugins(bevy_adapter::CommandProcessorPlugin);
    app.update();

    let history = app.world().resource::<bevy_adapter::CommandHistory>();
    assert_eq!(history.undo_stack.len(), 1, "SpawnPrefab 应产生一条 undo 记录");
    let (_forward, reverse) = &history.undo_stack[0];
    assert!(matches!(reverse.as_slice(), [EngineCommand::DeleteEntity { .. }]));
}
```

> 注：先确认 `CommandProcessorPlugin`、`CommandHistory`、`PendingCommands`、`BevyAdapter` 的真实导出路径（见 `crates/bevy-adapter/src/lib.rs:27` 附近）。若 `AssetServer` 在 `MinimalPlugins` 下不可用，按现有测试惯例补 `AssetPlugin`（上方已加）。

- [ ] **Step 3：运行测试**

Run: `cargo test -p bevy-adapter test_spawn_prefab_reverse_is_delete`
Expected: PASS（undo_stack 长度为 1，反向为 `DeleteEntity`）。

- [ ] **Step 4：提交**

```bash
git add crates/bevy-adapter/src/command_processor.rs crates/bevy-adapter/tests/adapter_core_tests.rs
git commit -m "feat(undo): add reverse (DeleteEntity) for SpawnPrefab commands"
```

---

### Task 3：SetSpriteTexture 的 undo 反向命令（旧 E7 之一，中等）

**问题：** `cmd_set_sprite_texture`(`commands.rs:522-557`) 有两种情况：(a) 实体已有 `Sprite` → 覆盖 `sprite.image`（旧 handle 丢失）；(b) 实体无 `Sprite` → 插入新 `Sprite`。当前落入通用 `_` 分支、无反向。需在应用**之前**捕获 `Sprite` 是否已存在。

**已知约束：** `SetSpriteTexture` 只接受 `asset_handle: String`，无法用现有命令把一个已存在的 `Handle<Image>` 还原成原始字符串。因此“覆盖已有纹理”的情况无法忠实还原原纹理。可忠实还原的是“原本没有 Sprite”的情况——反向用 `RemoveComponent { component_type: "Sprite" }`（该变体已存在，见 `adapter_core_tests.rs:675`）。

**Files:**
- Read first: `crates/bevy-adapter/src/command_processor.rs:166-187`（`SetSpriteColor` 分支，复制其“映射 entity + 应用前捕获 `world.get::<Sprite>(be)`”的精确写法）
- Modify: `crates/bevy-adapter/src/command_processor.rs`（新增 `SetSpriteTexture` 显式分支）
- Test: `crates/bevy-adapter/tests/adapter_core_tests.rs`

- [ ] **Step 1：读取 SetSpriteColor 分支，确认 entity 映射与预捕获写法**

Run: 阅读 `crates/bevy-adapter/src/command_processor.rs:166-187`。
确认：如何从 `entity_id`(u64) 经 `id_to_bevy` 拿到 Bevy `Entity`，以及如何用 `world.get::<Sprite>(be).cloned()` 在应用前取旧状态。下一步严格复用该惯用法。

- [ ] **Step 2：新增 SetSpriteTexture 显式分支**

在通用 `_` 分支之前插入（`be` 的获取方式以 Step 1 读到的 `SetSpriteColor` 写法为准）：

```rust
            EngineCommand::SetSpriteTexture { entity_id, .. } => {
                // 应用前捕获：实体当前是否已有 Sprite。
                let be = id_to_bevy.get(entity_id).copied();
                let had_sprite = be
                    .map(|e| world.get::<bevy::sprite::Sprite>(e).is_some())
                    .unwrap_or(false);

                let result = world.resource_scope(|w, mut adapter: Mut<BevyAdapter>| {
                    adapter.apply_engine_command(cmd.clone(), w)
                });
                match result {
                    Ok(r) if r.success => {
                        applied += 1;
                        if !had_sprite {
                            // 之前没有 Sprite：忠实反向 = 移除我们新插入的 Sprite。
                            reverse_commands.push(EngineCommand::RemoveComponent {
                                entity_id: *entity_id,
                                component_type: "Sprite".into(),
                            });
                        } else {
                            // 之前已有 Sprite 且纹理被覆盖：当前命令集无法用字符串还原旧 Handle<Image>，
                            // 暂不生成反向（记日志）。还原旧纹理需后续引入“按 handle 还原”的能力。
                            log::warn!(
                                "SetSpriteTexture on entity {} overwrote an existing texture; no faithful reverse available yet",
                                entity_id
                            );
                        }
                    }
                    _ => { failed += 1; }
                }
            }
```

- [ ] **Step 3：加单元测试（无 Sprite → 反向为移除 Sprite）**

```rust
#[test]
fn test_set_sprite_texture_reverse_removes_added_sprite() {
    use bevy::prelude::*;
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(bevy::asset::AssetPlugin::default());
    app.init_resource::<bevy_adapter::BevyAdapter>();
    app.init_resource::<bevy_adapter::CommandHistory>();
    app.init_resource::<bevy_adapter::PendingCommands>();
    app.add_plugins(bevy_adapter::CommandProcessorPlugin);

    // 先创建一个没有 Sprite 的实体并在 adapter 注册，拿到 agent id。
    let e = app.world_mut().spawn(Name::new("Target")).id();
    let aid = app.world_mut().resource_mut::<bevy_adapter::BevyAdapter>().register_entity(e).0;

    app.world_mut().resource_mut::<bevy_adapter::PendingCommands>().commands.push(
        EngineCommand::SetSpriteTexture { entity_id: aid, asset_handle: "textures/p.png".into() },
    );
    app.update();

    let history = app.world().resource::<bevy_adapter::CommandHistory>();
    assert_eq!(history.undo_stack.len(), 1);
    let (_f, reverse) = &history.undo_stack[0];
    assert!(matches!(
        reverse.as_slice(),
        [EngineCommand::RemoveComponent { component_type, .. }] if component_type == "Sprite"
    ));
}
```

- [ ] **Step 4：运行测试 + 提交**

Run: `cargo test -p bevy-adapter test_set_sprite_texture_reverse`
Expected: PASS。

```bash
git add crates/bevy-adapter/src/command_processor.rs crates/bevy-adapter/tests/adapter_core_tests.rs
git commit -m "feat(undo): reverse SetSpriteTexture when it adds a new Sprite; log when overwriting"
```

**已知限制（写入计划）：** “覆盖已有纹理”的忠实 undo 需要后续能力（按 `Handle<Image>` 或缓存原始 `asset_handle` 字符串还原）。记为 v0.4 视觉批次的跟进项。

---

### Task 4：LoadAsset 的 undo 决策（旧 E7 之一，最难）

**问题：** `cmd_load_asset`(`commands.rs:481-520`) 只向 `self.asset_references`(HashMap) 插入一条记录并触发异步加载，不改任何实体（`entity_id: None`），且计算出的 `handle_id` 未通过 `EngineCommandResult` 暴露。当前无 `UnloadAsset`/`RemoveAssetReference` 变体可作反向。

**决策（推荐方案 A，v0.2 落地）：** 把 `LoadAsset` 标记为“有意不可撤销”。它本质幂等、低风险（注册资产引用 + 触发异步加载，重复加载只是覆盖同 key）。为其强加反向属于范围蔓延。

**备选方案 B（推迟）：** 新增 `EngineCommand::RemoveAssetReference { handle: String }`，并让 `cmd_load_asset` 通过 `EngineCommandResult.entity_id` 之外的途径回传 handle（需扩展结果结构）。成本高、收益低，推迟到确有“资源热卸载”需求时再做。

**Files:**
- Modify: `crates/bevy-adapter/src/command_processor.rs`（通用 `_` 分支内为 `LoadAsset` 加注释说明，或新增显式分支只 `applied += 1` 并注释）
- Modify: `docs/windwave-version-plan.md`（在风险/约束处登记“LoadAsset 暂不可撤销”）

- [ ] **Step 1：在通用 `_` 分支为 LoadAsset 增加显式注释分支（表达意图）**

在 `_` 之前插入：

```rust
            EngineCommand::LoadAsset { .. } => {
                // 有意不可撤销：仅注册资产引用 + 触发异步加载，幂等且低风险。
                // 若将来需要资源热卸载，再引入 RemoveAssetReference 变体（见 execution-plan Task 4 方案 B）。
                let result = world.resource_scope(|w, mut adapter: Mut<BevyAdapter>| {
                    adapter.apply_engine_command(cmd.clone(), w)
                });
                match result {
                    Ok(r) if r.success => { applied += 1; }
                    _ => { failed += 1; }
                }
            }
```

- [ ] **Step 2：登记约束到版本规划**

在 [`docs/windwave-version-plan.md`](windwave-version-plan.md) 的“验收门禁 / 写入型命令 undo”附近补一行：`LoadAsset 有意不可撤销（幂等、低风险），如需热卸载再引入 RemoveAssetReference。`

- [ ] **Step 3：运行门禁 + 提交**

Run: `cargo build -p bevy-adapter && cargo clippy -p bevy-adapter -- -D warnings`
Expected: PASS。

```bash
git add crates/bevy-adapter/src/command_processor.rs docs/windwave-version-plan.md
git commit -m "docs(undo): mark LoadAsset as intentionally non-undoable with rationale"
```

---

### Task 5：SceneIndex 删除对账，清理 ghost 实体（旧 E4）

**问题：** `incremental_update`(`integration/scene_index.rs:55-125`) 只调用 `add_entity`，从不删除。被删实体的旧条目要等到每 ~300 帧的强制全量 rebuild 才消失，期间 Agent 会基于不存在的实体规划。`SceneIndex::remove_entity(id: u64)`(`scene_index.rs:171-203`) 已实现但未被调用。

**方案选择：** 用**“活实体对账”**（diff live ids）而非 `RemovedComponents`。原因：被 despawn 的实体经 `adapter.get_agent_id` 可能已因 `unregister_entity` 清空 `reverse_map` 而返回 `None`（见 `adapter/mod.rs:104-108`），且 `unwrap_or(0)` 有 id=0 碰撞风险。对账法只信任“当前仍存活且能解析出 id>0”的集合，鲁棒且可纯函数化测试。

**Files:**
- Modify: `crates/bevy-adapter/src/scene_index.rs`（在 `remove_entity` 之后新增纯函数方法 `reconcile_deletions`）
- Modify: `crates/bevy-adapter/src/integration/scene_index.rs:55-125`（`incremental_update` 末尾调用对账）
- Test: `crates/bevy-adapter/src/scene_index.rs` 的 `#[cfg(test)]` 模块

- [ ] **Step 1：在 `SceneIndex` 上新增可单测的纯对账方法**

在 `crates/bevy-adapter/src/scene_index.rs` 的 `impl SceneIndex` 内、`remove_entity`(171-203) 之后新增：

```rust
    /// 删除对账：移除所有“已不在存活集合中”的索引条目（清理 ghost）。
    /// `live_ids` 应只包含当前 ECS 中仍存在、且 id != 0 的 agent id。
    pub fn reconcile_deletions(&mut self, live_ids: &std::collections::HashSet<u64>) {
        let indexed_ids: Vec<u64> = self.entities_by_name.values().copied().collect();
        for id in indexed_ids {
            if id != 0 && !live_ids.contains(&id) {
                self.remove_entity(id);
            }
        }
    }
```

- [ ] **Step 2：先写失败测试（对账应删 ghost、保留存活）**

在该文件的 `#[cfg(test)] mod tests` 中新增：

```rust
    #[test]
    fn test_reconcile_deletions_removes_ghosts() {
        let mut idx = SceneIndex::new();
        idx.add_entity("Player".into(), 1, vec![]);
        idx.add_entity("Enemy".into(), 2, vec![]);
        assert_eq!(idx.entities_by_name.len(), 2);

        // 只有 id=1 仍存活，id=2 已被删除。
        let live: std::collections::HashSet<u64> = [1u64].into_iter().collect();
        idx.reconcile_deletions(&live);

        assert!(idx.entities_by_name.contains_key("Player"));
        assert!(!idx.entities_by_name.contains_key("Enemy"));
        assert_eq!(idx.entities_by_name.len(), 1);
        // id=2 不应残留在任何组件索引或 root 列表中。
        assert!(idx.root_entities.iter().all(|n| n.id != 2));
    }
```

- [ ] **Step 3：运行，确认通过**

Run: `cargo test -p bevy-adapter reconcile_deletions`
Expected: PASS（`remove_entity` 已能清三处存储）。

- [ ] **Step 4：在增量更新末尾接入对账**

在 `crates/bevy-adapter/src/integration/scene_index.rs` 的 `incremental_update` 中，`for (entity, ...) in all_entities.iter() { ... }` 循环**结束之后**、方法闭合大括号之前插入：

```rust
        // 非强制路径下做删除对账：drop 掉已不在存活 ECS 中的索引条目。
        // force 路径已经整索引重建，无需对账。
        if !force {
            let live_ids: std::collections::HashSet<u64> = all_entities
                .iter()
                .filter_map(|(e, ..)| adapter.get_agent_id(e).map(|id| id.0))
                .filter(|id| *id != 0)
                .collect();
            self.0.reconcile_deletions(&live_ids);
        }
```

- [ ] **Step 5：运行门禁**

Run: `cargo build -p bevy-adapter && cargo test -p bevy-adapter && cargo clippy -p bevy-adapter -- -D warnings`
Expected: 全 PASS，无新 warning。

- [ ] **Step 6：提交**

```bash
git add crates/bevy-adapter/src/scene_index.rs crates/bevy-adapter/src/integration/scene_index.rs
git commit -m "fix(scene-index): reconcile deletions in incremental update to clear ghost entities"
```

---

## M2：闭环执行接入（含必要的“先读后改”步骤）

> M2 的目标是让 `src/main.rs::handle_agent_input` 选中的路径**真正驱动已存在的 ReAct 循环**，而不是停在占位实现。以下任务含明确的“先读取确认”步骤——因为 `executor.rs:26-147`、`has_llm`/`has_react_agent` 的函数体未在调研中逐字捕获，落地前必须按给出的精确行号读取确认。

### Task 6：打通 `llm_client`/`has_llm` 缺口

**已知：** `DirectorRuntime::new`(`director/mod.rs:88-201`) 自动建 LLM 客户端并创建 `ReActAgent`，但在 133-138 行存储为 `(None, Some(react))`——即 `llm_client` 字段留空，只有 `react_agent` 持有客户端。`src/main.rs::handle_agent_input` 用 `director.0.has_llm()` 决定是否走 `execute_with_llm()`；若 `has_llm()` 看的是 `llm_client` 字段，则永远为 false，主路径退回规则计划器，ReAct 永不触发。

**Files:**
- Read first: `crates/agent-core/src/director/mod.rs:88-201` 与 `grep` `fn has_llm`、`fn has_react_agent`
- Modify: `crates/agent-core/src/director/mod.rs`（择一：填充 `llm_client` 字段，或让 `has_llm` 反映 `react_agent` 在场）

- [ ] **Step 1：读取并确认现状**

Run（用 Grep 工具，非 shell）：搜索 `crates/agent-core/src/director/` 下 `fn has_llm`、`fn has_react_agent`、`react_agent`、`llm_client`。
读取 `director/mod.rs:88-201`，确认 133-138 行为何留 `llm_client = None`（调研指出此处有注释说明）。

- [ ] **Step 2：选择并实施修复（推荐：让能力判断反映真实在场）**

推荐做法：保持 `react_agent` 持有客户端，让 `has_llm()` 返回 `self.llm_client.is_some() || self.react_agent.is_some()`，并确保 `handle_user_request_async`/`execute_with_llm` 在 `has_react_agent()` 为真时进入 ReAct 分支。

> 若 Step 1 发现 `has_llm` 已包含 `react_agent`，则本步骤改为“仅补测试”，不改逻辑。给出最终态由 Step 1 的真实代码决定，**不要凭空改写未读到的函数体**。

- [ ] **Step 3：加测试 + 门禁 + 提交**

新增 `agent-core` 测试：构造一个带 mock LLM 的 `DirectorRuntime`（见 Task 9 的 mock），断言 `has_llm()`/`has_react_agent()` 为真。

```bash
cargo test -p agent-core director
git add crates/agent-core/src/director/
git commit -m "fix(director): has_llm reflects an injected ReAct agent so the main path enters the LLM loop"
```

---

### Task 7：让主路径真正运行 ReAct 循环（而非占位）

**已知：** `execute_with_llm`(`executor.rs:26-147`) 在有 tokio runtime 时只 spawn 一个发出 “ReActAgent is thinking...” 的占位任务（47-53 行），并不真正跑循环；真正的循环是 `execute_with_react`(`react_runner/react_loop.rs:14`)，由 `handle_user_request_async`(`executor.rs:429-458`) 在 `has_react_agent()` 时调用。

**Files:**
- Read first: `crates/agent-core/src/director/react_runner/executor.rs:26-147`
- Modify: `executor.rs`（把占位替换为对 `handle_user_request_async`/`execute_with_react` 的实际 `block_on`，复用 79-108 行已有的 current-thread runtime 模式）

- [ ] **Step 1：读取确认 `execute_with_llm` 与 `handle_user_request_async` 全文**

读取 `executor.rs:26-147` 与 `executor.rs:429-458`，确认现有 runtime 获取/构建方式（47-53 的 spawn 占位、79-108 的 current-thread runtime hack）。

- [ ] **Step 2：用真实调用替换占位**

把 47-53 行的占位 spawn 改为：在已有 runtime 上 `block_on(self.execute_with_react(request_text))`（或经 `handle_user_request_async`），把返回的 `Vec<EditorEvent>` 落到 `self.events`/event_bus，失败则退回 `fallback_engine.execute`。最终代码以 Step 1 读到的签名为准（注意 `execute_with_react` 返回 `Result<Vec<EditorEvent>, String>`，需处理 Err 分支）。

> 这是 v0.2 的“心脏”改动。务必小步：先让 tokio-runtime 分支调用真实循环，保留无 runtime 分支的现有逻辑，避免一次改两条路径。

- [ ] **Step 3：手动冒烟 + 提交**

Run: `cargo run --bin agent-edit`，在聊天里输入“创建一个红色敌人放在右边”，观察是否产生真实计划/命令而非仅 “thinking...” 文案。

```bash
git add crates/agent-core/src/director/react_runner/executor.rs
git commit -m "feat(director): drive the real ReAct loop from execute_with_llm instead of a placeholder"
```

---

### Task 8：给 ReAct 的工具注册表注册真实工具

**已知：** `director/mod.rs:102` 为 `ReActAgent` 创建的 `ToolRegistry` 是**空的**；而场景/代码工具是另行注册到别处的（如 `specialized_agents.rs:35` 调 `code_tools::register_code_tools`）。空注册表会让 ReAct 的 `act()` 无工具可用。

**Files:**
- Read first: `crates/agent-core/src/director/mod.rs:88-138`、`grep` `register_.*_tools`、`create_react_agent`
- Modify: `director/mod.rs`（在 102 行创建的注册表里注册真实场景/文件/代码工具，再传给 `create_react_agent`）

- [ ] **Step 1：清点可注册的工具入口**

Run（Grep）：在 `crates/agent-core/src` 搜索 `pub fn register_` 与 `register_code_tools`、`register_scene_tools`、`register_file_tools` 等，列出可用注册函数及签名。

- [ ] **Step 2：把真实工具注册进 ReAct 的 ToolRegistry**

在 `director/mod.rs` 创建该注册表处（约 102 行）改为：

```rust
let mut react_tool_registry = crate::tool::ToolRegistry::new();
crate::scene_tools::register_scene_tools(&mut react_tool_registry);
crate::code_tools::register_code_tools(&mut react_tool_registry);
// 其余真实工具按 Step 1 清点结果补齐。
```

再把它（按 `create_react_agent` 需要的 `Arc<Mutex<ToolRegistry>>` 形态）传入 `create_react_agent(...)`。具体包装以 `strategy.rs:529` 的签名为准。

- [ ] **Step 3：测试 ReAct 能列出/执行工具 + 提交**

新增测试：构造带 mock LLM 的 `ReActAgent`，让 mock 返回一个 `Action`（如 `create_entity`），断言对应 `EngineCommand`/`ToolResult` 产生。

```bash
cargo test -p agent-core react
git add crates/agent-core/src/director/mod.rs
git commit -m "feat(react): register real scene/code tools into the ReAct agent tool registry"
```

---

### Task 9：端到端验收场景测试——“创建一个红色敌人放在右边”

**目标：** 把版本规划里的核心验收场景固化为自动化测试 + 手动 QA 脚本。用 mock `LlmClient` 返回脚本化的 ReAct 步骤（think → action: create_entity → action: set_color red → action: set_position right → final answer），断言产出 `EngineCommand` 序列，且 SceneIndex 出现新实体、可 undo。

**Files:**
- Create: `crates/agent-core/tests/scenario_red_enemy.rs`
- Reference: `LlmClient` trait(`llm/types.rs:241-266`)、`LlmResponse`/`ToolCall{name,arguments}`(`llm/types.rs:203-215`)

- [ ] **Step 1：实现脚本化 mock LlmClient**

```rust
use agent_core::llm::{LlmClient, LlmRequest, LlmResponse, LlmError, LlmProvider, ToolCall, TokenUsage};

struct ScriptedLlm { steps: std::sync::Mutex<std::vec::IntoIter<LlmResponse>> }

#[async_trait::async_trait]
impl LlmClient for ScriptedLlm {
    async fn chat(&self, _req: LlmRequest) -> Result<LlmResponse, LlmError> {
        self.steps.lock().unwrap().next().ok_or(LlmError::InvalidResponse("no more scripted steps".into()))
    }
    async fn chat_stream(&self, req: LlmRequest, _on: agent_core::llm::StreamCallback) -> Result<LlmResponse, LlmError> {
        self.chat(req).await
    }
    fn is_ready(&self) -> bool { true }
    fn provider(&self) -> LlmProvider { LlmProvider::OpenAi }
}
```

> 注：`LlmResponse{ content, tool_calls, usage }`、`ToolCall{ name, arguments }`、`LlmProvider` 的精确变体名以 `llm/types.rs` 为准（落地前快速读取确认枚举名，如 `LlmProvider::OpenAi`）。

- [ ] **Step 2：脚本化三步动作并断言命令产出**

构造 `ScriptedLlm`，依次返回：创建实体、设红色、设到右侧坐标、最终回答。把它注入 `DirectorRuntime`（或直接驱动 `ReActAgent::run`），断言：
- 产出包含 `CreateEntity` / `SetSpriteColor{ rgba: [1,0,0,1] }` / `SetTransform`(x>0) 的 `EngineCommand`；
- 应用后 SceneIndex `total_count` 增加；
- `CommandHistory.undo_stack` 非空，可还原。

- [ ] **Step 3：写手动 QA 脚本**

在测试文件顶部用注释写明手动验证步骤：`cargo run --bin agent-edit` → 输入该 prompt → 期望屏幕出现红色敌人于右侧 → 点 undo 复原。

- [ ] **Step 4：运行 + 提交**

```bash
cargo test -p agent-core --test scenario_red_enemy
git add crates/agent-core/tests/scenario_red_enemy.rs
git commit -m "test(scenario): end-to-end 'red enemy on the right' through the ReAct loop"
```

---

### Task 10：把修订/反思接进 ReAct 循环本身

**已知：** 反思（`reflection_engine`：`classify_error`/`generate_reflection`/`generate_alternative_strategy`）已在 `react_tools.rs:66-81` 的工具失败处调用；计划修订（`plan_revision::generate_alternative_step`）在 `plan_executor.rs:325` 使用。但调研指出修订/反思在**规则计划器路径**最强，ReAct 主循环里的接入需要核验与补强。

**Files:**
- Read first: `crates/agent-core/src/director/react_runner/react_loop.rs:14-160`、`react_tools.rs:60-90`
- Modify（按核验结果）：`react_loop.rs` / `react_tools.rs`

- [ ] **Step 1：核验 ReAct 失败路径是否触发反思并影响下一步**

读取 `react_loop.rs` 的 `execute_with_react` 主循环与 `react_tools.rs` 的工具执行；确认工具失败时是否：(a) 调 `reflection_engine`，(b) 把反思/替代策略喂回下一轮 `step` 的输入。

- [ ] **Step 2：补强缺口（仅在 Step 1 发现断点时）**

若失败后未把替代策略反馈给下一轮，则在 observation 注入 `generate_alternative_strategy(...)` 的结果，使下一轮 `think()` 能看到。代码以 Step 1 读到的循环结构为准。

- [ ] **Step 3：失败→反思→重试 测试**

用 Task 9 的 mock：第一轮返回会失败的 action（如对不存在实体设色），断言循环产生反思并在后续轮尝试替代策略，最终成功或明确报告失败。

- [ ] **Step 4：门禁 + 提交**

```bash
cargo test -p agent-core react
git add crates/agent-core/src/director/react_runner/
git commit -m "feat(react): feed reflection/alternative strategy back into the loop on tool failure"
```

**M2 完成定义：** `cargo test --workspace` 通过；手动跑“红色敌人”场景能看到真实场景变化并可 undo；ReAct 失败时有可解释的反思与重试。完成后更新 [`docs/windwave-version-plan.md`](windwave-version-plan.md) 的 v0.2 勾稽与 `PROJECT_STATUS.md` 的真实测试数。

---

## M3：关键词收敛 + 回归测试

### Task 11：散落关键词判断迁移到 `KeywordMatcher`（旧 E1，已部分完成）

**已知：** `KeywordMatcher`(`keyword_matcher.rs`) 已存在且被 `router.rs` 使用。仍散落 keyword `.contains(...)` 的位置：`director/mod.rs`(`lookup_skill_for_step`)、`specialized_agents.rs:91-99`(CodeAgent)、`director/agent_dispatch.rs:20-30`、`scene_agent.rs`、`reflection_engine`、`planner/rule_based/keywords.rs`、`prompt/context.rs`、`layered_context_builder.rs`、`director/plan_revision.rs`、`memory_injector/compressor.rs`。

**Files（逐站点迁移，每站点一次提交）:**
- Modify: 上述各文件；统一调用 `KeywordMatcher::classify` / `extract_entities`
- Test: `crates/agent-core/src/keyword_matcher.rs` 测试模块补分类用例

- [ ] **Step 1：核对 `KeywordMatcher` 现有公共 API**（`classify`、`extract_entities`、各 `*_KW` 常量、`assess_risk` 等）。若缺少 `agent_dispatch` 需要的能力分类，先在 `keyword_matcher.rs` 扩展并加测试。
- [ ] **Step 2：先迁移 `agent_dispatch.rs:20-30`**（能力路由），用 `KeywordMatcher` 替换内联 `.contains`，加断言“相同输入 → 相同 capability”的测试，提交。
- [ ] **Step 3：再迁移 `specialized_agents.rs:91-99`**（CodeAgent 生成/分析判断），提交。
- [ ] **Step 4：逐个迁移其余站点**，每个站点：替换 → `cargo test -p agent-core` → 单独提交（保持小步、可回滚）。
- [ ] **Step 5：清点收尾**，Grep 残留 `.contains("创建"|"删除"|"create"|...)`，确认只剩 `KeywordMatcher` 内部定义。

提交信息样式：`refactor(keywords): route <site> through KeywordMatcher`。

### Task 12：AgentSnapshot 确定性回归（旧 C3）

- [ ] 用 Task 9 的 mock LLM 固定输入；保存“请求 + 工具调用序列 + 最终 EngineCommand”为快照；回放比对。Files：`crates/agent-core/tests/snapshot/`。确保无外部 LLM 波动下输出稳定。

### Task 13：补集成测试（旧 G2）

- [ ] 为 `edit_ops`、`layout`、`memory`、`event_stream`、SceneIndex 增量更新各补集成测试。目标：把这些子系统纳入 CI 回归，覆盖删除对账（Task 5）与 undo（Task 2/3）的端到端路径。

**M3 完成定义：** 关键词判断单一来源；核心场景有确定性快照；上述子系统有集成测试；`cargo test --workspace` 数量较 M2 增加且全绿。

---

## M4+：记忆 / 视觉 / 生态（里程碑提纲，非代码级）

> 这些主题依赖 M2 的执行链路稳定，且多项“旧任务”其实已部分存在（见第 0 节）。到达时**先核验既有实现**，再各自拆成独立的 `writing-plans` 计划。这里只给入口与完成判据，不预先写易过期的代码。

### v0.3 记忆 / 上下文 / 提示词（多为“验证 + 完善 + 测试”）
- **MemoryInjector（旧 B1，已存在）**：核验 `memory_injector` 的 project profile / top concepts / key files 注入与 ~2000 token 预算控制；补预算截断测试。入口：`memory_injector/`、`director` 调 `memory_injector.inject` 处。
- **分层上下文 L0-L3（旧 C1，已存在 LayeredContext）**：核验四级上下文与压缩策略（truncate/summarize/token_budget/priority_queue）是否齐全；补缺失策略 + 测试。
- **LLM 记忆压缩 / 情景记忆 / 自动学习（旧 B2/B3/B4）**：依赖 ReAct 主循环稳定后接入；先核验 `memory/` 现有能力再补。
- **Few-shot（旧 C2）**：按请求类型注入示例。
- 完成判据：相同 prompt 在 mock LLM 下稳定；记忆注入不超预算；失败案例能影响后续同类请求。

### v0.4 视觉 / 混合编辑（多为“接 UI + 闭环重试”）
- **视觉手动入口（旧 D1）**：`agent-ui` 加“分析场景”按钮 → 截图 → `VisionClient` → 结果回 Director Desk/Chat。VGRC 桥接与每 30 帧截图已存在（`src/main.rs::vgrc_bridge_system`）。
- **视觉反馈闭环（旧 D2）**：Operation → Screenshot → Vision verify → mismatch 时重试/修订。复用 Task 10 的反思机制。
- **HybridEditorController（旧 D3）**：明确何时用规则/LLM/视觉/人工确认。`hybrid_controller` 已存在，需补策略选择器。
- **SetSpriteTexture 覆盖纹理的忠实 undo**（M1 Task 3 跟进项）放这一批。
- 完成判据：可视化修改后 UI 显示截图与目标检查结果；Vision 判失败能触发可解释修正。

### v0.5 生态 / 扩展
- **真实 Multica server smoke test**：不止 mock；`cd multica && make selfhost` 后跑 `complete_task_flow` 等示例。
- **团队模块接入 DirectorRuntime（旧 E6）**：`team_structure`/`hr_agent`/`cli_agent`/`team_context` 真正进 `init_internal_agents`；配合 M1 Task 1 的“确认后执行删除/添加”补全 apply-on-confirm。
- **CodeGraph + ImpactAnalyzer + CodeContextGenerator（旧 F1）**：`code_graph.rs` 已有雏形，需核验后扩展。
- **多选择工具（旧 F2，selection_tools.rs 已存在）/ Transform Palette（F3）**：核验后接 UI。
- **Godot/Unreal adapter（旧 F5）**：从 TODO stub 推进到最小可运行读取/命令/回滚链路。
- **叙事 Agent（旧 F4）**。
- 完成判据：真实 Multica 能完成分发/进度/技能/状态同步；至少一个非 Bevy adapter 跑通最小链路。

---

## 自检（Self-Review）

- **覆盖核对：** 版本规划 P0-P3 待办均有对应任务——P0 测试数字/身份边界（Task 6 前置门禁 + 版本规划已对齐）、P1 闭环执行（M2）、P1 正确性（M1 T1/T5、undo T2-T4）、P1 关键词收敛（T11）、P2 记忆/视觉/测试（M3 + M4+）、P3 扩展（v0.5 提纲）。
- **占位扫描：** M1 全部为可编译的真实代码；M2 中凡未逐字读到的函数体，均以“先读取指定行号 → 再按既有惯用法修改”的显式步骤呈现，未凭空编造函数体。
- **类型一致性：** `EngineCommand::{SpawnPrefab, SetSpriteTexture, LoadAsset, DeleteEntity, RemoveComponent, ReparentChildren}`、`AgentResultKind::{NeedUserInput, Success, Failed}`、`SceneIndex::{add_entity, remove_entity, reconcile_deletions, entities_by_name}`、`LlmClient::{chat, chat_stream, is_ready, provider}`、`ToolRegistry::{new, register, execute}` 均与调研到的真实签名一致。
- **风险点：** M2 Task 7（让主路径真正跑 ReAct）是最高风险改动，要求小步 + 手动冒烟 + 可回滚；Task 6/7/8/10 含“先读后改”步骤是有意为之，确保落地前对齐真实代码。

---

## 执行交接

计划已保存到 `docs/windwave-execution-plan.md`。两种执行方式：

1. **Subagent-Driven（推荐）** — 每个任务派发独立 subagent，任务间复核，迭代快。REQUIRED SUB-SKILL：superpowers:subagent-driven-development。
2. **Inline Execution** — 在本会话内按批执行，带检查点复核。REQUIRED SUB-SKILL：superpowers:executing-plans。

建议从 **M1**（低风险、代码级完整、可立即落地）开始，再进 M2（闭环执行）。要我现在就开始执行 M1 吗？
