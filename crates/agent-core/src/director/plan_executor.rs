//! Plan execution pipeline — permission, ReAct execution, and internal step execution.

use super::types::{DirectorRuntime, DirectorTraceEntry, EditorEvent, SceneBridgeSkillHandler};
use crate::keyword_matcher::KeywordMatcher;
use crate::permission::PermissionRequirement;
use crate::plan::{EditPlan, EditPlanStatus, EditPlanStep, TargetModule};
use crate::rollback::{OperationType, SnapshotEntity};
use crate::types::now_millis;

fn execute_step_via_bridge(
    step: &EditPlanStep,
    skill_def: Option<&crate::skill::SkillDefinition>,
    bridge: &mut dyn crate::scene_bridge::SceneBridge,
    skill_executor: &mut crate::skill::SkillExecutor,
) -> Result<String, String> {
    match step.target_module {
        TargetModule::Scene => {
            if let Some(name) = step.title.strip_prefix("创建实体: ") {
                let mut props = std::collections::HashMap::new();
                props.insert("color".into(), serde_json::json!([1.0, 1.0, 1.0, 1.0]));
                let sprite_patch = crate::scene_bridge::ComponentPatch {
                    type_name: "Sprite".into(),
                    properties: props,
                };
                return bridge
                    .create_entity(name.trim(), None, &[sprite_patch])
                    .map(|_id| format!("Created entity {}", name.trim()))
                    .map_err(|e| format!("Failed to create entity: {}", e));
            }

            if let Some((prefix, color_name)) = step.title.split_once(" 颜色为 ") {
                let name = prefix.strip_prefix("设置 ").unwrap_or(prefix).trim();
                let rgba = match color_name.trim() {
                    "红色" => [1.0, 0.0, 0.0, 1.0],
                    "蓝色" => [0.0, 0.0, 1.0, 1.0],
                    "绿色" => [0.0, 1.0, 0.0, 1.0],
                    "黄色" => [1.0, 1.0, 0.0, 1.0],
                    "黑色" => [0.0, 0.0, 0.0, 1.0],
                    "白色" => [1.0, 1.0, 1.0, 1.0],
                    _ => [1.0, 0.0, 0.0, 1.0],
                };
                let entities = bridge.query_entities(Some(name), None);
                if let Some(entity) = entities.iter().find(|e| e.name == name) {
                    let mut props = std::collections::HashMap::new();
                    props.insert("color".into(), serde_json::json!(rgba));
                    return bridge
                        .update_component(entity.id, "Sprite", props)
                        .map(|()| format!("Changed color of entity {} to {}", name, color_name))
                        .map_err(|e| format!("Failed to change color: {}", e));
                }
                return Err(format!("Entity '{}' not found for color change", name));
            }

            if let Some(rest) = step.title.strip_prefix("放置 ") {
                if let Some((name, position_desc)) = rest.split_once(" 在 ") {
                    let entities = bridge.query_entities(Some(name.trim()), None);
                    if let Some(entity) = entities.iter().find(|e| e.name == name.trim()) {
                        let lower = position_desc.to_lowercase();
                        let position = if lower.contains("右") || lower.contains("right") {
                            [100.0, 0.0, 0.0]
                        } else if lower.contains("左") || lower.contains("left") {
                            [-100.0, 0.0, 0.0]
                        } else if lower.contains("上") || lower.contains("above") {
                            [0.0, 100.0, 0.0]
                        } else if lower.contains("下") || lower.contains("below") {
                            [0.0, -100.0, 0.0]
                        } else {
                            [0.0, 0.0, 0.0]
                        };
                        let mut props = std::collections::HashMap::new();
                        props.insert("position".into(), serde_json::json!(position));
                        return bridge
                            .update_component(entity.id, "Transform", props)
                            .map(|()| format!("Placed entity {} at {}", name.trim(), position_desc))
                            .map_err(|e| format!("Failed to place entity: {}", e));
                    }
                    return Err(format!("Entity '{}' not found for placement", name.trim()));
                }
            }

            if let Some(skill_def) = skill_def {
                let mut handler = SceneBridgeSkillHandler { bridge };
                match skill_executor.execute_with_handler(skill_def, &mut handler) {
                    Ok(results) => {
                        let names: Vec<&str> = results.iter().map(|r| r.title.as_str()).collect();
                        Ok(format!(
                            "Skill '{}' executed: {} nodes [{}]",
                            skill_def.name,
                            results.len(),
                            names.join(", ")
                        ))
                    }
                    Err(e) => Err(format!("Skill '{}' failed: {}", skill_def.name, e)),
                }
            } else {
                let parts: Vec<&str> = step.title.split_whitespace().collect();

                match KeywordMatcher::resolve_skill_name(&step.title) {
                    Some("create_entity") => {
                        let name = parts.last().unwrap_or(&"entity");
                        bridge
                            .create_entity(name, None, &[])
                            .map(|_id| format!("Created entity {}", name))
                            .map_err(|e| format!("Failed to create entity: {}", e))
                    }
                    Some("delete_entity") => {
                        let entities = bridge.query_entities(None, None);
                        let name = parts.last().unwrap_or(&"");
                        if let Some(entity) = entities.iter().find(|e| e.name == *name) {
                            bridge
                                .delete_entity(entity.id)
                                .map(|()| format!("Deleted entity {}", name))
                                .map_err(|e| format!("Failed to delete entity: {}", e))
                        } else {
                            Err(format!("Entity '{}' not found for deletion", name))
                        }
                    }
                    Some("modify_entity_transform") => {
                        let entities = bridge.query_entities(None, None);
                        let name = parts.last().unwrap_or(&"");
                        if let Some(entity) = entities.iter().find(|e| e.name == *name) {
                            let mut props = std::collections::HashMap::new();
                            props.insert("position".into(), serde_json::json!([0.0, 0.0]));
                            bridge
                                .update_component(entity.id, "Transform", props)
                                .map(|()| format!("Moved entity {} to origin", name))
                                .map_err(|e| format!("Failed to move entity: {}", e))
                        } else {
                            Err(format!("Entity '{}' not found for move", name))
                        }
                    }
                    Some("modify_entity_color") => {
                        let entities = bridge.query_entities(None, None);
                        let name = parts.last().unwrap_or(&"");
                        if let Some(entity) = entities.iter().find(|e| e.name == *name) {
                            let mut props = std::collections::HashMap::new();
                            props.insert("color".into(), serde_json::json!([1.0, 0.0, 0.0, 1.0]));
                            bridge
                                .update_component(entity.id, "Sprite", props)
                                .map(|()| format!("Changed color of entity {} to red", name))
                                .map_err(|e| format!("Failed to change color: {}", e))
                        } else {
                            Err(format!("Entity '{}' not found for color change", name))
                        }
                    }
                    Some("query_scene") => {
                        let entities = bridge.query_entities(None, None);
                        let names: Vec<String> = entities.iter().map(|e| e.name.clone()).collect();
                        Ok(format!(
                            "Scene entities ({}): {}",
                            names.len(),
                            names.join(", ")
                        ))
                    }
                    _ => Err(format!(
                        "No matching skill, keyword, or tool for step: '{}'",
                        step.title
                    )),
                }
            }
        }
        _ => Ok(format!(
            "Step '{}' executed (module {:?})",
            step.title, step.target_module
        )),
    }
}

