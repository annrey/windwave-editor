use super::*;
use crate::permission::OperationRisk;
use crate::plan::{EditPlanStatus, ExecutionMode};
use crate::planner::{ComplexityLevel, PlannerContext};

/// 辅助函数：创建测试用 PlannerContext
fn test_context() -> PlannerContext {
    PlannerContext {
        task_id: 1,
        available_tools: vec![
            "create_entity".into(),
            "update_component".into(),
            "delete_entity".into(),
            "query_entities".into(),
        ],
        scene_entity_names: vec!["Player".into(), "Camera".into(), "Ground".into()],
        memory_context: None,
    }
}

#[test]
fn test_new_planner() {
    let planner = RuleBasedPlanner::new();
    let _ = planner; // 验证构造不 panic
}

#[test]
fn test_default_planner() {
    let planner = RuleBasedPlanner::default();
    let _ = planner;
}

#[test]
fn test_create_plan_simple_create() {
    let planner = RuleBasedPlanner::new();
    let plan = planner.create_plan("创建一个敌人", 1, test_context());

    assert_eq!(plan.task_id, 1);
    assert_eq!(plan.title, "创建一个敌人");
    assert!(!plan.steps.is_empty());
    assert_eq!(plan.status, EditPlanStatus::Draft);
}

#[test]
fn test_estimate_complexity_simple() {
    let planner = RuleBasedPlanner::new();
    let complexity = planner.estimate_complexity("创建一个敌人");
    assert_eq!(complexity, ComplexityLevel::Simple);
}

#[test]
fn test_estimate_complexity_medium_color() {
    let planner = RuleBasedPlanner::new();
    // 场景 + 视觉 = 2 个领域
    let complexity = planner.estimate_complexity("创建一个红色敌人");
    assert_eq!(complexity, ComplexityLevel::Medium);
}

#[test]
fn test_estimate_complexity_complex() {
    let planner = RuleBasedPlanner::new();
    // 场景 + 视觉 + 代码 = 3 个领域
    let complexity = planner.estimate_complexity("创建一个红色敌人并为其添加AI代码脚本");
    assert_eq!(complexity, ComplexityLevel::Complex);
}

#[test]
fn test_estimate_complexity_code_only() {
    let planner = RuleBasedPlanner::new();
    let complexity = planner.estimate_complexity("生成一个移动脚本");
    assert_eq!(complexity, ComplexityLevel::Simple);
}

#[test]
fn test_estimate_risk_low() {
    let planner = RuleBasedPlanner::new();
    let risk = planner.estimate_risk("创建一个敌人");
    assert_eq!(risk, OperationRisk::LowRisk);
}

#[test]
fn test_estimate_risk_high() {
    let planner = RuleBasedPlanner::new();
    let risk = planner.estimate_risk("删除这个敌人");
    assert_eq!(risk, OperationRisk::HighRisk);
}

#[test]
fn test_estimate_risk_destructive() {
    let planner = RuleBasedPlanner::new();
    let risk = planner.estimate_risk("清空所有实体");
    assert_eq!(risk, OperationRisk::Destructive);
}

#[test]
fn test_estimate_risk_medium_batch() {
    let planner = RuleBasedPlanner::new();
    let risk = planner.estimate_risk("批量修改所有实体颜色");
    assert_eq!(risk, OperationRisk::MediumRisk);
}

#[test]
fn test_choose_execution_mode_direct_simple() {
    let planner = RuleBasedPlanner::new();
    let mode = planner.choose_execution_mode(&ComplexityLevel::Simple, &OperationRisk::LowRisk);
    assert_eq!(mode, ExecutionMode::Direct);
}

#[test]
fn test_choose_execution_mode_plan_high_risk() {
    let planner = RuleBasedPlanner::new();
    let mode = planner.choose_execution_mode(&ComplexityLevel::Simple, &OperationRisk::HighRisk);
    assert_eq!(mode, ExecutionMode::Plan);
}

#[test]
fn test_choose_execution_mode_team_complex() {
    let planner = RuleBasedPlanner::new();
    let mode = planner.choose_execution_mode(&ComplexityLevel::Complex, &OperationRisk::LowRisk);
    assert_eq!(mode, ExecutionMode::Team);
}

#[test]
fn test_choose_execution_mode_destructive() {
    let planner = RuleBasedPlanner::new();
    let mode = planner.choose_execution_mode(&ComplexityLevel::Simple, &OperationRisk::Destructive);
    assert_eq!(mode, ExecutionMode::Plan);
}

#[test]
fn test_extract_title_short() {
    let title = RuleBasedPlanner::extract_title("创建一个红色敌人");
    assert_eq!(title, "创建一个红色敌人");
}

#[test]
fn test_extract_title_long() {
    let long_text = "这是一个非常长的请求文本，用于测试标题提取功能是否能够正确地截断过长的标题内容，这里需要写很多文字才能超过六十个字符的限制，继续补充更多文字内容以达到测试目的";
    let title = RuleBasedPlanner::extract_title(long_text);
    assert!(title.chars().count() <= 60);
    assert!(title.ends_with("..."));
}

