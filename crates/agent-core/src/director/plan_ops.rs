//! Plan execution and rollback operations for DirectorRuntime

use crate::director::types::EditorEvent;
use crate::director::{DirectorRuntime, DirectorTraceEntry};
use crate::rollback::SnapshotEntity;
use crate::types::now_millis;
use std::collections::HashSet;

impl DirectorRuntime {
    /// Execute a plan by its ID (must be in Approved status).
    pub fn execute_plan(&mut self, plan_id: &str) -> Vec<EditorEvent> {
        let status = self.plan_manager.get_status(plan_id);
        if status != crate::plan::EditPlanStatus::Approved {
            return vec![EditorEvent::Error {
                message: format!("Plan '{}' is not approved (status: {:?})", plan_id, status),
            }];
        }

        let plan = match self.plan_manager.get(plan_id) {
            Some(p) => p.clone(),
            None => {
                return vec![EditorEvent::Error {
                    message: format!("Plan '{}' not found", plan_id),
                }];
            }
        };

        let task_id = crate::squad::TaskId(plan.task_id);

        // S3.5: Start reasoning trace for this plan execution
        let trace_id = self.reasoning_bank.start_trace(
            task_id,
            plan.title.clone(),
            format!(
                "Executing plan '{}' with {} steps",
                plan_id,
                plan.steps.len()
            ),
        );

        // Record initial thinking step
        self.reasoning_bank.record_think(
            trace_id,
            "Plan analysis".to_string(),
            format!(
                "Plan '{}' has {} steps. Goal: {}",
                plan_id,
                plan.steps.len(),
                plan.title
            ),
        );

        // Execute the plan
        let events = self.execute_plan_internal(plan_id);

        // Record execution events as reasoning steps
        let mut all_success = true;
        for event in &events {
            match event {
                EditorEvent::Error { message } => {
                    all_success = false;
                    self.reasoning_bank.record_observe(
                        trace_id,
                        "Execution error".to_string(),
                        message.clone(),
                    );
                }
                EditorEvent::StepFailed {
                    plan_id: _,
                    step_id,
                    title,
                    error,
                } => {
                    all_success = false;
                    self.reasoning_bank.record_observe(
                        trace_id,
                        format!("Step failed: {}", title),
                        format!("{}: {}", step_id, error),
                    );
                }
                EditorEvent::TransactionCommitted { transaction_id } => {
                    self.reasoning_bank.record_act(
                        trace_id,
                        "Transaction committed".to_string(),
                        transaction_id.clone(),
                        "success".to_string(),
                        true,
                    );
                }
                EditorEvent::StepCompleted {
                    step_id,
                    title,
                    result,
                    ..
                } => {
                    self.reasoning_bank.record_act(
                        trace_id,
                        format!("Step completed: {}", title),
                        step_id.clone(),
                        result.clone(),
                        true,
                    );
                }
                EditorEvent::ExecutionCompleted { success, .. } => {
                    if !success {
                        all_success = false;
                    }
                }
                _ => {}
            }
        }

        // Complete the reasoning trace
        let tags: Vec<String> = if all_success {
            vec!["success".to_string(), "automated".to_string()]
        } else {
            vec!["partial_failure".to_string(), "needs_review".to_string()]
        };
        self.reasoning_bank
            .bank_mut()
            .mark_trace_complete(trace_id, all_success, tags);

        // S3.6: Post-execution reflection for error analysis
        if !all_success {
            let error_events: Vec<String> = events
                .iter()
                .filter_map(|e| match e {
                    EditorEvent::Error { message } => Some(message.clone()),
                    EditorEvent::StepFailed { error, .. } => Some(error.clone()),
                    _ => None,
                })
                .collect();

            if !error_events.is_empty() {
                for error_msg in &error_events {
                    let classification = self.reflection_engine.classify_error(error_msg);
                    let reflection = self.reflection_engine.generate_reflection(
                        &format!("execute_plan({})", plan_id),
                        error_msg,
                        &classification,
                    );
                    self.trace_entries.push(DirectorTraceEntry {
                        timestamp_ms: now_millis(),
                        actor: "ReflectionEngine".into(),
                        summary: format!(
                            "Reflection on plan '{}': {}...",
                            plan_id,
                            &reflection[..reflection.len().min(200)]
                        ),
                    });
                }
            }
        }

        // S3.7: Dynamic planner feedback
        {
            let internal_revision_reviewed = events.iter().any(|event| {
                matches!(
                    event,
                    EditorEvent::ReviewCompleted { decision, .. } if decision == "needs_revision"
                )
            });
            if internal_revision_reviewed {
                return events;
            }

            let plan_id_str = plan_id.to_string();
            for (i, event) in events.iter().enumerate() {
                let observation = match event {
                    EditorEvent::StepFailed { error, .. } => error.clone(),
                    EditorEvent::StepCompleted { result, .. } => result.clone(),
                    _ => continue,
                };

                if let Some(revision) =
                    self.dynamic_planner
                        .analyze_observation(&observation, i, &plan_id_str)
                {
                    if revision.is_safe_auto_apply() {
                        if let Some(plan) = self.plan_manager.get_mut(plan_id) {
                            let _ = self.dynamic_planner.apply_revision(
                                plan,
                                revision.clone(),
                                i,
                                &observation,
                                true,
                            );
                            self.trace_entries.push(DirectorTraceEntry {
                                timestamp_ms: now_millis(),
                                actor: "DynamicPlanner".into(),
                                summary: format!(
                                    "Auto-applied revision to plan '{}': {}",
                                    plan_id,
                                    revision.describe()
                                ),
                            });
                        }
                    } else {
                        self.trace_entries.push(DirectorTraceEntry {
                            timestamp_ms: now_millis(),
                            actor: "DynamicPlanner".into(),
                            summary: format!(
                                "Suggested revision for plan '{}': {} (requires user confirmation)",
                                plan_id,
                                revision.describe()
                            ),
                        });
                    }
                }
            }
        }

        events
    }