impl DirectorRuntime {
    /// Sprint 1: Execute plan steps with ReActAgent support for dynamic revision.
    pub(crate) async fn execute_plan_with_permission_and_react(
        &mut self,
        plan: EditPlan,
    ) -> Vec<EditorEvent> {
        let plan_id = plan.id.clone();
        self.plan_manager.insert(plan_id.clone(), plan.clone());

        self.events.push(EditorEvent::EditPlanCreated {
            plan_id: plan_id.clone(),
            title: plan.title.clone(),
            risk: format!("{:?}", plan.risk_level),
            mode: format!("{:?}", plan.mode),
            steps_count: plan.steps.len(),
        });

        let permission = self.plan_manager.check_permission(&plan_id);

        match permission {
            PermissionRequirement::AutoApproved => {
                self.plan_manager
                    .set_status(&plan_id, EditPlanStatus::Approved);
                self.events.push(EditorEvent::PermissionResolved {
                    plan_id: plan_id.clone(),
                    approved: true,
                    reason: None,
                });
                if self.has_react_agent() {
                    self.execute_plan_with_react(&plan_id).await
                } else {
                    self.execute_plan_internal(&plan_id)
                }
            }
            PermissionRequirement::NeedUserConfirmation { risk, reason } => {
                self.events.push(EditorEvent::PermissionRequested {
                    plan_id: plan_id.clone(),
                    risk: format!("{:?}", risk),
                    reason: reason.clone(),
                });
                self.plan_manager.add_pending(plan_id.clone());
                self.plan_manager
                    .set_status(&plan_id, EditPlanStatus::WaitingForApproval);
                self.recent_events_internal(3)
            }
            PermissionRequirement::Forbidden { reason } => {
                self.events.push(EditorEvent::Error {
                    message: format!("Plan forbidden: {}", reason),
                });
                self.plan_manager
                    .set_status(&plan_id, EditPlanStatus::Rejected);
                self.recent_events_internal(3)
            }
        }
    }

