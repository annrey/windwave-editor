# AgentEdit Code Wiki

## 项目概述

**AgentEdit** 是一个由 AI Agent 驱动的游戏编辑器，采用 Rust + Bevy 引擎构建。项目实现了基于 OpenClaw/OpenManus 设计模式的 Agent 架构，支持自然语言指令到游戏场景操作的自动化转换。

## 项目结构

```
风浪/
├── src/
│   └── main.rs                    # 主入口，Bevy应用配置
├── crates/
│   ├── agent-core/               # 核心Agent框架
│   │   ├── src/
│   │   │   ├── lib.rs            # 模块入口与re-export
│   │   │   ├── types.rs          # 核心类型(EntityId, Message, AgentAction等)
│   │   │   ├── agent.rs          # BaseAgent状态机
│   │   │   ├── director.rs       # DirectorRuntime编排器
│   │   │   ├── planner.rs        # RuleBasedPlanner计划生成器
│   │   │   ├── router.rs         # SmartRouter路由选择
│   │   │   ├── registry.rs       # AgentRegistry注册中心
│   │   │   ├── skill.rs          # Skill系统(DAG执行引擎)
│   │   │   ├── scene_bridge.rs   # SceneBridge接口
│   │   │   ├── plan.rs           # 编辑计划类型定义
│   │   │   ├── goal_checker.rs   # 目标状态检测器
│   │   │   ├── permission.rs     # 权限引擎
│   │   │   ├── rollback.rs       # 回滚管理器
│   │   │   ├── transaction.rs    # 事务系统
│   │   │   ├── event.rs          # 事件总线系统
│   │   │   ├── memory.rs         # 三层内存系统
│   │   │   ├── builtin_skills.rs # 内置技能定义
│   │   │   ├── code_tools.rs     # 代码生成工具
│   │   │   ├── scene_tools.rs    # 场景操作工具
│   │   │   ├── engine_tools.rs   # 引擎控制工具
│   │   │   ├── specialized_agents.rs # 专业Agent实现
│   │   │   ├── scene_agent.rs    # 场景Agent
│   │   │   ├── tool.rs           # 工具注册与执行系统
│   │   │   ├── prompt.rs         # 分层提示词系统
│   │   │   ├── llm.rs            # LLM客户端(OpenAI)
│   │   │   ├── metrics.rs        # 性能指标追踪
│   │   │   ├── review.rs         # 审查决策引擎
│   │   │   └── fallback.rs       # 降级引擎(无LLM时)
│   │   └── Cargo.toml
│   ├── bevy-adapter/             # Bevy引擎适配器
│   │   ├── src/
│   │   │   ├── lib.rs            # EngineCommand DSL + BevyAdapter
│   │   │   ├── integration.rs    # 集成系统(SceneIndex构建/Vision)
│   │   │   └── scene_index.rs    # 场景层级索引结构
│   │   └── Cargo.toml
│   └── agent-ui/                 # UI组件(egui)
│       ├── src/
│       │   └── director_desk.rs  # 导演控制台UI
│       └── Cargo.toml
├── design/                       # 设计文档
├── session-log/                  # 会话日志
└── Cargo.toml                    # 工作区配置
```

---

## 核心架构

### 三层架构设计

| 层级 | 组件 | 职责 |
|------|------|------|
| **Agent层** | DirectorRuntime, Planner, Router, Agents | 处理自然语言请求，生成执行计划 |
| **桥接层** | SceneBridge, EngineCommand | 引擎抽象接口，解耦Agent与引擎 |
| **引擎层** | BevyAdapter, SceneIndex | 真实Bevy ECS操作执行 |

### 执行流程

```
User Request → DirectorRuntime.handle_user_request()
                    │
                    ├─→ FallbackEngine.execute() → 检查本地模板/规则
                    │       │
                    │       └─→ TemplateApplied / RuleMatched → 直接执行
                    │       └─→ LlmUnavailable → 进入LLM路径
                    │
                    ├─→ SmartRouter.route() → 选择执行模式(Direct/Plan/Team)
                    │
                    ├─→ RuleBasedPlanner.create_plan() → 生成编辑计划
                    │
                    ├─→ PermissionEngine.check() → 权限验证
                    │
                    ├─→ execute_plan_internal() → 执行计划
                    │       │
                    │       ├─→ SkillExecutor.execute() → DAG技能执行
                    │       │       │
                    │       │       └─→ SceneBridgeSkillHandler → 桥接调用
                    │       │
                    │       └─→ drain_bridge_commands() → 收集引擎命令
                    │
                    ├─→ GoalChecker.check() → 验证目标达成
                    │
                    └─→ Reviewer.review() → 审查决策(Accept/Retry/Rollback/AskUser)
                              │
                              └─→ BevyAdapter.apply_engine_command() → ECS操作
```

