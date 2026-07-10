# Multica Bridge 架构文档

## 架构概览

Multica Bridge 是 WindWave 游戏编辑器与 Multica 协作系统之间的集成桥梁。它实现了任务系统、记忆系统、场景感知和消息处理的深度融合。

## 核心设计原则

1. **接口隔离** - 通过 trait 定义接口，避免强耦合
2. **线程安全** - 使用 Arc<Mutex<T>> 实现多线程安全访问
3. **事件驱动** - 通过事件总线实现模块间松耦合通信
4. **分层架构** - 四层记忆系统实现不同粒度的数据管理
5. **双向同步** - 支持 WindWave 和 Multica 之间的数据双向流动

## 系统架构

```
┌─────────────────────────────────────────────────────────────┐
│                    WindWave Game Editor                      │
├─────────────────────────────────────────────────────────────┤
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────┐  │
│  │  Agent-Core  │  │   Agent-UI   │  │  Scene Manager   │  │
│  │  (Memory)    │  │  (UI Panel)  │  │  (Entities)      │  │
│  └──────┬───────┘  └──────┬───────┘  └────────┬─────────┘  │
│         │                 │                    │            │
│         └─────────────────┼────────────────────┘            │
│                           │                                 │
├───────────────────────────┼─────────────────────────────────┤
│                  multica-bridge                              │
│                           │                                 │
│  ┌────────────────────────┼────────────────────────────┐    │
│  │  Four-Tier Memory System                            │    │
│  │  ┌──────────────────────────────────────────────┐   │    │
│  │  │  Working Memory (L3) - 活跃场景状态           │   │    │
│  │  │  Episodic Memory (L2) - 场景变更历史          │   │    │
│  │  │  Semantic Memory (L1) - 场景关系知识图谱      │   │    │
│  │  │  Procedural Memory (L0) - 操作模式和决策模式  │   │    │
│  │  └──────────────────────────────────────────────┘   │    │
│  │                                                     │    │
│  │  ┌──────────────┐  ┌──────────────────────────┐    │    │
│  │  │   Memory     │  │   Task Synchronizer      │    │    │
│  │  │   Injector   │  │   (Bidirectional Sync)   │    │    │
│  │  └──────┬───────┘  └──────────┬───────────────┘    │    │
│  │         │                     │                    │    │
│  └─────────┼─────────────────────┼────────────────────┘    │
│            │                     │                         │
│  ┌─────────┼─────────────────────┼────────────────────┐    │
│  │  Scene Event Bus  │  Message Handler Pipeline      │    │
│  │  (Event-Driven)   │  (10 Message Types)            │    │
│  └─────────┬─────────┴──────────┬─────────────────────┘    │
│            │                    │                          │
│  ┌─────────┼────────────────────┼─────────────────────┐    │
│  │         ▼                    ▼                     │    │
│  │  WebSocket Client (Auto-Reconnect, Message Queue)  │    │
│  └──────────────────────────┬─────────────────────────┘    │
└─────────────────────────────┼───────────────────────────────┘
                              │
                    ┌─────────┴─────────┐
                    │   Multica Server   │
                    │   (WebSocket)      │
                    └───────────────────┘
```

## 模块详细说明

### 1. WebSocket 客户端 (ws_client.rs)

**职责**：管理与 Multica 服务器的 WebSocket 连接

**核心组件**：
- `ConnectionMonitor` - 连接状态监控，支持状态追踪和重连计数
- `MessageQueue` - 消息队列，缓存断线期间的消息（最大1000条）
- `MulticaWebSocketClient` - 主客户端，实现自动重连和指数退避

**关键特性**：
- 自动重连（指数退避：1s, 2s, 4s, 8s...最大30s）
- 消息队列（离线消息缓存）
- 心跳机制（可配置间隔，默认30秒）
- 连接状态监控（5种状态）

**重连逻辑**：
```
连接失败 → 检查是否可重连 → 等待退避时间 → 清理旧连接 → 尝试重连
```

### 2. 任务同步模块 (task_sync_module.rs)

**职责**：实现 WindWave 和 Multica 之间的双向任务同步

