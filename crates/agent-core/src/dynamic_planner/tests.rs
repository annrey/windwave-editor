use super::*;
use crate::permission::OperationRisk;
use crate::plan::{EditPlan, EditPlanStep, ExecutionMode, TargetModule};

#[test]
fn test_dynamic_planner_creation() {
    let planner = DynamicPlanner::new();
    assert!(
        planner.patterns.len() >= 6,
        "Should have at least 6 default patterns"
    );
    assert_eq!(planner.total_revisions(), 0);
}

#[test]
fn test_pattern_entity_already_exists() {
    let mut planner = DynamicPlanner::new();
    let revision =
        planner.analyze_observation("Entity 'Player' already exists in scene", 0, "plan_1");

    assert!(revision.is_some(), "Should detect 'already exists' pattern");
    match revision.unwrap() {
        RevisionType::Skip { reason, .. } => {
            assert!(
                reason.contains("Player"),
                "Reason should mention entity name"
            );
            assert!(
                reason.contains("already exists"),
                "Reason should explain skip"
            );
        }
        other => unreachable!("Expected Skip revision, got {:?}", other),
    }
}

#[test]
fn test_pattern_entity_not_found() {
    let mut planner = DynamicPlanner::new();
    let revision = planner.analyze_observation("Entity 'Enemy' not found in scene", 0, "plan_2");

    assert!(revision.is_some(), "Should detect 'not found' pattern");
    // Should either insert prerequisite or adapt
    match revision.unwrap() {
        RevisionType::InsertBefore { step, .. } => {
            assert!(
                step.title.contains("Create"),
                "Should suggest creating entity"
            );
            assert!(
                step.title.contains("Enemy"),
                "Should mention missing entity"
            );
        }
        RevisionType::Adapt { adaptation, .. } => {
            assert!(
                adaptation.contains("not found"),
                "Adaptation should mention error"
            );
        }
        other => unreachable!("Expected InsertBefore or Adapt, got {:?}", other),
    }
}

#[test]
fn test_pattern_permission_denied() {
    let mut planner = DynamicPlanner::new();
    let revision = planner.analyze_observation(
        "Permission denied: operation requires admin privileges",
        2,
        "plan_3",
    );

    assert!(
        revision.is_some(),
        "Should detect 'permission denied' pattern"
    );
    match revision.unwrap() {
        RevisionType::Adapt {
            from_risk, to_risk, ..
        } => {
            assert_eq!(from_risk, OperationRisk::HighRisk);
            assert_eq!(to_risk, OperationRisk::LowRisk);
        }
        other => unreachable!("Expected Adapt revision, got {:?}", other),
    }
}

#[test]
fn test_apply_skip_revision() {
    let mut planner = DynamicPlanner::new();
    let mut plan = EditPlan::new("test_plan", 1, "Test", "Test plan", ExecutionMode::Plan);
    plan.steps.push(EditPlanStep {
        id: "step1".into(),
        title: "Create Player".into(),
        target_module: TargetModule::Scene,
        action_description: "Create player entity".into(),
        risk: OperationRisk::LowRisk,
        validation_requirements: vec![],
    });
    plan.steps.push(EditPlanStep {
        id: "step2".into(),
        title: "Create Enemy".into(),
        target_module: TargetModule::Scene,
        action_description: "Create enemy entity".into(),
        risk: OperationRisk::LowRisk,
        validation_requirements: vec![],
    });

    let revision = RevisionType::Skip {
        count: 1,
        reason: "Entity already exists".into(),
    };
    planner
        .apply_revision(&mut plan, revision, 0, "Test obs", true)
        .unwrap();

    assert!(plan.steps[0].title.starts_with("[SKIPPED]"));
    assert!(!plan.steps[1].title.starts_with("[SKIPPED]"));
    assert_eq!(planner.total_revisions(), 1);
}

