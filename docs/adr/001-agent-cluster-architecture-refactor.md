# ADR-001: Agent 集群架构重构

**状态**: Proposed  
**日期**: 2026-05-16  
**决策者**: WindWave 架构团队  
**相关审查**: Code Review 2026-05-16

---

## 上下文 (Context)

当前 WindWave 的 Agent 系统存在以下架构问题：

1. **CEO 层缺失**：当前 `TeamRole` 只有 5 种（Director/Planner/Executor/Reviewer/Hr），没有战略层。无法实现"监控多个 Director → 调整资源分配"的愿景。

2. **Director 和 Planner 职责重叠**：Director 负责"调度、优先级、协调"，Planner 负责"分析请求、创建执行计划"。职责边界模糊，导致代码耦合。

3. **四层记忆未隔离**：`MemorySystem` 是全局共享的，`TeamAgentContext` 只管理 `MessageBuffer`（对话记忆）。每个 Agent 无法有独立的四层记忆空间，违反设计目标。

4. **HrAgent 实现过于简单**：基于关键词匹配处理请求，没有 LLM 驱动的智能决策。

5. **信息流转层级过多**：用户请求 → CEO → Director → Planner → Executor → Reviewer，中间层级过多，延迟高。

---

## 决策 (Decision)

### 1. 新增 CEO 层（战略层）

在现有架构之上添加 CEO 层，形成三层架构：

```
┌─────────────────────────────────────────────────────────┐
│                    CEO (战略层)                          │
│  - 接收用户请求 → 分解为高层目标                          │
│  - 监控多个 Director 的执行状态                           │
│  - 调整资源分配                                          │
│  - 独立的 Agent 模块，工具集：规划 + 调度                  │
└──────────────────────┬──────────────────────────────────┘
                       │ 下达目标/约束
┌──────────────────────▼──────────────────────────────────┐
│              ProjectManager (编排层)                      │
│  - 合并 Director + Planner 职责                           │
│  - 接收 CEO 的目标 → 分解为 EditPlan                      │
│  - 调度 Agent 执行 → 监控进度                             │
│  - 持有 AgentRegistry 和 SkillRegistry                   │
│  - 通过 SceneBridge 与引擎交互                           │
└──────────────────────┬──────────────────────────────────┘
                       │ 调度
┌──────────────────────▼──────────────────────────────────┐
│              Agent 集群 (执行层)                          │
│  - CEO/PM/Executor/Reviewer/HR 等角色                    │
│  - 每个 Agent 有独立的四层记忆空间                        │
│  - 通过 CommunicationHub 共享公共知识                     │
│  - Agent 之间直接通信                                    │
│  - 调度基于优先级和依赖                                  │
└─────────────────────────────────────────────────────────┘
```

#### 1.1 新增 TeamRole

在 `crates/agent-core/src/team_structure.rs` 中修改：

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TeamRole {
    /// Strategic layer — monitors multiple Directors, allocates resources.
    Ceo,
    /// Orchestration layer — merged Director + Planner responsibilities.
    ProjectManager,
    /// Carries out individual plan steps (scene/code/asset).
    Executor,
    /// Validates execution results against goals.
    Reviewer,
    /// Manages team membership.
    Hr,
}
```

#### 1.2 CeoAgent 模块

**文件位置**: `crates/agent-core/src/ceo/mod.rs`（新增）

```rust
//! CEO Agent — strategic layer for multi-Director orchestration.
//!
//! The CEO is the top-level orchestrator that:
//! 1. Receives user requests and decomposes them into high-level goals
//! 2. Monitors multiple Director instances and their execution status
//! 3. Allocates resources (LLM tokens, compute budget, time limits)
//! 4. Adjusts priorities when conflicts arise between Directors

use crate::registry::{Agent, AgentId, AgentRequest, AgentResponse, AgentResultKind, CapabilityKind};
use crate::team_structure::TeamRole;
use std::collections::HashMap;

pub struct CeoAgent {
    id: AgentId,
    name: String,
    director_registry: DirectorRegistry,
    goal_queue: Vec<HighLevelGoal>,
    resource_budget: ResourceBudget,
}

