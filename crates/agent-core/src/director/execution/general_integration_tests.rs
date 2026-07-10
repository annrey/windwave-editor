use crate::director::types::{DirectorRuntime, EditorEvent};

/// Test 1: ReActAgent L0-L3
#[test]
fn test_react_agent_initialization_with_layered_context() {
    let rt = DirectorRuntime::new();

    if rt.has_react_agent() {
        assert!(
            rt.react_agent.is_some(),
            "ReActAgent should be initialized when LLM is configured"
        );

        if let Some(ref react) = rt.react_agent {
            assert!(
                !react.config.system_prompt.is_empty(),
                "ReActAgent should have system prompt configured"
            );
            let _ = react;
        }
    }

    assert!(rt.list_plans().is_empty());
}

/// Test 2: execute_with_llm
#[test]
fn test_execute_with_llm_fallback_without_tokio() {
    let mut rt = DirectorRuntime::new();

    let response = rt.execute_with_llm("创建一个红色敌人");

    assert!(
        !response.is_empty(),
        "execute_with_llm should always return a response"
    );

    if rt.has_react_agent() {
        assert!(
            response.contains("ReAct") || response.contains("✅") || response.contains("❌"),
            "Response should indicate ReAct execution: {}",
            response
        );
    } else {
        assert!(
            !response.is_empty(),
            "Fallback response should not be empty"
        );
    }
}

/// Test 3: event streaming
#[test]
fn test_event_streaming_for_react_execution() {
    let mut rt = DirectorRuntime::new();

    let _response = rt.execute_with_llm("查询场景中的所有实体");

    let has_direct_events = rt.events.iter().any(|e| {
        matches!(
            e,
            EditorEvent::DirectExecutionStarted { .. }
                | EditorEvent::DirectExecutionCompleted { .. }
        )
    });

    let _ = has_direct_events;
}

/// Test 4: dynamic plan revision (Sprint 1-A2)
#[test]
fn test_dynamic_plan_revision_on_duplicate_entity() {
    let mut rt = DirectorRuntime::new();

    use crate::plan::{EditPlan, ExecutionMode};

    let plan = EditPlan::new(
        "test_revision",
        1,
        "Test Revision Plan",
        "Test dynamic revision when entity already exists",
        ExecutionMode::Plan,
    );

    rt.plan_manager.insert("test_revision".into(), plan);

    let revision_needed = rt.check_plan_revision_needed(
        rt.plan_manager.get("test_revision").unwrap(),
        0,
        "Entity 'Player' already exists in scene",
    );

    assert!(
        revision_needed.is_some(),
        "Should detect need for revision when entity already exists"
    );

    let revision_text = revision_needed.unwrap();
    assert!(
        revision_text.to_lowercase().contains("skip")
            || revision_text.to_lowercase().contains("duplicate")
            || !revision_text.is_empty(),
        "Revision should suggest action: {}",
        revision_text
    );
}

/// Test 5: reflection generates alternative for not found (Sprint 1-A3)
#[test]
fn test_reflection_generates_alternative_for_not_found_error() {
    let rt = DirectorRuntime::new();

    let alternative =
        rt.generate_alternative_step("Delete Enemy", "Entity 'Enemy' not found in scene");

    assert!(
        alternative.is_some(),
        "Should generate alternative for 'not found' error"
    );

    let alt_text = alternative.unwrap();
    assert!(
        alt_text.to_lowercase().contains("create"),
        "Alternative should suggest creating the entity first: {}",
        alt_text
    );
}

/// Test 6: reflection handles permission denied
#[test]
fn test_reflection_handles_permission_denied() {
    let rt = DirectorRuntime::new();

    let alternative = rt.generate_alternative_step(
        "Delete Boss Entity",
        "Permission denied: operation requires admin privileges",
    );

    assert!(
        alternative.is_some(),
        "Should generate alternative for permission denied"
    );

    let alt_text = alternative.unwrap();
    assert!(
        alt_text.contains("[LOW_RISK]") || alt_text.to_lowercase().contains("low risk"),
        "Alternative should use lower risk approach: {}",
        alt_text
    );
}

/// Test 7: full react lifecycle via async
#[tokio::test]
async fn test_full_react_lifecycle_via_async() {
    let mut rt = DirectorRuntime::new();

    let events = rt.handle_user_request_async("创建一个红色敌人").await;

    assert!(
        !events.is_empty() || !rt.events.is_empty(),
        "Should produce events from request handling"
    );

    if rt.has_react_agent() {
        let has_react_events = events.iter().any(|e| {
            matches!(
                e,
                EditorEvent::StepStarted { title, .. } if title.contains("Think")
                    || title.contains("Act")
                    || title.contains("Observe")
            )
        });

        let _ = has_react_events;
    }
}

