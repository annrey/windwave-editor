use super::*;
use crate::llm::{
    LlmClient, LlmError, LlmProvider, LlmRequest, LlmResponse, StreamCallback, StreamChunk,
    TokenUsage,
};
use crate::plan::{EditPlanStatus, ExecutionMode};
use crate::strategy::{ReActAgent, ReActConfig};
use crate::tool::ToolRegistry;
use std::sync::Arc;
use std::sync::Mutex;

/// Mock LLM client that returns canned ReAct-format responses.
///
/// Each call to `chat` returns the next response from the scripted list.
/// Useful for testing the ReAct loop end-to-end without a real LLM.
struct ScriptedLlm {
    responses: Vec<String>,
    index: Mutex<usize>,
}

impl ScriptedLlm {
    fn new(responses: Vec<String>) -> Self {
        Self {
            responses,
            index: Mutex::new(0),
        }
    }
}

#[async_trait::async_trait]
impl LlmClient for ScriptedLlm {
    async fn chat(&self, _request: LlmRequest) -> Result<LlmResponse, LlmError> {
        let mut idx = self.index.lock().unwrap();
        let content = if *idx < self.responses.len() {
            let c = self.responses[*idx].clone();
            *idx += 1;
            c
        } else {
            "Final Answer: done".to_string()
        };
        Ok(LlmResponse {
            content,
            tool_calls: vec![],
            usage: TokenUsage::default(),
        })
    }