/// A high-level goal assigned by the CEO to a ProjectManager.
pub struct HighLevelGoal {
    pub id: u64,
    pub description: String,
    pub priority: u8,           // 0-255, higher = more important
    pub assigned_pm: Option<ProjectManagerId>,
    pub status: GoalStatus,
    pub constraints: GoalConstraints,
}

pub enum GoalStatus {
    Pending,
    Assigned(ProjectManagerId),
    InProgress(ProjectManagerId),
    Completed(ProjectManagerId),
    Failed(ProjectManagerId, String),
}

pub struct GoalConstraints {
    pub max_tokens: Option<u64>,
    pub max_time_seconds: Option<u64>,
    pub required_capabilities: Vec<String>,
    pub forbidden_capabilities: Vec<String>,
}

/// Resource budget for a ProjectManager.
#[derive(Debug, Clone)]
pub struct ResourceBudget {
    pub token_budget: u64,
    pub time_budget_seconds: u64,
    pub max_parallel_agents: usize,
}

impl Default for ResourceBudget {
    fn default() -> Self {
        Self {
            token_budget: 100_000,
            time_budget_seconds: 300,
            max_parallel_agents: 4,
        }
    }
}

/// Registry of all ProjectManager instances managed by the CEO.
pub struct DirectorRegistry {
    directors: HashMap<ProjectManagerId, DirectorHandle>,
    next_director_id: u64,
}

pub struct DirectorHandle {
    pub id: ProjectManagerId,
    pub name: String,
    pub status: DirectorStatus,
    pub current_goal: Option<HighLevelGoal>,
    pub metrics: DirectorMetrics,
    pub budget: ResourceBudget,
}

pub enum DirectorStatus {
    Idle,
    Executing,
    WaitingApproval,
    Paused,
    Error(String),
}

pub struct DirectorMetrics {
    pub total_tasks_completed: u64,
    pub total_tasks_failed: u64,
    pub avg_completion_time_seconds: f64,
    pub token_usage_total: u64,
}

// CEO Agent implementation
impl CeoAgent {
    pub fn new(id: AgentId) -> Self {
        Self {
            id,
            name: "CEO".into(),
            director_registry: DirectorRegistry::new(),
            goal_queue: Vec::new(),
            resource_budget: ResourceBudget::default(),
        }
    }

    /// Assign a high-level goal to a ProjectManager.
    pub fn assign_goal(&mut self, goal: HighLevelGoal) -> ProjectManagerId {
        let pm_id = self.director_registry.find_idle_for_goal(&goal);
        if let Some(id) = pm_id {
            self.director_registry.assign_goal(id, goal.clone());
            goal.id;
            id
        } else {
            // No idle PM available — queue the goal
            self.goal_queue.push(goal);
            // Could also spawn a new PM here
            ProjectManagerId(0)
        }
    }

    /// Monitor all ProjectManagers and report status.
    pub fn monitor(&self) -> Vec<DirectorStatusReport> {
        self.director_registry.status_report()
    }

    /// Adjust resource allocation for a ProjectManager.
    pub fn adjust_budget(&mut self, pm_id: ProjectManagerId, new_budget: ResourceBudget) {
        self.director_registry.update_budget(pm_id, new_budget);
    }

    /// Re-prioritize goals when conflicts arise.
    pub fn reassign_priorities(&mut self, new_priorities: Vec<(GoalId, u8)>) {
        for (goal_id, priority) in new_priorities {
            // Update goal priority and potentially reassign ProjectManagers
        }
    }

    /// Spawn a new ProjectManager instance.
    pub fn spawn_director(&mut self, name: &str) -> ProjectManagerId {
        self.director_registry.spawn(name)
    }

    /// Terminate a ProjectManager instance.
    pub fn terminate_director(&mut self, pm_id: ProjectManagerId) {
        self.director_registry.terminate(pm_id);
    }
}