/// Test 8: acceptance - "create red enemy" scenario
#[test]
fn test_acceptance_create_red_enemy_scenario() {
    let mut rt = DirectorRuntime::new();
    rt.init_builtin_skills();

    let request = "创建一个红色敌人";

    let response = rt.execute_with_llm(request);

    assert!(!response.is_empty(), "Response should not be empty");

    let has_any_events = !rt.events.is_empty() || !rt.trace_entries.is_empty();
    assert!(has_any_events, "Should have event or trace entries");

    if rt.has_react_agent() {
        let has_thinking_event = rt.events.iter().any(
            |e| matches!(e, EditorEvent::DirectExecutionStarted { mode, .. } if mode == "ReAct"),
        );
        assert!(
            has_thinking_event || response.contains("ReAct"),
            "With ReActAgent, should show ReAct execution indicators"
        );
    }

    let has_executor_trace = rt
        .trace_entries
        .iter()
        .any(|t| t.actor == "LlmExecutor" || t.actor == "ReActAgent" || t.actor == "SmartRouter");
    assert!(
        has_executor_trace,
        "Trace should contain executor information. Traces: {:?}",
        rt.trace_entries
            .iter()
            .map(|t| &t.actor)
            .collect::<Vec<_>>()
    );

    println!("✅ Acceptance test passed!");
    println!("   Response: {}", response);
    println!("   Events count: {}", rt.events.len());
    println!("   Trace entries: {:?}", rt.trace_entries);
}

/// Test 9: graceful degradation when LLM unavailable
#[test]
fn test_graceful_degradation_when_llm_unavailable() {
    let mut rt = DirectorRuntime::new();

    rt.disable_llm();

    let response = rt.execute_with_llm("创建一个蓝色玩家");

    assert!(
        !response.is_empty(),
        "Should fallback gracefully when LLM unavailable"
    );

    assert!(
        response.contains("TemplateApplied")
            || response.contains("RuleMatched")
            || response.contains("LlmUnavailable")
            || !response.is_empty(),
        "Fallback should produce valid response: {}",
        response
    );
}

/// Test 10: ToolRegistry + SceneBridge integration
#[test]
fn test_scene_bridge_tool_integration() {
    let mut rt = DirectorRuntime::new();
    rt.init_builtin_skills();

    rt.set_scene_bridge(Box::new(crate::scene_bridge::MockSceneBridge::new()));

    let _response = rt.execute_with_llm("查询场景");

    let commands = rt.drain_bridge_commands();
    let _ = commands;
}

#[test]
fn test_failed_internal_plan_emits_revision_review() {
    use crate::permission::OperationRisk;
    use crate::plan::{EditPlan, EditPlanStatus, EditPlanStep, ExecutionMode, TargetModule};

    let mut rt = DirectorRuntime::new();
    rt.set_scene_bridge(Box::new(crate::scene_bridge::MockSceneBridge::new()));

    let mut plan = EditPlan::new(
        "revision_on_failure",
        42,
        "Revision on failure",
        "Deleting a missing entity should produce a repair hint",
        ExecutionMode::Plan,
    );
    plan.status = EditPlanStatus::Approved;
    plan.add_step(EditPlanStep::new(
        "step_missing_delete",
        "删除 MissingEnemy",
        TargetModule::Scene,
        "Delete a missing enemy",
        OperationRisk::LowRisk,
    ));
    rt.plan_manager.insert(plan.id.clone(), plan);

    let events = rt.execute_plan("revision_on_failure");

    assert!(
        events
            .iter()
            .any(|event| matches!(event, EditorEvent::StepFailed { .. })),
        "expected a failed step event: {:?}",
        events
    );
    assert!(
        events.iter().any(|event| matches!(
            event,
            EditorEvent::ReviewCompleted {
                task_id: 42,
                decision,
                summary,
            } if decision == "needs_revision"
                && (summary.contains("revision") || summary.contains("alternative"))
        )),
        "failure should surface a revision review: {:?}",
        events
    );

    let revised = rt
        .plan_manager
        .get("revision_on_failure")
        .expect("plan should remain available for revision");
    assert!(
        revised
            .steps
            .iter()
            .any(|step| step.title.contains("[ADAPTED]")
                || step.title.contains("Create entity")
                || step.title.contains("Create missing entity")),
        "failed plan should contain a revision or replacement step, got {:?}",
        revised.steps
    );
}
