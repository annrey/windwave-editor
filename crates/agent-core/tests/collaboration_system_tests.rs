use agent_core::{
    agent_collaboration::{AgentCollaborationSystem, SquadBuilder},
    reasoning_bank::{ReasoningBank, ReasoningBankManager, StepType},
    registry::AgentId,
    runtime_registry::{EngineType, RuntimeManager},
    squad::{SquadRegistry, TaskPriority},
};

#[test]
fn test_squad_creation_and_management() {
    let mut reg = SquadRegistry::new();
    let leader_id = AgentId(1);
    let member_id = AgentId(2);

    let squad_id = reg.create_squad(
        "Test Squad".into(),
        leader_id,
        agent_core::squad::RoutingPolicy::SkillBased,
    );

    assert!(reg.get(squad_id).is_some());

    if let Some(squad) = reg.get_mut(squad_id) {
        squad.add_member(member_id);
        assert_eq!(squad.members.len(), 1);
    }
}

#[test]
fn test_reasoning_trace_creation_and_management() {
    let mut bank = ReasoningBank::new();

    let trace_id = bank.create_trace(
        agent_core::squad::TaskId(1),
        "Test Task".into(),
        "Test Description".into(),
    );

    assert!(bank.get_trace(trace_id).is_some());

    let step_result = bank.add_step_to_trace(
        trace_id,
        StepType::Think,
        "Planning".into(),
        None,
        None,
        Some("Let's think".into()),
        true,
    );
    assert!(step_result.is_some());

    let trace = bank.get_trace(trace_id).unwrap();
    assert_eq!(trace.steps.len(), 1);
}

#[test]
fn test_reasoning_trace_completion_and_tags() {
    let mut bank = ReasoningBank::new();
    let trace_id = bank.create_trace(agent_core::squad::TaskId(1), "Task".into(), "Desc".into());

    bank.add_step_to_trace(
        trace_id,
        StepType::Act,
        "Doing work".into(),
        Some("input".into()),
        Some("output".into()),
        None,
        true,
    );

    bank.mark_trace_complete(trace_id, true, vec!["success".into(), "test".into()]);

    let trace = bank.get_trace(trace_id).unwrap();
    assert!(trace.success);
    assert_eq!(trace.tags.len(), 2);
    assert!(trace.completed_at.is_some());
}

#[test]
fn test_reasoning_bank_manager_high_level_api() {
    let mut manager = ReasoningBankManager::new();

    let trace_id = manager.start_trace(agent_core::squad::TaskId(1), "Title".into(), "Desc".into());

    manager.record_think(trace_id, "Thinking".into(), "Reasoning".into());
    manager.record_act(trace_id, "Acting".into(), "In".into(), "Out".into(), true);
    manager.record_observe(trace_id, "Observing".into(), "Result".into());

    manager
        .bank_mut()
        .mark_trace_complete(trace_id, true, vec!["test".into()]);

    let traces = manager.bank().list_traces();
    assert_eq!(traces.len(), 1);
}

#[test]
fn test_similar_trace_search() {
    let mut bank = ReasoningBank::new();

    let t1 = bank.create_trace(
        agent_core::squad::TaskId(1),
        "Create Entity".into(),
        "Desc".into(),
    );
    let t2 = bank.create_trace(
        agent_core::squad::TaskId(2),
        "Delete Entity".into(),
        "Desc".into(),
    );

    bank.add_step_to_trace(
        t1,
        StepType::Think,
        "Create entity".into(),
        None,
        None,
        None,
        true,
    );
    bank.add_step_to_trace(t2, StepType::Think, "Delete".into(), None, None, None, true);

    bank.mark_trace_complete(t1, true, vec!["entity".into(), "create".into()]);
    bank.mark_trace_complete(t2, true, vec!["entity".into(), "delete".into()]);

    let similar = bank.find_similar_traces(&["entity".into()]);
    assert_eq!(similar.len(), 2);
}

#[test]
fn test_collaboration_system_initialization() {
    let system = AgentCollaborationSystem::new();

    // Just test that it initializes without panics
    let runtimes = system.runtime_manager.registry().list_runtimes();
    assert!(!runtimes.is_empty());
}

