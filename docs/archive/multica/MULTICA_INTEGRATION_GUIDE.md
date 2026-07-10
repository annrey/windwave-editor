# Multica 与 WindWave 集成指南

## 概述

我们成功创建了一个完整的 `multica-bridge` 库，用于将 WindWave 游戏编辑器与 Multica 代理平台集成。这个集成允许 WindWave Agent 作为 Multica 的一等公民参与任务分配。

## 当前实现

### 已完成的功能

1. **类型定义 (`types.rs`)**
   - Multica WebSocket 消息类型
   - Task 相关 payloads (dispatch, progress, completed)
   - Daemon 相关 payloads (register, heartbeat)
   - Issue 和 Agent 枚举类型

2. **WebSocket 客户端 (`ws_client.rs`)**
   - 占位符实现（当前）
   - 可以连接 Multica 服务器
   - 发送和接收消息
   - 守护程序注册

3. **任务同步 (`task_sync.rs`)**
   - 本地任务与 Multica Issue 的映射
   - 处理任务分派
   - 发送进度更新
   - 完成任务通知

4. **Agent 代理 (`agent_proxy.rs`)**
   - WindWave Agent 信息管理
   - 与 Multica 集成的接口

5. **技能适配器 (`skill_adapter.rs`)**
   - 本地技能注册
   - 技能执行
   - 可以与 Multica Skills 集成

### 项目结构

```
风浪/
├── multica/                          # Multica 原始仓库
│   ├── server/                       # Go 后端实现
│   ├── packages/                     # TypeScript 包
│   └── ...
├── crates/
│   └── multica-bridge/               # 新建：桥接库
│       ├── src/
│       │   ├── lib.rs               # 导出公共 API
│       │   ├── types.rs             # 类型定义
│       │   ├── error.rs             # 错误处理
│       │   ├── ws_client.rs         # WebSocket 客户端
│       │   ├── task_sync.rs         # 任务同步
│       │   ├── agent_proxy.rs       # Agent 代理
│       │   └── skill_adapter.rs     # 技能适配
│       └── examples/
│           └── basic_usage.rs       # 基本使用示例
├── MULTICA_INTEGRATION_GUIDE.md    # 本文档
└── MULTICA_INTEGRATION_PLAN.md     # 原始计划
```

## 下一步

### 1. 运行 Multica 本地服务器

首先，让我们在本地运行 Multica 服务器：

```bash
cd multica

# 使用 Docker 运行（推荐）
make selfhost

# 或者从源码构建
make selfhost-build
```

这将启动：
- Multica 后端 (http://localhost:8080)
- Multica 前端 (http://localhost:3000)
- PostgreSQL 数据库

### 2. 完善 WebSocket 客户端

当前 WebSocket 客户端是占位符，让我们实现真正的连接：

```rust
// 在 Cargo.toml 中添加依赖
tokio-tungstenite = "0.23"
futures-util = { version = "0.3", features = ["sink", "stream"] }
url = "2.5"
```

然后在 `ws_client.rs` 中实现真正的 WebSocket 连接。

### 3. 与 agent-core 集成

修改 `task_sync.rs` 来使用真实的 agent-core Task：

```rust
// 使用 agent_core::task::Task 而不是 LocalTask
```

### 4. 实现完整的任务流

1. **创建 Issue** - 在 Multica UI 中创建 Issue
2. **分配给 WindWave Agent** - 将 Issue 分配给我们的 Agent
3. **接收分派** - 通过 WebSocket 接收 `task:dispatch`
4. **执行任务** - 在 WindWave 中执行编辑操作
5. **发送进度** - 通过 `task:progress` 发送实时进度
6. **完成任务** - 通过 `task:completed` 标记完成

### 5. 实现技能同步

- 将 Multica Skills 导入到 WindWave
- 将 WindWave 编辑操作导出为 Multica Skills
- 双向技能执行

## 快速开始

### 运行示例

```bash
# 编译并运行示例
cargo run --example basic_usage -p multica-bridge
```

### 基本使用

```rust
use multica_bridge::*;

#[tokio::main]
async fn main() -> Result<()> {
    // 配置
    let config = BridgeConfig::default();
    
    // 创建任务同步器
    let mut task_sync = TaskSync::new(config);
    
    // 创建技能适配器
    let mut skill_adapter = SkillAdapter::new();
    
    // 注册技能
    skill_adapter.register_skill(
        "create_player",
        "创建玩家",
        vec!["game".to_string()],
        |params| { /* 处理逻辑 */ Ok(json!({})) },
    );
    
    // 连接 Multica
    task_sync.connect().await?;
    
    Ok(())
}
```

## Multica 核心概念回顾

- **Agents**: 执行任务的 AI 代理
- **Squads**: Agent 团队，用于任务路由
- **Skills**: 可重用的技能/工具
- **Tasks**: 代理执行的工作单元
- **Daemon**: 在本地运行的代理运行时
- **Runtime**: 执行环境（本地机器）

## WindWave 集成优势

1. **任务可见性** - WindWave 编辑任务在 Multica 看板中可见
2. **Agent 协作** - WindWave Agent 可以与其他 Agent 协作
3. **技能复用** - 编辑操作可以作为技能共享
4. **进度追踪** - 实时查看编辑进度

## 相关文档

- [Multica 官方文档](https://github.com/multica-ai/multica)
- [Multica 自托管指南](multica/SELF_HOSTING.md)
- [WindWave 架构文档](CONTEXT-MAP.md)
