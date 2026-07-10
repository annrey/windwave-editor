//! 基本使用示例 - 展示如何在编辑器中集成和使用 LangChain、LlamaIndex 和 DSPy
//!
//! 运行示例:
//! ```bash
//! cd crates/ai-frameworks
//! cargo run --example basic_usage
//! ```

use ai_frameworks::*;
use log::info;

#[tokio::main]
async fn main() -> Result<()> {
    // 初始化日志
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    info!("=== AI 框架集成示例 ===\n");

    // 1. 创建配置
    let config = AIConfig {
        openai_api_key: Some(
            std::env::var("OPENAI_API_KEY").unwrap_or_else(|_| "demo_key".to_string()),
        ),
        anthropic_api_key: None,
        model: "gpt-4".to_string(),
        temperature: 0.7,
        max_tokens: Some(1024),
    };

    // 2. 创建 AI 编排器
    let mut orchestrator = AIOrchestrator::new(config);

    info!("✓ AI 编排器已创建\n");

    // 3. 示例 1: 注册一个 LangChain Agent
    info!("=== 示例 1: 注册 LangChain Agent ===");

    let coding_agent = AgentConfig {
        name: "coding_assistant".to_string(),
        system_prompt: "你是一个专业的游戏开发助手，擅长编写 Bevy 游戏代码。".to_string(),
        tools: vec![],
        framework: FrameworkType::LangChain,
    };

    orchestrator.register_agent(coding_agent).await?;

    info!("✓ Agent 'coding_assistant' 已注册\n");

    // 4. 示例 2: 创建知识库（使用 LlamaIndex）
    info!("=== 示例 2: 创建知识库（LlamaIndex）===");

    let game_docs = vec![
        Document {
            id: "bevy_intro".to_string(),
            content: "Bevy 是一个用 Rust 编写的数据驱动游戏引擎，使用 ECS 架构。".to_string(),
            metadata: serde_json::json!({"category": "game_engine"}),
        },
        Document {
            id: "ecs_basics".to_string(),
            content: "ECS 代表实体、组件、系统。实体是 ID，组件是数据，系统是逻辑。".to_string(),
            metadata: serde_json::json!({"category": "programming"}),
        },
    ];

    let index_id = orchestrator
        .create_knowledge_base("game_development", game_docs)
        .await?;

    info!("✓ 知识库 'game_development' 已创建，ID: {}\n", index_id);

    // 5. 示例 3: 创建 DSPy 管道
    info!("=== 示例 3: 创建 DSPy 优化管道 ===");

    let examples = vec![
        "创建一个玩家移动系统".to_string(),
        "添加碰撞检测组件".to_string(),
    ];

    let pipeline_id = orchestrator
        .create_pipeline("code_generator", "根据描述生成 Bevy 游戏代码", examples)
        .await?;

    info!("✓ DSPy 管道已创建，ID: {}\n", pipeline_id);

    // 6. 示例 4: 创建工作流
    info!("=== 示例 4: 创建多步骤工作流 ===");

    let workflow_steps = [
        WorkflowStep {
            name: "analyze".to_string(),
            agent: "coding_assistant".to_string(),
            input: "分析这个游戏需求: 创建一个简单的 2D 平台游戏".to_string(),
            dependencies: vec![],
        },
        WorkflowStep {
            name: "generate".to_string(),
            agent: "coding_assistant".to_string(),
            input: "基于分析结果，生成 Bevy 代码".to_string(),
            dependencies: vec!["analyze".to_string()],
        },
    ];

    info!("✓ 工作流已定义: {} 个步骤\n", workflow_steps.len());

    // 7. 示例 5: 查询知识库
    info!("=== 示例 5: 查询知识库 ===");

    let query = "什么是 ECS 架构？";
    let result = orchestrator
        .query_knowledge_base("game_development", query)
        .await?;

    info!("查询: '{}'", query);
    for (doc, score) in result.documents.iter().zip(result.scores.iter()) {
        info!("  - [得分: {:.2}] {}", score, doc.content);
    }
    info!("");

    info!("=== 所有示例完成 ===");
    info!("\n提示:");
    info!("  - 设置 OPENAI_API_KEY 环境变量来使用真实的 AI 功能");
    info!("  - 查看 crates/agent-core/src/ai_frameworks.rs 了解完整的 API");
    info!("  - 在编辑器中使用 AIFrameworkManager 来集成这些功能");

    Ok(())
}