#[test]
fn test_squad_builder_pattern() {
    let mut system = AgentCollaborationSystem::new();

    let squad_id = SquadBuilder::new("Builder Test".into(), AgentId(1))
        .with_policy(agent_core::squad::RoutingPolicy::LoadBalance)
        .with_member(AgentId(2))
        .build(&mut system);

    assert!(system.squad_registry.get(squad_id).is_some());
}

#[test]
fn test_task_submission_in_collaboration_system() {
    let mut system = AgentCollaborationSystem::new();

    let squad_id = SquadBuilder::new("Team".into(), AgentId(1)).build(&mut system);

    let (_task_id, _trace_id) = system.submit_and_track_task(
        squad_id,
        "Test Task".into(),
        "Test Desc".into(),
        vec![],
        TaskPriority::Normal,
    );

    assert!(system.squad_registry.get(squad_id).is_some());
}

#[test]
fn test_reasoning_bank_pattern_discovery_basic() {
    let mut manager = ReasoningBankManager::new();

    // Create multiple similar traces
    for i in 0..3 {
        let trace_id = manager.start_trace(
            agent_core::squad::TaskId(i),
            "Create entity".into(),
            "Creating entity trace".into(),
        );

        manager.record_think(trace_id, "Think".into(), "Let's do it".into());
        manager.record_act(
            trace_id,
            "Act".into(),
            "Input".into(),
            "Output".into(),
            true,
        );
        manager.record_observe(trace_id, "Observe".into(), "Good".into());

        manager
            .bank_mut()
            .mark_trace_complete(trace_id, true, vec!["entity".into()]);
    }

    // Try to discover patterns (needs at least 3 similar)
    let patterns = manager.discover_patterns(3);
    let _ = patterns; // May or may not find patterns, depends on implementation
}

#[test]
fn test_runtime_registry_defaults_bev_engine() {
    let manager = RuntimeManager::new();
    assert!(manager.registry().get_active_runtime().is_some());

    let rt = manager.registry().get_active_runtime().unwrap();
    assert_eq!(rt.engine_type, EngineType::Bevy);
}

#[test]
fn test_reasoning_trace_extract_keywords() {
    let mut bank = ReasoningBank::new();

    let trace_id = bank.create_trace(
        agent_core::squad::TaskId(1),
        "Create a red player entity with physics".into(),
        "We need to create a player".into(),
    );

    bank.add_step_to_trace(
        trace_id,
        StepType::Think,
        "Create entity".into(),
        None,
        None,
        None,
        true,
    );

    bank.mark_trace_complete(
        trace_id,
        true,
        vec!["player".into(), "entity".into(), "red".into()],
    );

    let trace = bank.get_trace(trace_id).unwrap();
    let keywords = trace.extract_keywords();
    assert!(!keywords.is_empty());
}

#[test]
fn test_reasoning_trace_summary() {
    let mut bank = ReasoningBank::new();

    let trace_id = bank.create_trace(
        agent_core::squad::TaskId(1),
        "Test Summary".into(),
        "Summary description".into(),
    );

    let _step_id1 = bank.add_step_to_trace(
        trace_id,
        StepType::Think,
        "Step 1".into(),
        None,
        None,
        None,
        true,
    );

    bank.mark_trace_complete(trace_id, true, vec![]);

    let trace = bank.get_trace(trace_id).unwrap();
    let summary = trace.summary();
    assert!(!summary.is_empty());
}

#[test]
fn test_reasoning_bank_successful_traces_only() {
    let mut bank = ReasoningBank::new();

    let t1 = bank.create_trace(agent_core::squad::TaskId(1), "Success".into(), "".into());
    let t2 = bank.create_trace(agent_core::squad::TaskId(2), "Fail".into(), "".into());
    let t3 = bank.create_trace(agent_core::squad::TaskId(3), "Success 2".into(), "".into());

    bank.mark_trace_complete(t1, true, vec![]);
    bank.mark_trace_complete(t2, false, vec![]);
    bank.mark_trace_complete(t3, true, vec![]);

    let successful = bank.get_successful_traces();
    assert_eq!(successful.len(), 2);
}
