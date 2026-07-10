/// 测试辅助函数：创建一个包含指定实体的 VisualObservation
fn make_test_observation(name: &str, color: [f32; 4]) -> VisualObservation {
    VisualObservation {
        visible_entities: vec![VisualEntity {
            name: name.into(),
            detected_type: "entity".into(),
            position: Some([0.0, 0.0, 0.0]),
            color: Some(color),
            bounding_box: None,
            confidence: 0.95,
        }],
        anomalies: Vec::new(),
        confidence: 0.95,
        raw_response: None,
    }
}

// ============================================================================
// 测试
// ============================================================================

use super::*;
use crate::types::EntityId;
use std::collections::HashMap;

#[test]
fn test_world_snapshot_describe() {
    let mut snapshot = WorldSnapshot::new();
    snapshot.entities.push(EntityDetail {
        id: EntityId(1),
        name: "Player".into(),
        components: vec![ComponentSummary {
            type_name: "Transform".into(),
            properties: HashMap::new(),
        }],
        children: Vec::new(),
        parent: None,
    });
    snapshot.metrics.total_entities = 1;
    snapshot.metrics.component_types = vec!["Transform".into()];

    let desc = snapshot.describe_for_llm(DetailLevel::Normal);
    assert!(desc.contains("Player"));
    assert!(desc.contains("1"));
}

#[test]
fn test_agent_world_view() {
    let mut view = AgentWorldView::new();

    let snapshot = WorldSnapshot::new();
    view.update_snapshot(snapshot);

    assert!(!view.summary.is_empty());
}

#[test]
fn test_vgcr_controller() {
    let mut controller = VgcrController::with_goal(
        "创建一个红色敌人",
        vec![
            VisualExpectation::EntityVisible("Enemy".into()),
            VisualExpectation::EntityColor("Enemy".into(), [1.0, 0.0, 0.0, 1.0]),
        ],
    );

    // 模拟视觉观察
    let observation = VisualObservation {
        visible_entities: vec![VisualEntity {
            name: "Enemy".into(),
            detected_type: "enemy".into(),
            position: Some([100.0, 0.0, 0.0]),
            color: Some([1.0, 0.0, 0.0, 1.0]),
            bounding_box: None,
            confidence: 0.9,
        }],
        anomalies: Vec::new(),
        confidence: 0.9,
        raw_response: None,
    };

    controller.vision(observation);

    let goal_check = controller.check_goal();
    assert!(goal_check.passed);
}

#[test]
fn test_colors_match() {
    assert!(colors_match([1.0, 0.0, 0.0, 1.0], [1.0, 0.0, 0.0, 1.0]));
    assert!(colors_match([1.0, 0.0, 0.0, 1.0], [0.95, 0.0, 0.0, 1.0])); // 允许误差
    assert!(!colors_match([1.0, 0.0, 0.0, 1.0], [0.0, 0.0, 1.0, 1.0]));
}

#[test]
fn test_scene_diff() {
    let mut before = AgentWorldView::new();
    before.update_snapshot(WorldSnapshot::new());

    let mut after = AgentWorldView::new();
    let mut snapshot = WorldSnapshot::new();
    snapshot.entities.push(EntityDetail {
        id: EntityId(1),
        name: "NewEntity".into(),
        components: Vec::new(),
        children: Vec::new(),
        parent: None,
    });
    snapshot.metrics.total_entities = 1;
    after.update_snapshot(snapshot);

    let diff = before.diff_since(&after);
    assert_eq!(diff.entities_added.len(), 1);
    assert_eq!(diff.entities_added[0].name, "NewEntity");
}

// ====================================================================
// Sprint 3-D2: VGRC 增强功能测试
// ====================================================================