    /// Rollback a committed transaction by its ID.
    pub fn rollback_transaction(&mut self, transaction_id: &str) -> Vec<EditorEvent> {
        let mut snapshot_entities: Vec<SnapshotEntity> = Vec::new();

        if self.rollback_manager.can_undo() {
            if let Some(op) = self.rollback_manager.undo() {
                snapshot_entities = op.snapshot.entities.clone();

                self.trace_entries.push(DirectorTraceEntry {
                    timestamp_ms: now_millis(),
                    actor: "RollbackManager".into(),
                    summary: format!(
                        "Undo operation {} ({:?}), restoring {} snapshot entities",
                        op.id.0,
                        op.operation_type,
                        snapshot_entities.len()
                    ),
                });
            }
        }

        if !snapshot_entities.is_empty() {
            if let Some(ref mut bridge) = self.scene_bridge {
                let current = bridge.query_entities(None, None);
                let current_names: HashSet<String> =
                    current.iter().map(|e| e.name.clone()).collect();
                let snapshot_names: HashSet<String> =
                    snapshot_entities.iter().map(|e| e.name.clone()).collect();

                for entity in &current {
                    if !snapshot_names.contains(&entity.name) {
                        let _ = bridge.delete_entity(entity.id);
                    }
                }

                for snap_entity in &snapshot_entities {
                    if current_names.contains(&snap_entity.name) {
                        if let Some(existing) =
                            bridge.query_entities(Some(&snap_entity.name), None).first()
                        {
                            let _ = bridge.update_component(
                                existing.id,
                                "Transform",
                                std::collections::HashMap::from([(
                                    "serialized_state".to_string(),
                                    snap_entity.serialized_state.clone(),
                                )]),
                            );
                        }
                    } else {
                        let patches: Vec<crate::scene_bridge::ComponentPatch> = snap_entity
                            .component_names
                            .iter()
                            .map(|cn| crate::scene_bridge::ComponentPatch {
                                type_name: cn.clone(),
                                properties: std::collections::HashMap::new(),
                            })
                            .collect();
                        let _ = bridge.create_entity(&snap_entity.name, None, &patches);
                    }
                }

                self.trace_entries.push(DirectorTraceEntry {
                    timestamp_ms: now_millis(),
                    actor: "SceneBridge".into(),
                    summary: format!(
                        "Restored scene to snapshot ({} entities)",
                        snapshot_entities.len()
                    ),
                });
            }
        }

        let events = vec![EditorEvent::TransactionRolledBack {
            transaction_id: transaction_id.to_string(),
        }];
        self.events.extend(events.clone());

        self.trace_entries.push(DirectorTraceEntry {
            timestamp_ms: now_millis(),
            actor: "TransactionStore".into(),
            summary: format!("Rolled back transaction '{}'", transaction_id),
        });

        events
    }
}