#[async_trait::async_trait]
impl Agent for CeoAgent {
    fn id(&self) -> AgentId { self.id }
    fn name(&self) -> &str { &self.name }
    fn role(&self) -> &str { "ceo" }
    fn capabilities(&self) -> &[CapabilityKind] {
        &[
            CapabilityKind::Orchestrate,
            CapabilityKind::Plan,
            CapabilityKind::Monitor,
            CapabilityKind::ResourceAllocate,
        ]
    }

    async fn handle(&mut self, request: AgentRequest) -> Result<AgentResponse, AgentError> {
        let ins = request.instruction.to_lowercase();

        if ins.contains("create") || ins.contains("spawn") || ins.contains("创建") {
            let name = request.context.get("name").and_then(|v| v.as_str()).unwrap_or("PM1");
            let pm_id = self.spawn_director(name);
            Ok(AgentResponse {
                agent_id: self.id,
                agent_name: self.name.clone(),
                result: AgentResultKind::Success {
                    summary: format!("Created ProjectManager '{}'", name),
                    output: serde_json::json!({ "pm_id": pm_id.0, "name": name }),
                },
                events: vec![],
            })
        } else if ins.contains("monitor") || ins.contains("监控") {
            let reports = self.monitor();
            Ok(AgentResponse {
                agent_id: self.id,
                agent_name: self.name.clone(),
                result: AgentResultKind::Success {
                    summary: format!("Monitoring {} ProjectManagers", reports.len()),
                    output: serde_json::json!({ "directors": reports }),
                },
                events: vec![],
            })
        } else if ins.contains("assign") || ins.contains("分配") {
            // Parse goal from context and assign
            let description = request.context.get("goal").and_then(|v| v.as_str()).unwrap_or("");
            let priority = request.context.get("priority").and_then(|v| v.as_u64()).unwrap_or(50) as u8;
            let goal = HighLevelGoal {
                id: self.director_registry.next_goal_id(),
                description: description.to_string(),
                priority,
                assigned_pm: None,
                status: GoalStatus::Pending,
                constraints: GoalConstraints::default(),
            };
            let pm_id = self.assign_goal(goal);
            Ok(AgentResponse {
                agent_id: self.id,
                agent_name: self.name.clone(),
                result: AgentResultKind::Success {
                    summary: format!("Assigned goal to ProjectManager {}", pm_id.0),
                    output: serde_json::json!({ "pm_id": pm_id.0 }),
                },
                events: vec![],
            })
        } else if ins.contains("adjust") || ins.contains("调整") {
            // Adjust budget for a PM
            let pm_id = request.context.get("pm_id").and_then(|v| v.as_u64()).unwrap_or(0);
            let token_budget = request.context.get("tokens").and_then(|v| v.as_u64());
            // ... update budget
            Ok(AgentResponse {
                agent_id: self.id,
                agent_name: self.name.clone(),
                result: AgentResultKind::Success {
                    summary: "Budget adjusted".into(),
                    output: serde_json::json!({}),
                },
                events: vec![],
            })
        } else {
            Ok(AgentResponse {
                agent_id: self.id,
                agent_name: self.name.clone(),
                result: AgentResultKind::Failed {
                    reason: "Unknown CEO command. Try: create <name>, monitor, assign <goal>, adjust <pm_id> <budget>".into(),
                },
                events: vec![],
            })
        }
    }
}
```

#### 1.3 CEO 与 ProjectManager 通信协议

通过 `CommunicationHub` 实现消息传递：

| 方向 | 消息类型 | 内容 |
|------|----------|------|
| CEO → PM | `GoalAssignment` | `{ goal_id, description, priority, constraints }` |
| PM → CEO | `StatusUpdate` | `{ pm_id, status, progress, metrics }` |
| CEO → PM | `ResourceAdjustment` | `{ pm_id, new_budget }` |
| PM → CEO | `GoalComplete` | `{ goal_id, pm_id, result, output }` |
| PM → CEO | `GoalFailed` | `{ goal_id, pm_id, error }` |
| PM → PM | `DirectMessage` | `{ from_pm, to_pm, content }` |

---

### 2. Director + Planner 合并为 ProjectManager

#### 2.1 合并理由

| 当前问题 | 合并后方案 |
|----------|------------|
| Director 和 Planner 职责重叠 | 合并为 ProjectManager，职责清晰划分 |
| 信息流转层级过多（5 层） | 减少为 3 层（CEO → PM → Agent） |
| Director 持有 Planner 实例，耦合 | ProjectManager 内部模块化，松耦合 |

#### 2.2 ProjectManager 模块设计

**文件位置**: `crates/agent-core/src/project_manager/mod.rs`（新增）

```rust
//! ProjectManager — merged Director + Planner role.
//!
//! Responsibilities:
//! 1. GoalReceiver: Receive goals from CEO or user
//! 2. Planner: Decompose goals into EditPlans (static)
//! 3. DirectorScheduler: Schedule Agent execution (runtime)
//! 4. ProgressTracker: Monitor execution progress
//! 5. ApprovalHandler: Handle user approval requests

