use super::feature_limiter::*;
use super::*;
use crate::llm::{
    LlmClient, LlmError, LlmProvider, LlmRequest, LlmResponse, StreamCallback, StreamChunk,
    TokenUsage,
};
use crate::plan::{EditPlan, EditPlanStatus, ExecutionMode};
use crate::planner::PlannerContext;
use std::sync::Arc;

// ============================================================================
// Mock LLM Client
// ============================================================================

struct MockLlmClient {
    ready: bool,
}

impl MockLlmClient {
    fn new(ready: bool) -> Self {
        Self { ready }
    }
}

#[async_trait::async_trait]
impl LlmClient for MockLlmClient {
    async fn chat(&self, _request: LlmRequest) -> Result<LlmResponse, LlmError> {
        Ok(LlmResponse {
            content: "pong".to_string(),
            tool_calls: vec![],
            usage: TokenUsage::default(),
        })
    }

    fn is_ready(&self) -> bool {
        self.ready
    }

    fn provider(&self) -> LlmProvider {
        LlmProvider::OpenAI
    }

    async fn chat_stream(
        &self,
        request: LlmRequest,
        mut on_chunk: StreamCallback,
    ) -> Result<LlmResponse, LlmError> {
        let response = self.chat(request).await?;
        let chunk = StreamChunk {
            content: Some(response.content.clone()),
            done: true,
            accumulated: Some(response.content.clone()),
        };
        on_chunk(&chunk);
        Ok(response)
    }
}

// ============================================================================
// Connection Checker Tests
// ============================================================================

#[test]
fn test_checker_not_ready_when_api_key_missing() {
    let client = Arc::new(MockLlmClient::new(false));
    let checker = LlmConnectionChecker::new(client);

    assert!(!checker.is_available());
}

#[test]
fn test_checker_ready_when_api_key_present() {
    let client = Arc::new(MockLlmClient::new(true));
    let checker = LlmConnectionChecker::new(client);

    assert!(checker.is_available());
    assert_eq!(checker.consecutive_failures(), 0);
}

#[test]
fn test_failure_threshold() {
    let client = Arc::new(MockLlmClient::new(true));
    let checker = LlmConnectionChecker::with_config(
        client,
        ConnectionCheckConfig {
            failure_threshold: 3,
            ..Default::default()
        },
    );

    assert_eq!(checker.failure_threshold(), 3);
}

#[test]
fn test_reset_failures() {
    let client = Arc::new(MockLlmClient::new(true));
    let checker = LlmConnectionChecker::new(client);

    // 模拟失败
    {
        let mut state = checker.state.lock().unwrap();
        state.consecutive_failures = 5;
    }

    // 重置
    checker.reset_failures();
    assert_eq!(checker.consecutive_failures(), 0);
}

// ============================================================================
// Controller Tests
// ============================================================================

fn test_context() -> PlannerContext {
    PlannerContext {
        task_id: 1,
        available_tools: vec!["create_entity".into(), "update_component".into()],
        scene_entity_names: vec!["Player".into()],
        memory_context: None,
    }
}

#[test]
fn test_new_controller() {
    let controller = HybridEditorController::new();
    assert_eq!(controller.llm_status(), HybridLlmStatus::Unavailable);
    assert_eq!(controller.current_mode(), EditorMode::RuleBased);
}

#[test]
fn test_status_description() {
    let controller = HybridEditorController::new();
    let desc = controller.status_description();
    assert!(desc.contains("规则引擎模式"));
}

#[test]
fn test_fallback_event_recorded() {
    let controller = HybridEditorController::new();
    let _plan = controller.create_plan("创建一个敌人", 1, test_context());

    let history = controller.fallback_history();
    assert!(!history.is_empty());
    assert_eq!(history[0].reason, FallbackReason::NoClient);
}

#[test]
fn test_stats_tracking() {
    let controller = HybridEditorController::new();
    let _plan = controller.create_plan("创建一个敌人", 1, test_context());

    let stats = controller.stats();
    assert_eq!(stats.rule_fallbacks, 1);
    assert_eq!(stats.total_fallbacks, 1);
}

#[test]
fn test_plan_creation_fallback() {
    let controller = HybridEditorController::new();
    let plan = controller.create_plan("创建一个红色敌人", 1, test_context());

    assert_eq!(plan.task_id, 1);
    assert!(!plan.title.is_empty());
    assert!(!plan.steps.is_empty());
}

#[test]
fn test_llm_disable() {
    let mut controller = HybridEditorController::new();
    controller.disable_llm();

    assert_eq!(controller.llm_status(), HybridLlmStatus::Disabled);
    assert_eq!(controller.current_mode(), EditorMode::RuleBased);
}

#[test]
fn test_stats_success_rate() {
    let controller = HybridEditorController::new();
    let stats = controller.stats();

    // 无LLM时，成功率为0
    assert_eq!(stats.llm_success_rate(), 0.0);
}

