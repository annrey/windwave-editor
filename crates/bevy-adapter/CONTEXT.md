# Bevy Adapter

WindWave 的 Bevy 游戏引擎适配层。负责将 agent-core 抽象的 **SceneBridge** 操作转化为真实的 Bevy ECS 命令，并将引擎状态序列化为 **SceneIndex** 供 Agent 推理。

## Language

### Scene Representation

**SceneIndex (场景索引)**:
Bevy ECS 场景图的序列化快照，以层级树结构（**SceneEntityNode**）组织实体及其组件摘要。Agent 通过 SceneIndex 查询和推理场景状态，无需直接访问 Bevy World。
_Avoid_: SceneGraph, SceneSnapshot, EntityTree, WorldState

**SceneEntityNode (场景实体节点)**:
SceneIndex 树中的单个节点，包含实体 ID、名称、组件摘要列表和子节点列表。递归深度限制 MAX_TREE_DEPTH=256。
_Avoid_: EntityNode, SceneObject, EntityRecord

### Command Protocol

**EngineCommand (引擎命令)**:
Agent 向游戏引擎发送的操作指令 DSL。定义了 CreateEntity/DeleteEntity/SetTransform/SetSpriteColor/SetVisibility 等原子操作。
_Avoid_: EditorOp, SceneCommand, Action, Mutation

**ComponentPatch (组件补丁)**:
EngineCommand 中对单个组件的增量修改描述，指定组件类型和要更新的属性键值对。
_Avoid_: ComponentUpdate, PropertyDelta, ComponentDiff

### Bridge

**SceneBridge (场景桥接)**:
由 agent-core 定义的 trait（在 bevy-adapter 中实现），声明 query_entities/create_entity/update_component/delete_entity/get_scene_snapshot 等方法。是 agent-core 与引擎层的唯一契约。
_Avoid_: EngineAdapter, BackendInterface, Driver

## Relationships

- 一个 **SceneIndex** 包含多棵 **SceneEntityNode** 层级树
- 一条 **EngineCommand** 包含零或多个 **ComponentPatch**
- **BevyAdapter** 实现 **SceneBridge** trait
- **SceneBridge.receive_command(engine_command)** 解析 **EngineCommand** 并应用到 Bevy World
- **SceneBridge.get_scene_snapshot()** 遍历 Bevy World 构建 **SceneIndex**

## Example dialogue

> **Dev:** "Agent 怎么知道场景里有哪些实体？"
> **Domain expert:** "**BevyAdapter** 在每帧（或按需）遍历 Bevy World，将 Entity/Component 序列化为 **SceneIndex**。Agent 调用 `scene_bridge.query_entities()` 时拿到的是 **SceneIndex** 的副本，不直接接触 Bevy 类型。"
>
> **Dev:** "「把 Player 移到右边」这条指令最终怎么执行？"
> **Domain expert:** "Director 编排出的 Tool 调用通过 **SceneBridge** 发送一条 `SetTransform { entity_id: player_id, translation: [x,0,0] }` 形式的 **EngineCommand**，**BevyAdapter** 收到后在 Bevy World 中找到对应 Entity 并修改其 Transform 组件。"

## Flagged ambiguities

- "Adapter" 在本项目中有两层含义：bevy-adapter crate 本身（整个适配层）和 SceneBridge 的 Bevy 具体实现（BevyAdapter struct）— 前者是上下文名，后者是实现类
- "SceneBridge" trait 定义在 agent-core 中但实现在 bevy-adapter 中 — 这是跨上下文共享契约