    fn is_ready(&self) -> bool {
        true
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

#[test]
fn test_new_runtime_is_empty() {
    let rt = DirectorRuntime::new();
    assert!(rt.list_plans().is_empty());
    assert!(!rt.has_pending_approvals());
    assert!(rt.pending_approval_ids().is_empty());
    assert!(rt.trace().is_empty());
}

#[test]
fn test_handle_simple_request_auto_approves() {
    let mut rt = DirectorRuntime::new();
    let events = rt.handle_user_request("创建一个红色敌人");
    let has_direct_completed = events.iter().any(|e| {
        matches!(
            e,
            EditorEvent::DirectExecutionCompleted { success: true, .. }
        )
    });
    assert!(has_direct_completed);
    assert!(!rt.has_pending_approvals());
}

#[test]
fn test_handle_medium_risk_requires_approval() {
    let mut rt = DirectorRuntime::new();
    let events = rt.handle_user_request("批量创建多个红色敌人");
    let has_permission_requested = events
        .iter()
        .any(|e| matches!(e, EditorEvent::PermissionRequested { .. }));
    assert!(has_permission_requested);
    assert!(rt.has_pending_approvals());
}

#[test]
fn test_handle_high_risk_requires_approval() {
    let mut rt = DirectorRuntime::new();
    let events = rt.handle_user_request("删除所有红色敌人");
    assert!(rt.has_pending_approvals());
    let has_permission = events
        .iter()
        .any(|e| matches!(e, EditorEvent::PermissionRequested { .. }));
    assert!(has_permission);
}

#[test]
fn test_approve_and_execute() {
    let mut rt = DirectorRuntime::new();
    rt.handle_user_request("批量创建红色敌人");
    assert!(rt.has_pending_approvals());

    let pending = rt.pending_approval_ids();
    let plan_id = &pending[0];

    let events = rt.approve_plan(plan_id);
    assert!(!rt.has_pending_approvals());
    let has_completed = events
        .iter()
        .any(|e| matches!(e, EditorEvent::ExecutionCompleted { success: true, .. }));
    assert!(has_completed);
}

#[test]
fn test_reject_plan() {
    let mut rt = DirectorRuntime::new();
    rt.handle_user_request("批量创建红色敌人");
    let pending = rt.pending_approval_ids();
    let plan_id = &pending[0];

    let events = rt.reject_plan(plan_id, Some("Not needed"));
    assert!(!rt.has_pending_approvals());
    let has_resolved = events.iter().any(|e| {
        matches!(
            e,
            EditorEvent::PermissionResolved {
                approved: false,
                ..
            }
        )
    });
    assert!(has_resolved);

    let plan = rt.get_plan(plan_id).unwrap();
    assert_eq!(plan.status, EditPlanStatus::Rejected);
}

#[test]
fn test_agent_pending_approval_confirm_applies_hr_action() {
    let mut rt = DirectorRuntime::new();
    let mut registry = crate::registry::AgentRegistry::new();
    registry.register(Box::new(crate::hr_agent::HrAgent::new(
        crate::registry::AgentId(5),
        crate::team_structure::TeamRoster::new(),
    )));

    let events = rt.dispatch_to_agent("hire agent", &mut registry);
    assert!(events
        .iter()
        .any(|e| matches!(e, EditorEvent::PermissionRequested { .. })));
    assert!(rt.has_pending_approvals());

    rt.set_agent_registry(registry);
    let pending = rt.pending_approval_ids();
    let events = rt.approve_plan(&pending[0]);

    assert!(!rt.has_pending_approvals());
    assert!(events
        .iter()
        .any(|e| matches!(e, EditorEvent::PermissionResolved { approved: true, .. })));
    assert!(events.iter().any(
        |e| matches!(e, EditorEvent::StepCompleted { result, .. } if result.contains("Added agent"))
    ));
}

#[test]
fn test_agent_pending_approval_reject_clears_hr_action() {
    let mut rt = DirectorRuntime::new();
    let mut registry = crate::registry::AgentRegistry::new();
    registry.register(Box::new(crate::hr_agent::HrAgent::new(
        crate::registry::AgentId(5),
        crate::team_structure::TeamRoster::new(),
    )));

    rt.dispatch_to_agent("hire agent", &mut registry);
    assert!(rt.has_pending_approvals());

    rt.set_agent_registry(registry);
    let pending = rt.pending_approval_ids();
    let events = rt.reject_plan(&pending[0], Some("No new agent needed"));

    assert!(!rt.has_pending_approvals());
    assert!(events.iter().any(|e| matches!(
        e,
        EditorEvent::PermissionResolved {
            approved: false,
            ..
        }
    )));
}

#[test]
fn test_get_plan_and_list() {
    let mut rt = DirectorRuntime::new();
    rt.handle_user_request("批量创建红色敌人");

    let plans = rt.list_plans();
    assert_eq!(plans.len(), 1);
    assert_eq!(plans[0].title, "批量创建红色敌人");
}

#[test]
fn test_get_plan_not_found() {
    let rt = DirectorRuntime::new();
    assert!(rt.get_plan("nonexistent").is_none());
}

#[test]
fn test_approve_nonexistent_plan() {
    let mut rt = DirectorRuntime::new();
    let events = rt.approve_plan("nonexistent");
    assert!(events
        .iter()
        .any(|e| matches!(e, EditorEvent::Error { .. })));
}

#[test]
fn test_execute_non_approved_plan() {
    let mut rt = DirectorRuntime::new();
    rt.handle_user_request("批量修改颜色");
    let plan_id = {
        let plans = rt.list_plans();
        plans[0].id.clone()
    };

    let events = rt.execute_plan(&plan_id);
    assert!(events
        .iter()
        .any(|e| matches!(e, EditorEvent::Error { .. })));
}

#[test]
fn test_check_goal() {
    let mut rt = DirectorRuntime::new();
    rt.handle_user_request("批量创建红色敌人");

    let events = rt.check_goal(0);
    let has_goal = events
        .iter()
        .any(|e| matches!(e, EditorEvent::GoalChecked { .. }));
    assert!(has_goal);
}

#[test]
fn test_check_goal_no_task() {
    let mut rt = DirectorRuntime::new();
    let events = rt.check_goal(999);
    let has_goal = events.iter().any(|e| {
        matches!(
            e,
            EditorEvent::GoalChecked {
                all_matched: false,
                ..
            }
        )
    });
    assert!(has_goal);
}

#[test]
fn test_review_task() {
    let mut rt = DirectorRuntime::new();
    let events = rt.handle_user_request("批量创建红色敌人");

    let needs_approval = events
        .iter()
        .any(|e| matches!(e, EditorEvent::PermissionRequested { .. }));

    if needs_approval {
        let pending = rt.pending_approval_ids();
        if !pending.is_empty() {
            let plan_id = pending[0].clone();
            rt.approve_plan(&plan_id);
        }
    }

    let review = rt.review_task(0);
    assert_eq!(review.task_id, 0);
    assert!(
        review.decision == "approved" || review.decision == "needs_revision",
        "Expected 'approved' or 'needs_revision', got '{}'",
        review.decision
    );
}

#[test]
fn test_review_nonexistent_task() {
    let mut rt = DirectorRuntime::new();
    let review = rt.review_task(999);
    assert_eq!(review.decision, "no_data");
    assert!(!review.issues.is_empty());
}

#[test]
fn test_recent_events_clamps() {
    let mut rt = DirectorRuntime::new();
    rt.handle_user_request("创建红色敌人");

    let events = rt.recent_events(1);
    assert_eq!(events.len(), 1);

    let events = rt.recent_events(1000);
    assert!(events.len() < 1000);
}

#[test]
fn test_rollback_transaction() {
    let mut rt = DirectorRuntime::new();
    let events = rt.rollback_transaction("txn_test_1");
    assert!(events
        .iter()
        .any(|e| matches!(e, EditorEvent::TransactionRolledBack { .. })));
}

#[test]
fn test_trace_populated() {
    let mut rt = DirectorRuntime::new();
    rt.handle_user_request("批量创建红色敌人");

    let trace = rt.trace();
    assert!(!trace.is_empty());

    let actors: Vec<&str> = trace.iter().map(|t| t.actor.as_str()).collect();
    assert!(actors.contains(&"SmartRouter"));
    assert!(actors.contains(&"Planner"));
}

#[test]
fn test_empty_request_fallback_step() {
    let mut rt = DirectorRuntime::new();
    let _events = rt.handle_user_request("生成一个敌人AI的代码脚本");
    let plans = rt.list_plans();
    assert!(!plans.is_empty());
}

#[test]
fn test_code_request_mode_direct_or_plan() {
    let mut rt = DirectorRuntime::new();
    rt.handle_user_request("生成一个敌人AI的代码脚本");

    let plans = rt.list_plans();
    assert!(matches!(
        plans[0].mode,
        ExecutionMode::Direct | ExecutionMode::Plan
    ));
}

#[test]
fn test_default_impl() {
    let rt = DirectorRuntime::default();
    assert!(rt.list_plans().is_empty());
}

#[test]
fn test_react_agent_layered_context() {
    let rt = DirectorRuntime::new();

    if rt.has_react_agent() {
        if let Some(ref react) = rt.react_agent {
            assert!(
                !react.config.system_prompt.is_empty(),
                "System prompt should not be empty"
            );
        }
    }

    let layered = crate::prompt::LayeredContext {
        l0_system: crate::prompt::L0SystemContext::default_bevy(),
        ..Default::default()
    };
    assert!(
        !layered.l0_system.agent_name.is_empty(),
        "L0 agent name should be set"
    );
    assert!(
        !layered.l0_system.engine_name.is_empty(),
        "L0 engine name should be set"
    );
}

#[test]
fn test_few_shot_example_selection() {
    use crate::prompt::{FewShotExample, LayeredContext};

    let mut layered = LayeredContext::default();
    layered.add_few_shot(FewShotExample::create_entity_example());
    layered.add_few_shot(FewShotExample::update_component_example());
    layered.add_few_shot(FewShotExample::query_entities_example());

    let selected = layered.select_few_shot_examples("创建一个红色敌人", 2);
    assert!(!selected.is_empty(), "Should select at least one example");
    assert!(
        selected[0].action.contains("create"),
        "Should select create example first"
    );

    let selected = layered.select_few_shot_examples("把 Player 改成蓝色", 2);
    assert!(!selected.is_empty(), "Should select at least one example");
    assert!(
        selected[0].action.contains("update"),
        "Should select update example first"
    );
}

#[test]
fn test_dynamic_revision_skips_duplicate_steps() {
    use crate::permission::OperationRisk;
    use crate::plan::{EditPlan, EditPlanStatus, EditPlanStep, ExecutionMode, TargetModule};

    let mut rt = DirectorRuntime::new();

    let mut plan = EditPlan::new(
        "test_plan",
        1,
        "Test Plan",
        "Test dynamic revision",
        ExecutionMode::Plan,
    );
    plan.status = EditPlanStatus::Draft;
    plan.steps = vec![
        EditPlanStep {
            id: "step_1".into(),
            title: "Create Enemy".into(),
            target_module: TargetModule::Scene,
            action_description: "Create Enemy".into(),
            risk: OperationRisk::LowRisk,
            validation_requirements: vec![],
        },
        EditPlanStep {
            id: "step_2".into(),
            title: "Create Player".into(),
            target_module: TargetModule::Scene,
            action_description: "Create Player".into(),
            risk: OperationRisk::LowRisk,
            validation_requirements: vec![],
        },
    ];

    rt.plan_manager.insert("test_plan".into(), plan);

    rt.apply_plan_revision("test_plan", "Skip duplicate creation steps");

    let updated_plan = rt.plan_manager.get("test_plan").unwrap();
    assert!(
        updated_plan.steps[0].title.starts_with("[SKIPPED]"),
        "Step 1 should be skipped"
    );
    assert!(
        updated_plan.steps[1].title.starts_with("[SKIPPED]"),
        "Step 2 should be skipped"
    );
}

#[test]
fn test_reflection_alternative_step_generation() {
    let rt = DirectorRuntime::new();

    let alt = rt.generate_alternative_step("Delete Enemy", "Entity not found");
    assert!(
        alt.is_some(),
        "Should generate alternative for not found error"
    );
    let alt_text = alt.unwrap();
    assert!(
        alt_text.contains("Create entity"),
        "Alternative should suggest creating entity first"
    );

    let alt = rt.generate_alternative_step("Create Player", "Entity already exists");
    assert!(
        alt.is_some(),
        "Should generate alternative for already exists error"
    );
    let alt_text = alt.unwrap();
    assert!(
        alt_text.contains("Modify") || alt_text.contains("修改"),
        "Alternative should suggest modification"
    );

    let alt = rt.generate_alternative_step("Delete Boss", "Permission denied");
    assert!(
        alt.is_some(),
        "Should generate alternative for permission error"
    );
    let alt_text = alt.unwrap();
    assert!(
        alt_text.contains("LOW_RISK"),
        "Alternative should use low risk approach"
    );
}

// ================================================================
// Sprint 3: 模块集成测试
// ================================================================

#[test]
fn test_init_squad_enables_collaboration() {
    let mut rt = DirectorRuntime::new();
    assert!(rt.squad().is_none());

    rt.init_squad();
    assert!(rt.squad().is_some());

    let squad = rt.squad().unwrap();
    assert_eq!(squad.name, "default");
    assert_eq!(squad.id, crate::squad::SquadId(0));
}

#[test]
fn test_capture_pipeline_initialized() {
    let rt = DirectorRuntime::new();
    let _pipeline = rt.capture_pipeline();
    let ids = rt.pending_approval_ids();
    assert!(ids.is_empty());
}

#[test]
fn test_capture_pipeline_trigger_hook() {
    let mut rt = DirectorRuntime::new();

    let hook = crate::capture_pipeline::MemoryHook::UserRequest {
        request: "test request".to_string(),
        intent: Some("testing".to_string()),
    };
    let captured = rt.trigger_capture_hook(&hook);
    assert!(!captured.is_empty());

    let results = rt.trigger_capture_hook(&hook);
    assert!(results.is_empty());
}

#[test]
fn test_build_layered_context_basic() {
    let rt = DirectorRuntime::new();
    let ctx = rt.build_layered_context(Some("create a red cube"));
    assert!(!ctx.l0_system.agent_name.is_empty());
    assert!(!ctx.l0_system.engine_name.is_empty());
}

#[test]
fn test_build_context_prompt() {
    let rt = DirectorRuntime::new();
    let prompt = rt.build_context_prompt(Some("test request"));
    assert!(!prompt.is_empty());
}

#[test]
fn test_reasoning_bank_accessible() {
    let mut rt = DirectorRuntime::new();
    let bank = rt.reasoning_bank_mut();
    let traces = bank.bank().list_traces();
    assert!(traces.is_empty());
}

#[test]
fn test_skill_compound_accessible() {
    let mut rt = DirectorRuntime::new();
    let compound = rt.compound_mut();
    let skills = compound.registry().list_skills();
    assert!(skills.is_empty());
}

/// T9: End-to-end "红色敌人" scenario via ReAct loop.
///
/// Injects a ScriptedLlm that returns a canned Final Answer in the
/// parse_response format, and verifies the ReAct loop produces the
/// expected DirectExecutionStarted (mode="ReAct") and
/// DirectExecutionCompleted (success=true) events.
#[tokio::test]
async fn test_end_to_end_red_enemy_via_react() {
    let mut rt = DirectorRuntime::new();

    let response_text =
        "Thought: I need to create a red enemy first.\nFinal Answer: 红色敌人已创建";
    let script = ScriptedLlm::new(vec![response_text.to_string()]);

    let mut registry = ToolRegistry::new();
    crate::code_tools::register_code_tools(&mut registry);
    crate::file_tools::register_file_tools(&mut registry);
    let tool_registry = Arc::new(Mutex::new(registry));

    let config = ReActConfig {
        max_steps: 5,
        temperature: 0.0,
        include_observations: true,
        system_prompt: super::REACT_SYSTEM_PROMPT.to_string(),
    };

    let react = ReActAgent::new(config, Arc::new(script), tool_registry);
    rt.react_agent = Some(react);

    let events = rt.execute_with_react("创建一个红色敌人").await.unwrap();

    let has_started_react = events
        .iter()
        .any(|e| matches!(e, EditorEvent::DirectExecutionStarted { mode, .. } if mode == "ReAct"));
    assert!(has_started_react, "Should start with ReAct mode");

    let has_completed = events.iter().any(|e| {
        matches!(
            e,
            EditorEvent::DirectExecutionCompleted { success: true, .. }
        )
    });
    assert!(has_completed, "Should complete successfully");
}

// ------------------------------------------------------------------
// T12: Snapshot regression tests
// ------------------------------------------------------------------

/// Map each EditorEvent variant to a stable string name for snapshot comparison.
fn event_type_name(e: &EditorEvent) -> &'static str {
    match e {
        EditorEvent::EditPlanCreated { .. } => "EditPlanCreated",
        EditorEvent::PermissionRequested { .. } => "PermissionRequested",
        EditorEvent::PermissionResolved { .. } => "PermissionResolved",
        EditorEvent::PlanExecutionStarted { .. } => "PlanExecutionStarted",
        EditorEvent::StepStarted { .. } => "StepStarted",
        EditorEvent::StepCompleted { .. } => "StepCompleted",
        EditorEvent::StepFailed { .. } => "StepFailed",
        EditorEvent::TransactionStarted { .. } => "TransactionStarted",
        EditorEvent::TransactionCommitted { .. } => "TransactionCommitted",
        EditorEvent::TransactionRolledBack { .. } => "TransactionRolledBack",
        EditorEvent::GoalChecked { .. } => "GoalChecked",
        EditorEvent::ReviewCompleted { .. } => "ReviewCompleted",
        EditorEvent::ExecutionCompleted { .. } => "ExecutionCompleted",
        EditorEvent::ModeChanged { .. } => "ModeChanged",
        EditorEvent::Error { .. } => "Error",
        EditorEvent::DirectExecutionStarted { .. } => "DirectExecutionStarted",
        EditorEvent::DirectExecutionCompleted { .. } => "DirectExecutionCompleted",
    }
}

/// Snapshot: 红色敌人 scenario.
///
/// Uses a golden-file approach. First run creates the file in
/// `tests/snapshot/red_enemy.json`; subsequent runs assert the
/// event-type sequence matches. This catches unintended changes
/// in the ReAct execution path while keeping test input fixed.
#[tokio::test]
async fn test_snapshot_red_enemy() {
    let snapshot_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("snapshot");
    std::fs::create_dir_all(&snapshot_dir).unwrap();
    let snapshot_path = snapshot_dir.join("red_enemy.json");

    let request = "创建一个红色敌人";
    let llm_responses = vec![
        "Thought: I need to create a red enemy first.\nFinal Answer: 红色敌人已创建".to_string(),
    ];

    let mut rt = DirectorRuntime::new();
    let script = ScriptedLlm::new(llm_responses.clone());

    let mut registry = ToolRegistry::new();
    crate::code_tools::register_code_tools(&mut registry);
    crate::file_tools::register_file_tools(&mut registry);
    let tool_registry = Arc::new(Mutex::new(registry));

    let config = ReActConfig {
        max_steps: 5,
        temperature: 0.0,
        include_observations: true,
        system_prompt: super::REACT_SYSTEM_PROMPT.to_string(),
    };

    let react = ReActAgent::new(config, Arc::new(script), tool_registry);
    rt.react_agent = Some(react);

    let events = rt.execute_with_react(request).await.unwrap();

    let event_types: Vec<String> = events
        .iter()
        .map(|e| event_type_name(e).to_string())
        .collect();

    let snapshot = serde_json::json!({
        "request": request,
        "llm_responses": llm_responses,
        "expected_event_types": event_types,
    });

    if snapshot_path.exists() {
        let existing: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&snapshot_path).unwrap()).unwrap();
        let expected: Vec<String> = existing["expected_event_types"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().to_string())
            .collect();

