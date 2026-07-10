use super::super::types::{DirectorRuntime, DirectorTraceEntry, EditorEvent};
use crate::memory::MemoryQuery;
use crate::permission::PermissionRequirement;
use crate::plan::{EditPlanStatus, EditPlanStep, ExecutionMode, TargetModule};
use crate::planner::{PlannerContext, RuleBasedPlanner};
use crate::types::now_millis;

impl DirectorRuntime {
    /// Execute a request using LLM + ReAct strategy (§2.2, §5.4).
    ///
    /// Sprint 1 (FIXED): Now truly uses ReActAgent for think-act-observe loop.
    ///
    /// ## Behavior
    ///
    /// - If **Tokio runtime available** + **ReActAgent configured**:
    ///   Spawns async ReAct execution and returns "thinking..." immediately.
    ///   Results stream via `self.events` and `self.event_bus`.
    ///
    /// - If **no Tokio runtime** but **ReActAgent configured**:
    ///   Falls back to synchronous execution via `handle_user_request_async`.
    ///
    /// - If **no ReActAgent**:
    ///   Uses FallbackEngine (keyword matching) for backward compatibility.
    ///
    /// Returns a human-readable response string.
    pub fn execute_with_llm(&mut self, request_text: &str) -> String {
        let start = std::time::Instant::now();
        let task_id = self.plan_manager.allocate_task_id();

        self.trace_entries.push(DirectorTraceEntry {
            timestamp_ms: now_millis(),
            actor: "LlmExecutor".into(),
            summary: format!("LLM requested for: {}", request_text),
        });

        if self.react_agent.is_some() {
            let rt = tokio::runtime::Handle::try_current();
            match rt {
                Ok(handle) => {
                    eprintln!(
                        "[ReActExecutor] Tokio runtime detected, executing ReAct synchronously for: {}",
                        request_text
                    );

                    let mut placeholder = DirectorRuntime::default();
                    std::mem::swap(self, &mut placeholder);

                    let result: Vec<EditorEvent> = handle.block_on(async {
                        placeholder.handle_user_request_async(request_text).await
                    });

                    std::mem::swap(self, &mut placeholder);

                    self.metrics.record_tool_call(start.elapsed(), true);
                    self.trace_entries.push(DirectorTraceEntry {
                        timestamp_ms: now_millis(),
                        actor: "ReActAgent".into(),
                        summary: "ReAct execution completed on Tokio runtime".into(),
                    });

                    return match result.is_empty() {
                        true => "ReAct execution completed (check events for details)".to_string(),
                        false => {
                            let last_event = result
                                .last()
                                .map(|e| match e {
                                    EditorEvent::DirectExecutionCompleted { success, .. } => {
                                        if *success {
                                            "ReAct execution succeeded".to_string()
                                        } else {
                                            "ReAct execution failed".to_string()
                                        }
                                    }
                                    _ => "ReAct execution finished".to_string(),
                                })
                                .unwrap_or_else(|| "ReAct execution finished".to_string());
                            last_event
                        }
                    };
                }
                Err(_) => {
                    eprintln!("[DirectorRuntime] No Tokio runtime, using sync ReAct execution");

                    let rt = tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                        .unwrap();

                    let mut placeholder = DirectorRuntime::default();
                    std::mem::swap(self, &mut placeholder);

                    let result = rt.block_on(async {
                        placeholder.handle_user_request_async(request_text).await
                    });

                    std::mem::swap(self, &mut placeholder);

                    return match result.is_empty() {
                        true => "ReAct execution completed (check events for details)".to_string(),
                        false => {
                            let last_event = result
                                .last()
                                .map(|e| match e {
                                    EditorEvent::DirectExecutionCompleted { success, .. } => {
                                        if *success {
                                            "ReAct execution succeeded".to_string()
                                        } else {
                                            "ReAct execution failed".to_string()
                                        }
                                    }
                                    _ => "ReAct execution finished".to_string(),
                                })
                                .unwrap_or_else(|| "ReAct execution finished".to_string());
                            last_event
                        }
                    };
                }
            }
        }

        let response = self.fallback_engine.execute(request_text, task_id);
        let answer = match &response {
            crate::fallback::FallbackResult::TemplateApplied { description, .. } => {
                description.clone()
            }
            crate::fallback::FallbackResult::RuleMatched { rule_name, .. } => rule_name.clone(),
            crate::fallback::FallbackResult::LlmUnavailable { suggestion, .. } => {
                suggestion.clone()
            }
        };

        self.metrics.record_tool_call(start.elapsed(), true);

        self.trace_entries.push(DirectorTraceEntry {
            timestamp_ms: now_millis(),
            actor: "LlmExecutor".into(),
            summary: format!("LLM Fallback: {:?}", response),
        });

        let events = std::mem::take(&mut self.events);
        let mut event_bridge = std::mem::take(&mut self.event_bridge);
        event_bridge.process_events(&events, self.memory());
        let processed = event_bridge.events_processed;
        if processed > 0 {
            eprintln!("[EventBridge] {}", event_bridge.stats());
        }
        self.event_bridge = event_bridge;
        self.events = events;

        self.memory_injector
            .add_conversation_turn(request_text, &answer);

        answer
    }

