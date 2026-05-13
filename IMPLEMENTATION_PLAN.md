# AgentEdit 实施计划

> 基于 design/5.10规划.md 的详细实施方案
> 生成日期: 2026-05-11

---

## 优先级说明

根据5.10规划.md的核心建议:**当前项目最大的问题是Agent的LLM闭环从未真正打通**。所有"智能"行为实际上是通过关键词匹配和规则引擎完成的,LLM客户端虽然存在但未被主执行路径使用。

**Sprint 1是最高优先级**,完成后项目才能真正自称"AI Agent驱动"。

---

## Sprint 1: 核心闭环打通 (2周, ~20工时) - 立即开始

### 目标
让Agent真正能用LLM思考-行动-观察,从"伪智能"变为"真智能"

### 任务清单

#### A1: ReAct执行循环 (8h) - 第1周
- [x] 分析当前ReActAgent未接入主流程的原因
- [ ] 在DirectorRuntime中集成ReActAgent
- [ ] 修改execute_direct_internal使用ReAct循环
- [ ] 修改execute_plan_internal每步使用ReAct
- [ ] 处理Tokio Runtime冲突
- [ ] 测试基础think→act→observe循环

**关键改动**:
```rust
// director/types.rs
pub struct DirectorRuntime {
    // 新增
    pub react_agent: Option<ReActAgent>,
}

// director/execution.rs
fn execute_direct_internal(&mut self, request_text: &str) {
    if let Some(ref mut react) = self.react_agent {
        // 使用ReAct循环执行
        let result = react.run(request_text).await?;
    } else {
        // 降级到关键词匹配
        self.execute_direct_fallback(request_text)
    }
}
```

#### A2: Plan-and-Solve动态修订 (4h) - 第1周
- [ ] 实现执行中计划动态调整
- [ ] 根据中间结果修改后续步骤
- [ ] 集成到execute_plan_internal
- [ ] 测试计划自适应能力

#### A3: Reflection范式 (3h) - 第1周
- [ ] 审查结果回传Agent
- [ ] Agent根据ReviewerDecision自动修正
- [ ] 实现错误自动重试
- [ ] 测试自我修正能力

#### C1: L0-L3分层上下文 (3h) - 第2周
- [ ] 扩展ContextCollector实现完整L0-L3
- [ ] 系统层上下文(L0): 工具列表、能力、约束
- [ ] 会话层上下文(L1): 对话历史、用户偏好
- [ ] 任务层上下文(L2): 当前任务、中间结果
- [ ] 实体层上下文(L3): 场景状态、实体详情
- [ ] 集成到ReActAgent的prompt构建

#### C2: Few-shot示例注入 (2h) - 第2周
- [ ] 为每种工具类型准备调用示例
- [ ] 实现Few-shot示例选择器
- [ ] 集成到提示词构建
- [ ] 测试提示词质量提升

### 验收标准
- [ ] 输入"创建一个红色敌人" → Agent调用LLM分析 → 调用create_entity → 验证结果
- [ ] 执行中可动态调整计划(如实体已存在则跳过创建)
- [ ] 执行失败后Agent自动重试替代方案
- [ ] LLM请求中包含分层上下文信息

---

## Sprint 2: 记忆与上下文深化 (2周, ~15工时)

### 目标
Agent拥有持久记忆,能记住用户偏好和历史操作

### 任务清单

#### B1: MemoryInjector (5h)
- [ ] 实现自动捕获上下文写入记忆
- [ ] 集成到事件流管道
- [ ] 测试跨会话记忆

#### B2: LLM记忆压缩 (3h)
- [ ] 实现长对话自动摘要
- [ ] 使用LLM压缩对话历史
- [ ] 测试50+轮对话

#### B3: 情景记忆 (3h)
- [ ] 实现用户偏好记录
- [ ] 记住"上次用户喜欢红色"类型的信息
- [ ] 测试偏好提取

#### B4: 自动学习模式 (2h)
- [ ] 从用户反馈中学习
- [ ] 记录纠正模式
- [ ] 测试学习效果

#### E1: KeywordMatcher提取 (2h)
- [ ] 从router.rs中解耦
- [ ] 独立为fallback模块
- [ ] 保持向后兼容

### 验收标准
- [ ] 用户说"和上次一样" → Agent从记忆中检索上次操作
- [ ] 长对话(50+轮)自动压缩为摘要
- [ ] 用户纠正后Agent记住偏好

---

## Sprint 3: 视觉与交互升级 (2周, ~10工时)

### 目标
Agent能"看到"场景并基于视觉反馈操作

### 任务清单

#### D1: VisualUnderstanding UI (4h)
- [ ] 展示Agent看到了什么
- [ ] 标注截图显示
- [ ] 视觉分析结果展示

#### D2: 视觉反馈循环 (3h)
- [ ] 实现VGRC流程
- [ ] 截图→分析→操作→再截图验证
- [ ] 集成到执行循环

#### D3: HybridEditorController (2h)
- [ ] LLM + 规则混合决策
- [ ] LLM不可用时自动降级
- [ ] 测试降级机制

#### E2: self_modifying_agent沙盒 (1h)
- [ ] 修复沙盒隔离
- [ ] 安全检查
- [ ] 测试沙盒有效性

### 验收标准
- [ ] Agent操作后自动截图验证
- [ ] UI展示视觉分析结果(标注框、文字说明)
- [ ] LLM不可用时自动降级到规则引擎

---

## Sprint 4-8: 后续规划

详见 design/5.10规划.md:
- Sprint 4: 叙事系统基础 (3周)
- Sprint 5: 工程巩固与扩展 (2周)
- Sprint 6: 通用编辑器能力 (4周)
- Sprint 7: 叙事系统深化 + 多引擎 (3周)
- Sprint 8: 高级编辑器功能 (4周)

---

## 并行改进任务

这些任务可以与主Sprint并行进行:

### 文件系统工具 (OpenGame借鉴)
- [ ] ReadFileTool / WriteFileTool
- [ ] GrepTool / GlobTool
- [ ] EditFileTool (搜索替换)

### 影子Git系统
- [ ] ShadowGitService实现
- [ ] 文件级快照
- [ ] 文件级Undo

### 规则文件体系
- [ ] agent-rules.toml定义
- [ ] 规则加载和验证
- [ ] 规则检查集成

---

## 当前立即行动

### 第1步: 打通ReAct循环 (今天)
1. 在DirectorRuntime中添加react_agent字段
2. 初始化时创建ReActAgent实例
3. 修改execute_direct_internal使用ReAct
4. 测试基础功能

### 第2步: 分层上下文 (明天)
1. 完善ContextCollector的L0-L3实现
2. 集成到ReActAgent的prompt构建
3. 测试上下文质量

### 第3步: 验证效果 (后天)
1. 运行验收测试
2. 对比关键词匹配vs LLM推理
3. 记录改进效果

---

## 风险缓解

| 风险 | 缓解措施 |
|------|---------|
| LLM API成本过高 | 实现缓存层、本地小模型备选 |
| Tokio Runtime冲突 | 使用OnceLock<Runtime>模式 |
| 状态同步问题 | SharedSceneBridge访问 |
| 提示词工程 | 使用PromptSystem构建上下文 |
| 超时控制 | BaseAgent的超时和卡死检测 |

---

## 成功指标

**Sprint 1完成后应达到**:
- Agent能用LLM理解复杂指令
- 执行过程可观察(think→act→observe)
- 失败能自动重试
- 上下文信息完整

**最终目标**:
- 从"关键词匹配"升级为"LLM推理"
- 从"单次执行"升级为"反馈循环"
- 从"固定规则"升级为"自适应学习"
