//! Sprint 1-A2: DynamicPlanner

use crate::director::types::DirectorRuntime;

/// Test 17: DynamicPlanner initialization
#[test]
fn test_dynamic_planner_initialization() {
    use crate::DynamicPlanner;

    let planner = DynamicPlanner::new();

    assert!(
        planner.pattern_count() >= 6,
        "Should have at least 6 default patterns"
    );
    assert_eq!(planner.total_revisions(), 0);
}

/// Test 18: detect entity already exists
#[test]
fn test_dynamic_planner_detects_entity_already_exists() {
    use crate::DynamicPlanner;

    let mut planner = DynamicPlanner::new();
    let observation = "Entity 'Player' already exists in scene";

    let revision = planner.analyze_observation(observation, 0, "test_plan");

    assert!(revision.is_some(), "Should detect 'already exists' pattern");
    match revision.unwrap() {
        crate::dynamic_planner::RevisionType::Skip { reason, .. } => {
            assert!(
                reason.contains("Player"),
                "Reason should mention entity name"
            );
            assert!(reason.contains("already exists"));
        }
        other => unreachable!("Expected Skip revision, got {:?}", other),
    }
}

/// Test 19: detect entity not found
#[test]
fn test_dynamic_planner_detects_entity_not_found() {
    use crate::DynamicPlanner;

    let mut planner = DynamicPlanner::new();
    let observation = "Entity 'Enemy' not found in scene";

    let revision = planner.analyze_observation(observation, 0, "test_plan");

    assert!(revision.is_some(), "Should detect 'not found' pattern");
    match revision.unwrap() {
        crate::dynamic_planner::RevisionType::InsertBefore { step, .. } => {
            assert!(
                step.title.contains("Create"),
                "Should suggest creating entity"
            );
            assert!(step.title.contains("Enemy"));
        }
        crate::dynamic_planner::RevisionType::Adapt { adaptation, .. } => {
            assert!(adaptation.contains("not found"));
        }
        other => unreachable!("Expected InsertBefore or Adapt, got {:?}", other),
    }
}

/// Test 20: detect permission denied
#[test]
fn test_dynamic_planner_detects_permission_denied() {
    use crate::permission::OperationRisk;
    use crate::DynamicPlanner;

    let mut planner = DynamicPlanner::new();
    let observation = "Permission denied: operation requires admin privileges";

    let revision = planner.analyze_observation(observation, 2, "test_plan");

    assert!(
        revision.is_some(),
        "Should detect 'permission denied' pattern"
    );
    match revision.unwrap() {
        crate::dynamic_planner::RevisionType::Adapt {
            from_risk, to_risk, ..
        } => {
            assert_eq!(from_risk, OperationRisk::HighRisk);
            assert_eq!(to_risk, OperationRisk::LowRisk);
        }
        other => unreachable!("Expected Adapt revision, got {:?}", other),
    }
}

/// Test 21: apply Skip revision
#[test]
fn test_dynamic_planner_apply_skip_revision() {
    use crate::plan::{EditPlan, ExecutionMode};
    use crate::DynamicPlanner;

    let mut planner = DynamicPlanner::new();
    let mut plan = EditPlan::new("skip_test", 1, "Test Skip", "", ExecutionMode::Plan);
    plan.steps.push(crate::plan::EditPlanStep {
        id: "step1".into(),
        title: "Create Player".into(),
        target_module: crate::plan::TargetModule::Scene,
        action_description: "".into(),
        risk: crate::permission::OperationRisk::LowRisk,
        validation_requirements: vec![],
    });
    plan.steps.push(crate::plan::EditPlanStep {
        id: "step2".into(),
        title: "Create Enemy".into(),
        target_module: crate::plan::TargetModule::Scene,
        action_description: "".into(),
        risk: crate::permission::OperationRisk::LowRisk,
        validation_requirements: vec![],
    });

    let revision = crate::dynamic_planner::RevisionType::Skip {
        count: 1,
        reason: "Already exists".into(),
    };
    planner
        .apply_revision(&mut plan, revision, 0, "Test", true)
        .unwrap();

    assert!(plan.steps[0].title.starts_with("[SKIPPED]"));
    assert!(!plan.steps[1].title.starts_with("[SKIPPED]"));
    assert_eq!(planner.total_revisions(), 1);
}

