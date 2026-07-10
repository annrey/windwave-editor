
# Squad Team Collaboration System - Design Document

&gt; Version: 1.0 | Date: 2026-05-21 | Authors: AgentEdit Team

## Overview

The **Squad Team Collaboration System** is inspired by [Multica](https://github.com/multica-ai/multica) and adds multi-agent team coordination to the WindWave agent architecture.

A **Squad** is a group of specialized agents working together under a leader. You can assign work to a Squad by @mentioning it (e.g., `@FrontendTeam create a player character`), and the Squad Leader will automatically route the task to the most appropriate agent based on capability.

---

## 1. Core Architecture

### 1.1 Layered Team Structure

WindWave already has a three-layer team architecture (from `team_structure.rs`):

```
┌─────────────────────────────────────────────────┐
│         Layer 1: Strategic (CEO)                │
│  - Oversees multiple ProjectManagers            │
│  - Allocates resources                          │
│  - Adjusts priorities                           │
└─────────────┬───────────────────────────────────┘
              │
┌─────────────▼───────────────────────────────────┐
│    Layer 2: Orchestration (ProjectManager)      │
│  - Decomposes goals into plans                  │
│  - Schedules execution                          │
└─────────────┬───────────────────────────────────┘
              │
┌─────────────▼───────────────────────────────────┐
│    Layer 3: Execution (Specialized Agents)      │
│  - CodeAgent, SceneAgent, ReviewAgent, etc.    │
│  - Carry out actual tasks                      │
└─────────────────────────────────────────────────┘
```

### 1.2 Squad Architecture

Squads operate **within Layer 3** to coordinate specialized execution agents:

```
┌─────────────────────────────────────────────────────────┐
│              @FrontendTeam (Squad)                       │
│  ┌────────────────────────────────────────────────────┐ │
│  │  Squad Leader                                       │ │
│  │  ├── Routing Policy (SkillBased / RoundRobin / LB)  │ │
│  │  └── Task Queue &amp; Active Tasks                      │ │
│  └────────────────────────────────────────────────────┘ │
├─────────────────────────────────────────────────────────┤
│  Members:                                               │
│  ├── SceneAgent  (SceneRead/Write)                      │
│  ├── CodeAgent   (CodeRead/Write)                       │
│  └── AssetAgent  (AssetRead/Write)                      │
└─────────────────────────────────────────────────────────┘
```

---

## 2. Implementation Details

### 2.1 Core Types (squad.rs)

#### SquadId
Unique identifier for a Squad (similar to AgentId/ProjectManagerId).

#### RoutingPolicy
Defines how tasks are assigned to Squad members:
- **LeaderDecides**: Squad Leader uses LLM to decide best agent
- **RoundRobin**: Cycle through available agents equally
- **LoadBalance**: Assign to least busy agent
- **SkillBased**: Match task requirements to agent capabilities (default)

#### SquadTask
A task submitted to a Squad:
- `TaskId`: Unique task identifier
- `title`, `description`: Human-readable task info
- `priority`: Low/Normal/High/Urgent
- `status`: Pending/Assigned/InProgress/Completed/Failed/Cancelled
- `required_capabilities`: Vec&lt;CapabilityKind&gt; needed to execute
- `timestamps`: Creation, start, completion
- `parent_task`: Optional link to parent for subtasks

#### Squad
The main Squad container:
- `id`, `name`, `description`: Squad identity
- `leader`: AgentId of the leader
- `members`: Vec&lt;AgentId&gt; of members
- `policy`: RoutingPolicy to use
- `task_queue`: VecDeque&lt;SquadTask&gt; pending tasks (priority-sorted)
- `active_tasks`: HashMap&lt;TaskId, SquadTask&gt; currently executing
- `max_parallel_tasks`: Limit concurrent execution (default 4)

### 2.2 Squad Registry

Manages all Squads in the system:
- `create_squad(name, leader, policy)` - Creates new Squad
- `delete_squad(id)` - Removes a Squad
- `get/ get_mut(id)` - Access Squad by ID
- `list()` - All Squads
- `find_by_member(agent_id)` - Squads containing agent
- `find_by_leader(agent_id)` - Squads led by agent
- `next_task_id()` - Generates unique TaskId

### 2.3 Task Router

The `TaskRouter` handles task assignment:

```rust
impl<'a> TaskRouter<'a> {
    pub fn process_queue(&amp;mut self) -&gt; Vec&lt;(TaskId, AgentId)&gt;
    fn select_by_skill_match(&amp;self, task: &amp;SquadTask) -&gt; Option&lt;AgentId&gt;
    // ... other selection methods
}
```

### 2.4 Squad Agent (squad_agent.rs)

A special Agent that manages a Squad:
- Handles Squad creation/deletion
- Manages membership (add/remove agents)
- Accepts task submissions
- Monitors Squad status
- Returns agent capabilities (orchestration + scene access)

---

## 3. Integration with Existing Systems

### 3.1 Memory System
Each Squad member maintains independent four-layer memory (Working/Episodic/Semantic/Procedural), while sharing knowledge via `CommunicationHub::SharedContext`.

### 3.2 Permissions &amp; Rollback
All existing permission checks and transactional safeguards apply:
- Risk estimation for each task
- Auto-approve vs user-confirmation decisions
- Full undo/redo with scene snapshots
- Tamper-proof audit trail with hash chain

### 3.3 Team Context
Squads integrate with `TeamContext` (from `team_context.rs`) to share:
- Project info
- Team roster
- Learned patterns
- Bootstrap memory for new agents

---

## 4. Usage Examples

### 4.1 Creating a Squad

#### Via Code:
```rust
use agent_core::squad::{SquadRegistry, RoutingPolicy, SquadId, TaskId};
use agent_core::registry::AgentId;

// Initialize registry
let mut registry = SquadRegistry::new();

// Create Squad with skill-based routing
let squad_id = registry.create_squad(
    "GameDevTeam".into(),
    AgentId(1), // Leader
    RoutingPolicy::SkillBased
);

// Add members
if let Some(squad) = registry.get_mut(squad_id) {
    squad.add_member(AgentId(2)); // SceneAgent
    squad.add_member(AgentId(3)); // CodeAgent
    squad.add_member(AgentId(4)); // AssetAgent
}
```

#### Via Agent Interface:
```rust
let mut agent = SquadAgent::new(AgentId(100), "SquadManager".into(), squad_id, registry);
let req = AgentRequest {
    instruction: "create squad".into(),
    context: json!({
        "name": "GameDevTeam",
        "leader_id": 1,
        "policy": "skill_based"
    }),
    ..
};
```

### 4.2 Submitting Tasks

```rust
// Create and submit a task
let task_id = registry.next_task_id();
let task = SquadTask::new(
    task_id,
    "Create player".into(),
    "Create a player entity with a red mesh and physics".into(),
    vec![CapabilityKind::SceneRead, CapabilityKind::SceneWrite]
).with_priority(TaskPriority::High);

// Add to queue
if let Some(squad) = registry.get_mut(squad_id) {
    squad.submit_task(task);
    // Queue is kept sorted by priority automatically!
}

// Process queue (assign tasks)
let assigned = {
    let roster = TeamMember::default(); // placeholder
    let squad = registry.get_mut(squad_id).unwrap();
    let mut router = TaskRouter::new(&amp;roster, squad);
    router.process_queue()
};

// Returns: Vec&lt;(TaskId, AgentId)&gt;
println!("Assigned tasks: {:?}", assigned);
```

### 4.3 Monitoring Status

```rust
if let Some(squad) = registry.get(squad_id) {
    println!("Squad '{}'", squad.name);
    println!("  Leader: {:?}", squad.leader);
    println!("  Members: {}", squad.members.len());
    println!("  Active tasks: {}", squad.active_tasks.len());
    println!("  Queued tasks: {}", squad.task_queue.len());
}
```

---

## 5. Roadmap

### Phase 1 (Current) - Basic Squad System ✅
- Squad Registry and core types
- 4 routing policies (LeaderDecides/RoundRobin/LoadBalance/SkillBased)
- Task lifecycle (Pending → Assigned → InProgress → Completed)
- Priority-based queue
- Squad Agent interface
- Basic integration with existing systems

### Phase 2 - Enhanced Routing &amp; Skills
- **Skill matching improvements**
  - Capability score calculations
  - Preference learning from task success
- **Task decomposition**
  - Break complex tasks into subtasks
  - Assign different parts to different agents
- **Dependency management**
  - Task DAG within Squad
  - Block until prerequisites met
- **Progress &amp; status updates**
  - Active task monitoring
  - Completion events

### Phase 3 - Learning &amp; Adaptation
- **ReasoningBank** for task patterns
- **Performance metrics** for each agent
- **Auto-balancing** of routing policy
- **Federated multi-machine support**

---

## 6. Comparison with Multica

| Feature | WindWave | Multica |
|---------|----------|---------|
| **Architecture** | Three-layer + Squads | Plug-in orchestration |
| **Task Assignment** | RoutingPolicy + LLM | Agent selection + swarms |
| **Permissions** | 5-level risk + jailbreak detection | Sandboxed agents |
| **Rollback** | Full undo/redo with snapshots | Git-based history |
| **Audit** | Hash-chain tamper-proof | Structured logs |
| **Game Engine** | Bevy (Unity/Godot planned) | IDE/editor plugins |
| **Language** | 100% Rust | TypeScript + plugins |
| **Learning** | Phase 5 planned | ReasoningBank |

**Takeaways from Multica**:
- Squads as unified interfaces (@mentioning)
- Agent specialization + leader coordination
- Skill compounding from completed tasks
- Multi-workspace support
- Runtime unification

---

## 7. Files Added

### Core Implementation:
- `crates/agent-core/src/squad.rs` - Main Squad system
- `crates/agent-core/src/squad_agent.rs` - Squad Agent
- Updated `lib.rs` with re-exports

### Documentation:
- `docs/SQUAD_TEAM_COLLABORATION.md` (this document)

---

## 8. Future Directions

### 8.1 Named Squads &amp; @Mentions
```rust
// Syntax like this in user chat:
"@FrontendTeam create a red player character"

// Routes to SquadLeader for the @FrontendTeam squad
```

### 8.2 Human-in-the-loop
- Allow user assignment overrides
- Approve task reassignments
- Resolve stalemates

### 8.3 Performance Metrics
- Track per-agent throughput
- Per-task success rate
- Skill match quality feedback

---

## 9. Testing

Run tests for the Squad system:

```bash
cargo test -p agent-core squad -- --nocapture
```

Individual test modules:
- `squad::tests` - Core Squad functionality
- `squad_agent::tests` - Squad Agent interface

---

## Appendix

### A. Default Routing Policies Explained

**SkillBased (default)**:
- Score each member against task requirements
- Select highest-scoring available
- Scores degrade as agent gets busy

**LoadBalance**:
- Track active tasks per agent
- Assign to agent with fewest in-flight

**RoundRobin**:
- Simple rotation
- Good for homogeneous teams

**LeaderDecides**:
- Query the leader agent (LLM)
- Most flexible but highest latency

### B. State Transition Diagram

```
     Submit
       │
       ▼
   ┌─────────┐   Assign   ┌───────────┐
   │ Pending │───────────▶│ Assigned  │
   └─────────┘            └─────┬─────┘
         │                      │
         │                      │ Start
         │                      ▼
         │                 ┌───────────┐
         │                 │InProgress │
         │                 └─────┬─────┘
         │                      │
    Cancel                      │ Complete/Fail
         │              ┌───────┴───────┐
         ▼              ▼               ▼
   ┌──────────┐   ┌──────────┐   ┌──────────┐
   │ Cancelled│   │Completed │   │  Failed  │
   └──────────┘   └──────────┘   └──────────┘
```

---

*End of Document*

