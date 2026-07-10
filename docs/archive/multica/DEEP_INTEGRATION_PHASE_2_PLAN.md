
# WindWave + Multica 深度整合 - 第二阶段计划

## 概述

第一阶段已成功完成核心基础设施的搭建！现在进入第二阶段，重点是：

1. **记忆系统集成** - 与 agent-core 的四层记忆系统深度集成
2. **UI 集成** - 在 agent-ui 中添加任务面板组件
3. **Multica 网络连接完善** - 实现完整的双向通信和任务同步

---

## 项目当前状态总结

### 第一阶段已完成

| 模块 | 文件位置 | 功能 | 测试 |
|------|---------|------|------|
| 统一任务桥接 | `multica-bridge/src/task_bridge.rs` | 任务双向同步 | 4个 ✓ |
| 场景感知接口 | `multica-bridge/src/scene_context.rs` | 实体/组件操作 | 4个 ✓ |
| 游戏技能桥接 | `multica-bridge/src/game_skill_bridge.rs` | 4个预定义技能 | 3个 ✓ |
| 场景事件系统 | `multica-bridge/src/scene_event_bus.rs` | 事件订阅/广播 | 3个 ✓ |
| 统一数据访问层 | `multica-bridge/src/multica_db.rs` | 场景/实体/资源存储 | 5个 ✓ |
| 完整示例 | `multica-bridge/examples/deep_integration_example.rs` | 端到端演示 | 可运行 ✓ |

**总计**：22个单元测试，全部通过 ✓

---

## 第二阶段计划

### 任务 1：与 agent-core 记忆系统集成

#### 目标
- 将 multica-db 中的场景数据集成到 agent-core 的四层记忆系统
- 让 Agent 能够基于场景上下文进行推理和决策
- 实现场景变更的记忆记录和回溯

#### 实施内容

1. **创建 memory_scene_context 模块**
   - 文件位置：`crates/multica-bridge/src/memory_scene_context.rs`
   - 功能：
     - 实现 `SceneSnapshot` 到记忆系统的转换
     - 实现场景变更差异的记录
     - 提供场景状态的保存和恢复功能
     - 与 `scene_event_bus` 集成，自动记录场景变更事件

2. **集成到 agent-core 的四层记忆**
   - **Working Memory (L3)**：存储当前活跃场景的实时状态
   - **Episodic Memory (L2)**：记录场景变更的事件历史
   - **Semantic Memory (L1)**：构建场景实体和关系的知识图谱
   - **Procedural Memory (L0)**：存储常见的场景操作模式

3. **实现记忆注入器**
   - 创建 `MulticaMemoryInjector`，将 Multica 数据注入到 agent-core 的记忆系统
   - 支持任务执行过程中的记忆同步
   - 实现基于场景变更的记忆更新触发器

4. **数据结构设计**
   ```rust
   // 场景记忆条目
   struct SceneMemoryEntry {
       scene_id: String,
       snapshot: SceneSnapshot,
       timestamp: u64,
       context_tags: Vec<String>,
   }

   // 场景变更记录
   struct SceneChangeEvent {
       change_id: String,
       scene_id: String,
       diff: SceneDiff,
       affected_tasks: Vec<String>,
   }

   // 场景上下文注入器
   struct MulticaMemoryInjector {
       db: SharedMulticaDb,
       event_bus: SharedSceneEventBus,
   }
   ```

#### 验收标准
- ✅ 创建 memory_scene_context 模块，编译通过
- ✅ 实现场景快照保存和恢复功能
- ✅ 实现记忆系统与场景数据的双向同步
- ✅ 添加 5+ 单元测试，全部通过

#### 优先级：P0

---

### 任务 2：UI 集成 - 任务面板组件

#### 目标
- 在 agent-ui 中添加任务管理面板
- 提供任务列表展示、创建、状态更新功能
- 支持场景-任务关联的可视化

#### 实施内容

1. **创建任务面板模块**
   - 文件位置：`crates/agent-ui/src/task_panel.rs`
   - 功能：
     - 任务列表展示（统一任务视图）
     - 任务详细信息展示
     - 任务创建表单
     - 任务状态变更按钮
     - 场景关联可视化