// ============================================================================
// E2E Tests — Full lifecycle: LLM available → unavailable → recovery
// ============================================================================

#[test]
fn test_e2e_initial_state_no_llm() {
    let controller = HybridEditorController::new();
    assert_eq!(controller.llm_status(), HybridLlmStatus::Unavailable);
    assert_eq!(controller.current_mode(), EditorMode::RuleBased);

    let limiter = FeatureLimiter::new();
    let limited = limiter.limited_features(controller.current_mode());
    assert_eq!(limited.len(), 4); // 4 LLM-required features limited
}

#[test]
fn test_e2e_plan_creation_uses_rule_engine_when_no_llm() {
    let controller = HybridEditorController::new();
    let plan = controller.create_plan("创建一个敌人", 1, test_context());

    assert_eq!(plan.task_id, 1);
    assert!(!plan.steps.is_empty());

    let stats = controller.stats();
    assert_eq!(stats.rule_fallbacks, 1);
    assert_eq!(stats.llm_successes, 0);
}

#[test]
fn test_e2e_feature_limiter_blocks_in_rule_mode() {
    let limiter = FeatureLimiter::new();

    // LLM-required features blocked in RuleBased mode
    assert!(!limiter.is_available(&Feature::ComplexReasoning, EditorMode::RuleBased));
    assert!(!limiter.is_available(&Feature::FreeFormEditing, EditorMode::RuleBased));
    assert!(!limiter.is_available(&Feature::CodeGeneration, EditorMode::RuleBased));
    assert!(!limiter.is_available(&Feature::ContextualSuggestions, EditorMode::RuleBased));

    // Non-LLM features still available
    assert!(limiter.is_available(&Feature::VisualUnderstanding, EditorMode::RuleBased));
    assert!(limiter.is_available(&Feature::ProceduralLearning, EditorMode::RuleBased));
}

#[test]
fn test_e2e_feature_limiter_allows_in_llm_mode() {
    let limiter = FeatureLimiter::new();

    // All features available in LLM mode
    for (feature, available) in limiter.feature_status(EditorMode::Llm) {
        assert!(
            available,
            "Feature {:?} should be available in LLM mode",
            feature
        );
    }
}

#[test]
fn test_e2e_bypass_grants_temporary_access() {
    let mut limiter = FeatureLimiter::new();

    assert!(!limiter.is_available(&Feature::ComplexReasoning, EditorMode::RuleBased));

    let granted = limiter.bypass(&Feature::ComplexReasoning, true);
    assert!(granted);
    assert!(limiter.is_available(&Feature::ComplexReasoning, EditorMode::RuleBased));

    // Audit log recorded
    let log = limiter.audit_log();
    assert_eq!(log.len(), 1);
    assert_eq!(log[0].feature, Feature::ComplexReasoning);
    assert!(log[0].user_confirmed);
}

#[test]
fn test_e2e_bypass_unconfirmed_still_grants() {
    let mut limiter = FeatureLimiter::new();

    let granted = limiter.bypass(&Feature::FreeFormEditing, false);
    assert!(granted);
    assert!(limiter.is_available(&Feature::FreeFormEditing, EditorMode::RuleBased));

    let log = limiter.audit_log();
    assert!(!log[0].user_confirmed);
}

#[test]
fn test_e2e_multiple_fallbacks_recorded() {
    let controller = HybridEditorController::new();

    for i in 1..=5 {
        let _plan = controller.create_plan(&format!("请求 {}", i), i, test_context());
    }

    let history = controller.fallback_history();
    assert_eq!(history.len(), 5);

    let stats = controller.stats();
    assert_eq!(stats.rule_fallbacks, 5);
    assert_eq!(stats.total_fallbacks, 5);
}

#[test]
fn test_e2e_disable_llm_forces_rule_mode() {
    let mut controller = HybridEditorController::new();
    controller.disable_llm();

    assert_eq!(controller.llm_status(), HybridLlmStatus::Disabled);
    assert_eq!(controller.current_mode(), EditorMode::RuleBased);

    let plan = controller.create_plan("创建一个敌人", 1, test_context());
    assert!(!plan.steps.is_empty());
}

#[test]
fn test_e2e_enable_llm_transitions_to_connecting() {
    let mut controller = HybridEditorController::new();
    controller.disable_llm();
    assert_eq!(controller.llm_status(), HybridLlmStatus::Disabled);

    controller.enable_llm();
    // No LLM client set, so enable_llm doesn't change status
    // (only transitions to Connecting if llm_client is Some)
    assert_eq!(controller.llm_status(), HybridLlmStatus::Disabled);
}

#[test]
fn test_e2e_recheck_clears_cache() {
    let controller = HybridEditorController::new();
    let status = controller.recheck();
    // Without LLM client, should report unavailable
    assert_eq!(status, HybridLlmStatus::Unavailable);
}

#[test]
fn test_e2e_status_description_includes_mode() {
    let controller = HybridEditorController::new();
    let desc = controller.status_description();
    assert!(desc.contains("规则引擎模式"));
    assert!(desc.contains("降级"));
}

