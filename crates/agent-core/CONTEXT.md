# Agent Core

WindWave 的核心 Agent 编排框架。负责接收用户自然语言请求、理解意图、规划任务、编排执行，并通过 SceneBridge 与游戏引擎交互。

## Language

### Orchestration

**Director (导演)**:
中央编排器，协调 Planner、Permission、Executor、GoalChecker、Reviewer 和 FallbackEngine 完成用户请求的全生命周期处理。
_Avoid_: Orchestrator, Controller, Manager

**Planner (规划器)**:
将用户请求分解为结构化的 **EditPlan**。支持两种策略：RuleBasedPlanner（关键词匹配、零延迟）和 LlmPlanner（LLM 驱动的 CoT 推理）。
_Avoid_: Scheduler, Decomposer

**Agent (代理)**:
可注册到 **AgentRegistry** 的自治处理单元，实现特定能力（CodeRead/CodeWrite/SceneRead/SceneWrite/Orchestrate）。每个 Agent 拥有独立的工具集和内部状态。
_Avoid_: Bot, Worker, Handler

### Execution

**Skill (技能)**:
DAG 工作流，定义一组有序的 **Tool** 调用及其条件转移逻辑（Always/OnSuccess/OnFailure）。由 **SkillExecutor** 以拓扑排序执行。
_Avoid_: Workflow, Pipeline, TaskChain

**Tool (工具)**:
原子操作单元，是 Agent 可执行的最小能力。注册到 **ToolRegistry**，按 Category 分组（Scene/Code/Asset/Engine/External/Utility）。
_Avoid_: Action, Operation, Command, Function

**EditPlan (编辑计划)**:
Planner 输出的结构化任务分解，包含步骤序列、风险等级和执行模式（Direct/Plan/Team）。
_Avoid_: Task, Job, Recipe

### Memory

**Working Memory (工作记忆, L3)**:
短期存储，保存当前对话的实体引用、计算值和临时变量。基于类型索引 + TTL 过期。
_Avoid_: ShortTermMemory, Scratchpad, Buffer

**Episodic Memory (情景记忆, L2)**:
中期存储，记录用户请求、工具调用和执行结果等事件序列。支持 BM25 + 时间衰减检索。
_Avoid_: EventLog, History, Journal

**Semantic Memory (语义记忆, L1)**:
长期存储，维护实体/组件/系统的概念图谱。支持 TF-IDF 余弦相似度语义检索。
_Avoid_: KnowledgeBase, ConceptGraph, Glossary

**Procedural Memory (程序记忆, L0)**:
长期存储，沉淀工作流模板和决策模式。基于关键词匹配 + 成功率排序。
_Avoid_: TemplateLibrary, PatternStore, Playbook

### Bridge

**SceneBridge (场景桥接)**:
引擎无关的场景操作抽象接口，解耦 agent-core 与具体游戏引擎实现。定义 query/create/update/delete 实体的契约。
_Avoid_: EngineAdapter, Backend, Driver

## Relationships

- 一个 **Director** 持有一个 **Planner** 和一个 **AgentRegistry**
- 一个 **Planner** 生成 **EditPlan**，**EditPlan** 包含多个 **Skill** 引用
- 一个 **Skill** 编排一个或多个 **Tool** 的 DAG 执行
- 一个 **Agent** 拥有专属的 **Tool** 集合
- 四层 **Memory** 构成层级检索链：Procedural → Semantic → Episodic → Working
- **Director** 通过 **SceneBridge** 向引擎发送命令，不直接依赖 Bevy 类型
- **SceneBridge** 在 bevy-adapter 中由 `BevyAdapter` 实现

## Example dialogue

> **Dev:** "当用户说「创建一个红色敌人」时，**Director** 怎么处理？"
> **Domain expert:** "**Director** 先将请求交给 **Planner** 生成 **EditPlan**（包含 create_entity + update_component 两步），然后 **PermissionEngine** 评估风险为 LowRisk，接着 **SkillExecutor** 按 DAG 顺序执行对应 **Tool**，最后 **Reviewer** 验证实体已创建且颜色正确。"
>
> **Dev:** "如果 LLM 不可用呢？"
> **Domain expert:** "**FallbackEngine** 接管——先匹配 **Procedural Memory** 中的模板，再尝试 **RuleBasedPlanner** 规则匹配，都失败则提示用户。"

## Flagged ambiguities

- "Memory" 在 v2 CODE_WIKI 中指三层架构（Conversation/Session/Persistent），v3 重构为四层（Working/Episodic/Semantic/Procedural）— 已统一为标准四层命名
- "Agent" 在通用 AI 语境中指 LLM 本身，在本项目中指注册到 AgentRegistry 的结构化处理单元 — 后者为本项目领域含义
