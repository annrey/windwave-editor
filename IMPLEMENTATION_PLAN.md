# WindWave Implementation Plan

> Based on design/5.10规划.md detailed implementation scheme
> Generated: 2026-05-11
> [中文](#中文) | [日本語](#日本語)

---

## Priority Notes

The core issue identified in design/5.10规划.md: **The Agent's LLM closed loop has never been truly connected**. All "intelligent" behaviors were actually completed through keyword matching and rule engines. While the LLM client exists, it was not used by the main execution path.

**Sprint 1 is the highest priority**. After completion, the project can truly claim to be "AI Agent-driven".

---

## Sprint 1: Core Loop Connection (2 weeks, ~20 hours) — Started

### Goal
Enable the Agent to truly use LLM think-act-observe, upgrading from "pseudo-intelligent" to "truly intelligent"

### Task List

#### A1: ReAct Execution Loop (8h) — Week 1
- [x] Analyze why ReActAgent is not connected to the main flow
- [x] Integrate ReActAgent into DirectorRuntime
- [x] Modify execute_direct_internal to use ReAct loop
- [x] Modify execute_plan_internal to use ReAct per step
- [x] Handle Tokio Runtime conflicts
- [x] Test basic think→act→observe loop

**Key Changes**:
```rust
// director/types.rs
pub struct DirectorRuntime {
    // Added
    pub react_agent: Option<ReActAgent>,
}

// director/execution.rs
fn execute_direct_internal(&mut self, request_text: &str) {
    if let Some(ref mut react) = self.react_agent {
        // Use ReAct loop for execution
        let result = react.run(request_text).await?;
    } else {
        // Fallback to keyword matching
        self.execute_direct_fallback(request_text)
    }
}
```

#### A2: Plan-and-Solve Dynamic Revision (4h) — Week 1
- [x] Implement dynamic plan adjustment during execution
- [x] Modify subsequent steps based on intermediate results
- [x] Integrate into execute_plan_internal
- [x] Test plan self-adaptation capability

#### A3: Reflection Paradigm (3h) — Week 1
- [x] Feed review results back to Agent
- [x] Agent auto-corrects based on ReviewerDecision
- [x] Implement automatic error retry
- [x] Test self-correction capability

#### C1: L0-L3 Layered Context (3h) — Week 2
- [x] Extend ContextCollector for full L0-L3 implementation
- [x] System-level context (L0): tool list, capabilities, constraints
- [x] Session-level context (L1): dialogue history, user preferences
- [x] Task-level context (L2): current task, intermediate results
- [x] Entity-level context (L3): scene state, entity details
- [x] Integrate into ReActAgent prompt building

#### C2: Few-shot Example Injection (2h) — Week 2
- [x] Prepare call examples for each tool type
- [x] Implement Few-shot example selector
- [x] Integrate into prompt building
- [x] Test prompt quality improvement

### Acceptance Criteria
- [x] Input "Create a red enemy" → Agent calls LLM analysis → calls create_entity → verifies result
- [x] Execution can dynamically adjust plan (e.g., skip creation if entity already exists)
- [x] Agent automatically retries alternative after execution failure
- [x] LLM request contains layered context information

---

## Sprint 2: Memory & Context Deepening (2 weeks, ~15 hours)

### Goal
Agent has persistent memory, can remember user preferences and historical operations

### Task List

#### B1: MemoryInjector (5h)
- [x] Implement automatic context capture into memory
- [x] Integrate into event stream pipeline
- [ ] Test cross-session memory

#### B2: LLM Memory Compression (3h)
- [ ] Implement long dialogue automatic summarization
- [ ] Use LLM to compress dialogue history
- [ ] Test 50+ round dialogues

#### B3: Episodic Memory (3h)
- [ ] Implement user preference recording
- [ ] Remember "user liked red last time" type information
- [ ] Test preference extraction

#### B4: Auto-Learning Mode (2h)
- [ ] Learn from user feedback
- [ ] Record correction patterns
- [ ] Test learning effectiveness

#### E1: KeywordMatcher Extraction (2h)
- [ ] Decouple from router.rs
- [ ] Independent fallback module
- [ ] Maintain backward compatibility

### Acceptance Criteria
- [ ] User says "same as last time" → Agent retrieves last operation from memory
- [ ] Long dialogues (50+ rounds) automatically compressed into summary
- [ ] Agent remembers preferences after user correction

---

## Sprint 3: Vision & Interaction Upgrade (2 weeks, ~10 hours)

### Goal
Agent can "see" the scene and operate based on visual feedback

### Task List

#### D1: VisualUnderstanding UI (4h)
- [ ] Display what Agent sees
- [ ] Annotated screenshot display
- [ ] Visual analysis result display

#### D2: Visual Feedback Loop (3h)
- [ ] Implement VGRC flow
- [ ] Screenshot → analysis → operation → re-screenshot verification
- [ ] Integrate into execution loop

#### D3: HybridEditorController (2h)
- [ ] LLM + rule hybrid decision-making
- [ ] Automatic fallback when LLM unavailable
- [ ] Test fallback mechanism

#### E2: self_modifying_agent Sandbox (1h)
- [ ] Fix sandbox isolation
- [ ] Security checks
- [ ] Test sandbox effectiveness

### Acceptance Criteria
- [ ] Agent automatically screenshots for verification after operation
- [ ] UI displays visual analysis results (annotation boxes, text descriptions)
- [ ] Automatic fallback to rule engine when LLM unavailable

---

## Sprint 4-8: Follow-up Planning

See design/5.10规划.md:
- Sprint 4: Narrative System Foundation (3 weeks)
- Sprint 5: Engineering Consolidation & Extension (2 weeks)
- Sprint 6: Universal Editor Capabilities (4 weeks)
- Sprint 7: Narrative System Deepening + Multi-Engine (3 weeks)
- Sprint 8: Advanced Editor Features (4 weeks)

---

## Parallel Improvement Tasks

These tasks can proceed in parallel with main Sprints:

### File System Tools (OpenGame-inspired)
- [ ] ReadFileTool / WriteFileTool
- [ ] GrepTool / GlobTool
- [ ] EditFileTool (search & replace)

### Shadow Git System
- [ ] ShadowGitService implementation
- [ ] File-level snapshots
- [ ] File-level Undo

### Rule File System
- [ ] agent-rules.toml definition
- [ ] Rule loading and validation
- [ ] Rule check integration

---

## Immediate Actions

### Step 1: Connect ReAct Loop (Today)
1. Add react_agent field to DirectorRuntime
2. Create ReActAgent instance during initialization
3. Modify execute_direct_internal to use ReAct
4. Test basic functionality

### Step 2: Layered Context (Tomorrow)
1. Complete ContextCollector L0-L3 implementation
2. Integrate into ReActAgent prompt building
3. Test context quality

### Step 3: Verify Effect (Day After)
1. Run acceptance tests
2. Compare keyword matching vs LLM reasoning
3. Record improvement effects

---

## Risk Mitigation

| Risk | Mitigation |
|------|-----------|
| High LLM API costs | Implement cache layer, local small model fallback |
| Tokio Runtime conflicts | Use OnceLock<Runtime> pattern |
| State synchronization issues | SharedSceneBridge access |
| Prompt engineering | Use PromptSystem to build context |
| Timeout control | BaseAgent timeout and deadlock detection |

---

## Success Metrics

**After Sprint 1**:
- Agent can use LLM to understand complex instructions
- Execution process is observable (think→act→observe)
- Failures can auto-retry
- Context information is complete

**Ultimate Goal**:
- Upgrade from "keyword matching" to "LLM reasoning"
- Upgrade from "single execution" to "feedback loop"
- Upgrade from "fixed rules" to "adaptive learning"

---

## 中文

### AgentEdit 实施计划

> 基于 design/5.10规划.md 的详细实施方案
> 生成日期: 2026-05-11

**Sprint 1 是最高优先级**，完成后项目才能真正自称"AI Agent驱动"。

**Sprint 1 目标**: 让Agent真正能用LLM思考-行动-观察，从"伪智能"变为"真智能"
- A1: ReAct执行循环 ✅
- A2: Plan-and-Solve动态修订 ✅
- A3: Reflection范式 ✅
- C1: L0-L3分层上下文 ✅
- C2: Few-shot示例注入 ✅

详见上方英文文档获取完整信息。

---

## 日本語

### WindWave 実装計画

> design/5.10规划.md に基づく詳細実装方案
> 生成日: 2026-05-11

**Sprint 1 が最優先**です。完了後、プロジェクトは初めて「AI Agent駆動」と名乗ることができます。

**Sprint 1 の目標**: AgentがLLMを使って思考-行動-観察できるようにし、「疑似知能」から「真の知能」へ
- A1: ReAct実行ループ ✅
- A2: Plan-and-Solve動的改訂 ✅
- A3: Reflectionパラダイム ✅
- C1: L0-L3レイヤードコンテキスト ✅
- C2: Few-shot例注入 ✅

詳細は上記の英語ドキュメントをご参照ください。