use crate::plan::EditPlan;
use crate::registry::{AgentRegistry, AgentId, CapabilityKind};
use crate::scene_bridge::SceneBridge;
use crate::event::EventBus;
use crate::skill::SkillRegistry;
use crate::rollback::RollbackManager;
use crate::fallback::FallbackEngine;
use crate::memory::AgentMemorySystem;
use crate::team_context::TeamAgentContext;
use std::sync::Arc;
use std::collections::HashMap;

pub struct ProjectManager {
    id: AgentId,
    name: String,
    
    // Sub-modules
    planner: Box<dyn Planner>,
    scheduler: DirectorScheduler,
    progress_tracker: ProgressTracker,
    approval_handler: ApprovalHandler,
    
    // Dependencies
    agent_registry: AgentRegistry,
    skill_registry: SkillRegistry,
    scene_bridge: Option<Box<dyn SceneBridge>>,
    event_bus: EventBus,
    rollback_manager: RollbackManager,
    fallback_engine: FallbackEngine,
    memory_system: AgentMemorySystem,
    context: TeamAgentContext,
}

/// Static plan decomposition.
pub trait Planner {
    fn decompose(&mut self, goal: &str) -> EditPlan;
    fn revise(&mut self, plan: &EditPlan, feedback: &str) -> EditPlan;
}

/// Runtime scheduling of Agent execution.
pub struct DirectorScheduler {
    agent_registry: AgentRegistry,
    current_plan: Option<EditPlan>,
    current_step: usize,
}

impl DirectorScheduler {
    pub fn new(agent_registry: AgentRegistry) -> Self {
        Self {
            agent_registry,
            current_plan: None,
            current_step: 0,
        }
    }

    pub fn set_plan(&mut self, plan: EditPlan) {
        self.current_plan = Some(plan);
        self.current_step = 0;
    }

    pub fn next_step(&mut self) -> Option<EditPlanStep> {
        let plan = self.current_plan.as_ref()?;
        let step = plan.steps.get(self.current_step).cloned()?;
        Some(step)
    }

    pub fn advance(&mut self) {
        self.current_step += 1;
    }

    pub fn dispatch_step(&self, step: &EditPlanStep) -> AgentDispatch {
        // Find the best Agent for this step based on capabilities
        let agent = self.agent_registry.find_by_capability(&step.required_capability);
        AgentDispatch {
            agent_id: agent.id(),
            step: step.clone(),
        }
    }
}

pub struct AgentDispatch {
    pub agent_id: AgentId,
    pub step: EditPlanStep,
}

/// Track execution progress.
pub struct ProgressTracker {
    plan_id: String,
    total_steps: usize,
    completed_steps: usize,
    failed_steps: Vec<(usize, String)>,
}

impl ProgressTracker {
    pub fn new(plan_id: &str, total_steps: usize) -> Self {
        Self {
            plan_id: plan_id.to_string(),
            total_steps,
            completed_steps: 0,
            failed_steps: Vec::new(),
        }
    }

    pub fn mark_completed(&mut self) {
        self.completed_steps += 1;
    }