    /// Approve a pending plan and execute it.
    /// Returns events from plan execution.
    pub fn approve_plan(&mut self, plan_id: &str) -> Vec<EditorEvent> {
        if let Some(agent_id) = self.agent_pending_approvals.remove(plan_id) {
            let mut events = vec![EditorEvent::PermissionResolved {
                plan_id: plan_id.to_string(),
                approved: true,
                reason: None,
            }];

            let result = self
                .agent_registry
                .as_mut()
                .ok_or_else(|| "Agent registry unavailable".to_string())
                .and_then(|registry| {
                    registry
                        .confirm_pending_user_input(agent_id)
                        .map_err(|e| e.to_string())
                });

            match result {
                Ok(Some(response)) => {
                    let result_text = match response.result {
                        crate::registry::AgentResultKind::Success { summary, .. } => summary,
                        crate::registry::AgentResultKind::PartialSuccess {
                            summary,
                            warnings,
                            ..
                        } => {
                            if warnings.is_empty() {
                                summary
                            } else {
                                format!("{} (warnings: {})", summary, warnings.join(", "))
                            }
                        }
                        crate::registry::AgentResultKind::NeedUserInput { question } => {
                            format!("Still needs input: {}", question)
                        }
                        crate::registry::AgentResultKind::Failed { reason } => {
                            format!("Failed: {}", reason)
                        }
                    };
                    events.push(EditorEvent::StepCompleted {
                        plan_id: plan_id.to_string(),
                        step_id: format!("agent_{:?}", agent_id),
                        title: response.agent_name,
                        result: result_text,
                    });
                }
                Ok(None) => {
                    events.push(EditorEvent::Error {
                        message: format!("Agent {:?} had no pending action to approve", agent_id),
                    });
                }
                Err(message) => {
                    events.push(EditorEvent::Error {
                        message: format!("Agent approval '{}' failed: {}", plan_id, message),
                    });
                }
            }

            self.events.extend(events.clone());
            return events;
        }

        match self.plan_manager.approve(plan_id) {
            Ok(()) => {
                self.events.push(EditorEvent::PermissionResolved {
                    plan_id: plan_id.to_string(),
                    approved: true,
                    reason: None,
                });
                if self.has_react_agent() {
                    vec![] // ReAct handles execution asynchronously
                } else {
                    self.execute_plan_internal(plan_id)
                }
            }
            Err(id) => {
                vec![EditorEvent::Error {
                    message: format!("Plan '{}' not found for approval", id),
                }]
            }
        }
    }

    /// Reject a pending plan.
    /// Returns rejection events.
    pub fn reject_plan(&mut self, plan_id: &str, reason: Option<&str>) -> Vec<EditorEvent> {
        if let Some(agent_id) = self.agent_pending_approvals.remove(plan_id) {
            let reason_msg = reason.unwrap_or("Rejected by user");
            if let Some(registry) = self.agent_registry.as_mut() {
                let _ = registry.reject_pending_user_input(agent_id);
            }
            let events = vec![EditorEvent::PermissionResolved {
                plan_id: plan_id.to_string(),
                approved: false,
                reason: Some(reason_msg.to_string()),
            }];
            self.events.extend(events.clone());
            return events;
        }

        match self.plan_manager.reject(plan_id) {
            Ok(()) => {
                let reason_msg = reason.unwrap_or("Rejected by user");
                self.events.push(EditorEvent::PermissionResolved {
                    plan_id: plan_id.to_string(),
                    approved: false,
                    reason: Some(reason_msg.to_string()),
                });
                vec![EditorEvent::PermissionResolved {
                    plan_id: plan_id.to_string(),
                    approved: false,
                    reason: Some(reason_msg.to_string()),
                }]
            }
            Err(id) => {
                vec![EditorEvent::Error {
                    message: format!("Plan '{}' not found for rejection", id),
                }]
            }
        }
    }

