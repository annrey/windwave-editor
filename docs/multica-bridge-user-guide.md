# Multica-Bridge 用户使用指南

## 目录
- [概述](#概述)
- [快速开始](#快速开始)
- [核心功能使用](#核心功能使用)
  - [任务管理](#任务管理)
  - [场景记忆](#场景记忆)
  - [四层记忆集成](#四层记忆集成)
  - [任务同步](#任务同步)
- [UI 操作指南](#ui-操作指南)
- [常见问题](#常见问题)
- [高级用法](#高级用法)

## 概述

Multica-Bridge 是 WindWave 游戏开发工具与 Multica 协作系统之间的集成桥接模块。它实现了任务管理、场景记忆、四层记忆集成、双向任务同步等功能，为游戏开发提供强大的 AI 协作支持。

## 快速开始

### 1. 环境要求

- **Rust**: 1.75+
- **操作系统**: macOS / Linux / Windows
- **内存**: 至少 8GB RAM
- **网络**: 需要访问 Multica 服务器（可选）

### 2. 安装

```bash
# 克隆项目
git clone <repository-url>
cd 风浪

# 安装依赖
cargo build

# 运行测试
cargo test -p multica-bridge
```

### 3. 基本配置

在 `Cargo.toml` 中添加依赖：

```toml
[dependencies]
multica-bridge = { path = "crates/multica-bridge" }
```

在代码中初始化：

```rust
use multica_bridge::prelude::*;

// 创建场景记忆
let scene_memory = create_shared_scene_context_memory();

// 创建记忆注入器
let injector_config = MemoryInjectorConfig::default();
let injector = create_shared_memory_injector(scene_memory.clone(), injector_config);

// 注入场景记忆
injector.inject_scene("main-scene")?;
```

## 核心功能使用

### 任务管理

#### 创建任务

```rust
// 通过消息系统创建任务
let message = Message {
    message_type: "task:created".to_string(),
    payload: json!({
        "id": {"bridge_id": 1, "multica_id": 100},
        "title": "我的任务",
        "description": "任务描述",
        "status": "pending",
        "scene_id": "main-scene",
        "entity_ids": [1, 2],
        "resource_ids": [],
        "scene_snapshot": null,
        "multica_task": null,
        "created_at": "2024-01-01T00:00:00Z",
        "updated_at": "2024-01-01T00:00:00Z"
    }),
};

// 处理消息
let handler = create_shared_message_handler_with_sync(synchronizer);
let result = handler.process_message(&message);
```

#### 任务状态更新

```rust
// 任务开始
injector.on_task_start("task-001", "main-scene")?;

// 任务完成
injector.on_task_complete("task-001", "main-scene")?;

// 任务失败
injector.on_task_failed("task-001", "main-scene", "错误信息")?;
```

#### 查询任务

```rust
// 获取场景相关任务
let tasks = injector.get_tasks_for_scene("main-scene");

// 获取任务统计
let stats = injector.get_stats();
```

### 场景记忆

#### 创建场景快照

```rust
let snapshot = SceneSnapshot {
    version: 1,
    timestamp: "2024-01-01T00:00:00Z".to_string(),
    entities: vec![
        SceneEntity {
            id: 1,
            name: "Player".to_string(),
            components: vec![
                ("position".to_string(), ComponentData::Vector3([0.0, 0.0, 0.0])),
            ],
            position: Some([0.0, 0.0, 0.0]),
        },
    ],
};

scene_memory.lock().unwrap().save_scene_snapshot(
    "main-scene".to_string(),
    "MainScene".to_string(),
    snapshot,
)?;
```

#### 比较场景变更

```rust
let changes = scene_memory.lock().unwrap().compare_snapshots(
    "main-scene",
    &snapshot_v1,
    &snapshot_v2,
)?;

for change in &changes {
    println!("变更类型: {:?}", change.change_type);
    println!("实体: {:?}", change.entity);
}
```

#### 订阅场景事件

```rust
// 创建事件总线
let event_bus = Arc::new(SceneEventBus::new());

// 订阅实体变更事件
event_bus.subscribe(
    "entity-changes".to_string(),
    Arc::new(Box::new(|event| {
        println!("收到实体变更事件: {:?}", event);
    })),
);

// 发布事件
event_bus.publish(SceneEvent::entity_changed("main-scene", vec![1, 2]));
```

### 四层记忆集成

#### 配置记忆注入

```rust
let config = MemoryInjectorConfig {
    enable_working_memory: true,
    enable_episodic_memory: true,
    enable_semantic_memory: true,
    enable_procedural_memory: true,
    max_scenes_cached: 100,
    ..MemoryInjectorConfig::default()
};

let injector = create_shared_memory_injector(scene_memory.clone(), config);
```

#### 记忆层说明

| 记忆层 | 用途 | 触发时机 |
|--------|------|----------|
| Working Memory | 当前任务上下文 | 任务开始/场景切换 |
| Episodic Memory | 历史事件记录 | 任务完成/失败 |
| Semantic Memory | 知识积累 | 场景变更后 |
| Procedural Memory | 技能经验 | 技能使用后 |

### 任务同步

#### 配置同步方向

```rust
let sync_config = TaskSyncConfig {
    direction: SyncDirection::Bidirectional,  // 双向同步
    conflict_resolution: ConflictResolution::LastWriteWins,  // 以最新时间为准
    auto_sync_interval: Duration::from_secs(30),  // 自动同步间隔
    ..TaskSyncConfig::default()
};

let synchronizer = create_shared_task_synchronizer(sync_config);
```

#### 冲突解决策略

| 策略 | 说明 |
|------|------|
| WindWaveWins | 以 WindWave 为准 |
| MulticaWins | 以 Multica 为准 |
| LastWriteWins | 以最新更新时间为准 |
| Manual | 手动解决（记录冲突待处理） |

#### 执行同步

```rust
// 手动同步
let stats = synchronizer.sync()?;

// 查看同步统计
println!("成功: {}", stats.successful_syncs);
println!("失败: {}", stats.failed_syncs);
println!("冲突: {}", stats.conflicts);
```

## UI 操作指南

### 打开任务面板

在游戏中按下快捷键（默认 `F1`）打开任务面板。

### 任务面板功能

1. **搜索任务**: 在搜索栏输入关键词搜索任务
2. **筛选任务**: 点击"全部"、"待处理"、"进行中"、"已完成"、"高优先级"按钮筛选
3. **创建任务**: 点击"+ 新建"按钮创建新任务
4. **刷新任务**: 点击"🔄"按钮刷新任务列表
5. **操作任务**: 点击"选择"、"开始"、"完成"、"删除"按钮操作任务

### 创建任务对话框

1. 点击"+ 新建"按钮
2. 填写标题（必填）
3. 填写描述（可选）
4. 填写场景 ID（可选）
5. 选择优先级（1-5）
6. 点击"创建"按钮提交

## 常见问题

### Q: 如何配置 Multica 服务器连接？

A: 在初始化时设置服务器地址：

```rust
let ws_client = MulticaWsClient::new(
    "ws://localhost:8080",  // 服务器地址
    Some("your-auth-token"),  // 认证令牌（可选）
)?;
```

### Q: 任务同步失败怎么办？

A: 检查以下几点：
1. 网络连接是否正常
2. 服务器地址是否正确
3. 认证令牌是否有效
4. 查看日志获取详细错误信息

### Q: 如何查看记忆注入统计？

A: 调用 `injector.get_stats()` 获取统计信息：

```rust
let stats = injector.get_stats();
println!("总注入次数: {}", stats.total_injections);
println!("成功注入: {}", stats.successful_injections);
println!("失败注入: {}", stats.failed_injections);
```

### Q: 如何自定义消息处理器？

A: 实现 `MessageHandler` trait：

```rust
struct MyHandler;

impl MessageHandler for MyHandler {
    fn handle_message(&self, message: &Message) -> Result<MessageHandleResult> {
        // 处理消息
        Ok(MessageHandleResult {
            success: true,
            message_type: message.message_type.clone(),
            error: None,
            processed_at: chrono::Utc::now().to_rfc3339(),
            metadata: HashMap::new(),
        })
    }
    
    fn handler_name(&self) -> &str {
        "my-handler"
    }
}

// 注册自定义处理器
handler.register_handler("my-handler", Box::new(MyHandler));
```

## 高级用法

### 场景感知 Agent

```rust
// 创建场景上下文
let context = SceneContext::new(scene_memory.clone(), "main-scene");

// 查询场景实体
let entities = context.get_entities()?;

// 获取实体组件
let components = context.get_entity_components(entity_id)?;

// 执行场景操作
context.execute_operation("move_entity", json!({
    "entity_id": entity_id,
    "position": [10.0, 0.0, 0.0]
}))?;
```

### 游戏技能桥接

```rust
// 注册游戏技能
let bridge = GameSkillBridge::new(scene_context.clone());

bridge.register_skill(
    "create_entity",
    "创建实体",
    Arc::new(|params| {
        // 技能实现
        Ok(json!({"success": true}))
    }),
);

// 执行技能
let result = bridge.execute_skill("create_entity", params)?;
```

### 守护进程管理

```rust
// 创建守护进程
let daemon = MulticaDaemon::new(
    ws_client,
    handler,
    DaemonConfig {
        heartbeat_interval: Duration::from_secs(30),
        reconnect_interval: Duration::from_secs(5),
        max_reconnect_attempts: 10,
        ..DaemonConfig::default()
    },
)?;

// 启动守护进程
daemon.start()?;

// 停止守护进程
daemon.stop()?;
```
