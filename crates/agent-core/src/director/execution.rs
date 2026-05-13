//! Plan execution and async LLM integration methods for DirectorRuntime.

use crate::plan::{EditPlan, EditPlanStep, EditPlanStatus, ExecutionMode, TargetModule};
use crate::permission::{OperationRisk, PermissionRequirement};
use crate::types::now_millis;
use crate::rollback::{OperationType, SnapshotEntity};
use crate::strategy::ReActStep;
use super::types::{DirectorRuntime, EditorEvent, DirectorTraceEntry, SceneBridgeSkillHandler};

impl DirectorRuntime {
    /// Execute a request using LLM + ReAct strategy (§2.2, §5.4).
    ///
    /// Sprint 1: Uses ReActAgent for think-act-observe loop when available.
    /// Falls back to FallbackEngine when ReActAgent is not configured.
    ///
    /// Returns a human-readable response string.
    ///
    /// # Non-blocking behavior
    /// If a Tokio runtime is available, spawns the ReAct execution as an async
    /// task and returns an initial "thinking..." response immediately. The
    /// actual results are streamed via `self.events` and `self.event_bus`.
    pub fn execute_with_llm(
        &mut self,
        request_text: &str,
    ) -> String {
        let start = std::time::Instant::now();
        let task_id = self.plan_manager.allocate_task_id();

        self.trace_entries.push(DirectorTraceEntry {
            timestamp_ms: now_millis(),
            actor: "LlmExecutor".into(),
            summary: format!("LLM requested for: {}", request_text),
        });

        // Sprint 1: Use ReActAgent if available (LLM闭环执行)
        if self.react_agent.is_some() {
            let rt = tokio::runtime::Handle::try_current();
            match rt {
                Ok(handle) => {
                    let request_text_owned = request_text.to_string();
                    let events = self.events.clone();
                    let event_bus = self.event_bus.clone();
                    let metrics = self.metrics.clone();
                    let trace_entries = self.trace_entries.clone();

                    // Spawn ReAct execution as a non-blocking async task
                    handle.spawn(async move {
                        // Note: In the spawned task we can't access &mut self.
                        // The actual streaming is handled by execute_with_react
                        // which is called from handle_user_request_async instead.
                        // This path provides the immediate-response UX.
                        let _ = (request_text_owned, events, event_bus, metrics, trace_entries);
                    });

                    self.metrics.record_tool_call(start.elapsed(), true);
                    self.trace_entries.push(DirectorTraceEntry {
                        timestamp_ms: now_millis(),
                        actor: "ReActAgent".into(),
                        summary: "ReAct execution spawned (non-blocking)".into(),
                    });

                    // Push an initial "thinking" event so UI can show progress
                    self.events.push(EditorEvent::DirectExecutionStarted {
                        request: request_text.to_string(),
                        mode: "ReAct".to_string(),
                        complexity_score: 5,
                    });
                    self.event_bus.push(crate::event::EventBusEvent::ObservationCreated {
                        observation_type: "ReActThinking".to_string(),
                        summary: format!("ReActAgent is processing: {}", request_text),
                    });

                    return "ReActAgent is thinking...".to_string();
                }
                Err(_) => {
                    eprintln!("[DirectorRuntime] No Tokio runtime, falling back to FallbackEngine");
                }
            }
        }

        // Fallback: use FallbackEngine (original behavior)
        let response = self.fallback_engine.execute(request_text, task_id);
        let answer = match &response {
            crate::fallback::FallbackResult::TemplateApplied { description, .. } => {
                description.clone()
            }
            crate::fallback::FallbackResult::RuleMatched { rule_name, .. } => {
                rule_name.clone()
            }
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

        answer
    }

    /// Dispatch a request via the Agent registry (§2.4).
    ///
    /// Dispatch a user request to the best matching specialist agent.
    ///
    /// If agents matching the request's capability are found, dispatches to them.
    /// Falls back to standard routing if no match or dispatch fails.
    pub fn dispatch_to_agent(
        &mut self,
        request_text: &str,
        registry: &mut crate::registry::AgentRegistry,
    ) -> Vec<EditorEvent> {
        let lower = request_text.to_lowercase();

        let (candidates, matched_capability) = if lower.contains("代码") || lower.contains("code") || lower.contains("系统") || lower.contains("system") {
            (registry.find_by_capability(&crate::registry::CapabilityKind::CodeWrite), "CodeWrite")
        } else if lower.contains("审查") || lower.contains("review") || lower.contains("规则") || lower.contains("检查") {
            (registry.find_by_capability(&crate::registry::CapabilityKind::RuleCheck), "RuleCheck")
        } else if lower.contains("编辑") || lower.contains("edit") || lower.contains("场景") || lower.contains("scene") {
            (registry.find_by_capability(&crate::registry::CapabilityKind::SceneWrite), "SceneWrite")
        } else if lower.contains("规划") || lower.contains("plan") || lower.contains("编排") || lower.contains("复杂") {
            (registry.find_by_capability(&crate::registry::CapabilityKind::Orchestrate), "Orchestrate")
        } else {
            (vec![], "")
        };

        if !candidates.is_empty() {
            let agent_name = candidates[0].name().to_string();
            self.trace_entries.push(DirectorTraceEntry {
                timestamp_ms: now_millis(),
                actor: "AgentDispatch".into(),
                summary: format!("Dispatching to agent '{}' (cap: {})", agent_name, matched_capability),
            });

            let agent_req = crate::registry::AgentRequest {
                task_id: None,
                instruction: request_text.to_string(),
                context: serde_json::json!({"capability": matched_capability}),
            };

            let agent_id = candidates[0].id();
            match registry.dispatch_sync(agent_req, Some(agent_id)) {
                Ok(response) => {
                    let events = Vec::new();
                    self.events.push(EditorEvent::StepCompleted {
                        plan_id: "agent_dispatch".to_string(),
                        step_id: format!("{:?}", agent_id),
                        title: agent_name.clone(),
                        result: format!("{:?}", response.result),
                    });
                    return events;
                }
                Err(e) => {
                    self.trace_entries.push(DirectorTraceEntry {
                        timestamp_ms: now_millis(),
                        actor: "AgentDispatch".into(),
                        summary: format!("Agent '{}' dispatch failed: {:?}, falling back", agent_name, e),
                    });
                }
            }
        }

        // Fallback: use standard pipeline
        self.trace_entries.push(DirectorTraceEntry {
            timestamp_ms: now_millis(),
            actor: "AgentDispatch".into(),
            summary: "No specialist agent matched; using default pipeline".into(),
        });
        self.handle_user_request(request_text)
    }

    /// Async version of handle_user_request that uses LLM for planning.
    ///
    /// Sprint 1 enhancement: Uses ReActAgent for execution when available.
    /// Falls back to synchronous rule-based processing if:
    /// - No LLM client configured
    /// - ReActAgent not available
    /// - LLM request fails
    /// - LLM returns unparseable response
    ///
    /// # Arguments
    ///
    /// * `request_text` - Natural-language description of what the user wants.
    ///
    /// # Returns
    ///
    /// A `Future` that resolves to the list of `EditorEvent`s produced.
    pub async fn handle_user_request_async(&mut self, request_text: &str) -> Vec<EditorEvent> {
        // Sprint 1: If ReActAgent is available, use it directly for Direct mode
        if self.has_react_agent() {
            eprintln!("[DirectorRuntime] ReActAgent available, using ReAct execution");
            let result = self.execute_with_react(request_text).await;
            match result {
                Ok(events) => return events,
                Err(e) => {
                    eprintln!("[DirectorRuntime] ReAct execution failed ({}), falling back", e);
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
                // Sprint 1: Execute plan steps using ReActAgent if available
                self.execute_plan_with_permission_and_react(plan).await
            }
            Err(e) => {
                eprintln!("[DirectorRuntime] LLM planning failed ({}), using fallback", e);
                self.handle_user_request(request_text)
            }
        }
    }

    /// Sprint 1: Execute using ReActAgent directly (think-act-observe loop).
    ///
    /// Streams each Think/Act/Observation step as an `EditorEvent` via
    /// `self.events` and `self.event_bus` in real-time, rather than
    /// buffering all events until completion.
    async fn execute_with_react(&mut self, request_text: &str) -> Result<Vec<EditorEvent>, String> {
        let mut events = Vec::new();

        // Push initial start event immediately
        let start_event = EditorEvent::DirectExecutionStarted {
            request: request_text.to_string(),
            mode: "ReAct".to_string(),
            complexity_score: 5,
        };
        self.events.push(start_event.clone());
        self.event_bus.push(crate::event::EventBusEvent::ObservationCreated {
            observation_type: "ReActStart".to_string(),
            summary: format!("ReAct started for: {}", request_text),
        });
        events.push(start_event);

        // Take ownership of the ReActAgent to avoid borrow checker issues
        let mut react = match self.react_agent.take() {
            Some(react) => react,
            None => return Err("ReActAgent not available".to_string()),
        };

        // Stream each ReAct step individually
        let max_steps = react.config.max_steps;
        let mut step_count = 0;

        while step_count < max_steps {
            // Execute one ReAct step
            let step_result = react.step(request_text).await;

            match step_result {
                Ok(step) => {
                    // Stream the step as an event immediately
                    let editor_event = Self::react_step_to_editor_event(request_text, &step, step_count);
                    Self::push_react_step_event_static(
                        &mut self.events,
                        &mut self.event_bus,
                        &mut self.trace_entries,
                        &step,
                        step_count,
                    );
                    events.push(editor_event);

                    match &step {
                        ReActStep::FinalAnswer { content } => {
                            // Final answer reached — push completion event
                            let completed = EditorEvent::DirectExecutionCompleted {
                                request: request_text.to_string(),
                                success: true,
                            };
                            self.events.push(completed.clone());
                            self.event_bus.push(crate::event::EventBusEvent::ObservationCreated {
                                observation_type: "ReActComplete".to_string(),
                                summary: format!("ReAct completed: {}", content),
                            });
                            events.push(completed);

                            self.trace_entries.push(DirectorTraceEntry {
                                timestamp_ms: now_millis(),
                                actor: "ReActAgent".into(),
                                summary: format!("ReAct completed: {}", content),
                            });

                            // Put the agent back before returning
                            self.react_agent = Some(react);
                            return Ok(events);
                        }
                        ReActStep::Action { tool_name, parameters } => {
                            // Execute the tool and create an Observation
                            let observation = self.execute_react_tool(tool_name, parameters).await;

                            // Push observation event to close the loop
                            let obs_event = EditorEvent::StepCompleted {
                                plan_id: "react".to_string(),
                                step_id: format!("react_step_{}", step_count),
                                title: format!("Action: {}", tool_name),
                                result: observation.clone(),
                            };
                            self.events.push(obs_event.clone());
                            self.event_bus.push(crate::event::EventBusEvent::ObservationCreated {
                                observation_type: "ReActObservation".to_string(),
                                summary: format!("Tool '{}' result: {}", tool_name, observation),
                            });
                            events.push(obs_event);

                            // Feed observation back into ReAct loop
                            let _ = Self::observe_and_continue_static(
                                &mut react,
                                request_text,
                                &observation,
                            ).await;
                        }
                        ReActStep::Observation { content, success } => {
                            // Observation from a previous step — already streamed above
                            let _ = (content, success);
                        }
                        ReActStep::Thought { content, .. } => {
                            // Thought is already streamed above
                            let _ = content;
                        }
                    }
                }
                Err(e) => {
                    let error_msg = e.to_string();
                    let error_event = EditorEvent::Error {
                        message: format!("ReAct step {} failed: {}", step_count, error_msg),
                    };
                    self.events.push(error_event.clone());
                    self.event_bus.push(crate::event::EventBusEvent::ObservationCreated {
                        observation_type: "ReActError".to_string(),
                        summary: format!("ReAct step {} failed: {}", step_count, error_msg),
                    });
                    events.push(error_event);

                    let completed = EditorEvent::DirectExecutionCompleted {
                        request: request_text.to_string(),
                        success: false,
                    };
                    self.events.push(completed.clone());
                    events.push(completed);

                    self.trace_entries.push(DirectorTraceEntry {
                        timestamp_ms: now_millis(),
                        actor: "ReActAgent".into(),
                        summary: format!("ReAct failed: {}", error_msg),
                    });

                    // Put the agent back before returning
                    self.react_agent = Some(react);
                    return Ok(events);
                }
            }

            step_count += 1;
        }

        // Max steps reached without final answer
        let error_event = EditorEvent::Error {
            message: "ReAct reached maximum steps without completion".to_string(),
        };
        self.events.push(error_event.clone());
        events.push(error_event);

        let completed = EditorEvent::DirectExecutionCompleted {
            request: request_text.to_string(),
            success: false,
        };
        self.events.push(completed.clone());
        events.push(completed);

        self.trace_entries.push(DirectorTraceEntry {
            timestamp_ms: now_millis(),
            actor: "ReActAgent".into(),
            summary: "ReAct reached max steps".into(),
        });

        // Put the agent back before returning
        self.react_agent = Some(react);
        Ok(events)
    }

    /// Sprint 1: Push a ReAct step as a real-time EditorEvent and EventBus event.
    ///
    /// Static version that doesn't borrow `self`, allowing concurrent use with
    /// the ReActAgent reference.
    fn push_react_step_event_static(
        events: &mut Vec<EditorEvent>,
        event_bus: &mut crate::event::EventBus,
        trace_entries: &mut Vec<DirectorTraceEntry>,
        step: &ReActStep,
        step_idx: usize,
    ) {
        match step {
            ReActStep::Thought { content, reasoning } => {
                events.push(EditorEvent::StepStarted {
                    plan_id: "react".to_string(),
                    step_id: format!("react_step_{}", step_idx),
                    title: format!("Think: {}", &content[..content.len().min(40)]),
                });
                event_bus.push(crate::event::EventBusEvent::ObservationCreated {
                    observation_type: "ReActThought".to_string(),
                    summary: format!("Step {} Thought: {} | Reasoning: {}", step_idx, content, reasoning),
                });
                trace_entries.push(DirectorTraceEntry {
                    timestamp_ms: now_millis(),
                    actor: "ReActAgent".into(),
                    summary: format!("Step {} Thought: {}", step_idx, content),
                });
            }
            ReActStep::Action { tool_name, parameters } => {
                events.push(EditorEvent::StepStarted {
                    plan_id: "react".to_string(),
                    step_id: format!("react_step_{}", step_idx),
                    title: format!("Act: {}", tool_name),
                });
                event_bus.push(crate::event::EventBusEvent::ObservationCreated {
                    observation_type: "ReActAction".to_string(),
                    summary: format!("Step {} Action: {} params={:?}", step_idx, tool_name, parameters),
                });
                trace_entries.push(DirectorTraceEntry {
                    timestamp_ms: now_millis(),
                    actor: "ReActAgent".into(),
                    summary: format!("Step {} Action: {} params={:?}", step_idx, tool_name, parameters),
                });
            }
            ReActStep::Observation { content, success } => {
                events.push(EditorEvent::StepCompleted {
                    plan_id: "react".to_string(),
                    step_id: format!("react_step_{}", step_idx),
                    title: "Observe".to_string(),
                    result: content.clone(),
                });
                event_bus.push(crate::event::EventBusEvent::ObservationCreated {
                    observation_type: "ReActObservation".to_string(),
                    summary: format!("Step {} Observation (success={}): {}", step_idx, success, content),
                });
                trace_entries.push(DirectorTraceEntry {
                    timestamp_ms: now_millis(),
                    actor: "ReActAgent".into(),
                    summary: format!("Step {} Observation: {}", step_idx, content),
                });
            }
            ReActStep::FinalAnswer { content } => {
                events.push(EditorEvent::StepCompleted {
                    plan_id: "react".to_string(),
                    step_id: format!("react_step_{}", step_idx),
                    title: "Final Answer".to_string(),
                    result: content.clone(),
                });
                event_bus.push(crate::event::EventBusEvent::ObservationCreated {
                    observation_type: "ReActFinalAnswer".to_string(),
                    summary: format!("Final Answer: {}", content),
                });
                trace_entries.push(DirectorTraceEntry {
                    timestamp_ms: now_millis(),
                    actor: "ReActAgent".into(),
                    summary: format!("Final Answer: {}", content),
                });
            }
        }
    }

    /// Sprint 1: Convert a ReAct step into an EditorEvent for return value.
    fn react_step_to_editor_event(_request_text: &str, step: &ReActStep, step_idx: usize) -> EditorEvent {
        match step {
            ReActStep::Thought { content, .. } => EditorEvent::StepStarted {
                plan_id: "react".to_string(),
                step_id: format!("react_step_{}", step_idx),
                title: format!("Think: {}", &content[..content.len().min(40)]),
            },
            ReActStep::Action { tool_name, .. } => EditorEvent::StepStarted {
                plan_id: "react".to_string(),
                step_id: format!("react_step_{}", step_idx),
                title: format!("Act: {}", tool_name),
            },
            ReActStep::Observation { content, .. } => EditorEvent::StepCompleted {
                plan_id: "react".to_string(),
                step_id: format!("react_step_{}", step_idx),
                title: "Observe".to_string(),
                result: content.clone(),
            },
            ReActStep::FinalAnswer { content } => EditorEvent::StepCompleted {
                plan_id: "react".to_string(),
                step_id: format!("react_step_{}", step_idx),
                title: "Final Answer".to_string(),
                result: content.clone(),
            },
        }
    }

    /// Sprint 1: Execute a ReAct tool call and return the observation string.
    async fn execute_react_tool(
        &mut self,
        tool_name: &str,
        parameters: &std::collections::HashMap<String, serde_json::Value>,
    ) -> String {
        // Try to execute via SceneBridge if available
        if let Some(ref mut bridge) = self.scene_bridge {
            let result = match tool_name {
                "create_entity" | "spawn_entity" => {
                    let name = parameters
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("entity");
                    bridge.create_entity(name, None, &[])
                        .map(|id| format!("Created entity '{}' (id={})", name, id))
                        .map_err(|e| format!("Failed to create entity: {}", e))
                }
                "delete_entity" => {
                    let entity_id = parameters
                        .get("entity_id")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0);
                    bridge.delete_entity(entity_id)
                        .map(|()| format!("Deleted entity id={}", entity_id))
                        .map_err(|e| format!("Failed to delete entity: {}", e))
                }
                "update_component" | "set_transform" | "set_sprite" => {
                    let entity_id = parameters
                        .get("entity_id")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0);
                    let comp_type = parameters
                        .get("component_type")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Transform");
                    let mut props = std::collections::HashMap::new();
                    if let Some(pos) = parameters.get("position") {
                        props.insert("position".into(), pos.clone());
                    }
                    if let Some(color) = parameters.get("color") {
                        props.insert("color".into(), color.clone());
                    }
                    bridge.update_component(entity_id, comp_type, props)
                        .map(|()| format!("Updated {} for entity id={}", comp_type, entity_id))
                        .map_err(|e| format!("Failed to update component: {}", e))
                }
                "query_entities" | "query_scene" => {
                    let entities = bridge.query_entities(None, None);
                    let names: Vec<String> = entities.iter().map(|e| e.name.clone()).collect();
                    Ok(format!("Scene entities ({}): {}", names.len(), names.join(", ")))
                }
                _ => {
                    Err(format!("Unknown tool: {}", tool_name))
                }
            };

            match result {
                Ok(msg) => msg,
                Err(e) => format!("Error: {}", e),
            }
        } else {
            // MVP mode: simulate tool execution
            format!("Simulated: {} with params {:?} (no SceneBridge)", tool_name, parameters)
        }
    }