    pub fn mark_failed(&mut self, step_idx: usize, reason: &str) {
        self.failed_steps.push((step_idx, reason.to_string()));
    }

    pub fn progress_percent(&self) -> f64 {
        if self.total_steps == 0 {
            return 0.0;
        }
        (self.completed_steps as f64 / self.total_steps as f64) * 100.0
    }

    pub fn is_complete(&self) -> bool {
        self.completed_steps >= self.total_steps
    }
}

/// Handle user approval requests.
pub struct ApprovalHandler {
    pending_approvals: HashMap<String, ApprovalRequest>,
}

pub struct ApprovalRequest {
    pub plan_id: String,
    pub risk_level: RiskLevel,
    pub summary: String,
    pub created_at: u64,
}

pub enum RiskLevel {
    Low,
    Medium,
    High,
}

impl ApprovalHandler {
    pub fn new() -> Self {
        Self {
            pending_approvals: HashMap::new(),
        }
    }

    pub fn request_approval(&mut self, plan_id: &str, risk_level: RiskLevel, summary: &str) {
        self.pending_approvals.insert(
            plan_id.to_string(),
            ApprovalRequest {
                plan_id: plan_id.to_string(),
                risk_level,
                summary: summary.to_string(),
                created_at: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
            },
        );
    }

    pub fn approve(&mut self, plan_id: &str) -> bool {
        self.pending_approvals.remove(plan_id).is_some()
    }

    pub fn reject(&mut self, plan_id: &str, reason: &str) -> bool {
        self.pending_approvals.remove(plan_id).is_some()
    }

    pub fn has_pending(&self) -> bool {
        !self.pending_approvals.is_empty()
    }
}
```

#### 2.3 职责边界

```
┌─────────────────────────────────────────────────────────────┐
│                   ProjectManager                             │
│  ┌─────────────────┐  ┌─────────────────┐                   │
│  │  GoalReceiver   │  │   Planner       │                   │
│  │  (接收目标)      │→ │ (静态分解)       │                   │
│  └─────────────────┘  └────────┬────────┘                   │
│                                │                             │
│                                ▼                             │
│  ┌─────────────────┐  ┌─────────────────┐                   │
│  │ ApprovalHandler │  │ DirectorScheduler│                  │
│  │ (审批处理)       │← │ (运行时调度)      │                   │
│  └─────────────────┘  └────────┬────────┘                   │
│                                │                             │
│                                ▼                             │
│  ┌─────────────────┐  ┌─────────────────┐                   │
│  │ ProgressTracker │  │   AgentRegistry  │                  │
│  │ (进度监控)       │  │ (Agent 集群)      │                   │
│  └─────────────────┘  └─────────────────┘                   │
└─────────────────────────────────────────────────────────────┘
```

| 子模块 | 职责 | 输入 | 输出 |
|--------|------|------|------|
| GoalReceiver | 接收 CEO 或用户的请求 | `GoalAssignment` 消息 | `HighLevelGoal` |
| Planner | 将目标分解为 EditPlan | `HighLevelGoal` | `EditPlan` |
| DirectorScheduler | 调度 Agent 执行计划步骤 | `EditPlan` | `AgentDispatch` |
| ProgressTracker | 跟踪执行进度 | `AgentDispatch` 结果 | `ProgressReport` |
| ApprovalHandler | 处理用户审批请求 | `EditPlan`（高风险） | `ApprovalDecision` |

---

### 3. 四层记忆隔离方案

#### 3.1 当前问题分析

当前 `MemorySystem` 是全局共享的：

```rust
// crates/agent-core/src/memory/system.rs
pub struct MemorySystem {
    pub working_memory: WorkingMemory,
    pub episodic_memory: EpisodicMemory,
    pub semantic_memory: SemanticMemory,
    pub procedural_memory: ProceduralMemory,
    pub retriever: HybridRetriever,
}
```

但 `TeamAgentContext` 只管理 `MessageBuffer`（对话记忆），不包含四层记忆。

#### 3.2 MemorySystemRegistry 设计

**文件位置**: `crates/agent-core/src/memory/registry.rs`（新增）

```rust
//! Memory System Registry — per-agent isolated memory spaces.
//!
//! Each agent has its own independent four-layer memory:
//! - Procedural Memory (L0): Workflow templates, decision patterns
//! - Semantic Memory (L1): Entity/component concept graph
//! - Episodic Memory (L2): Event history
//! - Working Memory (L3): Short-term context
//!
//! Plus: Shared public knowledge pool (CommunicationHub::SharedContext)

