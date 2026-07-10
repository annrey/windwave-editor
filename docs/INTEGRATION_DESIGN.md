
# DirectorRuntime 集成设计文档

## 概述

本文档描述了如何将以下新模块集成到现有的 DirectorRuntime 中：
- Squad（团队协作）
- SkillsCompound（技能复合）
- ReasoningBank（推理银行）
- RuntimeRegistry（运行时注册）
- AgentCollaboration（代理协作枢纽）

## 架构设计

### 集成点 1：DirectorRuntime 扩展

在 `director/mod.rs` 中的 `DirectorRuntime` 结构体中添加以下字段：

```rust
pub struct DirectorRuntime {
    // ... 现有字段 ...
    
    // 新添加的集成字段
    collaboration: Option&lt;agent_collaboration::AgentCollaborationSystem&gt;,
}
```

### 集成点 2：ReAct 执行循环与 ReasoningBank 集成

在 `react_runner.rs` 中：
- 在执行前获取 ReasoningBank 的推荐
- 在执行过程中记录 ReAct 步骤到 ReasoningBank
- 在执行完成后触发 SkillsCompound

### 集成点 3：团队任务与 Squad 集成

添加新的方法来处理团队任务：
- `assign_task_to_squad()` - 将任务分配给 Squad
- `get_squad_recommendations()` - 获取 Squad 推荐

### 集成点 4：记忆系统集成

将 ReasoningBank 与现有的四层记忆系统桥接：
- 从 EpisodicMemory 导出到 ReasoningBank
- 从 ReasoningBank 导回 SemanticMemory

## API 设计

### DirectorRuntime 新方法

```rust
impl DirectorRuntime {
    // 初始化协作系统
    pub fn init_collaboration(&amp;mut self);
    
    // 获取协作系统（如果可用）
    pub fn collaboration(&amp;self) -&gt; Option&lt;&amp;agent_collaboration::AgentCollaborationSystem&gt;;
    pub fn collaboration_mut(&amp;mut self) -&gt; Option&lt;&amp;mut agent_collaboration::AgentCollaborationSystem&gt;;
    
    // 创建并配置 Squad
    pub fn create_squad(&amp;mut self, name: &amp;str, policy: squad::RoutingPolicy) -&gt; squad::SquadId;
    
    // 将任务分配给 Squad
    pub fn assign_task_to_squad(&amp;mut self, task: SquadTask) -&gt; (squad::TaskId, reasoning_bank::ReasoningTraceId);
    
    // 获取基于历史的推荐
    pub fn get_task_recommendations(&amp;self, request: &amp;str) -&gt; agent_collaboration::TaskRecommendations;
}
```

## 执行流程

### 新的执行流程

```
用户请求
   ↓
SmartRouter (路由决策)
   ↓
[新] TaskRecommendations (从 ReasoningBank 获取)
   ↓
Plan / Direct 模式
   ↓
[新] ReActTraceRecorder (记录每个步骤到 ReasoningBank)
   ↓
执行
   ↓
[新] SkillsCompound (提取可重用模式)
   ↓
结果
```

## 集成阶段

### 阶段 1：基础集成（当前）

1. 添加协作系统到 DirectorRuntime
2. 基本的 API 方法
3. 初始化函数

### 阶段 2：深度集成（后续）

1. 在 ReAct 执行循环中自动记录
2. 自动技能提取
3. Squad 任务自动分配

### 阶段 3：高级功能（长期）

1. 完整的团队协作流程
2. 多引擎协调
3. 高级模式发现

## 兼容性保证

- 保持所有现有 API 不变
- 新功能作为可选扩展
- 向后兼容
