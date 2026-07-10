# AI Frameworks Integration

这个 crate 为你的游戏编辑器提供了三个主流 AI 框架的集成：
- **LangChain** - 生态最广的 LLM 框架，用于构建 AI agent
- **LlamaIndex** - 强大的数据和知识库系统（RAG）
- **DSPy** - Stanford 的声明式编程框架，自动优化提示词

## 功能特性

### 统一的编排器 (AIOrchestrator)
- 统一的 API 来管理所有三个框架
- Agent 注册和执行
- 知识库创建和查询
- 工作流编排
- DSPy 管道优化

### 框架集成
- **LangChain**: 构建灵活的 AI agent，支持多种工具
- **LlamaIndex**: RAG 知识库，处理文档检索
- **DSPy**: 自动提示词优化，少样本学习

## 安装和配置

### 1. Python 依赖
首先需要安装 Python 依赖：

```bash
cd crates/ai-frameworks
pip install -r requirements.txt
```

### 2. Rust 集成
在你的 `Cargo.toml` 中：

```toml
[dependencies]
agent-core = { path = "../agent-core", features = ["ai-frameworks"] }
```

或者直接使用：

```toml
[dependencies]
ai-frameworks = { path = "../ai-frameworks" }
```

### 3. 环境变量
设置 API key：

```bash
export OPENAI_API_KEY="your-api-key-here"
# 可选
export ANTHROPIC_API_KEY="your-anthropic-key"
```

## 快速开始

### 在编辑器中使用

```rust
use agent_core::ai_frameworks::AIFrameworkManager;
use ai_frameworks::types::{AIConfig, AgentConfig, FrameworkType};

#[tokio::main]
async fn main() -> Result<()> {
    // 1. 创建管理器
    let mut manager = AIFrameworkManager::new()
        .with_config(AIConfig {
            openai_api_key: Some("your-key".to_string()),
            ..Default::default()
        });
    
    // 2. 初始化
    manager.initialize().await?;
    
    // 3. 注册 agent
    let coding_agent = AgentConfig {
        name: "game_developer".to_string(),
        system_prompt: "你是专业的 Bevy 游戏开发助手".to_string(),
        tools: vec![],
        framework: FrameworkType::LangChain,
    };
    
    manager.register_agent(coding_agent).await?;
    
    // 4. 执行 agent
    let response = manager.execute_agent(
        "game_developer", 
        "帮我创建一个玩家移动系统"
    ).await?;
    
    println!("AI 回复: {}", response.content);
    
    Ok(())
}
```

### 运行示例

```bash
cd crates/ai-frameworks
cargo run --example basic_usage
```

## 架构说明

### 核心组件

```
crates/ai-frameworks/
├── src/
│   ├── lib.rs                          # 主入口
│   ├── error.rs                        # 错误类型
│   ├── types.rs                        # 共享类型定义
│   ├── langchain_integration.rs        # LangChain 集成
│   ├── llamaindex_integration.rs       # LlamaIndex 集成
│   ├── dspy_integration.rs             # DSPy 集成
│   └── unified_interface.rs            # 统一编排器
├── examples/
│   └── basic_usage.rs                  # 基本使用示例
├── requirements.txt                    # Python 依赖
└── README.md                           # 本文档
```

### 类型层次

- **AIConfig**: 全局配置（API keys, model settings）
- **AgentConfig**: Agent 配置（name, prompt, tools, framework）
- **Document**: 知识库文档
- **WorkflowStep**: 工作流步骤定义
- **AIResponse**: AI 响应（content, tool calls, usage）

### 特性标志

在 `agent-core` 中，AI 框架是可选功能：

```toml
# 启用 AI 框架集成
agent-core = { features = ["ai-frameworks"] }
```

## 高级用法

### 创建知识库

```rust
let documents = vec![
    Document {
        id: "doc1".to_string(),
        content: "Bevy 使用 ECS 架构...".to_string(),
        metadata: json!({"category": "bevy"}),
    },
];

let index_id = manager.create_knowledge_base("bevy_docs", documents).await?;

// 查询
let result = manager.query_knowledge_base("bevy_docs", "什么是 ECS？").await?;
```

### 多 Agent 工作流

```rust
let steps = vec![
    WorkflowStep {
        name: "design".to_string(),
        agent: "designer".to_string(),
        input: "设计游戏机制".to_string(),
        dependencies: vec![],
    },
    WorkflowStep {
        name: "implement".to_string(),
        agent: "coder".to_string(),
        input: "实现设计: {design}".to_string(),
        dependencies: vec!["design".to_string()],
    },
];

let result = manager.execute_workflow(steps).await?;
println!("最终输出: {}", result.final_output);
```

### DSPy 优化管道

```rust
let examples = vec![
    "创建移动系统".to_string(),
    "添加碰撞检测".to_string(),
];

let pipeline_id = manager.create_pipeline(
    "code_generator",
    "生成 Bevy 代码",
    examples
).await?;

let response = manager.execute_pipeline(&pipeline_id, "创建健康系统").await?;
```

## 与现有系统集成

### 在 Agent Platform 中使用

在你的编辑器系统中，可以这样集成：

```rust
// 在 agent-core 的 AgentPlatform 中
pub struct AgentPlatform {
    // ... 现有字段
    ai_frameworks: Option<AIFrameworkManager>,
}

impl AgentPlatform {
    pub fn with_ai_frameworks(mut self, config: AIConfig) -> Self {
        let mut manager = AIFrameworkManager::new().with_config(config);
        // 初始化...
        self.ai_frameworks = Some(manager);
        self
    }
}
```

### 在 UI 中添加控制面板

在 `agent-ui` 中可以添加：
- 框架配置面板
- Agent 管理界面
- 知识库浏览器
- 工作流编辑器

## 开发指南

### 运行测试

```bash
cd crates/ai-frameworks
cargo test
```

### 扩展新框架

要添加新的 AI 框架集成：

1. 在 `src/` 中创建 `new_framework_integration.rs`
2. 实现核心方法（初始化、聊天、工具等）
3. 在 `unified_interface.rs` 中添加到 `AIOrchestrator`
4. 在 `types.rs` 中添加 `FrameworkType` 枚举变体

## 故障排除

### Python 初始化失败

确保：
1. Python 3.9+ 已安装
2. 所有依赖在 `requirements.txt` 中已安装
3. `pyo3` 能找到正确的 Python 解释器

### API key 问题

检查环境变量是否正确设置：

```bash
echo $OPENAI_API_KEY
```

### 性能问题

对于大型知识库：
- 使用批量索引
- 考虑本地嵌入模型
- 实现缓存层

## 许可证

与项目主许可证一致。

## 贡献

欢迎提交 PR！请确保：
1. 添加测试覆盖新功能
2. 更新文档
3. 保持代码风格一致

## 相关资源

- [LangChain 文档](https://python.langchain.com/)
- [LlamaIndex 文档](https://docs.llamaindex.ai/)
- [DSPy 文档](https://dspy-lang.org/)
- [PyO3 文档](https://pyo3.rs/)