    /// Look up a plan by id and set it to Running, emitting PlanExecutionStarted.
    /// Returns the plan clone on success, or already-pushed error events on failure.
    fn begin_plan_execution(&mut self, plan_id: &str) -> Result<EditPlan, Vec<EditorEvent>> {
        let plan = match self.plan_manager.get(plan_id) {
            Some(p) => p.clone(),
            None => {
                return Err(vec![EditorEvent::Error {
                    message: format!("Plan '{}' not found for execution", plan_id),
                }]);
            }
        };

        self.plan_manager
            .set_status(plan_id, EditPlanStatus::Running);

        self.events.push(EditorEvent::PlanExecutionStarted {
            plan_id: plan_id.to_string(),
        });

        Ok(plan)
    }

    /// Emit a StepStarted event.
    fn emit_step_started(&mut self, plan_id: &str, step: &EditPlanStep) {
        self.events.push(EditorEvent::StepStarted {
            plan_id: plan_id.to_string(),
            step_id: step.id.clone(),
            title: step.title.clone(),
        });
    }

    /// Set the final plan status and emit ExecutionCompleted.
    fn finish_plan_execution(&mut self, plan_id: &str, all_success: bool) {
        self.plan_manager.set_status(
            plan_id,
            if all_success {
                EditPlanStatus::Completed
            } else {
                EditPlanStatus::Failed
            },
        );

        self.events.push(EditorEvent::ExecutionCompleted {
            plan_id: plan_id.to_string(),
            success: all_success,
        });
    }

    /// Sprint 1: Execute plan using ReActAgent for each step (supports dynamic revision).
    async fn execute_plan_with_react(&mut self, plan_id: &str) -> Vec<EditorEvent> {
        let plan = match self.begin_plan_execution(plan_id) {
            Ok(p) => p,
            Err(err_events) => return err_events,
        };

        let mut all_success = true;
        let mut current_step_idx = 0;

        while current_step_idx < plan.steps.len() {
            let step = &plan.steps[current_step_idx];
            self.emit_step_started(plan_id, step);

            if let Some(ref mut react) = self.react_agent {
                let step_result = react.run(&step.title).await;

                match step_result {
                    Ok(result) => {
                        let result_clone = result.clone();
                        self.events.push(EditorEvent::StepCompleted {
                            plan_id: plan_id.to_string(),
                            step_id: step.id.clone(),
                            title: step.title.clone(),
                            result,
                        });

                        if let Some(revision) = self.dynamic_planner.analyze_observation(
                            &result_clone,
                            current_step_idx,
                            plan_id,
                        ) {
                            eprintln!(
                                "[DynamicPlanner] Revision at step {}: {}",
                                current_step_idx,
                                revision.describe()
                            );

                            if let Some(plan_mut) = self.plan_manager.get_mut(plan_id) {
                                if let Err(e) = self.dynamic_planner.apply_revision(
                                    plan_mut,
                                    revision,
                                    current_step_idx,
                                    &result_clone,
                                    true,
                                ) {
                                    eprintln!("[DynamicPlanner] Failed to apply revision: {}", e);
                                }
                            }

                            self.trace_entries.push(DirectorTraceEntry {
                                timestamp_ms: now_millis(),
                                actor: "DynamicPlanner".into(),
                                summary: format!(
                                    "Applied dynamic revision at step {} for plan '{}'",
                                    current_step_idx, plan_id
                                ),
                            });
                        }

                        current_step_idx += 1;
                    }
                    Err(e) => {
                        let error_msg = e.to_string();
                        self.events.push(EditorEvent::StepFailed {
                            plan_id: plan_id.to_string(),
                            step_id: step.id.clone(),
                            title: step.title.clone(),
                            error: error_msg.clone(),
                        });

                        if let Some(alternative) =
                            self.generate_alternative_step(&step.title, &error_msg)
                        {
                            eprintln!(
                                "[DirectorRuntime] Reflection: trying alternative: {}",
                                alternative
                            );
                            self.update_plan_step(plan_id, &step.id, &alternative);
                            continue;
                        }

                        all_success = false;
                        break;
                    }
                }
            } else {
                self.execute_plan_core(plan_id);
                return self.events.drain(..).collect();
            }
        }

        self.finish_plan_execution(plan_id, all_success);
        self.events.drain(..).collect()
    }

