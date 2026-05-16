# Context Map

WindWave (风浪) 的领域上下文映射。本项目是一个 AI Agent 驱动的游戏编辑器，采用 Workspace monorepo 架构，按 crate 划分为三个独立上下文。

## Contexts

- [agent-core](./crates/agent-core/CONTEXT.md) — Agent 编排框架：导演、规划器、记忆系统、技能引擎、工具系统
- [bevy-adapter](./crates/bevy-adapter/CONTEXT.md) — Bevy 引擎适配层：场景索引、引擎命令、场景桥接
- [agent-ui](./crates/agent-ui/CONTEXT.md) — UI 层：导演控制台、事件流可视化

## Relationships

- **agent-core → bevy-adapter**: agent-core 通过 **SceneBridge** 抽象接口向 bevy-adapter 发送 **EngineCommand**；bevy-adapter 将 Bevy ECS 状态序列化为 **SceneIndex** 返回给 agent-core
- **agent-core → agent-ui**: agent-core 通过 **EventStream** 向 agent-ui 实时推送执行状态；agent-ui 的 **DirectorDesk** 展示 Director 的运行时状态
- **bevy-adapter ↔ agent-ui**: bevy-adapter 提供截图能力供 agent-ui 渲染预览（Vision 功能，规划中）

## Shared Types

| Type | Defined In | Used By |
|------|-----------|---------|
| `EntityId` | `agent-core/types` | All contexts |
| `EngineCommand` | `bevy-adapter` | `agent-core` (via SceneBridge) |
| `OperationRisk` | `agent-core` | `agent-ui` (approval dialog) |
| `EditPlan` | `agent-core` | `agent-ui` (plan display) |
