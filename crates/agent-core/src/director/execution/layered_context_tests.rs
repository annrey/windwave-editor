//! Sprint 1-C1: L0-L3

/// Test 11: layered context builder basic
#[test]
fn test_layered_context_builder_basic() {
    use crate::LayeredContextBuilder;

    let builder = LayeredContextBuilder::new();
    let ctx = builder.build();

    assert!(!ctx.l0_system.agent_name.is_empty());
    assert!(!ctx.l0_system.engine_name.is_empty());
    assert!(ctx.few_shot_examples.len() >= 3);
}

/// Test 12: entity name auto-extraction
#[test]
fn test_layered_context_entity_extraction() {
    use crate::LayeredContextBuilder;

    let builder = LayeredContextBuilder::new().with_user_request("Player Enemy Boss");
    let ctx = builder.build();

    assert!(ctx
        .l2_task
        .selected_entities
        .contains(&"Player".to_string()));
    assert!(ctx.l2_task.selected_entities.contains(&"Enemy".to_string()));
    assert!(ctx.l2_task.selected_entities.contains(&"Boss".to_string()));

    println!(": {:?}", ctx.l2_task.selected_entities);
}

/// Test 13: goal auto-recognition
#[test]
fn test_layered_context_goal_extraction() {
    use crate::LayeredContextBuilder;

    let builder = LayeredContextBuilder::new().with_user_request("创建一个位于右侧的红色敌人");
    let ctx = builder.build();

    assert!(
        ctx.l2_task.goals.iter().any(|g| g.contains("Create")),
        "goals: {:?}",
        ctx.l2_task.goals
    );

    assert!(
        ctx.l2_task.constraints.iter().any(|c| c.contains("right")),
        "constraints: {:?}",
        ctx.l2_task.constraints
    );
}

/// Test 14: prompt assembly contains all layers
#[test]
fn test_layered_context_prompt_assembly() {
    use crate::LayeredContextBuilder;

    let builder = LayeredContextBuilder::new()
        .with_user_request("Player")
        .with_recent_actions(vec![
            "Created Player at (100, 200)".into(),
            "Moved Player to (150, 250)".into(),
        ]);

    let ctx = builder.build();
    let prompt = builder.build_prompt(&ctx);

    assert!(prompt.contains("SYSTEM CONTEXT (L0)"));
    assert!(prompt.contains("SESSION CONTEXT (L1)"));
    assert!(prompt.contains("TASK CONTEXT (L2)"));
    assert!(prompt.contains("Player"));
    assert!(prompt.contains("Created Player"));
    assert!(prompt.contains("Moved Player"));

    println!("=== ===\n{}", prompt);
}

/// Test 15: few-shot relevance selection
#[test]
fn test_few_shot_relevance_selection() {
    use crate::LayeredContextBuilder;

    let ctx = LayeredContextBuilder::new().build();

    let create_examples = ctx.select_few_shot_examples("create enemy", 1);
    assert_eq!(create_examples.len(), 1);
    assert!(
        create_examples[0].action.contains("create")
            || create_examples[0].action.contains("update")
            || create_examples[0].action.contains("query"),
        "create examples action: {}",
        create_examples[0].action
    );

    let update_examples = ctx.select_few_shot_examples("update Enemy position", 1);
    assert_eq!(update_examples.len(), 1);
    assert!(
        update_examples[0].action.contains("update")
            || update_examples[0].action.contains("create"),
        "update examples action: {}",
        update_examples[0].action
    );

    let query_examples = ctx.select_few_shot_examples("query all entities", 1);
    assert_eq!(query_examples.len(), 1);
    assert!(
        query_examples[0].action.contains("query") || query_examples[0].action.contains("list"),
        "query examples action: {}",
        query_examples[0].action
    );
}

/// Test 16: incremental context update preserves history
#[test]
fn test_incremental_context_update() {
    use crate::LayeredContextBuilder;

    let base_ctx = LayeredContextBuilder::new()
        .with_project("MyAwesomeGame")
        .build();

    let updated_ctx = LayeredContextBuilder::new()
        .with_base_context(base_ctx)
        .with_user_request("Boss")
        .with_recent_actions(vec!["Previous action".into()])
        .build();

    assert_eq!(updated_ctx.l1_session.project_name, "MyAwesomeGame");
    assert!(updated_ctx.l2_task.current_task.contains("Boss"));
    assert_eq!(updated_ctx.l1_session.recent_actions.len(), 1);
    assert!(updated_ctx.l1_session.recent_actions[0].contains("Previous action"));
}
