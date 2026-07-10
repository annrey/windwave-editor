//! Multica Bridge 基本使用示例
//!
//! 这个示例展示了如何使用 bridge 库与 Multica 集成。
//! 注意：要实际连接 Multica，需要先在本地运行 Multica 服务器。

use multica_bridge::*;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    println!("=== Multica Bridge 基本使用示例 ===\n");

    // 1. 配置桥接
    let config = BridgeConfig::default();
    println!("配置:");
    println!("  服务器: {}", config.server_url);
    println!("  工作空间: {}", config.workspace_id);
    println!("  Agent ID: {}", config.agent_id);
    println!("  Daemon ID: {}\n", config.daemon_id);

    // 2. 创建 Agent 代理
    let agent_proxy = AgentProxy::new(config.clone(), "WindWave-Editor".to_string());
    println!("创建 Agent:");
    println!("  名称: {}", agent_proxy.agent_info().name);
    println!("  描述: {}", agent_proxy.agent_info().description);
    println!("  提供方: {:?}\n", agent_proxy.agent_info().provider);

    // 3. 创建任务同步器
    let task_sync = TaskSync::new(config.clone());
    println!("任务同步器已创建，Runtime ID: {}", task_sync.runtime_id());

    // 4. 创建技能适配器
    let mut skill_adapter = SkillAdapter::new();

    // 5. 注册示例技能
    skill_adapter.register_skill(
        "create_player",
        "创建一个玩家角色",
        vec!["game".to_string(), "player".to_string()],
        |params| {
            println!("\n执行 create_player 技能，参数: {:?}", params);
            Ok(serde_json::json!({
                "success": true,
                "player_id": "player_001",
                "name": params.get("name").and_then(|n| n.as_str()).unwrap_or("Player")
            }))
        },
    );

    skill_adapter.register_skill(
        "add_obstacle",
        "添加一个障碍物",
        vec!["game".to_string(), "scene".to_string()],
        |params| {
            println!("\n执行 add_obstacle 技能，参数: {:?}", params);
            Ok(serde_json::json!({
                "success": true,
                "obstacle_id": "obstacle_001"
            }))
        },
    );

    println!("\n注册的技能:");
    for skill in skill_adapter.list_local_skills() {
        println!("  - {}: {}", skill.name, skill.description);
        println!("    标签: {:?}", skill.tags);
    }

    // 6. 测试执行技能
    println!("\n测试执行技能...");
    let result = skill_adapter
        .execute_local_skill(
            skill_adapter.list_local_skills()[0].id.as_str(),
            &serde_json::json!({"name": "Hero"}),
        )
        .await?;
    println!("技能执行结果: {:?}", result);

    // 7. 尝试连接 Multica（如果服务器运行）
    println!("\n提示: 要实际连接 Multica，请先运行 Multica 服务器：");
    println!("  cd multica && make selfhost");
    println!("\n然后打开 http://localhost:3000 登录并创建 Issue");

    // 8. 模拟处理任务的流程
    println!("\n=== 模拟任务处理流程 ===");
    println!("1. Multica 分配任务给 WindWave Agent");
    println!("2. Bridge 接收 task:dispatch 消息");
    println!("3. 任务被转换为本地任务并执行");
    println!("4. 通过 task:progress 发送进度更新");
    println!("5. 完成后发送 task:completed");

    Ok(())
}
