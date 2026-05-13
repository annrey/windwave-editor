//! Plan creation, routing, approval, and rejection methods for DirectorRuntime.

use crate::plan::{EditPlanStatus, ExecutionMode, EditPlanStep, TargetModule};
use crate::permission::PermissionRequirement;
use crate::types::now_millis;
use crate::planner::PlannerContext;
use super::types::{DirectorRuntime, EditorEvent, DirectorTraceEntry, ExecuteContext};

impl DirectorRuntime {
    /// Handle a user text request end-to-end with SmartRouter (§3.4).
    ///
    /// New flow:
    /// 1. **SmartRouter** analyzes request complexity/risk.
    /// 2. **Direct mode** (simple commands) → execute immediately.
    /// 3. **Plan mode** (complex/risky) → plan → permission → execute.
    ///
    /// Returns the list of `EditorEvent`s produced during this call.
    ///
    /// # Arguments
    ///
    /// * `request_text` - Natural-language description of what the user wants.
    ///
    /// # Example
    ///
    /// ```rust
    /// let mut runtime = agent_core::director::DirectorRuntime::new();
    /// let events = runtime.handle_user_request("创建一个红色敌人");
    /// assert!(!events.is_empty());
    /// ```
    pub fn handle_user_request(&mut self, request_text: &str) -> Vec<EditorEvent> {
        let start = std::time::Instant::now();

        // ---- Record user request in memory system ----
        self.memory_system.record_user_request(request_text, None);
        self.memory_system.set_intent(request_text);

        // ---- SmartRouter decision ----
        let decision = crate::router::SmartRouter::route(request_text);
        self.trace_entries.push(DirectorTraceEntry {
            timestamp_ms: now_millis(),
            actor: "SmartRouter".into(),
            summary: decision.reason.clone(),
        });
        self.metrics.record_thinking(start.elapsed());

        match decision.mode {
            ExecutionMode::Direct => {
                // Direct mode now checks permission for risky requests
                if matches!(decision.risk,
                    crate::permission::OperationRisk::HighRisk |
                    crate::permission::OperationRisk::Destructive
                ) {
                    self.events.push(EditorEvent::Error {
                        message: format!(
                            "High-risk request '{}' requires Plan mode for safety; direct execution blocked",
                            request_text
                        ),
                    });
                    self.recent_events_internal(1)
                } else {
                    let events = self.execute_direct_internal(request_text, &decision);
                    self.events.extend(events.clone());
                    self.recent_events_internal(events.len())
                }
            }
            ExecutionMode::Team => {
                let mem_ctx = self.memory_system.build_context(&crate::memory::MemoryQuery::new(request_text));
                let context = PlannerContext {
                    task_id: self.plan_manager.allocate_task_id(),
                    available_tools: vec![
                        "create_entity".into(),
                        "update_component".into(),
                        "delete_entity".into(),
                        "query_entities".into(),
                    ],
                    scene_entity_names: Vec::new(),
                    memory_context: Some(mem_ctx.clone()),
                };
                let mut plan = self.plan_manager.create_plan(request_text, context, Some(mem_ctx));

                // Fallback: if planner produced empty steps, try fallback engine
                if plan.steps.is_empty() {
                    let fallback = self.fallback_engine.execute(request_text, plan.task_id);
                    if fallback.is_ok() {
                        let desc = match &fallback {
                            crate::fallback::FallbackResult::TemplateApplied { description, .. } => description.clone(),
                            crate::fallback::FallbackResult::RuleMatched { rule_name, .. } => format!("rule: {}", rule_name),
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
                                validation_requirements: Vec::new(),
                            }],
                            risk_level: crate::permission::OperationRisk::LowRisk,
                            status: EditPlanStatus::Draft,
                        };
                    }
                }

                let plan_id = plan.id.clone();
                let step_count = plan.steps.len();
                self.plan_manager.insert(plan_id.clone(), plan.clone());

                self.events.push(EditorEvent::EditPlanCreated {
                    plan_id: plan_id.clone(),
                    title: plan.title.clone(),
                    risk: format!("{:?}", plan.risk_level),
                    mode: "Team".to_string(),
                    steps_count: step_count,
                });

                if let Some(ref mut registry) = self.agent_registry {
                    let event_count = if let Some(team_plan) = self.plan_manager.get(&plan_id).cloned() {
                        // Share plan context via CommunicationHub for agent awareness
                        let _ = self.comm_hub.share_context(
                            crate::registry::AgentId::default(),
                            format!("plan:{}", plan_id),
                            serde_json::to_value(&team_plan).unwrap_or_default(),
                        );

                        let result = registry.dispatch_team_plan_sync(&team_plan);

                        let mut team_events = Vec::new();
                        for sr in &result.step_results {
                            if sr.success {
                                team_events.push(EditorEvent::StepCompleted {
                                    plan_id: plan_id.clone(),
                                    step_id: sr.step_id.clone(),
                                    title: sr.step_id.clone(),
                                    result: format!("{:?}", sr.result),
                                });
                            } else {
                                team_events.push(EditorEvent::StepFailed {
                                    plan_id: plan_id.clone(),
                                    step_id: sr.step_id.clone(),
                                    title: sr.step_id.clone(),
                                    error: sr.error.clone().unwrap_or_default(),
                                });
                            }
                        }

                        let success = result.step_results.iter().all(|s| s.success);
                        self.plan_manager.set_status(&plan_id, EditPlanStatus::Completed);
                        self.events.push(EditorEvent::ExecutionCompleted {
                            plan_id: plan_id.clone(),
                            success,
                        });
                        self.events.extend(team_events);
                        // Count: step_results + ExecutionCompleted
                        result.step_results.len() + 1
                    } else {
                        self.events.push(EditorEvent::Error {
                            message: format!("Team mode: plan {} not found after creation", plan_id),
                        });
                        1
                    };
                    self.recent_events_internal(event_count)
                } else {
                    // Fallback: no AgentRegistry → use Plan mode's permission pipeline
                    self.events.push(EditorEvent::Error {
                        message: "Team mode: no AgentRegistry available, falling back to Plan".into(),
                    });
                    let permission = self.plan_manager.check_permission(&plan_id);
                    let count = match permission {
                        PermissionRequirement::AutoApproved => {
                            self.plan_manager.set_status(&plan_id, EditPlanStatus::Approved);
                            self.events.push(EditorEvent::PermissionResolved {
                                plan_id: plan_id.clone(), approved: true, reason: None,
                            });
                            let exec_events = self.execute_plan_internal(&plan_id);
                            let n = exec_events.len();
                            self.events.extend(exec_events);
                            n + 1
                        }
                        PermissionRequirement::NeedUserConfirmation { risk, reason } => {
                            self.events.push(EditorEvent::PermissionRequested {
                                plan_id: plan_id.clone(),
                                risk: format!("{:?}", risk),
                                reason: reason.clone(),
                            });
                            self.plan_manager.add_pending(plan_id.clone());
                            self.plan_manager.set_status(&plan_id, EditPlanStatus::WaitingForApproval);
                            1
                        }
                        PermissionRequirement::Forbidden { reason } => {
                            self.events.push(EditorEvent::Error {
                                message: format!("Plan forbidden: {}", reason),
                            });
                            self.plan_manager.set_status(&plan_id, EditPlanStatus::Rejected);
                            self.plan_manager.remove_pending(&plan_id);
                            1
                        }
                    };
                    self.recent_events_internal(count)
                }
            }
            ExecutionMode::Plan => {
                // 1. Plan
                let mem_ctx = self.memory_system.build_context(&crate::memory::MemoryQuery::new(request_text));
                let context = PlannerContext {
                    task_id: self.plan_manager.allocate_task_id(),
                    available_tools: vec![
                        "create_entity".into(),
                        "update_component".into(),
                        "delete_entity".into(),
                        "query_entities".into(),
                    ],
                    scene_entity_names: Vec::new(),
                    memory_context: Some(mem_ctx.clone()),
                };
                let mut plan = self.plan_manager.create_plan(request_text, context, Some(mem_ctx));

                // Fallback: if planner produced empty steps, try fallback engine
                if plan.steps.is_empty() {
                    let fallback = self.fallback_engine.execute(request_text, plan.task_id);
                    if fallback.is_ok() {
                        let desc = match &fallback {
                            crate::fallback::FallbackResult::TemplateApplied { description, .. } => description.clone(),
                            crate::fallback::FallbackResult::RuleMatched { rule_name, .. } => format!("rule: {}", rule_name),
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
                                validation_requirements: Vec::new(),
                            }],
                            risk_level: crate::permission::OperationRisk::LowRisk,
                            status: EditPlanStatus::Draft,
                        };
                    }
                }

                let plan_id = plan.id.clone();

                self.metrics.record_thinking(start.elapsed());

                self.events.push(EditorEvent::EditPlanCreated {
                    plan_id: plan_id.clone(),
                    title: plan.title.clone(),
                    risk: format!("{:?}", plan.risk_level),
                    mode: format!("{:?}", plan.mode),
                    steps_count: plan.steps.len(),
                });

                self.trace_entries.push(DirectorTraceEntry {
                    timestamp_ms: now_millis(),
                    actor: "Planner".into(),
                    summary: format!(
                        "Created plan '{}' (mode={:?}, risk={:?}, {} steps)",
                        plan_id,
                        plan.mode,
                        plan.risk_level,
                        plan.steps.len()
                    ),
                });

                self.plan_manager.insert(plan_id.clone(), plan);

                // 2. Permission
                let permission = self.plan_manager.check_permission(&plan_id);
                match permission {
                    PermissionRequirement::AutoApproved => {
                        self.plan_manager.set_status(&plan_id, EditPlanStatus::Approved);

                        self.events.push(EditorEvent::PermissionResolved {
                            plan_id: plan_id.clone(),
                            approved: true,
                            reason: None,
                        });

                        let exec_events = self.execute_plan_internal(&plan_id);
                        self.events.extend(exec_events.clone());

                        self.recent_events_internal(exec_events.len() + 3)
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
                        self.plan_manager.remove_pending(&plan_id);
                        self.recent_events_internal(3)
                    }
                }
            }
        }
    }

    /// Execute a simple request directly (no plan → permission overhead).
    ///
    /// Used when SmartRouter selects `ExecutionMode::Direct`.
    /// Parses entity names, colors, and actions from the request text and
    /// dispatches them to the SceneBridge for immediate execution.
    pub(crate) fn execute_direct_internal(
        &mut self,
        request_text: &str,
        decision: &crate::router::RoutingDecision,
    ) -> Vec<EditorEvent> {
        let mut events = Vec::new();
        let step_start = std::time::Instant::now();

        // Parse execution context from request text
        let ctx = self.parse_execute_context(request_text);

        events.push(EditorEvent::DirectExecutionStarted {
            request: request_text.to_string(),
            mode: format!("{:?}", decision.mode),
            complexity_score: decision.complexity.total_score,
        });

        self.trace_entries.push(DirectorTraceEntry {
            timestamp_ms: now_millis(),
            actor: "DirectExecutor".into(),
            summary: format!(
                "Direct mode: action={}, entities={:?}, colors={:?}",
                ctx.action, ctx.entity_names, ctx.colors,
            ),
        });

        // Dispatch based on parsed action
        if let Some(ref mut bridge) = self.scene_bridge {
            match ctx.action.as_str() {
                "create" => {
                    for entity_name in &ctx.entity_names {
                        match bridge.create_entity(entity_name, None, &[]) {
                            Ok(id) => {
                                events.push(EditorEvent::StepCompleted {
                                    plan_id: "direct".into(),
                                    step_id: format!("create_{}", entity_name),
                                    title: format!("创建 {}", entity_name),
                                    result: format!("Created entity '{}' (id={})", entity_name, id),
                                });
                                self.trace_entries.push(DirectorTraceEntry {
                                    timestamp_ms: now_millis(),
                                    actor: "DirectExecutor".into(),
                                    summary: format!("Created entity '{}' (id={})", entity_name, id),
                                });
                                self.metrics.record_tool_call(step_start.elapsed(), true);
                            }
                            Err(e) => {
                                events.push(EditorEvent::StepFailed {
                                    plan_id: "direct".into(),
                                    step_id: format!("create_{}", entity_name),
                                    title: format!("创建 {}", entity_name),
                                    error: format!("Failed: {}", e),
                                });
                                self.metrics.record_tool_call(step_start.elapsed(), false);
                            }
                        }
                    }
                }
                "query" | "list" | "查询" | "列表" => {
                    let snapshot = bridge.get_scene_snapshot();
                    let entity_names: Vec<String> = snapshot
                        .iter()
                        .map(|e| e.name.clone())
                        .collect();
                    events.push(EditorEvent::StepCompleted {
                        plan_id: "direct".into(),
                        step_id: "query_scene".into(),
                        title: "查询场景".into(),
                        result: format!(
                            "Found {} entities: [{}]",
                            entity_names.len(),
                            entity_names.join(", ")
                        ),
                    });
                    self.metrics.record_tool_call(step_start.elapsed(), true);
                }
                _ => {
                    // Fallback: treat as create
                    for entity_name in &ctx.entity_names {
                        match bridge.create_entity(entity_name, None, &[]) {
                            Ok(id) => {
                                events.push(EditorEvent::StepCompleted {
                                    plan_id: "direct".into(),
                                    step_id: format!("create_{}", entity_name),
                                    title: format!("创建 {}", entity_name),
                                    result: format!("Created entity '{}' (id={})", entity_name, id),
                                });
                                self.metrics.record_tool_call(step_start.elapsed(), true);
                            }
                            Err(e) => {
                                events.push(EditorEvent::StepFailed {
                                    plan_id: "direct".into(),
                                    step_id: format!("create_{}", entity_name),
                                    title: format!("创建 {}", entity_name),
                                    error: format!("Failed: {}", e),
                                });
                                self.metrics.record_tool_call(step_start.elapsed(), false);
                            }
                        }
                    }
                }
            }
        } else {
            // MVP — no SceneBridge connected
            events.push(EditorEvent::StepCompleted {
                plan_id: "direct".into(),
                step_id: "simulated".into(),
                title: request_text.to_string(),
                result: "Simulated (no SceneBridge connected, MVP mode)".into(),
            });
            self.trace_entries.push(DirectorTraceEntry {
                timestamp_ms: now_millis(),
                actor: "DirectExecutor".into(),
                summary: "Direct execution simulated — no SceneBridge".into(),
            });
        }

        events.push(EditorEvent::DirectExecutionCompleted {
            request: request_text.to_string(),
            success: events.iter().all(|e| !matches!(e, EditorEvent::StepFailed { .. })),
        });

        // ---- Record execution result in memory system ----
        let success = events.iter().all(|e| !matches!(e, EditorEvent::StepFailed { .. }));
        self.memory_system.record_step(
            &format!("direct_execution: {}", request_text),
            &format!("Direct mode execution completed"),
            success,
            step_start.elapsed().as_millis() as u64,
        );

        events
    }

    /// Parse entity names, colors, and action types from raw request text.
    pub(crate) fn parse_execute_context(&self, text: &str) -> ExecuteContext {
        let lower = text.to_lowercase();
        let words: Vec<&str> = text.split_whitespace().collect();

        let action = if lower.contains("创建") || lower.contains("create") || lower.contains("生成") || lower.contains("spawn") {
            "create".to_string()
        } else if lower.contains("查询") || lower.contains("query") || lower.contains("列表") || lower.contains("list") {
            "query".to_string()
        } else if lower.contains("删除") || lower.contains("delete") || lower.contains("移除") || lower.contains("remove") {
            "delete".to_string()
        } else if lower.contains("移动") || lower.contains("move") || lower.contains("放置") || lower.contains("place") {
            "move".to_string()
        } else {
            "create".to_string()
        };

        // Extract colors
        let color_map = [
            ("红色", "red"), ("蓝色", "blue"), ("绿色", "green"),
            ("黄色", "yellow"), ("紫色", "purple"), ("白色", "white"), ("黑色", "black"),
            ("橙色", "orange"),
        ];
        let colors: Vec<String> = color_map.iter()
            .filter(|(cn, _)| lower.contains(cn))
            .map(|(_, en)| en.to_string())
            .collect();

        // Extract entity names (capitalized words)
        let entity_names: Vec<String> = words.iter()
            .filter(|w| {
                w.chars().next().map(|c| c.is_uppercase()).unwrap_or(false)
                    && w.len() > 1
            })
            .map(|w| w.to_string())
            .collect();

        let positions: Vec<String> = if lower.contains("左边") || lower.contains("left") {
            vec!["left".into()]
        } else if lower.contains("右边") || lower.contains("right") {
            vec!["right".into()]
        } else if lower.contains("上面") || lower.contains("above") {
            vec!["above".into()]
        } else if lower.contains("下面") || lower.contains("below") {
            vec!["below".into()]
        } else {
            vec![]
        };

        ExecuteContext {
            entity_names: if entity_names.is_empty() {
                vec!["entity".to_string()]
            } else {
                entity_names
            },
            colors,
            positions,
            action,
        }
    }

    /// Approve a pending plan and execute it.
    ///
    /// Returns the events produced during approval and execution.
    /// If the plan is not found or not pending, returns an error event.
    ///
    /// # Arguments
    ///
    /// * `plan_id` - The ID of the plan to approve.
    pub fn approve_plan(&mut self, plan_id: &str) -> Vec<EditorEvent> {
        match self.plan_manager.approve(plan_id) {
            Ok(()) => {}
            Err(id) => {
                return vec![EditorEvent::Error {
                    message: format!("Plan '{}' not found", id),
                }];
            }
        }

        self.events.push(EditorEvent::PermissionResolved {
            plan_id: plan_id.to_string(),
            approved: true,
            reason: None,
        });

        let exec_events = self.execute_plan_internal(plan_id);
        self.events.extend(exec_events.clone());

        let total = exec_events.len() + 1;
        self.recent_events_internal(total)
    }

    /// Reject a pending plan.
    ///
    /// Returns an error event if the plan is not found.
    ///
    /// # Arguments
    ///
    /// * `plan_id` - The ID of the plan to reject.
    /// * `reason` - Optional human-readable rejection reason.
    pub fn reject_plan(&mut self, plan_id: &str, reason: Option<&str>) -> Vec<EditorEvent> {
        match self.plan_manager.reject(plan_id) {
            Ok(()) => {}
            Err(id) => {
                return vec![EditorEvent::Error {
                    message: format!("Plan '{}' not found", id),
                }];
            }
        }

        self.events.push(EditorEvent::PermissionResolved {
            plan_id: plan_id.to_string(),
            approved: false,
            reason: reason.map(|s| s.to_string()),
        });

        self.recent_events_internal(2)
    }

    /// Execute an already-approved plan.
    ///
    /// If the plan is not in the Approved status, returns an error event.
    ///
    /// # Arguments
    ///
    /// * `plan_id` - The ID of the plan to execute.
    pub fn execute_plan(&mut self, plan_id: &str) -> Vec<EditorEvent> {
        match self.plan_manager.get(plan_id) {
            Some(plan) if plan.status == EditPlanStatus::Approved => {
                let events = self.execute_plan_internal(plan_id);
                self.events.extend(events.clone());
                events
            }
            Some(_) => {
                vec![EditorEvent::Error {
                    message: format!("Plan '{}' is not in Approved status", plan_id),
                }]
            }
            None => {
                vec![EditorEvent::Error {
                    message: format!("Plan '{}' not found", plan_id),
                }]
            }
        }
    }
}
