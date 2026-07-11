# WindWave / 风浪

**让 AI Agent 真正进入游戏编辑器。**

WindWave 是一个 Rust + Bevy 驱动的 AI Agent 游戏编辑器。长期愿景是让 AI
既能创造可玩的 3D 世界，也能自己进入世界游玩（个人服务器式世界，可邀请/加入）。
当前目标不是给编辑器加一个聊天框，而是让自然语言请求进入可规划、可执行、
可观察、可验证、可撤销的游戏编辑闭环，并先用小型开放世界切片证明内容真的能玩。

```text
说出意图 -> 生成计划 -> 修改场景 -> 观察结果 -> 修正偏差 -> 验证目标 -> 安全撤销
```

当前核心验收场景：

```text
创建一个红色敌人放在右边
```

Agent-native editing · Rust + Bevy · SceneBridge · SceneIndex · Undo/Redo ·
Director Desk · Multica-ready

## 为什么做 WindWave

游戏编辑器正在从“人操作工具”走向“人指挥系统”。传统编辑器擅长精确操作，
但复杂创作往往需要来回切换：设计意图、场景层级、资源、脚本、运行结果、
调试信息、版本回滚。LLM 可以理解意图，却常常缺少真实编辑器里的状态、
权限、撤销、观察和验证。

WindWave 试图补上中间这一层：

- **自然语言不是终点。** 用户说出的需求会被拆成结构化编辑计划和引擎命令。
- **Agent 必须看到世界。** 场景会被序列化为 `SceneIndex`，供 Agent 查询和推理。
- **每一步都应该可解释。** Director Desk 展示计划、事件、审批和执行状态。
- **创作必须能试错。** 写入型 `EngineCommand` 要有明确的 undo/redo 契约。
- **视觉结果要被验证。** 后续会用截图和 Vision feedback loop 检查真实画面。

## 快速开始

要求：

- Rust toolchain with Cargo
- macOS 或其他 Bevy/winit 支持的桌面环境
- 可选：VS Code / Cursor + `rust-analyzer`

运行编辑器：

```bash
make run
```

等价 Cargo 命令：

```bash
cargo run --bin agent-edit
```

常用质量门禁：

```bash
make check
make test
make clippy
make gate
```

等价 Cargo 命令：

```bash
cargo check
cargo test --workspace
cargo clippy --workspace -- -D warnings
```

运行当前 P0/P1 聚焦回归：

```bash
make smoke-p0-p1
```

这组 smoke 覆盖红色敌人闭环、HR approval、undo 反向命令、SceneIndex 删除
清理和失败计划修正等关键路径。

## 核心体验

### 1. 对编辑器说目标

用户可以把编辑动作写成自然语言，例如：

```text
创建一个红色敌人放在右边
```

WindWave 会把请求交给 Director。Director 负责选择规则路径、LLM 路径或团队
Agent 路径，并把任务变成可执行计划或直接的场景命令。

### 2. 让 Agent 生成可执行编辑

`agent-core` 中的 Planner、Skill、Tool、Permission、Review 和 Rollback 共同
构成执行链。低风险操作可以直接执行，高风险操作应该进入审批。

### 3. 通过 SceneBridge 修改真实场景

Agent 不直接操作 Bevy World。它通过 `SceneBridge` 发送引擎无关的场景操作，
再由 `bevy-adapter` 转换为 `EngineCommand` 并应用到 Bevy ECS。

### 4. 用 SceneIndex 观察结果

Bevy World 会被同步为 `SceneIndex`。Agent、UI 和测试都可以基于这个结构化
场景快照判断实体是否存在、位置是否正确、组件是否符合预期。

### 5. 在 Director Desk 里追踪和撤销

`agent-ui` 基于 egui / bevy_egui 构建，提供聊天、Director Desk、审批面板、
运行时 Agent 状态、Visual Understanding 状态和调试面板。用户应该能看到
Agent 做了什么，并在需要时撤销。

## 架构模型

```text
agent-ui
  Chat / Director Desk / Approval / Visual Understanding
        |
        v
agent-core
  Director -> Planner -> Permission -> Skill/Tool -> Review -> Rollback
        |
        v
SceneBridge
  引擎无关的场景操作契约
        |
        v
bevy-adapter
  EngineCommand -> Bevy ECS -> SceneIndex -> Screenshot / Perception
```

## Workspace 地图