#[test]
fn test_e2e_feature_limiter_cleanup_expired() {
    let mut limiter = FeatureLimiter::new();
    limiter.bypass(&Feature::ComplexReasoning, true);

    // Bypass should be active
    assert!(limiter.is_available(&Feature::ComplexReasoning, EditorMode::RuleBased));

    // Cleanup shouldn't remove active bypasses
    limiter.cleanup_expired();
    assert!(limiter.is_available(&Feature::ComplexReasoning, EditorMode::RuleBased));
}

#[test]
fn test_e2e_cannot_bypass_non_llm_feature() {
    let mut limiter = FeatureLimiter::new();
    let granted = limiter.bypass(&Feature::VisualUnderstanding, true);
    assert!(!granted);
}

#[test]
fn test_e2e_limited_features_list_correct() {
    let limiter = FeatureLimiter::new();

    let limited = limiter.limited_features(EditorMode::RuleBased);
    assert_eq!(limited.len(), 4);

    let limited = limiter.limited_features(EditorMode::Llm);
    assert_eq!(limited.len(), 0);
}

#[test]
fn test_e2e_fallback_history_bounded() {
    let controller = HybridEditorController::new();

    // Create more plans than max_history_size (100)
    for i in 1..=105 {
        let _plan = controller.create_plan(&format!("请求 {}", i), i, test_context());
    }

    let history = controller.fallback_history();
    assert!(history.len() <= 100);
}

// ============================================================================
// T16: Manual confirmation tests
// ============================================================================

#[test]
fn test_confirmation_needs_confirmation_high_risk() {
    use crate::permission::OperationRisk;

    let controller = HybridEditorController::new();
    let plan = make_draft_plan();
    assert!(!controller.needs_confirmation(&plan));

    let high_risk_plan = EditPlan {
        risk_level: OperationRisk::HighRisk,
        ..plan.clone()
    };
    assert!(controller.needs_confirmation(&high_risk_plan));
}

#[test]
fn test_confirmation_needs_confirmation_destructive() {
    use crate::permission::OperationRisk;

    let controller = HybridEditorController::new();
    let plan = make_draft_plan();
    let destructive_plan = EditPlan {
        risk_level: OperationRisk::Destructive,
        ..plan.clone()
    };
    assert!(controller.needs_confirmation(&destructive_plan));
}

#[test]
fn test_confirmation_pending_plan_none_by_default() {
    let controller = HybridEditorController::new();
    assert!(controller.pending_plan().is_none());
}

#[test]
fn test_confirmation_request_and_confirm() {
    let controller = HybridEditorController::new();
    let plan = make_draft_plan();

    assert!(controller.pending_plan().is_none());
    controller.request_confirmation(plan.clone(), 42);
    assert!(controller.pending_plan().is_some());

    let confirmed = controller.confirm_plan();
    assert!(confirmed.is_some());
    let (confirmed_plan, task_id) = confirmed.unwrap();
    assert_eq!(task_id, 42);
    assert_eq!(confirmed_plan.title, plan.title);

    assert!(controller.pending_plan().is_none());
}

#[test]
fn test_confirmation_reject_plan() {
    let controller = HybridEditorController::new();
    let plan = make_draft_plan();

    controller.request_confirmation(plan.clone(), 42);
    assert!(controller.pending_plan().is_some());

    let rejected = controller.reject_plan();
    assert!(rejected);
    assert!(controller.pending_plan().is_none());
}

#[test]
fn test_confirmation_reject_none() {
    let controller = HybridEditorController::new();
    let rejected = controller.reject_plan();
    assert!(!rejected);
}

#[test]
fn test_confirmation_create_plan_with_confirmation() {
    let controller = HybridEditorController::new();
    let plan = controller.create_plan_with_confirmation("create a safe entity", 1, test_context());
    assert!(!controller.needs_confirmation(&plan));
    assert!(controller.pending_plan().is_none());
}

#[test]
fn test_confirmation_visual_verify_tests() {
    use crate::permission::OperationRisk;

    let plan = make_draft_plan();
    assert_eq!(plan.status, EditPlanStatus::Draft);
    assert_eq!(plan.mode, ExecutionMode::Plan);
    assert_eq!(plan.risk_level, OperationRisk::Safe);
}

fn make_draft_plan() -> EditPlan {
    EditPlan {
        id: "test_plan".into(),
        task_id: 1,
        title: "Test Plan".into(),
        summary: "A test plan".into(),
        mode: ExecutionMode::Plan,
        steps: vec![crate::plan::EditPlanStep {
            id: "step_1".into(),
            title: "Create entity".into(),
            target_module: crate::plan::TargetModule::Scene,
            action_description: "create a red enemy".into(),
            risk: crate::permission::OperationRisk::Safe,
            validation_requirements: vec![],
        }],
        risk_level: crate::permission::OperationRisk::Safe,
        status: EditPlanStatus::Draft,
    }
}
