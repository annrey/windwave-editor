# Multica 与 WindWave 项目整合方案

## 项目分析总结

### Multica 项目核心特性
- **技术栈**: Go (后端) + Next.js (前端) + PostgreSQL + WebSocket
- **核心概念**: 
  - Agents as Teammates — 代理作为一等公民
  - Squads — 团队协作，代理分组路由
  - Skills — 可复用技能系统
  - Task Lifecycle — 完整的任务生命周期管理
  - Runtimes — 统一的运行时管理
- **支持的代理**: Claude Code, Codex, GitHub Copilot CLI, OpenClaw, OpenCode, Hermes, Gemini, Pi, Cursor Agent, Kimi, Kiro CLI

### WindWave 项目核心特性
- **技术栈**: Rust (Rust + Bevy ECS) + LLM Agent
- **核心概念**:
  - Four-Tier Memory System (四层记忆系统)
  - Pluggable Planner (可插拔规划器)
  - Streaming ReAct Execution (流式执行)
  - Layered Context (L0~L3 分层上下文)
  - SceneBridge (场景桥接)
- **目标**: AI 驱动的游戏编辑器

## 可整合的核心功能模块

### 1. 任务管理与 Issue 追踪系统
- Multica 的 Issue 分配、状态追踪
- 与 WindWave 的 Task System 结合

### 2. 代理管理与多代理协作
- Multica 的 Agent/Squad 架构
- WindWave 的 Multi-Agent 规划 (Phase 5)

### 3. 可复用技能系统
- Multica 的 Skills 功能
- WindWave 的 Skill Executor

### 4. 运行时管理
- Multica 的 Daemon Runtime 管理
- WindWave 的 Director Runtime

### 5. 实时 WebSocket 通信
- Multica 的实时更新框架
- WindWave 的 Event Stream

## 整合架构方案

### 方案一: 渐进式适配 (推荐)

#### 阶段 1: 核心功能整合
- 保留 WindWave 的 Rust + Bevy 核心架构
- 引入 Multica 的任务管理和代理协作层
- 使用 WebSocket 桥接两个系统

#### 阶段 2: 技能系统融合
- 将 Multica 的 Skills 与 WindWave 的 Skill Executor 结合
- 建立统一的技能注册与执行框架

#### 阶段 3: UI 层整合
- 使用 Multica 的 Next.js 前端作为任务管理界面
- 保留 WindWave 的 Bevy/egui 游戏编辑器界面
- 通过 API 桥接两个 UI

### 方案二: 深度重构
- 完整采用 Multica 的前后端架构
- 将 WindWave 的 Agent Core 改造为特殊的 Agent Runtime
- 游戏编辑功能作为 Skill 或专用工具

## 推荐整合结构

```
风浪/
├── multica/                    # Multica 原始项目 (作为子模块)
├── crates/
│   ├── agent-core/            # WindWave 核心 (保留)
│   ├── agent-ui/              # WindWave UI (保留)
│   ├── bevy-adapter/          # Bevy 适配 (保留)
│   └── multica-bridge/        # 新增: Multica 桥接层
│       ├── src/
│       │   ├── task_sync.rs   # 任务同步
│       │   ├── agent_proxy.rs # 代理桥接
│       │   └── skill_adapter.rs # 技能适配
│       └── Cargo.toml
├── multica-integrated/        # 新增: 整合后的服务
│   ├── go-server/             # Go 后端 (来自 Multica)
│   │   ├── windwave-plugin/   # WindWave 插件
│   │   └── ...
│   └── web-app/               # Next.js 前端 (来自 Multica)
└── docs/
    └── integration/           # 整合文档
```

## 核心桥接模块设计

### 1. Task Bridge (任务桥接)
- 将 WindWave 的 EditPlan 映射为 Multica 的 Issue
- 同步任务状态和进度
- 通过 WebSocket 实时更新

### 2. Agent Proxy (代理代理)
- WindWave 的 Agent 作为 Multica 的特殊 Runtime 类型
- 支持双向任务分配
- 保留 WindWave 的记忆系统优势

### 3. Skill Adapter (技能适配器)
- Multica 的 Skills 可在 WindWave 中执行
- WindWave 的 Game Skills 可注册到 Multica
- 统一的技能描述格式

## 下一步行动

1. 确认整合方向
2. 创建 `multica-bridge` crate
3. 实现基础的任务同步功能
4. 建立 WebSocket 通信桥梁
