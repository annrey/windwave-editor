# 风浪 (WindWave) — AI 驱动的游戏编辑器

> **当前状态**: 早期开发阶段 (Pre-Alpha) | **核心架构**: Rust + Bevy ECS + LLM Agent

风浪是一个由 AI Agent 驱动的游戏编辑器。用户通过自然语言与 Agent 交互，Agent 理解意图、规划任务、执行操作，直接修改游戏场景和代码。

---

## 核心特性

### 自然语言驱动开发
- 用中文或英文描述需求，Agent 自动执行
- 示例: *"创建一个玩家角色，添加 WASD 移动，设置蓝色精灵"*

### 四层记忆系统
借鉴 [agentmemory](https://github.com/rohitg00/agentmemory) 的四层架构：

| 层级 | 名称 | 用途 | 检索方式 |
|------|------|------|----------|
| L3 | Working Memory | 短期工作记忆（对话、实体引用、计算值） | 类型索引 + TTL |
| L2 | Episodic Memory | 情节记忆（用户请求、工具调用、执行记录） | BM25 + 时间衰减 |
| L1 | Semantic Memory | 语义记忆（概念图谱：Entity/Component/System） | TF-IDF 余弦相似度 |
| L0 | Procedural Memory | 程序记忆（工作流模板、决策模式） | 关键词匹配 + 成功率 |

- **三流混合检索**: BM25 + Vector + Recency 通过 RRF 融合
- **Token 预算感知**: 自动截断记忆上下文，适配 LLM 上下文窗口

### 可插拔规划器
- **RuleBasedPlanner**: 关键词匹配，零延迟，适合简单任务
- **LlmPlanner**: LLM 驱动的 CoT 规划，适合复杂任务
- 运行时动态切换

### 流式 ReAct 执行
- Think → Act → Observe 闭环
- 每个步骤实时推送到 EventBus，UI 可实时显示思考过程
- 工具执行结果作为 Observation 反馈给 LLM

### 分层上下文 (L0~L3)
借鉴 [UI-TARS-desktop](https://github.com/bytedance/UI-TARS-desktop)：
- **L0 System**: 全局系统提示、工具定义
- **L1 Session**: 项目上下文、会话历史
- **L2 Task**: 当前任务、目标、约束
- **L3 Entity**: 选中实体的详细信息

### 权限与安全
- 五级风险评估 (Safe → Destructive)
- 高风险操作需用户确认
- 审计日志记录所有 Agent 行为

### 多引擎支持 (规划中)
- **Bevy** (已实现): Rust ECS 引擎
- **Unity** (规划中): 通过 gRPC/REST 适配
- **Godot** (规划中): 通过 GDExtension 适配

---

## 架构

```
┌─────────────────────────────────────────────────────────────┐
│                        Agent UI (egui)                       │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────────┐  │
│  │Chat Panel│ │Director  │ │Approval  │ │Token Usage   │  │
│  │          │ │Desk      │ │Dialog    │ │Display       │  │
│  └──────────┘ └──────────┘ └──────────┘ └──────────────┘  │
└────────────────────────┬────────────────────────────────────┘
                         │ Agent-UI Protocol (Event Stream)
┌────────────────────────▼────────────────────────────────────┐
│                      Agent Core                              │
│  ┌──────────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐  │
│  │Director      │ │Planner   │ │Memory    │ │Prompt    │  │
│  │Runtime       │ │(trait)   │ │System    │ │System    │  │
│  └──────────────┘ └──────────┘ └──────────┘ └──────────┘  │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────────┐  │
│  │Skill     │ │Permission│ │Rollback  │ │EventBus    │  │
│  │Executor  │ │Engine    │ │Manager   │ │            │  │
│  └──────────┘ └──────────┘ └──────────┘ └──────────────┘  │
└────────────────────────┬────────────────────────────────────┘
                         │ Engine Adapter Protocol
┌────────────────────────▼────────────────────────────────────┐
│                    Engine Adapters                           │
│  ┌──────────────┐ ┌──────────────┐ ┌──────────────────────┐│
│  │Bevy Adapter  │ │Unity Adapter │ │Godot Adapter         ││
│  │(SceneBridge) │ │(gRPC/REST)   │ │(GDExtension)         ││
│  └──────────────┘ └──────────────┘ └──────────────────────┘│
└─────────────────────────────────────────────────────────────┘
```

### Crate 结构

| Crate | 职责 | 状态 |
|-------|------|------|
| `agent-core` | Agent 编排、规划、记忆、通信 | 骨架完成，核心能力填充中 |
| `agent-ui` | egui/bevy_egui 界面渲染 | 功能丰富，需 Event Stream 解耦 |
| `bevy-adapter` | Bevy ECS 桥接、SceneIndex、命令执行 | 基础实现，组件支持需扩展 |

---

## 快速开始

### 环境要求
- Rust 1.80+
- LLM API Key (OpenAI 兼容格式)

### 配置 LLM
编辑 `crates/agent-core/src/llm.rs` 或运行时配置：
```rust
let llm = LlmClient::new()
    .with_base_url("https://api.openai.com/v1")
    .with_model("gpt-4o-mini")
    .with_api_key(std::env::var("OPENAI_API_KEY").unwrap());
```

### 运行
```bash
cargo run
```

---

## 开发路线图

### Phase 0: 基线验证 (已完成)
- [x] 三层架构 (agent-core / agent-ui / bevy-adapter)
- [x] 基础 DirectorRuntime 编排
- [x] 五级权限系统
- [x] 基础 Rollback/Transaction

### Phase 1: LLM 主链路 (进行中)
- [x] Planner trait + RuleBasedPlanner/LlmPlanner
- [x] ReAct 流式执行 + Observation 闭环
- [x] RuntimeContextCollector + TokenBudget
- [x] MemoryContext 集成到 PromptSystem
- [ ] SceneBridge 空实现填充
- [ ] 模型名称可配置化

### Phase M: 记忆系统升级 (部分完成)
- [x] 四层记忆架构 (Working/Episodic/Semantic/Procedural)
- [x] BM25 + TF-IDF 混合检索
- [x] RRF 融合排序
- [ ] 持久化存储 (磁盘序列化)
- [ ] 记忆生命周期管理 (衰减/归档)

### Phase 2: 权限 UI (待开始)
- [ ] 可视化权限配置面板
- [ ] 审计日志查看器

### Phase 3: Undo/Redo (待开始)
- [ ] 命令模式完善
- [ ] 历史可视化

### Phase 4: Vision (待开始)
- [ ] 截图 + VLM 分析
- [ ] 视觉反馈闭环

### Phase 5: 多 Agent (待开始)
- [ ] A2A 能力发现
- [ ] 任务协调器

### Phase 9: 多引擎适配 (待开始)
- [ ] Unity 适配器
- [ ] Godot 适配器

---

## 参考与借鉴

本项目在架构设计上参考了以下开源项目：

| 项目 | 借鉴内容 |
|------|----------|
| [hello-agents](https://github.com/datawhalechina/hello-agents) | ReAct/Plan-and-Solve/Reflection 范式 |
| [dive-into-llms](https://github.com/Lordog/dive-into-llms) | CoT 推理增强、Prompt 工程 |
| [supersplat](https://github.com/playcanvas/supersplat) | EditOp 命令模式、EditHistory 序列化 |
| [UI-TARS-desktop](https://github.com/bytedance/UI-TARS-desktop) | Agent Event Stream、L0~L3 分层上下文 |
| [agentmemory](https://github.com/rohitg00/agentmemory) | 四层记忆、三流混合检索 |
| [code-review-graph](https://github.com/tirth8205/code-review-graph) | 代码结构图谱、影响半径分析 |

---

## 许可证

MIT License

---

> **注意**: 本项目处于早期开发阶段，API 可能频繁变更。欢迎 Issue 和 PR！