    /// Synchronous handler that uses SmartRouter for mode selection.
    ///
    /// For full LLM-driven execution (ReAct + CoT), use `handle_user_request_async`.
    pub fn handle_user_request(&mut self, request_text: &str) -> Vec<EditorEvent> {
        let start = std::time::Instant::now();

        // Auto-compress memory if conversation turns exceed threshold
        self.auto_compress_memory();

        // Record in memory (default agent ID 0 for director)
        let default_id = crate::memory::AgentMemoryId(0);
        let mem = self.memory_registry.get_mut(default_id);
        mem.system.record_user_request(request_text, None);
        mem.system.set_intent(request_text);
        self.memory_injector
            .inject(&format!("user_request: {}", request_text), request_text);

        // Rule system check — block denied actions before SmartRouter dispatch
        let rule_result = self.check_rule(request_text);
        if let crate::rule_system::RuleResult::Denied { reason } = rule_result {
            let error_event = EditorEvent::Error {
                message: format!("Blocked by rule system: {}", reason),
            };
            self.events.push(error_event.clone());
            self.trace_entries.push(DirectorTraceEntry {
                timestamp_ms: now_millis(),
                actor: "RuleSystem".into(),
                summary: format!("Blocked: {}", reason),
            });
            return vec![error_event];
        }
        if matches!(
            rule_result,
            crate::rule_system::RuleResult::NeedsConfirmation
        ) {
            self.trace_entries.push(DirectorTraceEntry {
                timestamp_ms: now_millis(),
                actor: "RuleSystem".into(),
                summary: format!("Needs confirmation: {}", request_text),
            });
        }

        // Determine LLM mode: emit ModeChanged if needed, but always route through SmartRouter
        let llm_mode = self.check_llm_mode();
        if llm_mode == "rule" {
            if self.previous_llm_mode != "rule" {
                self.events.push(EditorEvent::ModeChanged {
                    mode: "rule".to_string(),
                });
                self.trace_entries.push(DirectorTraceEntry {
                    timestamp_ms: now_millis(),
                    actor: "HybridController".into(),
                    summary: "LLM unavailable, using RuleBasedPlanner for plan generation".into(),
                });
            }
            self.previous_llm_mode = "rule".to_string();
        } else if self.previous_llm_mode == "rule" {
            self.events.push(EditorEvent::ModeChanged {
                mode: "llm".to_string(),
            });
            self.trace_entries.push(DirectorTraceEntry {
                timestamp_ms: now_millis(),
                actor: "HybridController".into(),
                summary: "LLM recovered, routing to SmartRouter".into(),
            });
            self.previous_llm_mode = "llm".to_string();
        }

        // SmartRouter decision (LLM semantic when available, keyword fallback)
        let llm_client: Option<&dyn crate::llm::LlmClient> =
            self.react_agent.as_ref().map(|a| a.llm.as_ref());
        let decision = crate::router::SmartRouter::route_with_llm(request_text, llm_client);
        self.trace_entries.push(DirectorTraceEntry {
            timestamp_ms: now_millis(),
            actor: "SmartRouter".into(),
            summary: decision.reason.clone(),
        });
        self.metrics.record_thinking(start.elapsed());

        match decision.mode {
            ExecutionMode::Direct => {
                if matches!(
                    decision.risk,
                    crate::permission::OperationRisk::HighRisk
                        | crate::permission::OperationRisk::Destructive
                ) {
                    self.events.push(EditorEvent::Error {
                        message: format!(
                            "High-risk request '{}' requires Plan mode; direct execution blocked",
                            request_text
                        ),
                    });
                    self.auto_save_memory();
                    self.recent_events_internal(1)
                } else {
                    let planner = RuleBasedPlanner::new();
                    let task_id = self.plan_manager.allocate_task_id();
                    let context = PlannerContext {
                        task_id,
                        available_tools: vec![
                            "create_entity".into(),
                            "update_component".into(),
                            "delete_entity".into(),
                            "query_entities".into(),
                        ],
                        scene_entity_names: vec![],
                        memory_context: None,
                    };
                    let plan = planner.create_plan(request_text, task_id, context);
                    let plan_id = plan.id.clone();
                    self.plan_manager.insert(plan_id.clone(), plan.clone());

                    let exec_events = self.execute_plan_internal(&plan_id);
                    let success = exec_events
                        .iter()
                        .all(|e| !matches!(e, EditorEvent::StepFailed { .. }));

                    for event in &exec_events {
                        match event {
                            EditorEvent::StepCompleted { title, result, .. } => {
                                self.reasoning_bank
                                    .bank_mut()
                                    .record_trace(title, result, true);
                            }
                            EditorEvent::StepFailed { title, error, .. } => {
                                self.reasoning_bank
                                    .bank_mut()
                                    .record_trace(title, error, false);
                            }
                            _ => {}
                        }
                    }

                    self.trace_entries.push(DirectorTraceEntry {
                        timestamp_ms: now_millis(),
                        actor: "Planner".into(),
                        summary: format!("Direct plan created: {}", plan.title),
                    });

                    let mut result = vec![EditorEvent::DirectExecutionStarted {
                        request: request_text.to_string(),
                        mode: "Direct".to_string(),
                        complexity_score: decision.complexity.total_score,
                    }];
                    result.extend(exec_events);
                    result.push(EditorEvent::DirectExecutionCompleted {
                        request: request_text.to_string(),
                        success,
                    });

                    self.events.extend(result.clone());

                    // D1: VGRC check after direct execution
                    let _vgrc = self.run_vgrc_check(&[], request_text);

                    self.auto_save_memory();
                    self.recent_events_internal(result.len())
                }
            }
            ExecutionMode::Plan | ExecutionMode::Team => {
                let task_id = self.plan_manager.allocate_task_id();
                let mem_ctx = self
                    .memory_registry
                    .get_system(default_id)
                    .cloned()
                    .map(|mut sys| sys.build_context(&MemoryQuery::new(request_text)));
                let context = PlannerContext {
                    task_id,
                    available_tools: vec![
                        "create_entity".into(),
                        "update_component".into(),
                        "delete_entity".into(),
                        "query_entities".into(),
                    ],
                    scene_entity_names: vec![],
                    memory_context: mem_ctx.clone(),
                };
                let mut plan = self
                    .plan_manager
                    .create_plan(request_text, context, mem_ctx);

                // Fallback for empty plans
                if plan.steps.is_empty() {
                    let fallback = self.fallback_engine.execute(request_text, plan.task_id);
                    if fallback.is_ok() {
                        let desc = match &fallback {
                            crate::fallback::FallbackResult::TemplateApplied {
                                description,
                                ..
                            } => description.clone(),
                            crate::fallback::FallbackResult::RuleMatched { rule_name, .. } => {
                                format!("rule: {}", rule_name)
                            }
                            _ => "fallback".into(),
                        };
                        plan = crate::plan::EditPlan {
                            id: format!("plan_fallback_{}", plan.task_id),
                            task_id: plan.task_id,
                            title: format!("Fallback: {}", desc),
                            summary: format!("Fallback-generated plan for: {}", request_text),
                            mode: ExecutionMode::Direct,
                            steps: vec![EditPlanStep {
                                id: "step_fb_1".into(),
                                title: desc,
                                target_module: TargetModule::Scene,
                                action_description: format!("Fallback: {}", request_text),
                                risk: crate::permission::OperationRisk::LowRisk,
                                validation_requirements: vec![],
                            }],
                            risk_level: crate::permission::OperationRisk::LowRisk,
                            status: EditPlanStatus::Draft,
                        };
                    }
                }

                let plan_id = plan.id.clone();
                let step_count = plan.steps.len();
                self.plan_manager.insert(plan_id.clone(), plan.clone());
                self.trace_entries.push(DirectorTraceEntry {
                    timestamp_ms: now_millis(),
                    actor: "Planner".into(),
                    summary: format!("Plan created: {}", plan.title),
                });

                let mut result = vec![EditorEvent::EditPlanCreated {
                    plan_id: plan_id.clone(),
                    title: plan.title.clone(),
                    risk: format!("{:?}", plan.risk_level),
                    mode: format!("{:?}", decision.mode),
                    steps_count: step_count,
                }];

                let permission = self.plan_manager.check_permission(&plan_id);
                match permission {
                    PermissionRequirement::AutoApproved => {
                        self.plan_manager
                            .set_status(&plan_id, EditPlanStatus::Approved);
                        result.push(EditorEvent::PermissionResolved {
                            plan_id: plan_id.clone(),
                            approved: true,
                            reason: None,
                        });
                        let exec_events = self.execute_plan_internal(&plan_id);
                        let success = exec_events
                            .iter()
                            .all(|e| !matches!(e, EditorEvent::StepFailed { .. }));
                        for event in &exec_events {
                            match event {
                                EditorEvent::StepCompleted { title, result, .. } => {
                                    self.reasoning_bank
                                        .bank_mut()
                                        .record_trace(title, result, true);
                                }
                                EditorEvent::StepFailed { title, error, .. } => {
                                    self.reasoning_bank
                                        .bank_mut()
                                        .record_trace(title, error, false);
                                }
                                _ => {}
                            }
                        }
                        result.extend(exec_events);
                        result.push(EditorEvent::ExecutionCompleted {
                            plan_id: plan_id.clone(),
                            success,
                        });
                    }
                    PermissionRequirement::NeedUserConfirmation { risk, reason } => {
                        self.plan_manager
                            .set_status(&plan_id, EditPlanStatus::WaitingForApproval);
                        self.plan_manager.add_pending(plan_id.clone());
                        result.push(EditorEvent::PermissionRequested {
                            plan_id: plan_id.clone(),
                            risk: format!("{:?}", risk),
                            reason,
                        });
                    }
                    PermissionRequirement::Forbidden { reason } => {
                        result.push(EditorEvent::Error {
                            message: format!("Plan blocked: {}", reason),
                        });
                    }
                }

                self.events.extend(result.clone());

                // D1: VGRC check after plan execution
                let _vgrc = self.run_vgrc_check(&[], request_text);

                self.auto_save_memory();
                self.recent_events_internal(result.len())
            }
        }
    }