#[test]
fn test_extract_entity_name_chinese() {
    let name = RuleBasedPlanner::extract_entity_name("创建一个红色敌人");
    // 应该提取 "敌人"
    assert!(name.contains("敌人") || name.contains("entity"));
}

#[test]
fn test_extract_entity_name_english() {
    let name = RuleBasedPlanner::extract_entity_name("create a red enemy");
    assert!(name.contains("enemy") || name.contains("entity"));
}

#[test]
fn test_extract_color_red() {
    let (name, rgba) = RuleBasedPlanner::extract_color("创建一个红色敌人").unwrap();
    assert_eq!(name, "红色");
    assert_eq!(rgba, [1.0, 0.0, 0.0, 1.0]);
}

#[test]
fn test_extract_color_blue() {
    let (name, rgba) = RuleBasedPlanner::extract_color("蓝色方块").unwrap();
    assert_eq!(name, "蓝色");
    assert_eq!(rgba, [0.0, 0.0, 1.0, 1.0]);
}

#[test]
fn test_extract_color_english() {
    let (name, rgba) = RuleBasedPlanner::extract_color("create a green enemy").unwrap();
    assert_eq!(name, "绿色");
    assert_eq!(rgba, [0.0, 1.0, 0.0, 1.0]);
}

#[test]
fn test_extract_color_none() {
    let result = RuleBasedPlanner::extract_color("创建一个敌人");
    assert!(result.is_none());
}

#[test]
fn test_extract_position_with_reference() {
    let ctx = test_context();
    let result = RuleBasedPlanner::extract_position("放在玩家右侧", &ctx);
    assert!(result.is_some());
    let (desc, _) = result.unwrap();
    assert!(desc.contains("Player") || desc.contains("玩家"));
}

#[test]
fn test_extract_position_none() {
    let ctx = test_context();
    let result = RuleBasedPlanner::extract_position("创建一个敌人", &ctx);
    assert!(result.is_none());
}

#[test]
fn test_build_steps_create_with_color() {
    let planner = RuleBasedPlanner::new();
    let ctx = test_context();
    let steps = planner.build_steps("创建一个红色敌人", &ctx, &ExecutionMode::Direct);
    assert!(steps.len() >= 2);
    // 第一个步骤应该是创建实体
    assert!(steps[0].action_description.contains("create_entity"));
    // 第二个步骤应该是设置颜色
    assert!(steps[1].action_description.contains("Sprite"));
}

#[test]
fn test_build_steps_delete() {
    let planner = RuleBasedPlanner::new();
    let ctx = test_context();
    let steps = planner.build_steps("删除敌人", &ctx, &ExecutionMode::Plan);
    assert!(!steps.is_empty());
    assert!(steps[0].action_description.contains("delete_entity"));
}

#[test]
fn test_build_steps_generic() {
    let planner = RuleBasedPlanner::new();
    let ctx = test_context();
    let steps = planner.build_steps("分析当前场景的性能", &ctx, &ExecutionMode::Plan);
    // 无法识别具体意图时应该生成一个通用步骤
    assert!(!steps.is_empty());
}

#[test]
fn test_build_steps_team_mode() {
    let planner = RuleBasedPlanner::new();
    let ctx = test_context();
    let steps = planner.build_steps("创建红色敌人并添加AI脚本", &ctx, &ExecutionMode::Team);
    // Team 模式应该附加协调步骤
    assert!(steps.iter().any(|s| s.title.contains("协调")));
}

#[test]
fn test_plan_id_format() {
    let planner = RuleBasedPlanner::new();
    let plan = planner.create_plan("测试", 42, test_context());
    assert_eq!(plan.id, "plan_42");
}

#[test]
fn test_build_steps_batch_operation() {
    let planner = RuleBasedPlanner::new();
    let ctx = test_context();
    let steps = planner.build_steps("批量修改颜色", &ctx, &ExecutionMode::Plan);
    assert!(!steps.is_empty());
    assert!(steps.iter().any(|s| s.risk == OperationRisk::MediumRisk));
}

#[test]
fn test_build_system_identity_prompt() {
    let planner = RuleBasedPlanner::new();
    let prompt = planner.build_system_identity("bevy", "MyGame");
    assert!(prompt.contains("bevy"));
    assert!(prompt.contains("MyGame"));
    assert!(prompt.len() > 100);
}

#[test]
fn test_with_prompt_system_custom() {
    let mut ps = PromptSystem::with_defaults();
    ps.register_user(
        "custom",
        crate::prompt::PromptTemplate {
            name: "test".into(),
            template: "Hello {agent_name}".into(),
        },
    );
    let planner = RuleBasedPlanner::with_prompt_system(ps);
    let prompt = planner.build_system_identity("bevy", "Test");
    assert!(prompt.contains("bevy"));
}
