# Agent UI

WindWave 的用户界面层，基于 egui/bevy_egui 构建。负责可视化 Agent 团队的运行时状态、展示执行计划、呈现审批对话框，并通过 EventStream 接收 agent-core 的实时事件。

## Language

### Visualization

**DirectorDesk (导演控制台)**:
多面板 UI 组件，可视化展示 Director 运行时状态。包含 Current Plan（当前计划）、Agent Status（Agent 在线状态）、Pending Approval（待审批任务）、Undo/Redo Log（回滚日志）等面板。
_Avoid_: Dashboard, ControlPanel, Monitor, Console

**EventStream (事件流)**:
agent-core → agent-ui 的实时数据管道。Director 执行的每一步（思考/行动/观察）通过 EventBus 推送，UI 订阅后渲染为可读的事件流和执行追踪。
_Avoid_: EventPipe, DataChannel, Stream, Feed, LogStream

## Relationships

- **DirectorDesk** 订阅 **EventStream** 来更新面板数据
- **EventStream** 源自 agent-core 的 **EventBus**，经 Agent-UI Protocol 传递到 agent-ui
- **DirectorDesk** 的 Approval Dialog 面板展示 **OperationRisk** >= HighRisk 的待审批 **EditPlan**

## Example dialogue

> **Dev:** "用户怎么知道 Agent 正在做什么？"
> **Domain expert:** "**DirectorDesk** 的 Events & Trace 面板实时渲染 **EventStream** 数据——每条记录对应 ReAct 循环的一步（Think/Act/Observation），用户能看到 Agent 的完整推理过程。"
>
> **Dev:** "高风险操作怎么拦截？"
> **Domain expert:** "当 **Planner** 生成的 **EditPlan** 风险等级 >= HighRisk 时，**Director** 不自动执行，而是通过 **EventStream** 发送审批请求，**DirectorDesk** 的 Pending Approval 弹出对话框等待用户确认。"

## Flagged ambiguities

- "Desk" 在通用 UI 语境中可能指桌面应用，在本项目中特指 DirectorDesk 这个多面板组件 — 缩写时用 DD 而非 Desk
