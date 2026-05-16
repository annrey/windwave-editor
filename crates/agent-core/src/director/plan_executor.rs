//! Plan execution pipeline — permission, ReAct execution, and internal step execution.

use crate::plan::{EditPlan, EditPlanStatus, TargetModule};
use crate::permission::PermissionRequirement;
use crate::types::now_millis;
use crate::rollback::{OperationType, SnapshotEntity};
use super::types::{DirectorRuntime, EditorEvent, DirectorTraceEntry, SceneBridgeSkillHandler};

impl DirectorRuntime {
    /// Sprint 1: Execute plan steps with ReActAgent support for dynamic revision.
    pub(crate) async fn execute_plan_with_permission_and_react(&mut self, plan: EditPlan) -> Vec<EditorEvent> {
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
                self.plan_manager.set_status(&plan_id, EditPlanStatus::Approved);
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
                self.plan_manager.set_status(&plan_id, EditPlanStatus::WaitingForApproval);
                self.recent_events_internal(3)
            }
            PermissionRequirement::Forbidden { reason } => {
                self.events.push(EditorEvent::Error {
                    message: format!("Plan forbidden: {}", reason),
                });
                self.plan_manager.set_status(&plan_id, EditPlanStatus::Rejected);
                self.recent_events_internal(3)
            }
        }
    }

    /// Sprint 1: Execute plan using ReActAgent for each step (supports dynamic revision).
    async fn execute_plan_with_react(&mut self, plan_id: &str) -> Vec<EditorEvent> {
        let mut events = Vec::new();

        let plan = match self.plan_manager.get(plan_id) {
            Some(p) => p.clone(),
            None => {
                events.push(EditorEvent::Error {
                    message: format!("Plan '{}' not found for execution", plan_id),
                });
                return events;
            }
        };

        self.plan_manager.set_status(plan_id, EditPlanStatus::Running);

        events.push(EditorEvent::PlanExecutionStarted {
            plan_id: plan_id.to_string(),
        });

        let mut all_success = true;
        let mut current_step_idx = 0;

        while current_step_idx < plan.steps.len() {
            let step = &plan.steps[current_step_idx];

            events.push(EditorEvent::StepStarted {
                plan_id: plan_id.to_string(),
                step_id: step.id.clone(),
                title: step.title.clone(),
            });

            if let Some(ref mut react) = self.react_agent {
                let step_result = react.run(&step.title).await;

                match step_result {
                    Ok(result) => {
                        let result_clone = result.clone();
                        events.push(EditorEvent::StepCompleted {
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
                                current_step_idx, revision.describe()
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
                        events.push(EditorEvent::StepFailed {
                            plan_id: plan_id.to_string(),
                            step_id: step.id.clone(),
                            title: step.title.clone(),
                            error: error_msg.clone(),
                        });

                        if let Some(alternative) = self.generate_alternative_step(&step.title, &error_msg) {
                            eprintln!("[DirectorRuntime] Reflection: trying alternative: {}", alternative);
                            self.update_plan_step(plan_id, &step.id, &alternative);
                            continue;
                        }

                        all_success = false;
                        break;
                    }
                }
            } else {
                let exec_events = self.execute_plan_internal(plan_id);
                events.extend(exec_events);
                break;
            }
        }

        self.plan_manager.set_status(
            plan_id,
            if all_success {
                EditPlanStatus::Completed
            } else {
                EditPlanStatus::Failed
            },
        );

        events.push(EditorEvent::ExecutionCompleted {
            plan_id: plan_id.to_string(),
            success: all_success,
        });

        events
    }

    /// Internal plan executor.
    ///
    /// Simulates step-by-step execution with transaction wrapping per step.
    pub(crate) fn execute_plan_internal(&mut self, plan_id: &str) -> Vec<EditorEvent> {
        let mut events = Vec::new();

        let plan = match self.plan_manager.get(plan_id) {
            Some(p) => p.clone(),
            None => {
                events.push(EditorEvent::Error {
                    message: format!("Plan '{}' not found for execution", plan_id),
                });
                return events;
            }
        };

        self.plan_manager.set_status(plan_id, EditPlanStatus::Running);

        events.push(EditorEvent::PlanExecutionStarted {
            plan_id: plan_id.to_string(),
        });

        self.event_bus.push(crate::event::EventBusEvent::TransactionStarted {
            transaction_id: format!("txn_plan_{}", plan_id),
            step_id: "plan_execution".to_string(),
            task_id: 0,
        });

        let mut all_success = true;

        for step in &plan.steps {
            events.push(EditorEvent::StepStarted {
                plan_id: plan_id.to_string(),
                step_id: step.id.clone(),
                title: step.title.clone(),
            });

            let txn_id = format!("txn_{}_{}", plan_id, step.id);
            events.push(EditorEvent::TransactionStarted {
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
            let execution_result: Result<String, String> = if let Some(ref mut bridge) =
                self.scene_bridge
            {
                match step.target_module {
                    TargetModule::Scene => {
                        if let Some(ref skill_def) = skill_def {
                            let mut handler = SceneBridgeSkillHandler {
                                bridge: bridge.as_mut(),
                            };
                            match self.skill_executor.execute_with_handler(
                                skill_def,
                                &mut handler,
                            ) {
                                Ok(results) => {
                                    let names: Vec<&str> =
                                        results.iter().map(|r| r.title.as_str()).collect();
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
                            let title_lower = step.title.to_lowercase();
                            let parts: Vec<&str> = step.title.split_whitespace().collect();

                            if parts.iter().any(|p| *p == "创建" || *p == "Create") {
                                let name = parts.last().unwrap_or(&"entity");
                                bridge
                                    .create_entity(name, None, &[])
                                    .map(|_id| format!("Created entity {}", name))
                                    .map_err(|e| format!("Failed to create entity: {}", e))
                            } else if title_lower.contains("删除") || title_lower.contains("delete")
                                || title_lower.contains("移除") || title_lower.contains("remove")
                            {
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
                            } else if title_lower.contains("移动") || title_lower.contains("move") {
                                let entities = bridge.query_entities(None, None);
                                let name = parts.last().unwrap_or(&"");
                                if let Some(entity) = entities.iter().find(|e| e.name == *name) {
                                    let mut props = std::collections::HashMap::new();
                                    props.insert(
                                        "position".into(),
                                        serde_json::json!([0.0, 0.0]),
                                    );
                                    bridge
                                        .update_component(entity.id, "Transform", props)
                                        .map(|()| format!("Moved entity {} to origin", name))
                                        .map_err(|e| format!("Failed to move entity: {}", e))
                                } else {
                                    Err(format!("Entity '{}' not found for move", name))
                                }
                            } else if title_lower.contains("改色") || title_lower.contains("颜色")
                                || title_lower.contains("color")
                            {
                                let entities = bridge.query_entities(None, None);
                                let name = parts.last().unwrap_or(&"");
                                if let Some(entity) = entities.iter().find(|e| e.name == *name) {
                                    let mut props = std::collections::HashMap::new();
                                    props.insert(
                                        "color".into(),
                                        serde_json::json!([1.0, 0.0, 0.0, 1.0]),
                                    );
                                    bridge
                                        .update_component(entity.id, "Sprite", props)
                                        .map(|()| format!("Changed color of entity {} to red", name))
                                        .map_err(|e| format!("Failed to change color: {}", e))
                                } else {
                                    Err(format!(
                                        "Entity '{}' not found for color change",
                                        name
                                    ))
                                }
                            } else if title_lower.contains("查询") || title_lower.contains("query")
                                || title_lower.contains("列出") || title_lower.contains("list")
                            {
                                let entities = bridge.query_entities(None, None);
                                let names: Vec<String> =
                                    entities.iter().map(|e| e.name.clone()).collect();
                                Ok(format!(
                                    "Scene entities ({}): {}",
                                    names.len(),
                                    names.join(", ")
                                ))
                            } else {
                                Err(format!(
                                    "No matching skill, keyword, or tool for step: '{}'",
                                    step.title
                                ))
                            }
                        }
                    }
                    _ => Ok(format!(
                        "Step '{}' executed (module {:?})",
                        step.title, step.target_module
                    )),
                }
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

            let goal_ok = if self.goal_checker_enabled {
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
                        self.event_bus.push(crate::event::EventBusEvent::GoalChecked {
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
                        self.event_bus.push(crate::event::EventBusEvent::GoalChecked {
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
            } else {
                true
            };

            let validation_ok = step_success && goal_ok;

            if validation_ok {
                events.push(EditorEvent::TransactionCommitted {
                    transaction_id: txn_id.clone(),
                });

                self.event_bus.push(crate::event::EventBusEvent::EngineCommandApplied {
                    transaction_id: txn_id.clone(),
                    success: true,
                    message: format!("Step '{}' committed", step.id),
                });

                self.trace_entries.push(DirectorTraceEntry {
                    timestamp_ms: now_millis(),
                    actor: "TransactionStore".into(),
                    summary: format!("Committed transaction '{}'", txn_id),
                });

                events.push(EditorEvent::StepCompleted {
                    plan_id: plan_id.to_string(),
                    step_id: step.id.clone(),
                    title: step.title.clone(),
                    result: "Success".to_string(),
                });
            } else {
                let error_msg = step_error_msg.unwrap_or_else(|| "Validation failed".to_string());
                events.push(EditorEvent::StepFailed {
                    plan_id: plan_id.to_string(),
                    step_id: step.id.clone(),
                    title: step.title.clone(),
                    error: error_msg,
                });

                events.push(EditorEvent::TransactionRolledBack {
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

                all_success = false;
                break;
            }
        }

        self.plan_manager.set_status(
            plan_id,
            if all_success {
                EditPlanStatus::Completed
            } else {
                EditPlanStatus::Failed
            },
        );

        events.push(EditorEvent::ExecutionCompleted {
            plan_id: plan_id.to_string(),
            success: all_success,
        });

        self.trace_entries.push(DirectorTraceEntry {
            timestamp_ms: now_millis(),
            actor: "Director".into(),
            summary: format!(
                "Plan '{}' execution finished: {}",
                plan_id,
                if all_success { "success" } else { "failed" }
            ),
        });

        events
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
                EditorEvent::DirectExecutionCompleted { request, success, .. } => {
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
}