        assert_eq!(
            event_types,
            expected,
            "Snapshot mismatch for {}. Delete the file to regenerate.",
            snapshot_path.display(),
        );
    } else {
        let pretty = serde_json::to_string_pretty(&snapshot).unwrap();
        std::fs::write(&snapshot_path, &pretty).unwrap();
        panic!(
            "Snapshot golden file CREATED at {}. Review it, then re-run the test.",
            snapshot_path.display(),
        );
    }
}

// ------------------------------------------------------------------
// T15: Squad/HR dispatch integration tests
// ------------------------------------------------------------------

/// Verify that dispatch_to_agent routes squad creation to SquadAgent.
#[test]
fn test_dispatch_squad_create_routes_to_squad_agent() {
    let mut rt = DirectorRuntime::new();
    let mut registry = DirectorRuntime::init_internal_agents();

    let events = rt.dispatch_to_agent("create a squad for editing", &mut registry);
    assert!(
        !events.is_empty(),
        "should produce events for squad creation"
    );
}

/// Verify that dispatch_to_agent routes HR add to HrAgent.
#[test]
fn test_dispatch_hr_add_routes_to_hr_agent() {
    let mut rt = DirectorRuntime::new();
    let mut registry = DirectorRuntime::init_internal_agents();

    let events = rt.dispatch_to_agent("add executor to the team", &mut registry);
    assert!(
        !events.is_empty(),
        "should produce events for HR add request"
    );
}