/// Test 22: apply InsertBefore revision
#[test]
fn test_dynamic_planner_apply_insert_before_revision() {
    use crate::plan::{EditPlan, ExecutionMode};
    use crate::DynamicPlanner;

    let mut planner = DynamicPlanner::new();
    let mut plan = EditPlan::new("insert_test", 1, "Test Insert", "", ExecutionMode::Plan);
    plan.steps.push(crate::plan::EditPlanStep {
        id: "step1".into(),
        title: "Update Player".into(),
        target_module: crate::plan::TargetModule::Scene,
        action_description: "".into(),
        risk: crate::permission::OperationRisk::MediumRisk,
        validation_requirements: vec![],
    });

    let prereq = crate::plan::EditPlanStep {
        id: "prereq".into(),
        title: "Create Player".into(),
        target_module: crate::plan::TargetModule::Scene,
        action_description: "Prerequisite".into(),
        risk: crate::permission::OperationRisk::LowRisk,
        validation_requirements: vec![],
    };
    let revision = crate::dynamic_planner::RevisionType::InsertBefore {
        index: 0,
        step: prereq,
        reason: "Not found".into(),
    };
    planner
        .apply_revision(&mut plan, revision, 0, "Obs", true)
        .unwrap();

    assert_eq!(plan.steps.len(), 2);
    assert_eq!(plan.steps[0].title, "Create Player");
    assert_eq!(planner.total_revisions(), 1);
}

/// Test 23: max revisions limit
#[test]
fn test_dynamic_planner_max_revisions_limit() {
    use crate::plan::{EditPlan, ExecutionMode};
    use crate::DynamicPlanner;

    let mut planner = DynamicPlanner::new();
    planner.set_max_revisions_per_plan(2);

    let mut plan = EditPlan::new("limited_plan", 1, "Limited", "", ExecutionMode::Plan);
    plan.steps.push(crate::plan::EditPlanStep {
        id: "s1".into(),
        title: "Step".into(),
        target_module: crate::plan::TargetModule::Scene,
        action_description: "".into(),
        risk: crate::permission::OperationRisk::LowRisk,
        validation_requirements: vec![],
    });

    for i in 0..2 {
        let result = planner.analyze_observation("Entity already exists", 0, "limited_plan");
        assert!(result.is_some(), "Revision {} should be allowed", i);
        if let Some(revision) = result {
            let _ = planner.apply_revision(&mut plan, revision, 0, "Test", true);
        }
    }

    let result = planner.analyze_observation("Entity already exists", 0, "limited_plan");
    assert!(
        result.is_none(),
        "Third revision should be blocked by limit (already applied 2)"
    );
}

/// Test 24: revision history
#[test]
fn test_dynamic_planner_revision_history() {
    use crate::plan::{EditPlan, ExecutionMode};
    use crate::DynamicPlanner;

    let mut planner = DynamicPlanner::new();
    let mut plan = EditPlan::new("history_test", 1, "History Test", "", ExecutionMode::Plan);
    plan.steps.push(crate::plan::EditPlanStep {
        id: "s1".into(),
        title: "Step".into(),
        target_module: crate::plan::TargetModule::Scene,
        action_description: "".into(),
        risk: crate::permission::OperationRisk::LowRisk,
        validation_requirements: vec![],
    });

    let revision = crate::dynamic_planner::RevisionType::Skip {
        count: 1,
        reason: "Test".into(),
    };
    planner
        .apply_revision(&mut plan, revision, 0, "Obs", true)
        .unwrap();

    let history = planner.get_plan_revisions("history_test");
    assert_eq!(history.len(), 1);
    assert!(history[0].auto_applied);
    assert_eq!(history[0].step_index, 0);
    println!("Revision entry: {}", history[0].summary());
}

/// Test 25: integration - DirectorRuntime has DynamicPlanner
#[test]
fn test_director_runtime_has_dynamic_planner() {
    let mut rt = DirectorRuntime::new();

    assert!(rt.dynamic_planner.pattern_count() >= 6);

    let revision = rt.dynamic_planner.analyze_observation(
        "Entity 'Boss' already exists",
        0,
        "integration_test",
    );

    assert!(
        revision.is_some(),
        "DirectorRuntime's DynamicPlanner should work"
    );
}