use crate::memory::{
    WorkingMemory, EpisodicMemory, SemanticMemory, ProceduralMemory,
    HybridRetriever, MemoryConfig,
};
use crate::registry::AgentId;
use std::collections::HashMap;

/// Per-agent isolated four-layer memory system.
#[derive(Debug)]
pub struct AgentMemorySystem {
    pub agent_id: AgentId,
    pub working_memory: WorkingMemory,
    pub episodic_memory: EpisodicMemory,
    pub semantic_memory: SemanticMemory,
    pub procedural_memory: ProceduralMemory,
    pub retriever: HybridRetriever,
    pub config: MemoryConfig,
}

impl AgentMemorySystem {
    pub fn new(agent_id: AgentId, config: MemoryConfig) -> Self {
        Self {
            agent_id,
            working_memory: WorkingMemory::new(config.working_memory_capacity),
            episodic_memory: EpisodicMemory::new(config.episodic_memory_capacity),
            semantic_memory: SemanticMemory::new(),
            procedural_memory: ProceduralMemory::new(),
            retriever: HybridRetriever::new(),
            config,
        }
    }

    /// Bootstrap from shared public knowledge.
    pub fn bootstrap_from_shared(&mut self, shared_context: &HashMap<String, serde_json::Value>) {
        // Copy relevant public knowledge into this agent's semantic memory
        if let Some(project_info) = shared_context.get("project:info") {
            self.semantic_memory.add_entity("project", project_info.clone());
        }
        if let Some(team_roster) = shared_context.get("team:roster") {
            self.semantic_memory.add_entity("team", team_roster.clone());
        }
        // Add common patterns to procedural memory
        if let Some(patterns) = shared_context.get("team:patterns") {
            if let Some(ops) = patterns.get("common_operations") {
                // Convert to workflow templates
            }
        }
    }

    /// Store a memory entry.
    pub fn store(&mut self, tier: MemoryTier, entry: MemoryEntry) {
        match tier {
            MemoryTier::Working => self.working_memory.add(entry),
            MemoryTier::Episodic => self.episodic_memory.add(entry),
            MemoryTier::Semantic => self.semantic_memory.add_entry(entry),
            MemoryTier::Procedural => self.procedural_memory.add_template(entry),
        }
    }

    /// Retrieve from memory using hybrid search.
    pub fn retrieve(&mut self, query: &str, tier: Option<MemoryTier>, limit: usize) -> Vec<MemoryEntry> {
        match tier {
            Some(MemoryTier::Working) => self.working_memory.search(query, limit),
            Some(MemoryTier::Episodic) => self.episodic_memory.search(query, limit),
            Some(MemoryTier::Semantic) => self.semantic_memory.search(query, limit),
            Some(MemoryTier::Procedural) => self.procedural_memory.search(query, limit),
            None => self.retriever.search(query, limit),
        }
    }
}

/// Registry of all agent memory systems.
pub struct MemorySystemRegistry {
    memories: HashMap<u64, AgentMemorySystem>,
    default_config: MemoryConfig,
}

impl MemorySystemRegistry {
    pub fn new(default_config: MemoryConfig) -> Self {
        Self {
            memories: HashMap::new(),
            default_config,
        }
    }

    /// Register a new agent's memory system.
    pub fn register(&mut self, agent_id: AgentId, shared_context: &HashMap<String, serde_json::Value>) {
        let mut memory = AgentMemorySystem::new(agent_id, self.default_config.clone());
        memory.bootstrap_from_shared(shared_context);
        self.memories.insert(agent_id.0, memory);
    }

    /// Unregister an agent's memory system.
    pub fn unregister(&mut self, agent_id: AgentId) {
        self.memories.remove(&agent_id.0);
    }