    /// Internal plan executor (public entry point — includes begin + core + finish).
    pub(crate) fn execute_plan_internal(&mut self, plan_id: &str) -> Vec<EditorEvent> {
        if self.begin_plan_execution(plan_id).is_err() {
            return self.events.drain(..).collect();
        }

        self.event_bus
            .push(crate::event::EventBusEvent::TransactionStarted {
                transaction_id: format!("txn_plan_{}", plan_id),
                step_id: "plan_execution".to_string(),
                task_id: 0,
            });

        self.execute_plan_core(plan_id);
        self.events.drain(..).collect()
    }

    /// Core step execution loop — uses self.events directly (caller manages begin/finish).
    fn execute_plan_core(&mut self, plan_id: &str) {
        let plan = match self.plan_manager.get(plan_id) {
            Some(p) => p.clone(),
            None => {
                self.events.push(EditorEvent::Error {
                    message: format!("Plan '{}' not found for core execution", plan_id),
                });
                return;
            }
        };

        let mut all_success = true;

        for (step_index, step) in plan.steps.iter().enumerate() {
            self.emit_step_started(plan_id, step);

            let txn_id = format!("txn_{}_{}", plan_id, step.id);
            self.events.push(EditorEvent::TransactionStarted {
                transaction_id: txn_id.clone(),
                step_id: step.id.clone(),
            });

            self.trace_entries.push(DirectorTraceEntry {
                timestamp_ms: now_millis(),
                actor: "TransactionStore".into(),
                summary: format!("Began transaction '{}' for step '{}'", txn_id, step.id),
            });

            let skill_def = self.lookup_skill_for_step(step);

            if let Some(ref bridge) = self.scene_bridge {
                let snapshot_infos = bridge.get_scene_snapshot();
                let entities: Vec<SnapshotEntity> = snapshot_infos
                    .iter()
                    .map(|info| SnapshotEntity {
                        name: info.name.clone(),
                        component_names: info.components.clone(),
                        serialized_state: serde_json::json!({
                            "translation": info.translation,
                            "sprite_color": info.sprite_color,
                        }),
                    })
                    .collect();
                let snapshot = self.rollback_manager.capture_snapshot(entities);
                self.rollback_manager.record(
                    None,
                    OperationType::Custom(step.title.clone()),
                    Vec::new(),
                    snapshot,
                );
            }

            let step_start = std::time::Instant::now();
            let execution_result: Result<String, String> =
                if let Some(ref mut bridge) = self.scene_bridge {
                    execute_step_via_bridge(
                        step,
                        skill_def.as_ref(),
                        bridge.as_mut(),
                        &mut self.skill_executor,
                    )
                } else {
                    Ok(format!(
                        "Simulated: '{}' (no SceneBridge connected)",
                        step.title
                    ))
                };

            let step_success = execution_result.is_ok();
            let step_error_msg = execution_result.as_ref().err().cloned();

            match &execution_result {
                Ok(summary) => {
                    self.metrics.record_tool_call(step_start.elapsed(), true);
                    self.trace_entries.push(DirectorTraceEntry {
                        timestamp_ms: now_millis(),
                        actor: "Executor".into(),
                        summary: summary.clone(),
                    });
                }
                Err(e) => {
                    self.metrics.record_tool_call(step_start.elapsed(), false);
                    self.trace_entries.push(DirectorTraceEntry {
                        timestamp_ms: now_millis(),
                        actor: "Executor".into(),
                        summary: format!("Execution failed: {}", e),
                    });
                }
            }

            let goal_ok = self.check_step_goals(step);

            let validation_ok = step_success && goal_ok;

            if validation_ok {
                self.events.push(EditorEvent::TransactionCommitted {
                    transaction_id: txn_id.clone(),
                });

                self.event_bus
                    .push(crate::event::EventBusEvent::EngineCommandApplied {
                        transaction_id: txn_id.clone(),
                        success: true,
                        message: format!("Step '{}' committed", step.id),
                    });

                self.trace_entries.push(DirectorTraceEntry {
                    timestamp_ms: now_millis(),
                    actor: "TransactionStore".into(),
                    summary: format!("Committed transaction '{}'", txn_id),
                });

                self.events.push(EditorEvent::StepCompleted {
                    plan_id: plan_id.to_string(),
                    step_id: step.id.clone(),
                    title: step.title.clone(),
                    result: "Success".to_string(),
                });

                let skill_name = skill_def
                    .as_ref()
                    .map(|s| s.name.as_str())
                    .unwrap_or(&step.title);
                self.compound
                    .record_success(skill_name, &step.title, "Success");

                // D1: VGRC check after successful mutation
                let _vgrc = self.run_vgrc_check(&[], &step.title);
            } else {
                let error_msg = step_error_msg.unwrap_or_else(|| "Validation failed".to_string());
                self.events.push(EditorEvent::StepFailed {
                    plan_id: plan_id.to_string(),
                    step_id: step.id.clone(),
                    title: step.title.clone(),
                    error: error_msg,
                });

                self.events.push(EditorEvent::TransactionRolledBack {
                    transaction_id: txn_id.clone(),
                });

                self.trace_entries.push(DirectorTraceEntry {
                    timestamp_ms: now_millis(),
                    actor: "TransactionStore".into(),
                    summary: format!(
                        "Rolled back transaction '{}' due to execution/validation failure",
                        txn_id
                    ),
                });

                self.record_failure_revision(plan_id, plan.task_id, step_index, step);

                all_success = false;
                break;
            }
        }

        self.finish_plan_execution(plan_id, all_success);

        self.trace_entries.push(DirectorTraceEntry {
            timestamp_ms: now_millis(),
            actor: "Director".into(),
            summary: format!(
                "Plan '{}' execution finished: {}",
                plan_id,
                if all_success { "success" } else { "failed" }
            ),
        });
    }