    /// Async version of handle_user_request that uses LLM for planning.
    ///
    /// Sprint 1 enhancement: Uses ReActAgent for execution when available.
    /// Falls back to synchronous rule-based processing if:
    /// - No LLM client configured
    /// - ReActAgent not available
    /// - LLM request fails
    /// - LLM returns unparseable response
    pub async fn handle_user_request_async(&mut self, request_text: &str) -> Vec<EditorEvent> {
        if self.has_react_agent() {
            eprintln!("[DirectorRuntime] ReActAgent available, using ReAct execution");
            let result = self.execute_with_react(request_text).await;
            match result {
                Ok(events) => return events,
                Err(e) => {
                    eprintln!(
                        "[DirectorRuntime] ReAct execution failed ({}), falling back",
                        e
                    );
                }
            }
        }

        if !self.has_llm() {
            eprintln!("[DirectorRuntime] LLM not available, using synchronous fallback");
            return self.handle_user_request(request_text);
        }

        let start = std::time::Instant::now();

        match self.plan_with_llm(request_text).await {
            Ok(plan) => {
                self.metrics.record_thinking(start.elapsed());
                self.execute_plan_with_permission_and_react(plan).await
            }
            Err(e) => {
                eprintln!(
                    "[DirectorRuntime] LLM planning failed ({}), using fallback",
                    e
                );
                self.handle_user_request(request_text)
            }
        }
    }
}
