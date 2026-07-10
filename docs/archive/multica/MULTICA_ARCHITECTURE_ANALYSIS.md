
# Multica Go 后端核心架构分析

## 1. 整体架构概述

Multica 是一个 AI 原生的任务管理平台，采用 **Go 后端 + 单体前端** 架构。主要技术栈包括：

- **Web 框架**: [Chi](https://github.com/go-chi/chi) 路由
- **数据库**: PostgreSQL + [sqlc](https://github.com/sqlc-dev/sqlc) 代码生成
- **实时通信**: [Gorilla WebSocket](https://github.com/gorilla/websocket) + Redis 广播
- **核心包结构**:
  - `pkg/` - 可重用的库代码（Agent、DB、协议等）
  - `internal/` - 内部实现（服务、处理程序、实时系统等）

### 文件位置
- 主项目路径: `/Users/chengyongwei/Library/Mobile Documents/com~apple~CloudDocs/gameedit/风浪/multica`
- Go 后端代码: `multica/server/`

---

## 2. 任务生命周期管理

### 2.1 核心数据模型

任务状态机存储在 `agent_task_queue` 表中，包含以下核心状态：

```sql
-- 迁移文件: multica/server/migrations/001_init.up.sql:127-139
CREATE TABLE agent_task_queue (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    agent_id UUID NOT NULL REFERENCES agent(id) ON DELETE CASCADE,
    issue_id UUID NOT NULL REFERENCES issue(id) ON DELETE CASCADE,
    status TEXT NOT NULL DEFAULT 'queued'
        CHECK (status IN ('queued', 'dispatched', 'running', 'completed', 'failed', 'cancelled')),
    priority INT NOT NULL DEFAULT 0,
    dispatched_at TIMESTAMPTZ,
    started_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    result JSONB,
    error TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
```

### 2.2 任务流转状态

```
queued → dispatched → running → (completed/failed/cancelled)
  ↑                                 ↓
  └────────── 自动重试 (如适用) ──────┘
```

### 2.3 核心服务

**文件**: `multica/server/internal/service/task.go`

主要功能：
- `EnqueueTaskForIssue()` - 为问题排队任务
- `EnqueueTaskForMention()` - 为提到的 Agent 排队任务
- `ClaimTask()` - 原子性地声明下一个待处理任务
- `StartTask()` - 启动已声明的任务
- `CompleteTask()` - 完成任务
- `FailTask()` - 任务失败处理
- `CancelTask()` - 取消任务
- `MaybeRetryFailedTask()` - 自动重试失败任务

### 2.4 孤儿任务恢复

**文件**: `multica/server/internal/handler/task_lifecycle.go:24-56`

`RecoverOrphanedTasks()` 处理守护进程重启时的孤儿任务恢复：
1. 查找仍然认为属于该运行时的 dispatched/running 状态的任务
2. 原子性地使这些任务失败
3. 触发 `MaybeRetryFailedTask()` 进行自动重试

### 2.5 任务执行模式

任务支持多种触发方式：
- **问题分配** - 分配给 Agent 的 Issue
- **提到触发** - 评论中 @提及 Agent
- **快速创建** - 用户直接输入 prompt 让 Agent 创建 Issue
- **聊天会话** - 对话模式触发

---

## 3. Agent 管理系统

### 3.1 Agent 数据模型

```sql
-- 迁移文件: multica/server/migrations/001_init.up.sql:36-48
CREATE TABLE agent (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    workspace_id UUID NOT NULL REFERENCES workspace(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    avatar_url TEXT,
    runtime_mode TEXT NOT NULL CHECK (runtime_mode IN ('local', 'cloud')),
    runtime_config JSONB NOT NULL DEFAULT '{}',
    visibility TEXT NOT NULL DEFAULT 'workspace' CHECK (visibility IN ('workspace', 'private')),
    status TEXT NOT NULL DEFAULT 'offline' CHECK (status IN ('idle', 'working', 'blocked', 'error', 'offline')),
    max_concurrent_tasks INT NOT NULL DEFAULT 1,
    owner_id UUID REFERENCES "user"(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
```

### 3.2 Agent 后端统一接口

**文件**: `multica/server/pkg/agent/agent.go:16-101`

Multica 支持多种 Agent 后端（Claude Code、Codex、Copilot、OpenCode 等），通过统一的 `Backend` 接口抽象：

```go
// Backend 是执行提示词的统一接口
type Backend interface {
    Execute(ctx context.Context, prompt string, opts ExecOptions) (*Session, error)
}

// 支持的 Agent 类型
switch agentType {
case "claude":
case "codex":
case "copilot":
case "opencode":
case "openclaw":
case "hermes":
case "gemini":
case "pi":
case "cursor":
case "kimi":
case "kiro":
}
```

### 3.3 Agent 状态管理

Agent 状态会根据任务执行情况自动调整：
- `idle` - 空闲，等待任务
- `working` - 正在执行任务
- `blocked` - 被阻塞
- `error` - 发生错误
- `offline` - 离线状态

---

## 4. 技能系统

### 4.1 技能数据模型

技能是 Agent 可以使用的可重用工具/知识包。核心表包括：

```sql
-- skill 表 - 主要技能信息（迁移 008 定义）
CREATE TABLE skill (
    id UUID PRIMARY KEY,
    workspace_id UUID NOT NULL REFERENCES workspace(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    description TEXT,
    content TEXT,  -- SKILL.md 内容
    config JSONB,
    created_by UUID REFERENCES "user"(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(workspace_id, name)
);

-- skill_file 表 - 技能附加文件
CREATE TABLE skill_file (
    id UUID PRIMARY KEY,
    skill_id UUID NOT NULL REFERENCES skill(id) ON DELETE CASCADE,
    path TEXT NOT NULL,
    content TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(skill_id, path)
);

-- agent_skill 表 - Agent ↔ Skill 关联
CREATE TABLE agent_skill (
    agent_id UUID NOT NULL REFERENCES agent(id) ON DELETE CASCADE,
    skill_id UUID NOT NULL REFERENCES skill(id) ON DELETE CASCADE,
    PRIMARY KEY (agent_id, skill_id)
);
```

### 4.2 技能查询接口

**文件**: `multica/server/pkg/db/queries/skill.sql`

主要功能：
- `ListSkillsByWorkspace()` - 列出工作区所有技能（含完整内容）
- `ListSkillSummariesByWorkspace()` - 列出技能摘要（不含 content，优化列表性能）
- `GetSkillInWorkspace()` - 获取特定技能
- `CreateSkill()` / `UpdateSkill()` / `DeleteSkill()` - 技能 CRUD
- `ListAgentSkills()` - 列出 Agent 拥有的技能
- `AddAgentSkill()` / `RemoveAgentSkill()` - 管理 Agent 技能

---

## 5. WebSocket 实时事件系统

### 5.1 核心架构

**文件**: `multica/server/internal/realtime/hub.go`

Multica 采用 **Hub + Scope** 架构实现实时通信：

```
                    ┌─────────────────────────────────────┐
                    │           Hub (单例)                │
                    │  ┌───────────────────────────────┐  │
                    │  │  Rooms: map[scopeKey]Clients  │  │
                    │  └───────────────────────────────┘  │
                    │  ┌───────────────────────────────┐  │
                    │  │  Clients: map[*Client]bool    │  │
                    │  └───────────────────────────────┘  │
                    └─────────────────────────────────────┘
                              ↑
         ┌────────────────────┼────────────────────┐
         │                    │                    │
    ┌────┴────┐          ┌────┴────┐         ┌────┴────┐
    │ Client │          │ Client │         │ Client │
    └─────────┘          └─────────┘         └─────────┘
```

### 5.2 作用域类型 (Scope)

系统支持多种作用域用于消息广播：

```go
// 常用作用域常量
const (
    ScopeWorkspace = "workspace"  // 工作空间级别
    ScopeUser      = "user"       // 用户级别
    ScopeTask      = "task"       // 任务级别
    ScopeChat      = "chat"       // 聊天会话级别
)
```

### 5.3 Hub 核心功能

**Hub 职责** (`hub.go:178-290`):
- 管理客户端连接 (`register` / `unregister`)
- 维护作用域房间 (`rooms`)
- 消息广播 (`BroadcastToScope()`, `SendToUser()`)
- 订阅/取消订阅管理 (`subscribe()` / `unsubscribe()`)

### 5.4 客户端生命周期

**文件**: `multica/server/internal/realtime/hub.go:756-932`

连接流程：
1. 升级 HTTP → WebSocket (`HandleWebSocket()`)
2. 验证身份（Cookie 或第一条消息的 Token）
3. 自动订阅工作区和用户作用域
4. 启动 `readPump()` 和 `writePump()` 协程
5. 处理客户端消息（订阅/取消订阅、ping）
6. 断开连接时清理资源

### 5.5 消息去重机制

为防止 Redis 广播和本地快速路径重复投递，使用事件 ID 去重：

```go
// client.markSeen() - 记录已见事件，返回是否首次见
func (c *Client) markSeen(eventID string) bool {
    // 使用 LRU 缓存保留最多 128 个最近事件 ID
    // ...
}
```

---

## 6. 数据库核心 Schema

**文件**: `multica/server/migrations/001_init.up.sql`

### 6.1 核心表关系

```
User ───┐
        ├── Member ─── Workspace
        │
Workspace ─┬── Agent ─── AgentTaskQueue ─── Issue
           │
           ├── Issue ──┬── IssueLabel
           │           ├── Comment
           │           ├── IssueDependency
           │           └── InboxItem
           │
           ├── Skill ─── SkillFile
           │     └── AgentSkill (关联 Agent)
           │
           ├── Project
           │
           └── ChatSession ─── ChatMessage
```

### 6.2 关键表说明

| 表名 | 用途 |
|------|------|
| `user` | 用户信息 |
| `workspace` | 工作空间（租户隔离） |
| `member` | 用户-工作空间成员关系 |
| `agent` | AI Agent 配置 |
| `issue` | 任务/问题 |
| `comment` | 评论 |
| `agent_task_queue` | Agent 任务队列（核心） |
| `skill` | 技能 |
| `inbox_item` | 收件箱通知 |
| `activity_log` | 活动日志 |

---

## 7. 关键架构设计决策

### 7.1 租户隔离

- **SQL 层防御**: 所有查询包含 `workspace_id` 过滤
- **示例**: `DELETE FROM skill WHERE id = $1 AND workspace_id = $2` (skill.sql:51)

### 7.2 性能优化

1. **空声明缓存** (`EmptyClaimCache`):
   - 避免无任务时频繁轮询数据库
   - 任务入队时失效缓存

2. **摘要查询**:
   - `ListSkillSummariesByWorkspace()` - 省略大的 `content` 列
   - 减少列表页负载 (参见 `multica#2174`)

### 7.3 可靠性设计

1. **任务租约与恢复**:
   - `claimResponseRecoveryWindow` (90s) - 防止任务重复派发
   - `RecoverOrphanedTasks()` - 守护进程重启时恢复孤儿任务

2. **会话恢复**:
   - `session_id` / `work_dir` 持久化
   - 中断后可恢复对话

3. **自动重试**:
   - 可重试的失败原因: `runtime_offline`, `runtime_recovery`, `timeout`, `codex_semantic_inactivity`
   - 重试受 `max_attempts` 限制

---

## 8. 事件总线与广播

### 8.1 协议定义

**文件**: `multica/server/pkg/protocol/` (需要查看)

事件类型示例:
- `task:queued`
- `task:dispatched`
- `task:running`
- `task:completed`
- `task:failed`
- `task:cancelled`

### 8.2 双写广播器 (DualWriteBroadcaster)

本地 + Redis 双重广播，保证消息可靠性（虽然代码中未完整展示，但从 `hub.go` 中的 dedup 机制可推断）。

---

## 9. 总结

Multica Go 后端是一个精心设计的系统，具有以下特点：

✅ **清晰的分层架构** - pkg/ (库)、internal/ (实现)、handler/ (API)
✅ **健壮的任务状态机** - 完整的生命周期管理
✅ **灵活的 Agent 抽象** - 统一接口支持多种后端
✅ **可扩展的技能系统** - 模块化的能力增强
✅ **可靠的实时通信** - WebSocket + Redis 广播
✅ **注重性能与可靠性** - 缓存、去重、自动重试等机制
✅ **租户安全隔离** - 多层防护

### 关键文件速查表

| 功能 | 文件路径 |
|------|----------|
| 任务服务核心 | `server/internal/service/task.go` |
| 任务生命周期处理 | `server/internal/handler/task_lifecycle.go` |
| Agent 统一接口 | `server/pkg/agent/agent.go` |
| WebSocket Hub | `server/internal/realtime/hub.go` |
| 技能查询 | `server/pkg/db/queries/skill.sql` |
| 初始化 Schema | `server/migrations/001_init.up.sql` |
| 依赖管理 | `server/go.mod` |