    /// Get mutable reference to an agent's memory system.
    pub fn get_mut(&mut self, agent_id: AgentId) -> Option<&mut AgentMemorySystem> {
        self.memories.get_mut(&agent_id.0)
    }

    /// Get an agent's memory system.
    pub fn get(&self, agent_id: AgentId) -> Option<&AgentMemorySystem> {
        self.memories.get(&agent_id.0)
    }

    /// Publish to shared public knowledge (accessible by all agents).
    pub fn publish_shared(&mut self, key: &str, value: serde_json::Value) {
        // Update shared context — all new agents will bootstrap from this
        // Existing agents need to be notified via CommunicationHub
    }
}
```

#### 3.3 更新 TeamAgentContext

```rust
// crates/agent-core/src/team_context.rs

pub struct TeamAgentContext {
    pub memory: MessageBuffer,              // 对话记忆（独立）
    pub memory_system: Option<AgentMemorySystem>,  // 四层记忆（独立）
    pub agent_id: AgentId,
    pub initialized: bool,
}

impl TeamAgentContext {
    pub fn new(
        agent_id: AgentId, 
        max_messages: usize, 
        memory_config: MemoryConfig
    ) -> Self {
        Self {
            memory: MessageBuffer::new(max_messages),
            memory_system: Some(AgentMemorySystem::new(agent_id, memory_config)),
            agent_id,
            initialized: false,
        }
    }

    pub fn bootstrap(&mut self, hub: &CommunicationHub) {
        if self.initialized { return; }
        
        // Bootstrap both MessageBuffer and MemorySystem from shared context
        if let Some(ref mut mem) = self.memory_system {
            mem.bootstrap_from_shared(&hub.context);
        }
        // ... existing bootstrap logic for MessageBuffer
        self.initialized = true;
    }
}
```

#### 3.4 记忆同步策略

| 记忆层 | 隔离性 | 同步策略 |
|--------|--------|----------|
| Procedural Memory (L0) | 独立 | 可选同步：新工作流模板可共享到公共知识 |
| Semantic Memory (L1) | 独立 | 部分同步：公共概念图谱可共享，私有实体不共享 |
| Episodic Memory (L2) | 独立 | 不共享：每个 Agent 的事件历史独立 |
| Working Memory (L3) | 独立 | 不共享：短期上下文独立 |
| MessageBuffer（对话记忆） | 独立 | 不共享：每个 Agent 的对话历史独立 |

---

### 4. HrAgent LLM 化

#### 4.1 当前问题

当前 `HrAgent` 基于关键词匹配处理请求，没有 LLM 驱动的智能决策。

#### 4.2 重构方案

将 `HrAgent` 重构为 LLM 驱动的 Agent，使用 `ReActAgent` 作为基类：

```rust
// crates/agent-core/src/hr_agent.rs (重构后)

use crate::agent::BaseAgent;
use crate::strategy::create_react_agent;
use crate::registry::{Agent, AgentId, AgentRequest, AgentResponse, AgentResultKind};
use crate::team_structure::{TeamRole, TeamRoster};
use std::sync::Arc;

pub struct HrAgent {
    id: AgentId,
    name: String,
    roster: TeamRoster,
    react_agent: Option<ReActAgent>,  // LLM 驱动的 Agent
}