#[test]
fn test_apply_insert_before_revision() {
    let mut planner = DynamicPlanner::new();
    let mut plan = EditPlan::new("test_plan", 1, "Test", "Test plan", ExecutionMode::Plan);
    plan.steps.push(EditPlanStep {
        id: "step1".into(),
        title: "Update Player".into(),
        target_module: TargetModule::Scene,
        action_description: "Update player".into(),
        risk: OperationRisk::MediumRisk,
        validation_requirements: vec![],
    });

    let prereq = EditPlanStep {
        id: "prereq".into(),
        title: "Create Player".into(),
        target_module: TargetModule::Scene,
        action_description: "Create player first".into(),
        risk: OperationRisk::LowRisk,
        validation_requirements: vec![],
    };
    let revision = RevisionType::InsertBefore {
        index: 0,
        step: prereq,
        reason: "Entity not found".into(),
    };
    planner
        .apply_revision(&mut plan, revision, 0, "Not found", true)
        .unwrap();

    assert_eq!(plan.steps.len(), 2);
    assert_eq!(plan.steps[0].title, "Create Player"); // Prerequisite inserted first
    assert_eq!(planner.total_revisions(), 1);
}

#[test]
fn test_max_revisions_limit() {
    let mut planner = DynamicPlanner::new();
    planner.max_revisions_per_plan = 2; // Set low limit for testing

    let mut plan = EditPlan::new("limited_plan", 1, "Tracked", "", ExecutionMode::Plan);
    plan.steps.push(EditPlanStep {
        id: "s1".into(),
        title: "Step 1".into(),
        target_module: TargetModule::Scene,
        action_description: "".into(),
        risk: OperationRisk::LowRisk,
        validation_requirements: vec![],
    });

    // Apply 2 revisions (should succeed)
    for i in 0..2 {
        let _rev = RevisionType::Skip {
            count: 1,
            reason: format!("Test revision {}", i),
        };
        let result = planner.analyze_observation("Entity already exists", 0, "limited_plan");
        assert!(result.is_some(), "Revision {} should be allowed", i);
        let _ = planner.apply_revision(&mut plan, result.unwrap(), 0, "Obs", true);
    }

    // Third revision should be blocked
    let result = planner.analyze_observation("Entity already exists", 0, "limited_plan");
    assert!(
        result.is_none(),
        "Third revision should be blocked by limit"
    );
}

#[test]
fn test_revision_history_tracking() {
    let mut planner = DynamicPlanner::new();
    let mut plan = EditPlan::new("tracked_plan", 1, "Tracked", "", ExecutionMode::Plan);
    plan.steps.push(EditPlanStep {
        id: "s1".into(),
        title: "Step 1".into(),
        target_module: TargetModule::Scene,
        action_description: "".into(),
        risk: OperationRisk::LowRisk,
        validation_requirements: vec![],
    });

    // Apply revision
    let revision = RevisionType::Skip {
        count: 1,
        reason: "Test".into(),
    };
    planner
        .apply_revision(&mut plan, revision, 0, "Obs", true)
        .unwrap();

    // Check history
    let history = planner.get_plan_revisions("tracked_plan");
    assert_eq!(history.len(), 1);
    assert!(history[0].auto_applied);
    assert_eq!(history[0].step_index, 0);
    println!("Revision entry: {}", history[0].summary());
}

#[test]
fn test_extract_entity_name() {
    assert_eq!(
        helpers::extract_entity_name_from_obs("Entity 'Player' already exists"),
        Some("Player".into())
    );
    assert_eq!(
        helpers::extract_entity_name_from_obs("Entity \"Boss\" not found"),
        Some("Boss".into())
    );
    assert_eq!(
        helpers::extract_entity_name_from_obs("No entity mentioned here"),
        None
    );
}

#[test]
fn test_revision_type_describe() {
    let skip = RevisionType::Skip {
        count: 2,
        reason: "Duplicate".into(),
    };
    assert!(skip.describe().contains("Skip"));
    assert!(skip.describe().contains("2"));

    let abort = RevisionType::Abort {
        reason: "Fatal error".into(),
    };
    assert!(abort.describe().contains("ABORT"));
}

#[test]
fn test_is_safe_auto_apply() {
    assert!(RevisionType::Skip {
        count: 1,
        reason: "".into()
    }
    .is_safe_auto_apply());

    assert!(RevisionType::InsertBefore {
        index: 0,
        step: unimplemented_step(),
        reason: "".into()
    }
    .is_safe_auto_apply());

    assert!(!RevisionType::Abort { reason: "".into() }.is_safe_auto_apply());
}

fn unimplemented_step() -> EditPlanStep {
    EditPlanStep {
        id: "test".into(),
        title: "Test".into(),
        target_module: TargetModule::Scene,
        action_description: "".into(),
        risk: OperationRisk::LowRisk,
        validation_requirements: vec![],
    }
}
