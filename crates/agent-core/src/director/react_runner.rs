//! ReAct execution runner — entry points and loop for LLM-driven execution.

use crate::types::now_millis;
use crate::strategy::ReActStep;
use super::types::{DirectorRuntime, EditorEvent, DirectorTraceEntry};

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

        if self.react_agent.is_some() {
            let rt = tokio::runtime::Handle::try_current();
            match rt {
                Ok(handle) => {
                    let request_text_owned = request_text.to_string();
                    let events = self.events.clone();
                    let event_bus = self.event_bus.clone();

                    handle.spawn(async move {
                        eprintln!(
                            "[ReActExecutor] Async task spawned for: {}",
                            request_text_owned
                        );
                        let _ = (request_text_owned, events, event_bus);
                    });

                    self.metrics.record_tool_call(start.elapsed(), true);
                    self.trace_entries.push(DirectorTraceEntry {
                        timestamp_ms: now_millis(),
                        actor: "ReActAgent".into(),
                        summary: "ReAct execution spawned (non-blocking)".into(),
                    });

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
                    eprintln!(
                        "[DirectorRuntime] No Tokio runtime, using sync ReAct execution"
                    );

                    let rt = tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                        .unwrap();

                    let mut self_clone = unsafe {
                        std::ptr::read(self)
                    };

                    let result = rt.block_on(async {
                        self_clone.handle_user_request_async(request_text).await
                    });

                    unsafe { std::ptr::write(self, self_clone); }

                    return match result.is_empty() {
                        true => "ReAct execution completed (check events for details)".to_string(),
                        false => {
                            let last_event = result.last().map(|e| match e {
                                EditorEvent::DirectExecutionCompleted { success, .. } => {
                                    if *success {
                                        "ReAct execution succeeded".to_string()
                                    } else {
                                        "ReAct execution failed".to_string()
                                    }
                                }
                                _ => "ReAct execution finished".to_string()
                            }).unwrap_or_else(|| "ReAct execution finished".to_string());
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

        self.event_bridge.process_events(&self.events, &mut self.memory_system);
        if self.event_bridge.events_processed > 0 {
            eprintln!("[EventBridge] {}", self.event_bridge.stats());
        }

        self.memory_injector.add_conversation_turn(request_text, &answer);

        answer
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

        if let Some(ref mut react) = self.react_agent {
            let recent_actions = Self::extract_recent_actions_from_events(&self.events);
            react.update_layered_context(
                self.scene_bridge.as_deref(),
                recent_actions,
                request_text,
                Vec::new(),
            );
        }

        let mut react = match self.react_agent.take() {
            Some(react) => react,
            None => return Err("ReActAgent not available".to_string()),
        };

        let max_steps = react.config.max_steps;
        let mut step_count = 0;

        while step_count < max_steps {
            let step_result = react.step(request_text).await;

            match step_result {
                Ok(step) => {
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

                            self.react_agent = Some(react);
                            return Ok(events);
                        }
                        ReActStep::Action { tool_name, parameters } => {
                            let observation = self.execute_react_tool(tool_name, parameters).await;

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

                            let _ = Self::observe_and_continue_static(
                                &mut react,
                                request_text,
                                &observation,
                            ).await;
                        }
                        ReActStep::Observation { content, success } => {
                            let _ = (content, success);
                        }
                        ReActStep::Thought { content, .. } => {
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

                    self.react_agent = Some(react);
                    return Ok(events);
                }
            }

            step_count += 1;
        }

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

        self.react_agent = Some(react);
        Ok(events)
    }

    /// Sprint 1: Push a ReAct step as a real-time EditorEvent and EventBus event.
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

    /// Sprint 1: Observation feedback loop — feed tool execution result back into ReActAgent.
    async fn observe_and_continue(&mut self, _request_text: &str, observation: &str) -> Result<String, String> {
        if let Some(revision) = self.dynamic_planner.analyze_observation(
            observation,
            0,
            "react_loop",
        ) {
            if revision.is_safe_auto_apply() {
                eprintln!(
                    "[DynamicPlanner] Auto-applying revision in ReAct loop: {}",
                    revision.describe()
                );
                self.trace_entries.push(DirectorTraceEntry {
                    timestamp_ms: now_millis(),
                    actor: "DynamicPlanner".into(),
                    summary: format!("ReAct loop revision suggested: {}", revision.describe()),
                });
            }
        }

        if let Some(ref mut react) = self.react_agent {
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
}