#[test]
fn test_realize_with_executor() {
    let mut controller = VgcrController::with_goal(
        "test goal",
        vec![VisualExpectation::EntityVisible("Player".into())],
    );

    let call_count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let cc = call_count.clone();
    controller.set_executor(Box::new(ClosureRealizeExecutor::new(move |action| {
        cc.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(format!("done: {}", action))
    })));

    let result = controller.realize("create Player");
    assert!(result.is_ok());
    assert_eq!(call_count.load(std::sync::atomic::Ordering::SeqCst), 1);
    assert_eq!(controller.state.realize_attempts, 1);
}

#[test]
fn test_realize_without_executor_dry_run() {
    let mut controller = VgcrController::with_goal(
        "test goal",
        vec![VisualExpectation::EntityVisible("X".into())],
    );

    let result = controller.realize("do something");
    assert!(result.is_ok());
    assert!(result.unwrap().contains("dry-run"));
}

#[test]
fn test_realize_executor_failure() {
    let mut controller = VgcrController::with_goal("test", vec![VisualExpectation::NoAnomalies]);
    controller.set_executor(Box::new(ClosureRealizeExecutor::new(|_| {
        Err("tool error".into())
    })));

    let result = controller.realize("fail action");
    assert!(result.is_err());
    assert_eq!(result.err().unwrap(), "tool error");
}

#[test]
fn test_run_full_cycle_success_first_try() {
    let mut controller = VgcrController::with_goal(
        "创建红色玩家",
        vec![
            VisualExpectation::EntityVisible("Player".into()),
            VisualExpectation::EntityColor("Player".into(), [1.0, 0.0, 0.0, 1.0]),
        ],
    )
    .with_max_attempts(3);

    let initial_obs = make_test_observation("Player", [0.5, 0.5, 0.5, 1.0]);

    let mut attempt = 0;
    let result = controller.run_full_cycle(initial_obs, "move player to red", || {
        attempt += 1;
        make_test_observation("Player", [1.0, 0.0, 0.0, 1.0])
    });

    assert!(result.success);
    assert!(result.message.contains("VGRC"));
    assert!(result.final_observation.is_some());
    assert_eq!(result.attempts, 1);
}

#[test]
fn test_run_full_cycle_goal_already_met() {
    let mut controller = VgcrController::with_goal(
        "目标已满足",
        vec![VisualExpectation::EntityVisible("Existing".into())],
    )
    .with_max_attempts(2);

    let obs = make_test_observation("Existing", [1.0, 1.0, 1.0, 1.0]);
    let result = controller.run_full_cycle(obs, "no-op", || unreachable!("should not be called"));

    assert!(result.success);
    assert_eq!(result.attempts, 0);
    assert!(result.message.contains("操作前"));
}

#[test]
fn test_run_full_cycle_max_attempts_exceeded() {
    let mut controller = VgcrController::with_goal(
        "需要重试",
        vec![
            VisualExpectation::EntityVisible("Target".into()),
            VisualExpectation::EntityColor("Target".into(), [0.0, 1.0, 0.0, 1.0]),
        ],
    )
    .with_max_attempts(2);

    let initial_obs = make_test_observation("Target", [1.0, 0.0, 0.0, 1.0]);

    let mut call_count = 0;
    let result = controller.run_full_cycle(initial_obs, "change color to green", || {
        call_count += 1;
        make_test_observation("Target", [1.0, 0.0, 0.0, 1.0])
    });

    assert!(!result.success);
    assert!(result.message.contains("最大重试"));
    assert_eq!(result.attempts, 2);
}

#[test]
fn test_vgrc_with_max_attempts_builder() {
    let controller = VgcrController::with_goal("test", vec![]).with_max_attempts(5);
    assert_eq!(controller.max_attempts, 5);
}

#[test]
fn test_set_executor_replaces() {
    let mut controller = VgcrController::with_goal("test", vec![]);

    controller.set_executor(Box::new(ClosureRealizeExecutor::new(
        |_| Ok("first".into()),
    )));
    assert_eq!(controller.realize("a").unwrap(), "first");

    controller.set_executor(Box::new(ClosureRealizeExecutor::new(|_| {
        Ok("second".into())
    })));
    assert_eq!(controller.realize("b").unwrap(), "second");
}