**核心组件**：
- `TaskSynchronizer` - 任务同步器，管理本地和远程任务缓存
- `TaskSyncConfig` - 同步配置（方向、冲突策略、间隔等）
- `SyncStats` - 同步统计信息

**同步方向**：
- `WindWaveToMultica` - 单向：WindWave → Multica
- `MulticaToWindWave` - 单向：Multica → WindWave
- `Bidirectional` - 双向同步

**冲突解决策略**：
- `WindWaveWins` - 以 WindWave 为准
- `MulticaWins` - 以 Multica 为准
- `LastWriteWins` - 以最新更新时间为准
- `Manual` - 手动解决（记录冲突待处理）

**同步流程**：
```
1. 锁定同步状态（防止并发）
2. 比较本地和远程任务
3. 检测冲突
4. 根据策略解决冲突
5. 更新任务缓存
6. 更新统计信息
7. 释放同步锁
```

### 3. 记忆系统 (memory_scene_context.rs + four_tier_memory_integration.rs)

**职责**：管理场景记忆和四层记忆集成

**四层记忆架构**：

| 层级 | 名称 | 用途 | 内容 |
|------|------|------|------|
| L3 | Working Memory | 当前活跃场景的实时状态 | 场景快照、实体列表 |
| L2 | Episodic Memory | 场景变更事件历史 | 实体创建/更新/删除事件 |
| L1 | Semantic Memory | 场景实体关系知识图谱 | 实体关系节点和边 |
| L0 | Procedural Memory | 场景操作模式和决策模式 | 重复操作模式提取 |

**核心接口**：
- `WorkingMemoryInterface` - 工作记忆接口
- `EpisodicMemoryInterface` - 情节记忆接口
- `SemanticMemoryInterface` - 语义记忆接口
- `ProceduralMemoryInterface` - 程序记忆接口

**注入器**：
- `SceneWorkingMemoryInjector` - 注入活跃场景到工作记忆
- `SceneEpisodicMemoryInjector` - 注入场景变更事件到情节记忆
- `SceneSemanticMemoryInjector` - 注入场景关系图谱到语义记忆
- `SceneProceduralMemoryInjector` - 提取并注入操作模式到程序记忆
- `FourTierSceneInjector` - 统一管理所有四层记忆注入

### 4. 记忆注入器 (memory_injector.rs)

**职责**：实现任务执行过程中的记忆同步和基于场景变更的记忆更新

**核心组件**：
- `MulticaMemoryInjector` - 主记忆注入器
- `MemoryInjectorConfig` - 注入器配置
- `SceneMemoryEventHandler` - 场景事件处理器

**任务生命周期集成**：
```
任务开始 → on_task_start() → 注入场景记忆 → 关联任务
  ↓
任务执行 → 场景变更 → 自动更新记忆
  ↓
任务完成 → on_task_complete() → 更新记忆状态
  ↓
任务失败 → on_task_failed() → 记录失败信息
```

### 5. 场景事件总线 (scene_event_bus.rs)

**职责**：实现场景变更的事件驱动通知

**核心组件**：
- `SceneEventBus` - 事件总线，管理订阅者和事件历史
- `SceneEventSubscriber` - 订阅者 trait
- `SceneEvent` - 事件数据结构

**事件类型**：
- EntityCreated / EntityUpdated / EntityDeleted
- ComponentAdded / ComponentUpdated / ComponentRemoved
- Custom (自定义事件)

**订阅模型**：
```
订阅者注册 → 发布事件 → 通知所有订阅者 → 记录事件历史
```

### 6. 消息处理器 (message_handler.rs)

**职责**：处理来自 Multica 服务器的各类消息

**核心组件**：
- `MessageHandler` - 消息处理器 trait
- `DefaultMessageHandler` - 默认消息处理器
- `MessagePipeline` - 消息处理管道（支持多处理器串联）

**消息类型**：
- `task:created` - 任务创建
- `task:updated` - 任务更新
- `task:deleted` - 任务删除
- `heartbeat:response` - 心跳响应
- `daemon:register:response` - 守护进程注册响应
- `scene:changed` - 场景变更通知
- 自定义消息