2. **数据结构设计**
   ```rust
   // 任务面板状态
   #[derive(Resource)]
   struct TaskPanelState {
       selected_task: Option<BridgedTaskId>,
       filter: TaskFilter,
       tasks: Vec<UnifiedTask>,
       is_open: bool,
   }

   // 任务筛选
   enum TaskFilter {
       All,
       ByScene(String),
       ByStatus(UnifiedTaskStatus),
   }
   ```

3. **UI 布局设计**
   ```
   ┌───────────────────────────────────┐
   │ 任务面板          [×] [≡]        │
   ├───────────────────────────────────┤
   │ [🔍] 筛选: [场景 ▼] [状态 ▼]     │
   ├───────────────────────────────────┤
   │ ○ 创建玩家角色       [主场景]    │
   │ ● 实现物理系统       [关卡1]     │
   │ ✅ 添加UI交互        [UI场景]    │
   │ ...                               │
   ├───────────────────────────────────┤
   │ [+ 新建任务] [⟳ 刷新]            │
   └───────────────────────────────────┘
   ```

4. **集成点**
   - 与 `task_bridge` 集成
   - 与 `multica_db` 集成（获取场景数据）
   - 与 `scene_event_bus` 集成（实时更新）

5. **Beuy 插件集成**
   ```rust
   pub struct TaskPanelPlugin;

   impl Plugin for TaskPanelPlugin {
       fn build(&self, app: &mut App) {
           app.init_resource::<TaskPanelState>()
              .add_systems(Update, (
                  render_task_panel,
                  handle_task_actions,
              ).chain());
       }
   }
   ```

#### 验收标准
- ✅ 任务面板模块创建成功
- ✅ 任务列表展示正常
- ✅ 任务创建和状态更新功能正常
- ✅ 与后端集成测试通过
- ✅ UI 美观、交互流畅

#### 优先级：P1

---

### 任务 3：完善 Multica 网络连接

#### 目标
- 实现完整的 Multica 双向通信
- 实现真实的任务同步机制
- 添加错误处理和重连逻辑
- 完善与 Multica 服务器的交互

#### 实施内容

1. **完善 ws_client 模块**
   - 文件位置：`crates/multica-bridge/src/ws_client.rs`
   - 功能增强：
     - 自动重连机制
     - 消息队列（断线期间的消息缓存）
     - 连接状态监控
     - 错误恢复策略

2. **实现完整的任务同步**
   - 双向同步：WindWave → Multica，Multica → WindWave
   - 任务创建、更新、删除同步
   - 冲突处理策略
   - 同步状态显示

3. **消息处理器**
   ```rust
   // Multica 消息处理器
   struct MulticaMessageHandler {
       db: SharedMulticaDb,
       task_bridge: TaskBridge,
       event_bus: SharedSceneEventBus,
   }

   impl MulticaMessageHandler {
       // 处理任务创建消息
       fn handle_task_create(&mut self, msg: Message) -> Result<()>;
       
       // 处理任务更新消息
       fn handle_task_update(&mut self, msg: Message) -> Result<()>;
       
       // 处理代理请求
       fn handle_agent_request(&mut self, msg: Message) -> Result<()>;
   }
   ```

4. **守护进程集成**
   - 创建 `multica_daemon.rs` 模块
   - 后台持续运行的任务同步守护进程
   - 心跳机制
   - 状态报告

5. **配置增强**
   ```rust
   struct BridgeConfig {
       server_url: String,
       daemon_id: String,
       agent_id: String,
       auto_reconnect: bool,
       reconnect_interval: Duration,
       sync_interval: Duration,
   }
   ```

#### 验收标准
- ✅ WebSocket 连接稳定，支持断线重连
- ✅ 任务双向同步功能正常
- ✅ 守护进程稳定运行
- ✅ 集成测试通过（包括断线场景）

#### 优先级：P1

---

## 实施顺序

