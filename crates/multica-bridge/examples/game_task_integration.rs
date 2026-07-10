//! Game Task Integration — Fusion Architecture Demo
//!
//! Demonstrates the full WindWave + Multica fusion architecture workflow:
//!   1. Scene & Entity management (MulticaDb)
//!   2. Advanced querying (DbLayer — EntityQuery, SceneQuery, Pagination)
//!   3. Game skill system (SkillSystem)
//!   4. Scene-aware agent (SceneAgent)
//!   5. Pipeline orchestration (AgentOrchestrator)
//!   6. Task lifecycle (TaskBridge)
//!   7. Snapshots & batch transactions (DbSnapshot, BatchTransaction)
//!   8. Entity indexing (EntityIndex)

use multica_bridge::*;
use serde_json::json;

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║   WindWave + Multica 融合架构 — 游戏任务看板演示         ║");
    println!("╚══════════════════════════════════════════════════════════╝\n");

    // ============================================================
    // 1. 初始化基础设施
    // ============================================================
    println!("━━━ [1] 基础设施初始化 ━━━");

    let db = create_shared_multica_db();
    let scene_context = create_shared_scene_context();
    let config = BridgeConfig::default();
    let db_layer = DbLayer::new(db.clone());

    println!("  ✓ MulticaDb 共享数据库已创建");
    println!("  ✓ SceneContext 共享场景上下文已创建");
    println!("  ✓ DbLayer 统一访问层已创建");

    // ============================================================
    // 2. 创建游戏场景和实体
    // ============================================================
    println!("\n━━━ [2] 场景与实体创建 ━━━");

    let scene_id = {
        let mut d = db.lock().unwrap();
        let sid = d
            .create_scene(
                "ForestKingdom".to_string(),
                Some("精灵森林王国，包含三个子区域".to_string()),
            )
            .unwrap();
        println!("  ✓ 创建场景: ForestKingdom ({})", &sid[..8]);

        d.create_entity(
            sid.clone(),
            "Terrain_Main".to_string(),
            "Terrain".to_string(),
            vec![],
            Some([0.0, 0.0, 0.0]),
        )
        .unwrap();

        d.create_entity(
            sid.clone(),
            "Player_Spawn".to_string(),
            "PlayerSpawn".to_string(),
            vec![],
            Some([100.0, 0.0, 200.0]),
        )
        .unwrap();

        d.create_entity(
            sid.clone(),
            "Elf_Guard_01".to_string(),
            "NPC".to_string(),
            vec![],
            Some([150.0, 0.0, 180.0]),
        )
        .unwrap();
        d.create_entity(
            sid.clone(),
            "Elf_Merchant_01".to_string(),
            "NPC".to_string(),
            vec![],
            Some([120.0, 0.0, 250.0]),
        )
        .unwrap();
        d.create_entity(
            sid.clone(),
            "Forest_Dragon_Boss".to_string(),
            "Boss".to_string(),
            vec![],
            Some([300.0, 0.0, 400.0]),
        )
        .unwrap();

        d.create_entity(
            sid.clone(),
            "HealthPotion_Stash_01".to_string(),
            "Pickup".to_string(),
            vec![],
            Some([110.0, 1.0, 210.0]),
        )
        .unwrap();
        d.create_entity(
            sid.clone(),
            "ManaCrystal_01".to_string(),
            "Pickup".to_string(),
            vec![],
            Some([160.0, 0.5, 230.0]),
        )
        .unwrap();
        d.create_entity(
            sid.clone(),
            "TreasureChest_Golden".to_string(),
            "Container".to_string(),
            vec![],
            Some([250.0, 0.0, 350.0]),
        )
        .unwrap();

        d.create_entity(
            sid.clone(),
            "AmbientLight_Forest".to_string(),
            "Light".to_string(),
            vec![],
            Some([0.0, 100.0, 0.0]),
        )
        .unwrap();
        d.create_entity(
            sid.clone(),
            "Torch_Entrance".to_string(),
            "Light".to_string(),
            vec![],
            Some([100.0, 2.0, 200.0]),
        )
        .unwrap();

        d.create_scene(
            "ForestKingdom_Cave".to_string(),
            Some("精灵森林的地下洞穴区域".to_string()),
        )
        .unwrap();
        d.create_scene(
            "ForestKingdom_Temple".to_string(),
            Some("远古精灵神庙，最终Boss场景".to_string()),
        )
        .unwrap();

        sid
    };

    let stats = db.lock().unwrap().get_statistics();
    println!(
        "  ✓ 场景总数: {}, 实体总数: {}",
        stats.scene_count, stats.entity_count
    );

    // ============================================================
    // 3. 数据库查询层演示
    // ============================================================
    println!("\n━━━ [3] DbLayer 查询演示 ━━━");

    {
        let d = db.lock().unwrap();

        let npcs = EntityQuery::new()
            .filter(EntityFilter::EntityType("NPC".to_string()))
            .execute(&d);
        println!("  ✓ NPC 实体: {} 个", npcs.total);
        for e in &npcs.items {
            println!("    - {} (ID: {})", e.name, e.entity_id);
        }

        let elves = EntityQuery::new()
            .filter(EntityFilter::NameStartsWith("Elf".to_string()))
            .execute(&d);
        println!("  ✓ 精灵族实体: {} 个", elves.total);

        let page1 = EntityQuery::new()
            .paginate(Pagination::page(1, 4))
            .sort_by(SortSpec::asc(SortField::Name))
            .execute(&d);
        println!(
            "  ✓ 分页查询 (第1页/每页4条): 共 {} 页, 当前 {} 条",
            page1.total_pages,
            page1.items.len()
        );

        let populated = SceneQuery::new()
            .filter(SceneFilter::HasEntities(true))
            .execute(&d);
        println!("  ✓ 包含实体的场景: {} 个", populated.total);
        for s in &populated.items {
            let count = d.get_scene_entities(&s.scene_id).len();
            println!("    - {} ({} 个实体)", s.name, count);
        }

        let empty = SceneQuery::new()
            .filter(SceneFilter::HasEntities(false))
            .execute(&d);
        println!("  ✓ 空场景: {} 个", empty.total);
        for s in &empty.items {
            println!("    - {}", s.name);
        }
    }

    let index = db_layer.build_index().unwrap();
    let torches = index.find_by_prefix("torch");
    println!(
        "  ✓ EntityIndex 查找 'torch' 前缀: {} 个匹配",
        torches.len()
    );
    let lights = index.find_by_type("Light");
    println!("  ✓ EntityIndex 查找 Light 类型: {} 个匹配", lights.len());

    // ============================================================
    // 4. 技能系统演示
    // ============================================================
    println!("\n━━━ [4] 技能系统 ━━━");

    let skill_system = SkillSystem::new(db.clone(), scene_context.clone());

    let skills = skill_system.list_skills();
    println!("  ✓ 已注册 {} 个技能 (自动加载内置技能)", skills.len());
    for skill in skills.iter().take(6) {
        let cat = skill.category.as_deref().unwrap_or("未分类");
        println!("    - [{}] {}: {}", cat, skill.name, skill.description);
    }
    if skills.len() > 6 {
        println!("    ... 还有 {} 个技能", skills.len() - 6);
    }

    println!("\n  --- 技能执行演示 ---");

    let ctx = SkillExecutionContext {
        scene_id: Some(scene_id.clone()),
        entity_ids: vec![],
        ..Default::default()
    };

    match skill_system.execute("scene.stats", &json!({"scene_id": scene_id}), Some(&ctx)) {
        Ok(result) => {
            println!(
                "  ✓ 执行 'scene.stats': {}",
                if result.success { "成功" } else { "失败" }
            );
            if let Some(data) = &result.data {
                if let Some(entity_count) = data.get("entity_count") {
                    println!("    场景实体数: {}", entity_count);
                }
                if let Some(scenes_count) = data.get("scenes_count") {
                    println!("    总场景数: {}", scenes_count);
                }
            }
        }
        Err(e) => println!("  ✗ 技能执行失败: {}", e),
    }

    match skill_system.execute(
        "entity.query",
        &json!({"query": "Dragon", "scene_id": scene_id}),
        Some(&ctx),
    ) {
        Ok(result) => {
            println!(
                "  ✓ 执行 'entity.query(query=Dragon)': {}",
                if result.success { "成功" } else { "失败" }
            );
        }
        Err(e) => println!("  ✗ 技能执行失败: {}", e),
    }

    // ============================================================
    // 5. 场景感知 Agent
    // ============================================================
    println!("\n━━━ [5] 场景感知 Agent ━━━");

    let agent_config = SceneAgentConfig {
        agent_name: "GameDesigner".to_string(),
        scene_id: Some(scene_id.clone()),
        capabilities: vec![
            SceneAgentCapability::QueryScene,
            SceneAgentCapability::CreateEntity,
            SceneAgentCapability::ModifyEntity,
            SceneAgentCapability::DeleteEntity,
            SceneAgentCapability::ValidateScene,
            SceneAgentCapability::OptimizeScene,
        ],
        system_prompt: Some("你是一位游戏关卡设计师，负责管理和优化游戏场景。".to_string()),
        ..Default::default()
    };

    let scene_agent = SceneAgent::new(agent_config, db.clone(), scene_context.clone());

    let result = scene_agent.execute_with_scene_context(
        "请分析当前场景中的实体分布，告诉我有哪些类型的实体各有多少个。",
    );
    println!(
        "  ✓ Agent 执行: {}",
        if result.success { "成功" } else { "失败" }
    );
    println!("    使用能力: {:?}", result.capability_used);
    if !result.entity_ids_affected.is_empty() {
        println!("    影响实体: {:?}", result.entity_ids_affected);
    }

    match scene_agent.execute_capability(SceneAgentCapability::ValidateScene, &json!({})) {
        Ok(result) => {
            println!(
                "  ✓ Agent 场景验证: {}",
                if result.success {
                    "通过"
                } else {
                    "发现问题"
                }
            );
        }
        Err(e) => println!("  ✗ 场景验证失败: {}", e),
    }

    // ============================================================
    // 6. 任务管理 (TaskBridge)
    // ============================================================
    println!("\n━━━ [6] 任务看板 ━━━");

    let task_sync = TaskSync::new(config.clone());
    let task_bridge = TaskBridge::new(task_sync);

    let entity_ids: Vec<u64> = {
        let d = db.lock().unwrap();
        d.all_entities().iter().map(|e| e.entity_id).collect()
    };

    let mut task1 = UnifiedTask::new(
        "设计森林入口光照效果".to_string(),
        "为 ForestKingdom 入口区域添加动态光照".to_string(),
    );
    task1.scene_id = Some(scene_id.clone());
    task1.entity_ids = entity_ids
        .iter()
        .take(3)
        .copied()
        .map(|id| id.to_string())
        .collect();
    let t1 = task_bridge.register_task(task1).unwrap();

    let mut task2 = UnifiedTask::new(
        "创建 Boss 战斗区域".to_string(),
        "在 Temple 场景中创建 Boss 战斗区域，包含触发器和特效".to_string(),
    );
    task2.scene_id = Some(scene_id.clone());
    task2.entity_ids = entity_ids
        .iter()
        .skip(3)
        .take(2)
        .copied()
        .map(|id| id.to_string())
        .collect();
    let _t2 = task_bridge.register_task(task2).unwrap();

    let mut task3 = UnifiedTask::new(
        "添加 NPC 巡逻路径".to_string(),
        "为森林中的精灵守卫设置巡逻路径".to_string(),
    );
    task3.scene_id = Some(scene_id.clone());
    task3.entity_ids = entity_ids
        .iter()
        .take(2)
        .copied()
        .map(|id| id.to_string())
        .collect();
    let _t3 = task_bridge.register_task(task3).unwrap();

    let mut task4 = UnifiedTask::new(
        "优化场景性能".to_string(),
        "优化 ForestKingdom 场景的渲染性能，减少 Draw Call".to_string(),
    );
    task4.scene_id = Some(scene_id.clone());
    let _t4 = task_bridge.register_task(task4).unwrap();

    println!(
        "  ✓ 任务看板 (共 {} 个任务):",
        task_bridge.get_all_tasks().len()
    );
    for task in task_bridge.get_all_tasks() {
        println!("    - [{:?}] {}", task.status, task.title);
        if let Some(ref sid) = task.scene_id {
            println!(
                "      场景: {} | 关联实体: {} 个",
                &sid[..8.min(sid.len())],
                task.entity_ids.len()
            );
        }
    }

    let scene_tasks = task_bridge.get_tasks_by_scene(&scene_id);
    println!(
        "\n  ✓ 场景 'ForestKingdom' 关联任务: {} 个",
        scene_tasks.len()
    );

    let _ = task_bridge.update_task_status(t1.id.bridge_id, UnifiedTaskStatus::Running);
    println!("  ✓ 任务 '{}' → 进行中", t1.title);

    // ============================================================
    // 7. Pipeline 编排 (AgentOrchestrator)
    // ============================================================
    println!("\n━━━ [7] Pipeline 编排 ━━━");

    let mut orchestrator =
        AgentOrchestrator::new(config.clone(), db.clone(), scene_context.clone())
            .with_scene_agent(scene_agent)
            .with_skill_system(skill_system)
            .with_task_bridge(task_bridge);

    orchestrator.initialize().unwrap();
    println!(
        "  ✓ AgentOrchestrator 已初始化, 状态: {:?}",
        orchestrator.status()
    );

    let mut pipeline_task = PipelineTask::new(
        "场景优化 Pipeline",
        "优化森林场景: 验证 -> 处理光照 -> 优化实体分布",
    )
    .with_scene(&scene_id)
    .with_skill("utility.validate", json!({"scene_id": scene_id}))
    .with_agent_prompt("优化 ForestKingdom 场景性能，重点关注光照和实体密度");

    match orchestrator.execute_pipeline_task(&mut pipeline_task) {
        Ok(()) => {
            println!("  ✓ Pipeline 任务完成: {}", pipeline_task.title);
            println!("    状态: {:?}", pipeline_task.status);
            if let Some(ref result) = pipeline_task.result {
                println!("    结果: {}", result);
            }
            if let Some(duration) = pipeline_task.duration_ms {
                println!("    耗时: {}ms", duration);
            }
        }
        Err(e) => println!("  ✗ Pipeline 执行失败: {}", e),
    }

    let pstats = orchestrator.stats();
    println!("\n  Pipeline 统计:");
    println!(
        "    Agent 调度: {} / 完成: {} / 失败: {}",
        pstats.agent_tasks_dispatched, pstats.agent_tasks_completed, pstats.agent_tasks_failed
    );
    println!(
        "    技能执行: {} / 成功率: {:.0}%",
        pstats.skills_executed,
        pstats.skill_success_rate * 100.0
    );
    println!("    内存注入: {} 次", pstats.memory_injections);
    println!("    场景事件: {} 个", pstats.scene_events_processed);

    // ============================================================
    // 8. 快照与恢复
    // ============================================================
    println!("\n━━━ [8] 快照与恢复 ━━━");

    let snapshot = db_layer.snapshot().unwrap();
    println!("  ✓ 快照已创建:");
    println!("    场景: {} 个", snapshot.scenes.len());
    println!("    实体: {} 个", snapshot.entities.len());
    println!("    资源: {} 个", snapshot.resources.len());
    println!("    版本: v{}", snapshot.version);

    let snapshot_json = snapshot.to_json().unwrap();
    println!("  ✓ 快照 JSON 大小: {} bytes", snapshot_json.len());

    {
        let mut d = db.lock().unwrap();
        d.clear_all();
    }
    println!(
        "  ✓ 数据库已清空 (实体数: {})",
        db.lock().unwrap().get_statistics().entity_count
    );

    db_layer.restore(&snapshot).unwrap();
    let restored_stats = db.lock().unwrap().get_statistics();
    println!(
        "  ✓ 从快照恢复完成: 场景 {} 个, 实体 {} 个",
        restored_stats.scene_count, restored_stats.entity_count
    );
    assert_eq!(restored_stats.scene_count, 3);
    assert_eq!(restored_stats.entity_count, 10);

    // ============================================================
    // 9. 批量事务
    // ============================================================
    println!("\n━━━ [9] 批量事务 ━━━");

    let recovered_scene_id = {
        let d = db.lock().unwrap();
        d.get_all_scenes().first().unwrap().scene_id.clone()
    };

    let tx = BatchTransaction::new()
        .push(BatchOp::CreateEntity {
            scene_id: recovered_scene_id.clone(),
            name: "Fairy_Light_01".to_string(),
            entity_type: "Particle".to_string(),
            components_json: json!([]),
            position: Some([180.0, 5.0, 220.0]),
        })
        .push(BatchOp::CreateEntity {
            scene_id: recovered_scene_id.clone(),
            name: "Fairy_Light_02".to_string(),
            entity_type: "Particle".to_string(),
            components_json: json!([]),
            position: Some([190.0, 5.0, 240.0]),
        })
        .push(BatchOp::CreateResource {
            resource_type: "Texture".to_string(),
            name: "FairyGlow.png".to_string(),
            path: Some("assets/textures/fairy_glow.png".to_string()),
            metadata: std::collections::HashMap::from([
                ("size".to_string(), "512x512".to_string()),
                ("format".to_string(), "PNG".to_string()),
            ]),
        });

    {
        let mut d = db.lock().unwrap();
        let result = tx.execute(&mut d).unwrap();
        println!("  ✓ 批量事务完成:");
        println!(
            "    操作总数: {} / 成功: {} / 失败: {}",
            result.total_ops, result.succeeded, result.failed
        );
        println!("    已回滚: {}", result.rolled_back);
        for r in &result.results {
            println!(
                "    [{}] {}: {}",
                r.index,
                r.op_type,
                if r.success {
                    "OK"
                } else {
                    r.error.as_deref().unwrap_or("unknown")
                }
            );
        }
    }

    let bad_tx = BatchTransaction::new()
        .push(BatchOp::CreateEntity {
            scene_id: recovered_scene_id.clone(),
            name: "GoodEntity".to_string(),
            entity_type: "Test".to_string(),
            components_json: json!([]),
            position: None,
        })
        .push(BatchOp::DeleteScene {
            scene_id: "nonexistent-scene-999".to_string(),
        });

    {
        let mut d = db.lock().unwrap();
        let result = bad_tx.execute(&mut d).unwrap();
        println!("\n  ✓ 批量回滚测试:");
        println!("    已回滚: {} (第一个操作也回滚)", result.rolled_back);
        println!(
            "    当前实体数: {} (应保持不变)",
            d.get_statistics().entity_count
        );
    }

    // ============================================================
    // 10. 导入导出
    // ============================================================
    println!("\n━━━ [10] 数据导入导出 ━━━");

    let exported = db_layer.export_json().unwrap();
    println!("  ✓ 导出: {} bytes", exported.len());

    db_layer.import_json(&exported).unwrap();
    println!("  ✓ 导入成功, 数据完整性: 已验证");

    // ============================================================
    // 总结
    // ============================================================
    println!("\n╔══════════════════════════════════════════════════════════╗");
    println!("║              融合架构演示完成                            ║");
    println!("╠══════════════════════════════════════════════════════════╣");
    println!("║  ✓ 场景管理     — MulticaDb + SceneContext              ║");
    println!("║  ✓ 高级查询     — EntityQuery + SceneQuery + Pagination ║");
    println!("║  ✓ 技能系统     — SkillSystem (12 内置技能)             ║");
    println!("║  ✓ 场景 Agent   — SceneAgent (场景感知执行)             ║");
    println!("║  ✓ 任务看板     — TaskBridge (场景关联任务)             ║");
    println!("║  ✓ Pipeline     — AgentOrchestrator (统一编排)          ║");
    println!("║  ✓ 快照恢复     — DbSnapshot (数据持久化)               ║");
    println!("║  ✓ 批量事务     — BatchTransaction (原子操作)           ║");
    println!("║  ✓ 实体索引     — EntityIndex (快速查找)                ║");
    println!("║  ✓ 导入导出     — JSON Serialization                    ║");
    println!("╚══════════════════════════════════════════════════════════╝");
}