    /// Sprint 1-C1: Extract recent action descriptions from event history.
    pub(crate) fn extract_recent_actions_from_events(events: &[EditorEvent]) -> Vec<String> {
        let mut actions = Vec::new();

        for event in events.iter().rev().take(10) {
            match event {
                EditorEvent::StepCompleted { title, result, .. } => {
                    actions.push(format!("{}: {}", title, result));
                }
                EditorEvent::StepFailed { title, error, .. } => {
                    actions.push(format!("{} FAILED: {}", title, error));
                }
                EditorEvent::DirectExecutionCompleted {
                    request, success, ..
                } => {
                    let status = if *success { "OK" } else { "FAIL" };
                    actions.push(format!("{} {}", status, request));
                }
                _ => {}
            }
        }

        actions.reverse();
        actions.truncate(5);
        actions
    }

    fn check_step_goals(&mut self, step: &EditPlanStep) -> bool {
        if !self.goal_checker_enabled {
            return true;
        }
        if let Some(ref bridge) = self.scene_bridge {
            let snapshot = bridge.get_scene_snapshot();
            let checker = crate::goal_checker::GoalChecker::new();
            let reqs = self.build_step_requirements(step);
            let result = checker.check(&reqs, &snapshot);
            if result.all_matched {
                self.trace_entries.push(DirectorTraceEntry {
                    timestamp_ms: now_millis(),
                    actor: "GoalChecker".into(),
                    summary: format!("Goal check passed for step '{}'", step.id),
                });
                self.event_bus
                    .push(crate::event::EventBusEvent::GoalChecked {
                        task_id: 0,
                        all_matched: true,
                        summary: format!("Step '{}': all goals matched", step.id),
                    });
                true
            } else {
                let failures: Vec<String> = result
                    .requirement_results
                    .iter()
                    .filter(|r| !r.matched)
                    .map(|r| format!("{}: {:?}", r.description, r.message))
                    .collect();
                self.trace_entries.push(DirectorTraceEntry {
                    timestamp_ms: now_millis(),
                    actor: "GoalChecker".into(),
                    summary: format!("Goal check failed: {}", failures.join("; ")),
                });
                self.event_bus
                    .push(crate::event::EventBusEvent::GoalChecked {
                        task_id: 0,
                        all_matched: false,
                        summary: failures.join("; "),
                    });
                false
            }
        } else {
            self.trace_entries.push(DirectorTraceEntry {
                timestamp_ms: now_millis(),
                actor: "GoalChecker".into(),
                summary: "Goal check skipped (no SceneBridge, MVP mode)".into(),
            });
            true
        }
    }