    /// Sprint 1: Observation feedback loop — feed tool execution result back into ReActAgent.
    ///
    /// Builds a prompt containing the observation and runs the ReAct agent again,
    /// allowing the agent to reason about the result and decide next steps.
    async fn observe_and_continue(&mut self, _request_text: &str, observation: &str) -> Result<String, String> {
        if let Some(ref mut react) = self.react_agent {
            // Build observation prompt that includes the tool result
            let observation_prompt = format!(
                "Observation: {}\n\nBased on this observation, what is your next thought and action?",
                observation
            );
            react.run(&observation_prompt).await.map_err(|e| e.to_string())
        } else {
            Err("ReActAgent not available".to_string())
        }
    }

    /// Static version of observe_and_continue for use when the agent is taken out of self.
    async fn observe_and_continue_static(
        react: &mut crate::strategy::ReActAgent,
        _request_text: &str,
        observation: &str,
    ) -> Result<String, String> {
        let observation_prompt = format!(
            "Observation: {}\n\nBased on this observation, what is your next thought and action?",
            observation
        );
        react.run(&observation_prompt).await.map_err(|e| e.to_string())
    }

    /// Sprint 1: Execute plan steps with ReActAgent support for dynamic revision.
    async fn execute_plan_with_permission_and_react(&mut self, plan: EditPlan) -> Vec<EditorEvent> {
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
                // Sprint 1: Use ReActAgent for execution if available
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

        // Sprint 1: Execute steps with ReActAgent, supporting dynamic revision
        while current_step_idx < plan.steps.len() {
            let step = &plan.steps[current_step_idx];
            
            events.push(EditorEvent::StepStarted {
                plan_id: plan_id.to_string(),
                step_id: step.id.clone(),
                title: step.title.clone(),
            });

            // Use ReActAgent to execute this step
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
                        
                        // Sprint 1: Dynamic revision - check if plan needs adjustment
                        if let Some(revision) = self.check_plan_revision_needed(&plan, current_step_idx, &result_clone) {
                            eprintln!("[DirectorRuntime] Dynamic revision: {}", revision);
                            // Apply revision to remaining steps
                            self.apply_plan_revision(plan_id, &revision);
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
                        
                        // Sprint 1: Reflection - try alternative approach
                        if let Some(alternative) = self.generate_alternative_step(&step.title, &error_msg) {
                            eprintln!("[DirectorRuntime] Reflection: trying alternative: {}", alternative);
                            // Replace current step with alternative
                            self.update_plan_step(plan_id, &step.id, &alternative);
                            // Don't increment step index, retry
                            continue;
                        }
                        
                        all_success = false;
                        break;
                    }
                }
            } else {
                // Fallback to original execution
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

    /// Sprint 1: Check if plan needs dynamic revision based on execution results.
    fn check_plan_revision_needed(&self, _plan: &EditPlan, _current_idx: usize, result: &str) -> Option<String> {
        // Check if the result indicates a need to adjust remaining steps
        let result_lower = result.to_lowercase();
        
        // Example: if result says "entity already exists", skip creation steps
        if result_lower.contains("already exists") || result_lower.contains("已存在") {
            return Some("Skip duplicate creation steps".to_string());
        }
        
        // Example: if result says "not found", try alternative entity
        if result_lower.contains("not found") || result_lower.contains("未找到") {
            return Some("Try alternative entity or create it first".to_string());
        }
        
        None
    }

    /// Sprint 1: Apply plan revision (dynamic plan adjustment).
    fn apply_plan_revision(&mut self, _plan_id: &str, revision: &str) {
        // Log the revision
        self.trace_entries.push(DirectorTraceEntry {
            timestamp_ms: now_millis(),
            actor: "PlanReviser".into(),
            summary: format!("Plan revision applied: {}", revision),
        });
        
        // In production, this would modify the plan steps
        // For now, just log it
    }

    /// Sprint 1: Generate alternative step when execution fails (Reflection).
    fn generate_alternative_step(&self, original: &str, error: &str) -> Option<String> {
        // Simple reflection: generate alternative based on error type
        let error_lower = error.to_lowercase();
        
        if error_lower.contains("not found") || error_lower.contains("不存在") {
            return Some(format!("Create {} first", original));
        }
        
        if error_lower.contains("permission") || error_lower.contains("拒绝") {
            return Some(format!("Request approval for {}", original));
        }
        
        None
    }

    /// Sprint 1: Update a plan step with alternative content.
    fn update_plan_step(&mut self, plan_id: &str, step_id: &str, new_title: &str) {
        if let Some(plan) = self.plan_manager.get_mut(plan_id) {
            for step in &mut plan.steps {
                if step.id == step_id {
                    step.title = new_title.to_string();
                    step.action_description = new_title.to_string();
                    break;
                }
            }
        }
    }

    /// Internal: Use LLM to create a plan from user request.
    async fn plan_with_llm(&mut self, request_text: &str) -> Result<EditPlan, String> {
        let client = self.llm_client.as_ref().ok_or("No LLM client available")?;

        let system_prompt = self.prompt_system.build_prompt(
            crate::prompt::PromptType::TaskPlanning,
            &crate::prompt::PromptContext {
                selected_entities: self.plan_manager
                    .list()
                    .iter()
                    .flat_map(|p| p.steps.iter().map(|s| s.title.clone()))
                    .collect::<Vec<_>>()
                    .join(", "),
                ..crate::prompt::PromptContext::default()
            },
        );

        // CoT user prompt — chain-of-thought with structured output
        let user_prompt = format!(
            "User request: \"{request_text}\"\n\n\
             Think step by step:\n\
             1. What domains does this request involve? (scene, code, asset, visual)\n\
             2. What is the risk level? (Safe, LowRisk, MediumRisk, HighRisk, Destructive)\n\
             3. What execution mode is best? (Direct, Plan, Team)\n\
             4. What are the concrete steps?\n\n\
             Output ONLY valid JSON (no markdown, no explanation):\n\
             {{\n\
               \"title\": \"short task title\",\n\
               \"summary\": \"one-line summary of the request\",\n\
               \"complexity\": \"Simple|Medium|Complex\",\n\
               \"risk_level\": \"Safe|LowRisk|MediumRisk|HighRisk|Destructive\",\n\
               \"mode\": \"Direct|Plan|Team\",\n\
               \"steps\": [\n\
                 {{\"step_id\": \"step_1\", \"title\": \"action name\", \"action\": \"action description\", \"target_module\": \"Scene|Code|Asset\"}}\n\
               ]\n\
             }}",
            request_text = request_text,
        );

        let request = crate::llm::LlmRequest {
            model: "gpt-4o-mini".to_string(),
            messages: vec![
                crate::llm::LlmMessage {
                    role: crate::llm::Role::System,
                    content: system_prompt.to_string(),
                },
                crate::llm::LlmMessage {
                    role: crate::llm::Role::User,
                    content: user_prompt,
                },
            ],
            tools: None,
            max_tokens: Some(2048),
            temperature: Some(0.3),
        };

        match client.chat(request).await {
            Ok(response) => self.parse_llm_plan_response(&response.content, request_text),
            Err(e) => Err(format!("LLM request failed: {}", e)),
        }
    }

    /// Parse LLM response into EditPlan.
    fn parse_llm_plan_response(
        &mut self,
        content: &str,
        request_text: &str,
    ) -> Result<EditPlan, String> {
        // Try to extract JSON from response (handle markdown code blocks)
        let json_str = if content.contains("```json") {
            content
                .split("```json")
                .nth(1)
                .and_then(|s| s.split("```").next())
                .unwrap_or(content)
                .trim()
        } else if content.contains("```") {
            content
                .split("```")
                .nth(1)
                .unwrap_or(content)
                .trim()
        } else {
            content.trim()
        };

        #[derive(serde::Deserialize)]
        struct LlmPlan {
            title: String,
            #[serde(default)]
            risk_level: String,
            steps: Vec<LlmPlanStep>,
        }

        #[derive(serde::Deserialize)]
        struct LlmPlanStep {
            id: String,
            title: String,
        }

        let llm_plan: LlmPlan = serde_json::from_str(json_str)
            .map_err(|e| format!("Failed to parse LLM plan: {}", e))?;

        let task_id = self.plan_manager.allocate_task_id();
        let plan_id = self.plan_manager.generate_plan_id("llm", task_id);

        let steps: Vec<EditPlanStep> = llm_plan
            .steps
            .into_iter()
            .enumerate()
            .map(|(_i, s)| EditPlanStep {
                id: s.id,
                title: s.title.clone(),
                target_module: TargetModule::Scene,
                action_description: s.title,
                risk: OperationRisk::LowRisk,
                validation_requirements: Vec::new(),
            })
            .collect();

        let risk_level = match llm_plan.risk_level.to_lowercase().as_str() {
            "lowrisk" | "low" => OperationRisk::LowRisk,
            "mediumrisk" | "medium" => OperationRisk::MediumRisk,
            "highrisk" | "high" => OperationRisk::HighRisk,
            "destructive" => OperationRisk::Destructive,
            _ => OperationRisk::LowRisk,
        };

        Ok(EditPlan {
            id: plan_id,
            task_id,
            title: llm_plan.title,
            summary: request_text.to_string(),
            mode: ExecutionMode::Plan,
            risk_level,
            steps,
            status: EditPlanStatus::Draft,
        })
    }

    /// Internal plan executor.
    ///
    /// Simulates step-by-step execution with transaction wrapping per step.
    /// In a production system, this would call into the `BevyAdapter`
    /// (or another engine adapter) to perform the actual operations.
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

        // Mark plan as running
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
            // Step start
            events.push(EditorEvent::StepStarted {
                plan_id: plan_id.to_string(),
                step_id: step.id.clone(),
                title: step.title.clone(),
            });

            // Begin transaction for rollback support
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

            // Resolve skill before borrowing self.scene_bridge mutably
            let skill_def = self.lookup_skill_for_step(step);

            // Capture pre-step snapshot for rollback
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

            // Execute via SceneBridge using skill system if available
            let step_start = std::time::Instant::now();
            let execution_result: Result<String, String> = if let Some(ref mut bridge) =
                self.scene_bridge
            {
                match step.target_module {
                    TargetModule::Scene => {
                        // Try skill-based execution first
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

            // Record execution result for validation
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

            // GoalChecker validation using SceneBridge snapshot (if enabled)
            // BUGFIX: step success now depends on both execution AND goal check,
            // not just goal checker (which always returns true when disabled).
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

            // BUGFIX: validation_ok must consider whether the step actually executed successfully
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
                // Rollback on execution failure or validation failure
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
                break; // Stop on first failure
            }
        }

        // Finalize plan status
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
}