/// Verify that dispatch_to_agent routes HR list to HrAgent.
#[test]
fn test_dispatch_hr_list_routes_to_hr_agent() {
    let mut rt = DirectorRuntime::new();
    let mut registry = DirectorRuntime::init_internal_agents();

    let events = rt.dispatch_to_agent("list team", &mut registry);
    assert!(
        !events.is_empty(),
        "should produce events for HR list request"
    );
}

/// Verify that dispatch_to_registered_agent produces events for squad request.
#[test]
fn test_dispatch_to_registered_agent_squad() {
    let mut rt = DirectorRuntime::new();
    let events = rt.dispatch_to_registered_agent("create a squad for editing");
    assert!(
        !events.is_empty(),
        "should produce events for squad via registered dispatch"
    );
}

/// Verify that dispatch_to_registered_agent produces events for HR request.
#[test]
fn test_dispatch_to_registered_agent_hr() {
    let mut rt = DirectorRuntime::new();
    let events = rt.dispatch_to_registered_agent("add executor to the team");
    assert!(
        !events.is_empty(),
        "should produce events for HR via registered dispatch"
    );
}

/// Verify that init_internal_agents includes SquadAgent.
#[test]
fn test_init_internal_agents_includes_squad() {
    let registry = DirectorRuntime::init_internal_agents();
    let squad_candidates = registry.find_by_role(&crate::registry::AgentRole::Director);
    assert!(
        !squad_candidates.is_empty(),
        "should have agents with Orchestrate capability"
    );
}

/// Verify that agent_registry has all required agents registered.
#[test]
fn test_agent_registry_has_all_agents() {
    let registry = DirectorRuntime::init_internal_agents();
    assert!(
        registry.agent_count() >= 6,
        "should have at least 6 agents (scene, code, review, planner, hr, squad)"
    );
}
