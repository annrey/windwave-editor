# WindWave — AI-Powered Game Editor

> **Status**: Early Development (Pre-Alpha) | **Core Architecture**: Rust + Bevy ECS + LLM Agent
> **中文** | [日本語](#日本語)

WindWave is an AI Agent-driven game editor. Users interact with the Agent via natural language; the Agent understands intent, plans tasks, and executes operations to directly modify game scenes and code.

---

## Core Features

### Natural Language Driven Development
- Describe requirements in Chinese or English; the Agent executes automatically
- Example: *"Create a player character with WASD movement and a blue sprite"*

### Four-Tier Memory System
Inspired by [agentmemory](https://github.com/rohitg00/agentmemory):

| Tier | Name | Purpose | Retrieval |
|------|------|---------|-----------|
| L3 | Working Memory | Short-term (dialogue, entity refs, computed values) | Type index + TTL |
| L2 | Episodic Memory | Episodic (user requests, tool calls, execution records) | BM25 + time decay |
| L1 | Semantic Memory | Semantic (concept graph: Entity/Component/System) | TF-IDF cosine similarity |
| L0 | Procedural Memory | Procedural (workflow templates, decision patterns) | Keyword match + success rate |

- **Three-stream hybrid retrieval**: BM25 + Vector + Recency fused via RRF
- **Token-budget aware**: Automatic context truncation for LLM context windows

### Pluggable Planner
- **RuleBasedPlanner**: Keyword matching, zero latency, for simple tasks
- **LlmPlanner**: LLM-driven CoT planning, for complex tasks
- Runtime dynamic switching

### Streaming ReAct Execution
- Think → Act → Observe closed loop
- Each step streams to EventBus in real-time; UI displays the thinking process live
- Tool execution results feed back to LLM as Observations

### Layered Context (L0~L3)
Inspired by [UI-TARS-desktop](https://github.com/bytedance/UI-TARS-desktop):
- **L0 System**: Global system prompts, tool definitions
- **L1 Session**: Project context, session history
- **L2 Task**: Current task, goals, constraints
- **L3 Entity**: Detailed info of selected entities

### Permissions & Security
- Five-level risk assessment (Safe → Destructive)
- High-risk operations require user confirmation
- Audit log records all Agent actions

### Multi-Engine Support (Planned)
- **Bevy** (Implemented): Rust ECS engine
- **Unity** (Planned): via gRPC/REST adapter
- **Godot** (Planned): via GDExtension adapter

---

## Architecture

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

### Crate Structure

| Crate | Responsibility | Status |
|-------|---------------|--------|
| `agent-core` | Agent orchestration, planning, memory, communication | Skeleton complete, core capabilities being filled |
| `agent-ui` | egui/bevy_egui UI rendering | Feature-rich, needs Event Stream decoupling |
| `bevy-adapter` | Bevy ECS bridge, SceneIndex, command execution | Basic implementation, component support needs expansion |

---

## Quick Start

### Requirements
- Rust 1.80+
- LLM API Key (OpenAI-compatible format)

### Configure LLM
Edit `crates/agent-core/src/llm.rs` or configure at runtime:
```rust
let llm = LlmClient::new()
    .with_base_url("https://api.openai.com/v1")
    .with_model("gpt-4o-mini")
    .with_api_key(std::env::var("OPENAI_API_KEY").unwrap());
```

### Run
```bash
cargo run
```

---

## Development Roadmap

### Phase 0: Baseline Verification (Completed)
- [x] Three-layer architecture (agent-core / agent-ui / bevy-adapter)
- [x] Basic DirectorRuntime orchestration
- [x] Five-level permission system
- [x] Basic Rollback/Transaction

### Phase 1: LLM Main Pipeline (In Progress)
- [x] Planner trait + RuleBasedPlanner/LlmPlanner
- [x] ReAct streaming execution + Observation closed loop
- [x] RuntimeContextCollector + TokenBudget
- [x] MemoryContext integration into PromptSystem
- [ ] SceneBridge empty implementation filling
- [ ] Model name configurability

### Phase M: Memory System Upgrade (Partially Complete)
- [x] Four-tier memory architecture (Working/Episodic/Semantic/Procedural)
- [x] BM25 + TF-IDF hybrid retrieval
- [x] RRF fusion ranking
- [ ] Persistent storage (disk serialization)
- [ ] Memory lifecycle management (decay/archival)

### Phase 2: Permission UI (Not Started)
- [ ] Visual permission configuration panel
- [ ] Audit log viewer

### Phase 3: Undo/Redo (Not Started)
- [ ] Command pattern completion
- [ ] History visualization

### Phase 4: Vision (Not Started)
- [ ] Screenshot + VLM analysis
- [ ] Visual feedback closed loop

### Phase 5: Multi-Agent (Not Started)
- [ ] A2A capability discovery
- [ ] Task coordinator

### Phase 9: Multi-Engine Adapter (Not Started)
- [ ] Unity adapter
- [ ] Godot adapter

---

## References

This project draws architectural inspiration from the following open-source projects:

| Project | Inspiration |
|---------|-------------|
| [hello-agents](https://github.com/datawhalechina/hello-agents) | ReAct/Plan-and-Solve/Reflection paradigms |
| [dive-into-llms](https://github.com/Lordog/dive-into-llms) | CoT reasoning enhancement, Prompt engineering |
| [supersplat](https://github.com/playcanvas/supersplat) | EditOp command pattern, EditHistory serialization |
| [UI-TARS-desktop](https://github.com/bytedance/UI-TARS-desktop) | Agent Event Stream, L0~L3 layered context |
| [agentmemory](https://github.com/rohitg00/agentmemory) | Four-tier memory, three-stream hybrid retrieval |
| [code-review-graph](https://github.com/tirth8205/code-review-graph) | Code structure graph, impact radius analysis |

---

## License

MIT License

---

> **Note**: This project is in early development; APIs may change frequently. Issues and PRs welcome!

---

## 中文

### 风浪 — AI 驱动的游戏编辑器

> **当前状态**: 早期开发阶段 (Pre-Alpha) | **核心架构**: Rust + Bevy ECS + LLM Agent

风浪是一个由 AI Agent 驱动的游戏编辑器。用户通过自然语言与 Agent 交互，Agent 理解意图、规划任务、执行操作，直接修改游戏场景和代码。

**核心特性**: 自然语言驱动开发、四层记忆系统、可插拔规划器、流式 ReAct 执行、分层上下文 (L0~L3)、权限与安全、多引擎支持 (规划中)。

详见上方英文文档获取完整信息。

---

## 日本語

### WindWave — AI 駆動型ゲームエディタ

> **現在の状態**: 早期開発段階 (Pre-Alpha) | **コアアーキテクチャ**: Rust + Bevy ECS + LLM Agent

WindWaveは、AI Agentによって駆動されるゲームエディタです。ユーザーは自然言語でAgentと対話し、Agentは意図を理解し、タスクを計画し、操作を実行して、ゲームシーンとコードを直接変更します。

**コア機能**: 自然言語駆動開発、4層メモリシステム、プラガブルプランナー、ストリーミングReAct実行、レイヤードコンテキスト (L0~L3)、権限とセキュリティ、マルチエンジン対応 (計画中)。

詳細は上記の英語ドキュメントをご参照ください。