impl HrAgent {
    pub fn new(id: AgentId, roster: TeamRoster, llm_client: Arc<dyn LlmClient>) -> Self {
        let base = BaseAgent::new(AgentInstanceId(0), "HR");
        let tool_registry = Arc::new(std::sync::Mutex::new(crate::tool::ToolRegistry::new()));
        
        let hr_system_prompt = r#"You are an HR agent managing the AI agent team.
Your responsibilities:
1. Evaluate requests to add new agents — assess if the role is needed
2. Manage agent onboarding — provide shared context to new agents
3. Handle agent offboarding — archive their memory and clean up resources
4. Monitor team health — report on agent availability and performance

When evaluating a hire request, consider:
- Does the team already have this capability?
- Is the workload high enough to justify a new agent?
- What role should this agent play?

Always think step by step before making a decision.
"#;

        let react = create_react_agent(base, llm_client, tool_registry)
            .with_system_prompt(hr_system_prompt.to_string());

        Self {
            id,
            name: "HR".into(),
            roster,
            react_agent: Some(react),
        }
    }
}
```

---

## 后果 (Consequences)

### 正面影响

1. **架构清晰**：三层架构（CEO → PM → Agent）职责明确，易于理解和维护。
2. **信息流转减少**：从 5 层减少到 3 层，降低延迟。
3. **记忆隔离**：每个 Agent 有独立的四层记忆空间，避免记忆污染。
4. **弹性扩展**：CEO 可动态创建/终止 ProjectManager，支持弹性扩展。
5. **智能 HR**：HrAgent 使用 LLM 驱动，能智能评估团队需求。

### 负面影响

1. **代码量增加**：新增 `CeoAgent`、`ProjectManager`、`MemorySystemRegistry` 等模块，代码量增加约 2000 行。
2. **迁移成本**：现有 `DirectorRuntime` 需要重构为 `ProjectManager`，可能影响现有功能。
3. **记忆同步复杂性**：需要实现记忆同步协议，处理公共知识与独立记忆的边界。
4. **测试成本**：新增模块需要完整的单元测试和集成测试。

### 风险缓解

| 风险 | 缓解措施 |
|------|----------|
| 迁移成本 | 分阶段迁移：先添加 CEO 层，再合并 Director+Planner，最后重构记忆系统 |
| 记忆同步复杂性 | 先实现独立记忆，再逐步添加同步协议 |
| 测试成本 | 为每个新模块编写单元测试，使用现有测试框架 |

---

## 实施计划 (Implementation Plan)

### Phase 1: CEO 层（2-3 天）

- [ ] 在 `team_structure.rs` 中添加 `Ceo` 角色
- [ ] 创建 `crates/agent-core/src/ceo/mod.rs`
- [ ] 实现 `CeoAgent`、`DirectorRegistry`、`HighLevelGoal`
- [ ] 实现 CEO 与 PM 的通信协议
- [ ] 编写单元测试

### Phase 2: 四层记忆隔离（3-4 天）

- [ ] 创建 `crates/agent-core/src/memory/registry.rs`
- [ ] 实现 `AgentMemorySystem`、`MemorySystemRegistry`
- [ ] 更新 `TeamAgentContext` 集成 `AgentMemorySystem`
- [ ] 实现记忆同步协议（可选）
- [ ] 编写单元测试

### Phase 3: Director+Planner 合并（1-2 天）

- [ ] 创建 `crates/agent-core/src/project_manager/mod.rs`
- [ ] 实现 `ProjectManager` 及其子模块
- [ ] 迁移现有 `DirectorRuntime` 功能到 `ProjectManager`
- [ ] 更新 `TeamRole` 使用 `ProjectManager`
- [ ] 编写单元测试

### Phase 4: HrAgent LLM 化（1 天）

- [ ] 重构 `HrAgent` 使用 `ReActAgent` 基类
- [ ] 编写 HR 系统提示词
- [ ] 编写单元测试

### Phase 5: 集成测试（1-2 天）

- [ ] 端到端测试：用户请求 → CEO → PM → Agent → 引擎
- [ ] 压力测试：多个 PM 并发执行
- [ ] 记忆隔离测试：验证每个 Agent 的记忆独立

---

## 相关文档

- [IMPLEMENTATION_PLAN.md](../../IMPLEMENTATION_PLAN.md)
- [CONTEXT-MAP.md](../../CONTEXT-MAP.md)
- [crates/agent-core/CONTEXT.md](../../crates/agent-core/CONTEXT.md)

---

## 审查记录

| 日期 | 审查者 | 意见 |
|------|--------|------|
| 2026-05-16 | Code Review | 初始审查，发现问题 1-6 |
| 2026-05-16 | Architecture Discussion | 确认 CEO 层、记忆隔离、PM 合并方案 |