| 路径 | 作用 |
| --- | --- |
| `Cargo.toml` | Rust workspace 与 `agent-edit` 二进制入口 |
| `src/main.rs` | Bevy App 入口，注册 Agent、UI、SceneIndex、Vision、CommandProcessor 等插件 |
| `crates/agent-core` | Director、Planner、Memory、Skill、Tool、Permission、Review、Rollback、Runtime Agent |
| `crates/bevy-adapter` | Bevy ECS 适配、`SceneBridge` 实现、`EngineCommand`、`SceneIndex`、截图、perception、undo/redo |
| `crates/agent-ui` | egui UI、Director Desk、chat、runtime panel、visual understanding state |
| `crates/multica-bridge` | Multica 任务同步、WebSocket、Agent proxy、skill adapter、scene context、本地 test server |
| `crates/ai-frameworks` | LangChain、LlamaIndex、DSPy 风格工作流的统一接口实验 |
| `crates/game-simulator` | 不启动完整引擎的 headless 游戏逻辑仿真 |
| `Makefile` | 本地运行、检查、测试和 smoke suite 的统一入口 |

## 能构建什么

WindWave 当前适合探索这些方向：

- **自然语言场景编辑**：把“创建 / 移动 / 改颜色 / 删除”等请求变成可撤销场景操作。
- **Agent 可观察编辑器**：把 Agent 计划、行动、审批和失败原因展示在 Director Desk。
- **AI 辅助关卡原型**：通过规则、LLM 和 SceneIndex 快速迭代小型玩法场景。
- **多 Agent 编辑团队**：让 Scene Agent、Code Agent、Review Agent、Planner Agent 分工协作。
- **外部任务平台桥接**：通过 Multica bridge 同步任务、技能、场景上下文和执行状态。
- **未来多引擎适配**：以 SceneBridge 为边界，把 Bevy 作为第一个引擎实现。

## 发展路线

### v0.2 - 闭环执行

目标：把“会规划”推进到“能稳定改场景并可回滚”。

- 自然语言请求生成计划或规则执行路径
- `EngineCommand` 修改 Bevy World
- `SceneIndex` 观察到真实变化
- Review / GoalChecker 给出成功或修正路径
- Undo 能逆转写入操作
- UI、Director events、SceneIndex 对同一次操作描述一致

### v0.3 - 记忆与上下文

目标：让 Agent 带着项目上下文工作，而不是每次从零开始。

- Working / Episodic / Semantic / Procedural 四层记忆有明确注入策略
- 失败案例能影响后续相似请求
- prompt 注入受预算控制
- 关键任务有可复现回归测试

### v0.4 - 视觉反馈与混合编辑

目标：不只相信命令返回，还要看画面是否符合目标。

- operation -> screenshot -> vision verify -> revise
- Vision 失败能触发修正或清晰失败报告
- 规则、LLM、人工确认和视觉验证进入统一 hybrid policy
- Runtime Agent、Task Panel、Director events 状态同步

### v0.5 - 平台化与生态集成

目标：让 WindWave 从本地原型走向可扩展编辑平台。

- 真实 Multica server smoke test
- 多 Agent 团队流进入真实 DirectorRuntime
- CodeGraph / ImpactAnalyzer / CodeContextGenerator
- Prefab、asset、hierarchy、多选、Transform Palette 深化
- Godot / Unreal adapter 最小链路验证

## 当前状态

WindWave 处在早期但主体骨架已经成型的阶段。已有模块包括 Director、Planner、
SceneBridge、SceneIndex、Memory、EventStream、权限、回滚、审计、agent-ui、
bevy-adapter、open-world 切片与 VerificationBundle 等。

当前最重要的工程目标是 AI 居民沙盘（PRD：`docs/prd/ai-resident-sandbox.md`，
尚未实现；OpenWorldSlice01 主窗口 framebuffer 验收已于 2026-07-11 通过，回归用
`make accept-open-world-qa`）。新功能如果不能帮助「生成 → 可玩 → 可观测 → 可验证 →
可回放」，优先级应低于该证据链。详细派单见 `docs/remaining-work.md`。

## 仓库边界

这个 checkout 中仍混有历史或并置的 Understand Anything / Node workspace 内容，
例如 `package.json`、`pnpm-*`、`understand-anything-plugin/`、`READMEs/`、
`homepage/` 等。

WindWave 的真实运行入口是 Rust workspace：

```text
Cargo.toml
src/
crates/
```

请使用 Cargo / Makefile 判断 WindWave 的构建与测试健康状态。`pnpm build`、
`pnpm test` 等命令属于并置的 Node workspace，不代表 WindWave 编辑器状态。

## 文档与发布原则

开发、设计、规划等文档在稳定前应保留在本地项目文档中，不默认推送到 GitHub。
公开 README 应保持外宣和上手导向：说明愿景、能力、使用方式、当前边界和路线图，
避免把未完成的内部计划包装成已经交付的功能。

## License

See `LICENSE`.