**处理流程**：
```
接收消息 → 解析类型 → 查找处理器 → 执行处理 → 返回结果 → 更新统计
```

### 7. 守护进程 (multica_daemon.rs)

**职责**：管理后台守护进程，负责与 Multica 服务器的持续连接

**核心组件**：
- `MulticaDaemon` - 守护进程主类
- `DaemonStatus` - 守护进程状态
- `DaemonStats` - 守护进程统计

**启动流程**：
```
初始化 → 连接服务器 → 注册守护进程 → 启动消息循环 → 启动心跳循环
```

### 8. 任务面板 UI (task_panel.rs)

**职责**：在 agent-ui 中提供任务管理界面

**核心功能**：
- 任务列表展示（支持搜索和筛选）
- 任务创建对话框
- 任务状态更新（待处理 → 进行中 → 已完成）
- 任务删除
- 优先级显示（P1-P5）
- 场景标签和 Multica 标签
- 状态统计栏

## 数据流

### 任务同步数据流

```
WindWave 创建任务
    ↓
添加到本地任务缓存
    ↓
TaskSynchronizer.sync()
    ↓
检查远程任务缓存
    ↓
检测冲突（如有）
    ↓
根据策略解决冲突
    ↓
更新远程任务缓存
    ↓
发送 WebSocket 消息到 Multica
    ↓
Multica 接收并处理
```

### 记忆注入数据流

```
场景变更发生
    ↓
SceneEventBus 发布事件
    ↓
SceneMemoryEventHandler 接收事件
    ↓
记录变更到 SceneContextMemory
    ↓
触发记忆更新（如果启用自动触发）
    ↓
FourTierSceneInjector 注入到四层记忆
    ↓
更新统计信息
```

### 消息处理数据流

```
Multica 发送消息
    ↓
WebSocket 客户端接收
    ↓
放入消息通道
    ↓
MessageHandler 处理消息
    ↓
根据消息类型路由到对应处理器
    ↓
更新任务同步器（如果需要）
    ↓
返回处理结果
```

## 线程安全设计

所有共享状态使用以下策略保证线程安全：

1. **Arc<Mutex<T>>** - 用于需要同步访问的复杂数据结构
2. **Arc<std::sync::Mutex<T>>** - 用于非异步上下文
3. **tokio::sync::Mutex** - 用于异步上下文（如消息队列）
4. **AtomicBool** - 用于简单标志（如运行状态）
5. **Arc<AtomicUsize>** - 用于计数器（如重连次数）

## 错误处理

使用自定义 `BridgeError` 枚举：

```rust
pub enum BridgeError {
    ConnectionError(String),      // 连接错误
    WebSocketError(String),       // WebSocket 错误
    SerializationError(String),   // 序列化错误
    TaskNotFoundError(String),    // 任务未找到
    SceneNotFoundError(String),   // 场景未找到
    ConnectionClosed,             // 连接已关闭
    Other(String),                // 其他错误
}
```

所有可能失败的操作返回 `Result<T, BridgeError>`。

## 测试策略

### 单元测试
- 每个模块独立的单元测试
- 使用 Mock 对象模拟外部依赖
- 覆盖正常流程和边界情况

### 集成测试
- 跨模块协同工作测试
- 端到端场景测试
- 验证数据流正确性

### 测试覆盖
- **115个测试用例** (105个单元测试 + 10个端到端测试)
- 所有核心功能都有测试覆盖
- 测试包含在 `#[cfg(test)]` 模块中

## 性能考虑

1. **消息队列限制** - 最大1000条消息，避免内存泄漏
2. **同步锁** - 防止并发同步导致数据不一致
3. **指数退避** - 避免频繁重连导致服务器压力
4. **懒加载** - 场景数据按需加载到记忆
5. **批量操作** - 支持批量注入多个场景

## 扩展点

1. **自定义消息处理器** - 实现 `MessageHandler` trait
2. **自定义记忆接口** - 实现四层记忆接口 trait
3. **自定义事件订阅者** - 实现 `SceneEventSubscriber` trait
4. **冲突解决策略** - 扩展 `ConflictResolution` 枚举
5. **事件类型** - 扩展 `SceneEventType` 和 `MulticaMessageType`