---

## 核心模块详解

### 1. DirectorRuntime

**职责**: 中央编排器，协调 Planner、Permission、Executor、GoalChecker、Reviewer 和 FallbackEngine。

**核心方法**:

| 方法 | 功能 |
|------|------|
| `handle_user_request()` | 处理用户请求，智能路由选择 |
| `create_plan_internal()` | 创建编辑计划 |
| `check_permission_internal()` | 权限检查 |
| `execute_plan_internal()` | 执行计划 |
| `drain_bridge_commands()` | 收集待执行的引擎命令 |
| `event_bus()` | 获取事件总线引用 |

**文件**: [director.rs](file:///Users/chengyongwei/Library/Mobile Documents/com~apple~CloudDocs/gameedit/风浪/crates/agent-core/src/director.rs)

### 2. RuleBasedPlanner

**职责**: 基于规则的编辑计划生成器，分析用户请求并生成结构化的执行步骤。

**核心功能**:
- 复杂度评估 (Simple/Medium/Complex)
- 风险评估 (Safe/LowRisk/MediumRisk/HighRisk/Destructive)
- 执行模式选择 (Direct/Plan/Team)
- 步骤构建 (创建/删除/修改实体)

**关键类型**:
- `PlannerContext`: 计划上下文 (task_id, available_tools, scene_entity_names)
- `ComplexityScore`: 复杂度评分 (0-10)

**文件**: [planner.rs](file:///Users/chengyongwei/Library/Mobile Documents/com~apple~CloudDocs/gameedit/风浪/crates/agent-core/src/planner.rs)

### 3. SmartRouter

**职责**: 智能路由选择器，根据请求复杂度和风险自动选择执行模式。

**决策规则**:
- **Destructive/HighRisk** → Plan模式（必须人工确认）
- **代码生成** → Plan模式
- **批量操作** → Plan模式
- **复杂度得分 ≤ 3** → Direct模式（直接执行）

**文件**: [router.rs](file:///Users/chengyongwei/Library/Mobile Documents/com~apple~CloudDocs/gameedit/风浪/crates/agent-core/src/router.rs)

### 4. SceneBridge

**职责**: 引擎无关的场景操作接口，实现agent-core与真实游戏引擎的解耦。

**实现**:
- `MockSceneBridge`: 内存模拟实现（测试用）
- `SceneIndexSceneBridge`: 使用SceneIndexCache的实现

**核心方法**:
| 方法 | 功能 |
|------|------|
| `query_entities()` | 查询实体列表 |
| `create_entity()` | 创建实体 |
| `update_component()` | 更新组件属性 |
| `delete_entity()` | 删除实体 |
| `get_scene_snapshot()` | 获取场景快照 |

**文件**: [scene_bridge.rs](file:///Users/chengyongwei/Library/Mobile Documents/com~apple~CloudDocs/gameedit/风浪/crates/agent-core/src/scene_bridge.rs)

### 5. BevyAdapter

**职责**: Bevy引擎的具体适配实现，处理Agent与Bevy ECS的交互。

**核心功能**:
- 实体ID映射 (AgentId ↔ Bevy Entity)
- EngineCommand执行
- SceneIndex构建
- 回滚操作支持

**文件**: [lib.rs](file:///Users/chengyongwei/Library/Mobile Documents/com~apple~CloudDocs/gameedit/风浪/crates/bevy-adapter/src/lib.rs)

### 6. GoalChecker

**职责**: 目标状态检测器，验证编辑操作是否达到预期目标。

**支持的目标类型**:
- `EntityExists`: 检查实体是否存在
- `HasComponent`: 检查实体是否拥有特定组件
- `TransformNear`: 检查实体位置是否在容差范围内
- `SpriteColorIs`: 检查精灵颜色是否匹配

**文件**: [goal_checker.rs](file:///Users/chengyongwei/Library/Mobile Documents/com~apple~CloudDocs/gameedit/风浪/crates/agent-core/src/goal_checker.rs)

### 7. Skill系统

**职责**: DAG-based工作流引擎，用于Agent编排。

**核心组件**:
- `SkillDefinition`: 技能定义（节点+边+输入）
- `SkillNode`: DAG节点（工具调用+重试策略+回滚）
- `SkillEdge`: DAG边（条件转移：Always/OnSuccess/OnFailure）
- `SkillExecutor`: DAG执行器（拓扑排序+执行+验证）
- `SkillRegistry`: 技能注册中心

**关键类型**:
```rust
pub struct SkillDefinition {
    pub id: SkillId,
    pub name: String,
    pub description: String,
    pub inputs: Vec<SkillInput>,
    pub nodes: Vec<SkillNode>,
    pub edges: Vec<SkillEdge>,
}

pub struct SkillNode {
    pub id: String,
    pub title: String,
    pub required_capability: CapabilityKind,
    pub tool_name: Option<String>,
    pub input_mapping: serde_json::Value,
    pub retry: RetryPolicy,
    pub rollback: Option<String>,
}
```

**文件**: [skill.rs](file:///Users/chengyongwei/Library/Mobile Documents/com~apple~CloudDocs/gameedit/风浪/crates/agent-core/src/skill.rs)

### 8. 内置技能 (builtin_skills)

**职责**: 预定义的DAG工作流，覆盖常见编辑器操作。

| 技能名 | DAG流程 | 输入 |
|--------|---------|------|
| `create_entity` | query_scene → create_entity → verify_entity_exists | entity_type, position |
| `modify_entity_transform` | resolve_entity → update_transform → verify_transform | entity_name, new_position |
| `query_scene` | list_entities (单节点) | filter |
| `import_asset` | locate_file → validate_format → register_asset | file_path |

**注册函数**: `register_builtin_skills(registry: &mut SkillRegistry)`

**文件**: [builtin_skills.rs](file:///Users/chengyongwei/Library/Mobile Documents/com~apple~CloudDocs/gameedit/风浪/crates/agent-core/src/builtin_skills.rs)

### 9. 专业Agent实现 (specialized_agents)

**职责**: 将各子系统封装为Agent trait实现，可注册到AgentRegistry。

| Agent | 能力 | 内部组件 |
|-------|------|----------|
| `CodeAgent` | CodeRead, CodeWrite | ToolRegistry (code_tools) |
| `ReviewAgent` | RuleCheck, CodeRead | Reviewer |
| `EditorAgent` | 全部7种能力 | DirectorRuntime |
| `PlannerAgent` | Orchestrate | RuleBasedPlanner |

**Agent请求处理流程**:
```
AgentRequest { instruction, context, task_id }
    → Agent.handle()
    → 解析指令 → 调用内部工具/组件
    → AgentResponse { result: Success/Failed, events }
```

**文件**: [specialized_agents.rs](file:///Users/chengyongwei/Library/Mobile Documents/com~apple~CloudDocs/gameedit/风浪/crates/agent-core/src/specialized_agents.rs)

### 10. SceneAgent

**职责**: 专门处理场景实体读写操作的Agent。

**核心功能**:
- 内置NL指令解析器 `parse_instruction()`
- 支持中文/英文双语指令
- 自动将自然语言映射为工具调用序列

**指令解析示例**:
| 用户输入 | 解析结果 |
|----------|----------|
| "创建一个红色敌人放在右侧" | create_entity(enemy) + update_component(color=red) + update_component(pos=right) |
| "查询所有实体" | query_entities() |
| "hello" (未知指令) | 降级为 query_entities() |

**文件**: [scene_agent.rs](file:///Users/chengyongwei/Library/Mobile Documents/com~apple~CloudDocs/gameedit/风浪/crates/agent-core/src/scene_agent.rs)

### 11. 工具系统 (tool.rs)

**职责**: 统一的工具注册、查找与执行框架。

**核心类型**:

| 类型 | 说明 |
|------|------|
| `Tool` trait | 工具接口：name, description, parameters, category, execute |
| `ToolRegistry` | 工具注册中心：register, get, has, execute, execute_all |
| `ToolParameter` | 参数定义：name, param_type, required, default |
| `ParameterType` | 参数类型：String/Number/Boolean/EntityId/Vec2/Vec3/Color/Enum/Object/Array |
| `ToolCategory` | 工具分类：Scene/Code/Asset/Engine/External/Utility |
| `ToolResult` | 执行结果：success, message, data, execution_time_ms |
| `ToolCall` | 工具调用请求：tool_name, parameters, call_id |
| `ParameterBuilder` | 参数构建器（流式API） |

**LLM集成方法**:
- `describe_all()`: 生成所有工具描述供LLM理解
- `describe_relevant(context)`: 基于关键词匹配生成相关工具描述

**文件**: [tool.rs](file:///Users/chengyongwei/Library/Mobile Documents/com~apple~CloudDocs/gameedit/风浪/crates/agent-core/src/tool.rs)

### 12. 代码生成工具 (code_tools)

**职责**: 生成Bevy游戏代码和组件。

| 工具 | 功能 | 参数 |
|------|------|------|
| `GenerateComponentTool` | 生成Bevy Component结构体 | name, properties, derives |
| `GenerateSystemTool` | 生成Bevy System函数 | name, query, logic |
| `GenerateResourceTool` | 生成Bevy Resource结构体 | name, fields |
| `GenerateEventTool` | 生成Bevy Event结构体 | name, fields |
| `FormatCodeTool` | 基础代码格式化 | code |
| `AnalyzeCodeTool` | 代码结构分析(提取Component/System/Resource) | code |

**注册函数**: `register_code_tools(registry: &mut ToolRegistry)`

**文件**: [code_tools.rs](file:///Users/chengyongwei/Library/Mobile Documents/com~apple~CloudDocs/gameedit/风浪/crates/agent-core/src/code_tools.rs)

### 13. 场景操作工具 (scene_tools)

**职责**: 场景实体和组件的CRUD操作。

| 工具 | 功能 | 参数 |
|------|------|------|
| `QueryEntitiesTool` | 按名称/组件查询实体 | filter, with_component |
| `GetEntityTool` | 获取单个实体详情 | entity_id |
| `CreateEntityTool` | 创建新实体 | name, position, components |
| `UpdateComponentTool` | 更新组件属性 | entity_id, component, property, value |
| `DeleteEntityTool` | 删除实体(需确认) | entity_id, confirm |

**注册函数**: `register_scene_tools(registry: &mut ToolRegistry)`

**文件**: [scene_tools.rs](file:///Users/chengyongwei/Library/Mobile Documents/com~apple~CloudDocs/gameedit/风浪/crates/agent-core/src/scene_tools.rs)

### 14. 引擎控制工具 (engine_tools)

**职责**: 引擎级操作（构建、运行、导出、代码审查）。

| 工具 | 功能 | 参数 |
|------|------|------|
| `GetEngineStateTool` | 查询引擎状态(FPS/实体数/内存) | metrics |
| `BuildProjectTool` | 编译项目(cargo check/build/clippy) | mode |
| `PlayGameTool` | 启动游戏测试运行 | profile, dry_run |
| `ExportAssetTool` | 导出项目资源 | target_dir, asset_patterns |
| `ReviewCodeTool` | 代码审查(clippy+fmt+check) | checks |
| `ApplyCodeChangeTool` | 将代码变更写入文件 | file_path, content, dry_run |

**注册函数**: `register_engine_tools(registry: &mut ToolRegistry)`

**文件**: [engine_tools.rs](file:///Users/chengyongwei/Library/Mobile Documents/com~apple~CloudDocs/gameedit/风浪/crates/agent-core/src/engine_tools.rs)

### 15. 三层内存系统 (memory.rs)

**职责**: Agent的分层记忆架构，支持短期/中期/长期记忆。

| 层级 | 组件 | 生命周期 | 核心功能 |
|------|------|----------|----------|
| **短时** | `ConversationMemory` | 对话窗口内 | 消息历史、循环检测、上下文构建 |
| **中时** | `SessionMemory` | 编辑器会话期间 | 工作变量、实体引用、选择上下文、操作日志 |
| **长时** | `PersistentMemory` | 持久化(JSON文件) | 用户偏好、学习模式、实体知识库 |

**ConversationMemory**:
- 最大消息数限制，超限自动裁剪
- `detect_cycle()`: 循环检测（Agent卡住检测）
- `build_context()`: 构建LLM上下文字符串

**WorkingMemory**:
- 键值存储 (`MemoryValue`: String/Number/Bool/Entity/List/Json)
- 实体引用注册与查找
- 最近执行结果缓存（最多10条）

**SelectionContext**:
- 当前选中实体列表
- 活跃组件列表
- 上下文标签 (`ContextTag`: Entity/Topic/Command/Urgent)

**PersistentMemory**:
- `UserPreferences`: preferred_engine, code_style, confirmation_level, theme
- `ConfirmationLevel`: Always / Destructive / Never
- `LearnedPattern`: 预置6个Bevy代码模板 (component, system, player_movement, camera_follow, spawn_on_click, endless_scrolling_bg)
- `EntityKnowledge`: 实体常用操作和关联实体
- `save()` / `load()`: JSON序列化持久化

**文件**: [memory.rs](file:///Users/chengyongwei/Library/Mobile Documents/com~apple~CloudDocs/gameedit/风浪/crates/agent-core/src/memory.rs)

### 16. 事务系统 (transaction.rs)

**职责**: 原子性操作记录，支持操作回滚。

**核心类型**:
- `EditTransaction`: 单个事务记录 (id, operations, status, snapshot)
- `TransactionStore`: 事务存储管理
- `TransactionStatus`: Open → Committed / RolledBack / Failed

**文件**: [transaction.rs](file:///Users/chengyongwei/Library/Mobile Documents/com~apple~CloudDocs/gameedit/风浪/crates/agent-core/src/transaction.rs)

### 17. 事件总线系统 (event.rs)

**职责**: 进程内发布/订阅事件总线，解耦模块间通信。

**核心类型**:
- `EventBus`: 事件推送和订阅管理
- `EventBusEvent`: 事件类型枚举
  - `EngineCommandApplied { transaction_id, success, message }`
  - 其他引擎/Agent事件
- `EventSource`: 事件来源标识

**文件**: [event.rs](file:///Users/chengyongwei/Library/Mobile Documents/com~apple~CloudDocs/gameedit/风浪/crates/agent-core/src/event.rs)

### 18. 回滚管理器 (rollback.rs)

**职责**: 操作日志与撤销/重做栈管理。

**核心组件**:
- `RollbackManager`: 撤销/重做栈管理
- `OperationLog`: 操作日志条目
- `SceneSnapshot`: 场景快照（用于恢复）

**文件**: [rollback.rs](file:///Users/chengyongwei/Library/Mobile Documents/com~apple~CloudDocs/gameedit/风浪/crates/agent-core/src/rollback.rs)

### 19. 权限引擎 (permission.rs)

**职责**: 风险评估和操作审批。

**核心类型**:
- `OperationRisk`: Safe → LowRisk → MediumRisk → HighRisk → Destructive (五级风险)
- `PermissionPolicy`: 可配置的权限策略
- `PermissionRequirement`: 权限需求类型
- `PermissionDecision`: Allow / Deny / RequireApproval

**文件**: [permission.rs](file:///Users/chengyongwei/Library/Mobile Documents/com~apple~CloudDocs/gameedit/风浪/crates/agent-core/src/permission.rs)

### 20. 审查决策引擎 (review.rs)

**职责**: 任务完成后审查当前状态，决定下一步行动。

**审查决策**:
| 决策 | 条件 | 说明 |
|------|------|------|
| `Accept` | 所有需求满足 | 任务成功完成 |
| `Retry` | 少量失败(≤2) | Agent可自主重试 |
| `RetryOrAskUser` | 少量失败(≤2) | 重试或询问用户 |
| `Rollback` | 变更不正确 | 回滚后尝试其他方案 |
| `AskUser` | 大量失败(>2) | 需要用户手动干预 |

**审查模式**:
- `review()`: 常规审查（允许重试）
- `review_strict()`: 严格审查（高风险操作，必须全部通过）

**文件**: [review.rs](file:///Users/chengyongwei/Library/Mobile Documents/com~apple~CloudDocs/gameedit/风浪/crates/agent-core/src/review.rs)

### 21. 降级引擎 (fallback.rs)

**职责**: LLM不可用时的优雅降级，通过本地规则+模板库满足常见请求。

**三层降级策略**:
1. **模板库** (`TemplateLibrary`): 关键词匹配预定义模板
2. **规则引擎** (`RuleEngine`): 结构化字段匹配
3. **LLM不可用** (`LlmUnavailable`): 返回可用模板列表和建议

**预置模板**:
| 模板名 | 触发关键词 |
|--------|-----------|
| `delete_entity` | 删除, delete, 移除 |
| `create_player` | 创建玩家, create player |
| `create_enemy` | 创建敌人, create enemy, 生成敌人 |
| `query_scene` | 查询, query, 列出, list |
| `create_camera` | 相机, camera |

**降级结果**:
- `TemplateApplied`: 模板匹配成功
- `RuleMatched`: 规则匹配成功
- `LlmUnavailable`: 无本地路径可处理

**文件**: [fallback.rs](file:///Users/chengyongwei/Library/Mobile Documents/com~apple~CloudDocs/gameedit/风浪/crates/agent-core/src/fallback.rs)

### 22. 分层提示词系统 (prompt.rs)

**职责**: 三层提示词工程架构，组合基础模板、引擎扩展和用户自定义。

**三层模板**:
1. **Base**: 引擎无关的通用模板
2. **Engine-specific**: 特定引擎的扩展（如Bevy ECS指南）
3. **User-custom**: 用户自定义覆盖（最高优先级）

**提示词类型** (`PromptType`):
| 类型 | 用途 |
|------|------|
| `SystemIdentity` | Agent身份和角色定义 |
| `SystemCapabilities` | 能力描述 |
| `TaskPlanning` | 任务规划指令 |
| `TaskDecomposition` | 任务分解策略 |
| `ToolSelection` | 工具选择启发式 |
| `CodeGeneration` | 代码生成指南 |
| `SceneManipulation` | 场景操作指令 |
| `ResponseFormatting` | 响应格式规则 |
| `ErrorExplanation` | 错误解释模板 |
| `ClarificationRequest` | 澄清请求模板 |

**变量替换**: 支持 `{agent_name}`, `{engine_name}`, `{language}`, `{project_name}`, `{engine_version}`, `{selected_entities}` 及自定义变量

**构建流程**: `build_prompt(type, context)` → Base模板 → 追加Engine扩展 → 变量替换

**文件**: [prompt.rs](file:///Users/chengyongwei/Library/Mobile Documents/com~apple~CloudDocs/gameedit/风浪/crates/agent-core/src/prompt.rs)

### 23. LLM客户端 (llm.rs)

**职责**: 统一的LLM API接口，支持结构化输出和工具调用。

**支持提供商**:
- `OpenAI` (已实现) — gpt-4o-mini 默认模型
- `Claude` (预留接口)

**核心类型**:
| 类型 | 说明 |
|------|------|
| `LlmConfig` | 配置(provider, api_key, model, temperature, timeout) |
| `LlmClient` trait | 异步接口: chat(), is_ready(), provider() |
| `OpenAiClient` | OpenAI实现(基于reqwest) |
| `LlmRequest` | 请求(model, messages, tools, max_tokens, temperature) |
| `LlmResponse` | 响应(content, tool_calls, usage) |
| `ToolDefinition` | 函数调用工具定义 |
| `TokenUsage` | Token使用统计 |

**辅助函数**:
- `build_chat_request(model, messages)`: 构建简单请求
- `with_tools(request, tools)`: 添加工具定义
- `create_llm_client(config)`: 工厂方法创建客户端

**安全**: `LlmConfig` 的 `Debug` 实现隐藏 api_key

**文件**: [llm.rs](file:///Users/chengyongwei/Library/Mobile Documents/com~apple~CloudDocs/gameedit/风浪/crates/agent-core/src/llm.rs)

### 24. 性能指标追踪 (metrics.rs)

**职责**: Agent调用的结构化性能数据收集。

**AgentMetrics 指标**:
| 类别 | 字段 |
|------|------|
| **计时** | total_execution_time, llm_call_time, tool_execution_time, thinking_time |
| **调用统计** | llm_calls, tool_calls, tokens_sent, tokens_received |
| **成功率** | successful_operations, failed_operations, retried_operations |
| **内存** | peak_memory_mb, context_window_size |

**派生指标**:
- `avg_llm_latency_ms()`: 平均LLM延迟
- `success_ratio()`: 成功率 [0, 1]

**PerformanceTracer**: 通用计时辅助
- `trace(name, f)`: 异步闭包计时
- `trace_sync(name, f)`: 同步闭包计时

**文件**: [metrics.rs](file:///Users/chengyongwei/Library/Mobile Documents/com~apple~CloudDocs/gameedit/风浪/crates/agent-core/src/metrics.rs)

### 25. SceneIndex 场景索引 (bevy-adapter)

**职责**: Bevy ECS场景图的序列化快照，供Agent查询推理。

**核心结构**:
```rust
pub struct SceneIndex {
    pub root_entities: Vec<SceneEntityNode>,       // 层级实体树
    pub entities_by_name: HashMap<String, u64>,     // 名称→ID索引
    pub entities_by_component: HashMap<String, Vec<u64>>, // 组件→实体索引
}

pub struct SceneEntityNode {
    pub id: u64,
    pub name: String,
    pub components: Vec<ComponentSummary>,
    pub children: Vec<SceneEntityNode>,
}
```

**查询方法**:
| 方法 | 功能 |
|------|------|
| `get_entity_by_name()` | 按名称查找实体(递归) |
| `get_entities_with_component()` | 按组件类型查找实体 |
| `entity_names()` | 获取所有实体名称 |
| `to_entity_info_list()` | 转换为扁平列表(供GoalChecker使用) |

**安全**: 递归深度限制 `MAX_TREE_DEPTH = 256`

**文件**: [scene_index.rs](file:///Users/chengyongwei/Library/Mobile Documents/com~apple~CloudDocs/gameedit/风浪/crates/bevy-adapter/src/scene_index.rs)

### 26. DirectorDesk UI (agent-ui)

**职责**: 多面板导演控制台界面，可视化Agent团队运行状态。

**面板布局**:
- **左侧面板** (280px):
  - Current Plan: 当前执行计划及步骤状态
  - Agent Status: 5个Agent的在线状态 (Director/SceneAgent/CodeAgent/AssetAgent/RuleAgent)
  - Pending Approval: 待审批的高风险计划
  - Tasks: 任务列表及进度条
  - Goals: 目标检查结果
  - Undo/Redo Log: 回滚日志
- **底部面板** (180px):
  - Events & Trace: 事件流和执行追踪

**Bevy集成**: 作为 `DirectorDeskPlugin` 注册，自动初始化 `DirectorDeskState` Resource

**文件**: [director_desk.rs](file:///Users/chengyongwei/Library/Mobile Documents/com~apple~CloudDocs/gameedit/风浪/crates/agent-ui/src/director_desk.rs)

---

## 关键数据类型

### 核心类型 (types.rs)

```rust
pub struct EntityId(pub u64);                    // 实体唯一标识
pub struct AgentId(pub u64);                     // Agent唯一标识

pub enum MessageType { User, Agent, System, Action, Thought, Observation }

pub struct Message {
    pub id: String,
    pub message_type: MessageType,
    pub content: String,
    pub timestamp: u64,
    pub metadata: HashMap<String, String>,
}

pub enum TagType { Entity, Topic, Command, Urgent }

pub struct ContextTag { pub tag_type: TagType, pub value: String, pub active: bool }

pub enum PropertyValue {
    Float(f64), Int(i64), Bool(bool), String(String),
    Vec2 { x: f32, y: f32 }, Vec3 { x: f32, y: f32, z: f32 },
    Color { r: f32, g: f32, b: f32, a: f32 },
}

pub enum AgentAction {
    CreateComponent { entity_id, component_type, properties },
    UpdateComponent { entity_id, component_name, property, value },
    DeleteComponent { entity_id, component_name },
    GenerateCode { template, context },
    ExecuteCommand { command, args },
}

pub enum AdapterError {
    EntityNotFound, ComponentNotFound, EngineNotConnected,
    ActionNotSupported, InvalidProperty,
}

pub enum AgentError {
    MaxStepsReached, Stuck, LLMUnavailable, ToolError, Timeout, UserCancelled,
}
```

### EditPlan

编辑计划，将任务分解为可执行步骤：

```rust
pub struct EditPlan {
    pub id: String,
    pub task_id: u64,
    pub title: String,
    pub mode: ExecutionMode,       // Direct/Plan/Team
    pub steps: Vec<EditPlanStep>,
    pub risk_level: OperationRisk,  // Safe→Destructive
    pub status: EditPlanStatus,     // Draft/Approved/Running/Completed
}
```

### EngineCommand

引擎命令DSL，Agent向引擎发送的操作指令：

```rust
pub enum EngineCommand {
    CreateEntity { name, components: Vec<ComponentPatch> },
    DeleteEntity { entity_id: u64 },
    SetTransform { entity_id, translation, rotation, scale },
    SetSpriteColor { entity_id, rgba: [f32; 4] },
    SetVisibility { entity_id, visible: bool },
}
```

### OperationRisk

风险等级枚举：

```rust
pub enum OperationRisk {
    Safe,            // 安全操作
    LowRisk,         // 低风险
    MediumRisk,      // 中等风险
    HighRisk,        // 高风险(删除操作)
    Destructive,     // 破坏性(清空操作)
}
```

---

## 依赖关系

### Cargo依赖树

```
agent-edit (主应用)
├── agent-core          # 核心Agent框架
│   ├── serde + serde_json    # 序列化
│   ├── async-trait           # 异步trait
│   ├── tokio                 # 异步运行时
│   ├── chrono                # 时间处理
│   ├── regex                 # 正则匹配
│   ├── thiserror             # 错误派生
│   └── reqwest               # HTTP客户端(LLM API)
├── agent-ui            # UI组件
│   └── bevy_egui             # egui集成
└── bevy-adapter        # Bevy适配器
    ├── bevy                  # Bevy游戏引擎 0.17
    └── bevy_egui
```

### 编译特性

| 特性 | 功能 |
|------|------|
| `llm-openai` | 启用OpenAI LLM支持 |
| `default` | 默认包含`llm-openai` |

---

## 运行方式

### 环境要求

- Rust 1.65+ (Edition 2021)
- Bevy 0.17
- macOS/Linux/Windows

### 构建命令

```bash
# 构建项目
cargo build

# 构建发布版本
cargo build --release

# 运行应用
cargo run

# 运行测试
cargo test --all
```

### 启动参数

主应用默认启动一个带有示例场景的编辑器窗口：

```rust
Window {
    title: "AgentEdit - AI Agent Driven Game Editor",
    resolution: WindowResolution::new(1600, 900),
}
```

### 示例使用

启动后可通过自然语言指令与Agent交互：

```
👋 欢迎使用 Game Architect v3.0！

场景已加载 3 个实体 (Player, Enemy_01, Enemy_02)。
支持真实引擎命令执行：创建/删除/移动/改色。

示例：
• "创建一个红色敌人"
• "移动 Player 到右边"
• "将 Enemy_01 颜色改为蓝色"
```

---

## 扩展指南

### 添加新Agent

1. 实现 `Agent` trait:
```rust
use agent_core::registry::{Agent, AgentId, CapabilityKind, AgentRequest, AgentResponse};

struct MyAgent {
    id: AgentId,
    name: String,
}

#[async_trait::async_trait]
impl Agent for MyAgent {
    fn id(&self) -> AgentId { self.id }
    fn name(&self) -> &str { &self.name }
    fn capabilities(&self) -> &[CapabilityKind] {
        &[CapabilityKind::SceneWrite]
    }
    async fn handle(&mut self, request: AgentRequest) -> Result<AgentResponse, AgentError> {
        Ok(AgentResponse {
            agent_id: self.id,
            agent_name: self.name.clone(),
            result: AgentResultKind::Success {
                summary: "完成".into(),
                output: serde_json::json!({}),
            },
            events: vec![],
        })
    }
}
```

2. 注册到 `AgentRegistry`:
```rust
let mut registry = AgentRegistry::new();
registry.register(Box::new(MyAgent {
    id: AgentId(100),
    name: "MyAgent".into(),
}));
```

### 添加新工具

1. 实现 `Tool` trait:
```rust
use agent_core::tool::{Tool, ToolCategory, ToolParameter, ToolResult, ToolError, ParameterType};

pub struct MyTool;

impl Tool for MyTool {
    fn name(&self) -> &str { "my_tool" }
    fn description(&self) -> &str { "我的自定义工具" }
    fn parameters(&self) -> Vec<ToolParameter> { vec![] }
    fn category(&self) -> ToolCategory { ToolCategory::Utility }
    fn execute(&self, params: HashMap<String, Value>) -> Result<ToolResult, ToolError> {
        Ok(ToolResult::success("执行成功"))
    }
}
```

2. 注册到 `ToolRegistry`:
```rust
let mut tools = ToolRegistry::new();
tools.register(MyTool);
```

### 添加新技能

1. 定义DAG工作流:
```rust
use agent_core::skill::{SkillDefinition, SkillId, SkillNode, SkillEdge, SkillEdgeCondition};
use agent_core::registry::CapabilityKind;

let skill = SkillDefinition {
    id: SkillId(100),
    name: "my_skill".into(),
    description: "我的自定义技能".into(),
    inputs: vec![],
    nodes: vec![
        SkillNode {
            id: "step1".into(),
            title: "第一步".into(),
            required_capability: CapabilityKind::SceneRead,
            tool_name: Some("query_entities".into()),
            input_mapping: serde_json::json!({}),
            retry: Default::default(),
            rollback: None,
        },
    ],
    edges: vec![],
};
```

2. 注册到 `SkillRegistry`:
```rust
let mut registry = SkillRegistry::new();
registry.register(skill);
```

### 添加降级模板

```rust
use agent_core::fallback::{FallbackEngine, CodeTemplate};

engine.add_template(CodeTemplate {
    name: "my_template".into(),
    trigger_keywords: vec!["关键词".into()],
    description: "模板描述".into(),
    build_command: |req, tid| EditorCommand::CheckGoal { task_id: tid },
});
```

---

## 测试覆盖

项目包含完整的单元测试覆盖：

| 模块 | 测试数量 | 覆盖内容 |
|------|----------|----------|
| agent-core | ~150+ | Agent框架、Planner、Router、Skill、Memory、Tool、Fallback、Review |
| bevy-adapter | ~30+ | SceneIndex、Integration、Vision |
| agent-ui | ~5+ | DirectorDeskState |
| 总计 | ~185+ | 核心功能全覆盖 |

运行测试：
```bash
cargo test --all --verbose
```

---

## 设计文档

项目设计参考以下文档：

1. **agent-architecture.md** - Agent系统架构设计
2. **agent-architecture-part2.md** - Agent团队协作模式
3. **gpt-agent-team-task-event-skill-architecture.md** - Agent团队/任务/事件/技能架构

---

## 版本历史

| 版本 | 描述 |
|------|------|
| v3.0 | Phase 3: Real Engine Command Pipeline |
| v2.0 | Phase 2: GoalChecker + Skill系统 + Review + Fallback |
| v1.0 | Phase 1: BaseAgent框架 + 状态机 + 内存系统 + 工具系统 |