```
Phase 2.1 (Week 1)
├─ Task 1: 记忆系统集成（P0）
│  └─ 创建 memory_scene_context 模块
│  └─ 集成到 agent-core 的记忆系统
│  └─ 实现记忆注入器
│  └─ 单元测试
│
Phase 2.2 (Week 2)
├─ Task 2: UI 集成（P1）
│  └─ 创建任务面板模块
│  └─ UI 布局设计
│  └─ 与后端集成
│  └─ UI/UX 优化
│
Phase 2.3 (Week 3)
├─ Task 3: Multica 网络连接完善（P1）
│  └─ 完善 ws_client 模块
│  └─ 实现任务同步逻辑
│  └─ 创建守护进程
│  └─ 集成测试
│
Phase 2.4 (Week 4)
├─ 完整端到端测试
├─ 性能优化
├─ 文档完善
└─ 第二阶段总结
```

---

## 技术架构（第二阶段）

```
┌─────────────────────────────────────────────────────────────────────┐
│                        WindWave 编辑器 UI                          │
│  ┌────────────────────────────────────────────────────────────────┐│
│  │  Agent UI - agent-ui 包                                        ││
│  │  ┌──────────┐  ┌──────────┐  ┌────────────────────┐           ││
│  │  │Chat Panel│  │Task Panel│  │Director Desk       │           ││
│  │  └─────┬────┘  └─────┬────┘  └─────────┬──────────┘           ││
│  └────────┼───────────────┼────────────────┼──────────────────────┘│
└───────────┼───────────────┼────────────────┼───────────────────────┘
            │               │                │
            ▼               ▼                ▼
┌─────────────────────────────────────────────────────────────────────┐
│                         Agent Core                                    │
│  ┌──────────────────────┐  ┌───────────────────────────────────┐   │
│  │  Memory System       │◄─┤ memory_scene_context              │   │
│  │  (4-tier: W/E/S/P)   │  │  (Scene <-> Memory 桥接)         │   │
│  └──────────────────────┘  └───────────────────────────────────┘   │
│  ┌──────────────────────┐                                           │
│  │  Scene Agent         │                                           │
│  │  Task System         │                                           │
│  └──────────────────────┘                                           │
└─────────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────────┐
│                      Multica Bridge                                  │
│  ┌──────────────────────────────────────────────────────────────────┐│
│  │  multica-db             (统一数据访问)                            ││
│  │  ├─ Scene storage                                               ││
│  │  ├─ Entity storage                                              ││
│  │  ├─ Task-Scene relations                                         ││
│  │  └─ Query API                                                   ││
│  └──────────────────────────────────────────────────────────────────┘│
│  ┌──────────────────────┐┌───────────────────────────────────────┐│
│  │  task_bridge         ││  game_skill_bridge                     ││
│  │  (Unified Tasks)     ││  (Game Skill Execution)               ││
│  └──────────────────────┘└───────────────────────────────────────┘│
│  ┌────────────────────────────────────────────────────────────────┐ │
│  │  scene_event_bus       (Event System)                         │ │
│  └────────────────────────────────────────────────────────────────┘ │
│  ┌────────────────────────────────────────────────────────────────┐ │
│  │  ws_client + multica_daemon  (Network Layer)                   │ │
│  └────────────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────────┘
                              │
                              ▼
                  ┌──────────────────────┐
                  │   Multica Server    │
                  │  (Optional Remote)   │
                  └──────────────────────┘
```

---

## 风险与缓解措施

| 风险 | 可能性 | 影响 | 缓解措施 |
|------|--------|------|---------|
| 记忆系统集成复杂度高 | 中 | 高 | 分阶段实现，先做基础集成，再做高级功能 |
| UI 迭代需求多 | 高 | 中 | 先做MVP版本，再根据反馈迭代 |
| Multica 网络不稳定 | 中 | 高 | 实现健壮的重连和错误处理逻辑 |
| 性能问题 | 低 | 中 | 提前做性能测试，识别瓶颈优化 |

---

## 第二阶段完成标准

✅ Task 1 - 记忆系统集成完成，测试通过
✅ Task 2 - UI 任务面板完成，可正常使用
✅ Task 3 - Multica 网络连接完善，双向同步正常
✅ 完整端到端流程测试通过
✅ 文档完善（API文档、用户指南）
✅ 性能指标达标

---

## 下一步（第三阶段）

- 与真实 Multica 服务器的完整集成测试
- 多Agent 协作场景支持
- 高级游戏技能库扩展
- AI 辅助的任务生成

---

## 总结

第一阶段成功奠定了基础！第二阶段将让整个系统真正可用，从基础设施转向完整的产品功能！

预计时间：4周