    fn record_failure_revision(
        &mut self,
        plan_id: &str,
        task_id: u64,
        step_index: usize,
        step: &EditPlanStep,
    ) {
        let Some(error) = self.events.iter().rev().find_map(|event| match event {
            EditorEvent::StepFailed { error, .. } => Some(error.clone()),
            _ => None,
        }) else {
            return;
        };

        let classification = self.reflection_engine.classify_error(&error);
        let reflection =
            self.reflection_engine
                .generate_reflection(&step.title, &error, &classification);
        let reflection_preview: String = reflection.chars().take(200).collect();
        self.trace_entries.push(DirectorTraceEntry {
            timestamp_ms: now_millis(),
            actor: "ReflectionEngine".into(),
            summary: format!(
                "Step '{}' failed; reflection: {}",
                step.title, reflection_preview
            ),
        });

        let mut notes = Vec::new();
        let mut applied_revision = false;
        if let Some(revision) = self
            .dynamic_planner
            .analyze_observation(&error, step_index, plan_id)
        {
            let revision_desc = revision.describe();
            if revision.is_safe_auto_apply() {
                if let Some(plan) = self.plan_manager.get_mut(plan_id) {
                    match self
                        .dynamic_planner
                        .apply_revision(plan, revision, step_index, &error, true)
                    {
                        Ok(()) => {
                            applied_revision = true;
                            notes.push(format!("auto-applied revision: {}", revision_desc));
                            self.trace_entries.push(DirectorTraceEntry {
                                timestamp_ms: now_millis(),
                                actor: "DynamicPlanner".into(),
                                summary: format!(
                                    "Auto-applied revision to plan '{}': {}",
                                    plan_id, revision_desc
                                ),
                            });
                        }
                        Err(apply_error) => {
                            notes.push(format!(
                                "revision suggested but apply failed: {} ({})",
                                revision_desc, apply_error
                            ));
                            self.trace_entries.push(DirectorTraceEntry {
                                timestamp_ms: now_millis(),
                                actor: "DynamicPlanner".into(),
                                summary: format!(
                                    "Failed to apply revision to plan '{}': {} ({})",
                                    plan_id, revision_desc, apply_error
                                ),
                            });
                        }
                    }
                }
            } else {
                notes.push(format!("revision suggested: {}", revision_desc));
                self.trace_entries.push(DirectorTraceEntry {
                    timestamp_ms: now_millis(),
                    actor: "DynamicPlanner".into(),
                    summary: format!(
                        "Suggested revision for plan '{}': {}",
                        plan_id, revision_desc
                    ),
                });
            }
        }

        if !applied_revision {
            if let Some(alternative) = self.generate_alternative_step(&step.title, &error) {
                self.update_plan_step(plan_id, &step.id, &alternative);
                notes.push(format!("alternative step: {}", alternative));
                self.trace_entries.push(DirectorTraceEntry {
                    timestamp_ms: now_millis(),
                    actor: "ReflectionEngine".into(),
                    summary: format!(
                        "Updated failed step '{}' in plan '{}' to alternative '{}'",
                        step.id, plan_id, alternative
                    ),
                });
            }
        }

        let summary = if notes.is_empty() {
            format!(
                "Step '{}' failed: {}. No automatic revision was available.",
                step.title, error
            )
        } else {
            format!(
                "Step '{}' failed: {}. {}",
                step.title,
                error,
                notes.join("; ")
            )
        };

        self.events.push(EditorEvent::ReviewCompleted {
            task_id,
            decision: "needs_revision".to_string(),
            summary,
        });
    }
}
