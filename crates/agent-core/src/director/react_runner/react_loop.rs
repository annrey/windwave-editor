use super::super::types::{DirectorRuntime, DirectorTraceEntry, EditorEvent};
use super::guard::ReactAgentGuard;
use crate::strategy::ReActStep;
use crate::types::now_millis;

impl DirectorRuntime {
    /// Sprint 1: Execute using ReActAgent directly (think-act-observe loop).
    ///
    /// Streams each Think/Act/Observation step as an `EditorEvent` via
    /// `self.events` and `self.event_bus` in real-time, rather than
    /// buffering all events until completion.
    ///
    /// Phase 2 Integration: Automatically records all steps to ReasoningBank.
    pub(crate) async fn execute_with_react(
        &mut self,
        request_text: &str,
    ) -> Result<Vec<EditorEvent>, String> {
        let mut events = Vec::new();

        // Phase 2: Create ReasoningTrace for this ReAct execution
        let task_id = self.plan_manager.allocate_task_id();
        let trace_id = if let Some(ref mut collab) = self.collaboration {
            Some(collab.reasoning_bank.start_trace(
                crate::squad::TaskId(task_id),
                request_text.to_string(),
                format!("ReAct execution for: {}", request_text),
            ))
        } else {
            None
        };

        let start_event = EditorEvent::DirectExecutionStarted {
            request: request_text.to_string(),
            mode: "ReAct".to_string(),
            complexity_score: 5,
        };
        self.events.push(start_event.clone());
        self.event_bus
            .push(crate::event::EventBusEvent::ObservationCreated {
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

        let mut guard = match ReactAgentGuard::take_from(self) {
            Some(g) => g,
            None => return Err("ReActAgent not available".to_string()),
        };

        let max_steps = guard.get_mut().config.max_steps;
        let mut step_count = 0;

        while step_count < max_steps {
            let step_result = { guard.get_mut().step(request_text) }.await;

            match step_result {
                Ok(step) => {
                    let editor_event =
                        Self::react_step_to_editor_event(request_text, &step, step_count);
                    Self::push_react_step_event_static(
                        &mut self.events,
                        &mut self.event_bus,
                        &mut self.trace_entries,
                        &step,
                        step_count,
                    );
                    events.push(editor_event);

                    // Phase 2: Record each step to ReasoningBank
                    if let Some(trace_id) = trace_id {
                        if let Some(ref mut collab) = self.collaboration {
                            match &step {
                                ReActStep::Thought { content, reasoning } => {
                                    collab.reasoning_bank.record_think(
                                        trace_id,
                                        format!("Step {}: {}", step_count, content),
                                        reasoning.clone(),
                                    );
                                }
                                ReActStep::Action {
                                    tool_name,
                                    parameters,
                                } => {
                                    let params_str =
                                        serde_json::to_string(parameters).unwrap_or_default();
                                    collab.reasoning_bank.record_act(
                                        trace_id,
                                        tool_name.clone(),
                                        params_str,
                                        String::new(),
                                        true,
                                    );
                                }
                                ReActStep::Observation { content, .. } => {
                                    collab.reasoning_bank.record_observe(
                                        trace_id,
                                        format!("Step {}: {}", step_count, content),
                                        content.clone(),
                                    );
                                }
                                ReActStep::FinalAnswer { content } => {
                                    collab.reasoning_bank.record_think(
                                        trace_id,
                                        format!("Step {}: Final Answer", step_count),
                                        content.clone(),
                                    );
                                }
                            }
                        }
                    }

                    match &step {
                        ReActStep::FinalAnswer { content } => {
                            let completed = EditorEvent::DirectExecutionCompleted {
                                request: request_text.to_string(),
                                success: true,
                            };
                            self.events.push(completed.clone());
                            self.event_bus
                                .push(crate::event::EventBusEvent::ObservationCreated {
                                    observation_type: "ReActComplete".to_string(),
                                    summary: format!("ReAct completed: {}", content),
                                });
                            events.push(completed);

                            self.trace_entries.push(DirectorTraceEntry {
                                timestamp_ms: now_millis(),
                                actor: "ReActAgent".into(),
                                summary: format!("ReAct completed: {}", content),
                            });

                            // Phase 2: Complete the ReasoningTrace
                            if let Some(trace_id) = trace_id {
                                if let Some(ref mut collab) = self.collaboration {
                                    collab.reasoning_bank.bank_mut().mark_trace_complete(
                                        trace_id,
                                        true,
                                        vec!["ReAct".into(), "Success".into()],
                                    );
                                }
                            }

                            return Ok(events);
                        }
                        ReActStep::Action {
                            tool_name,
                            parameters,
                        } => {
                            let observation = self.execute_react_tool(tool_name, parameters).await;

                            let obs_event = EditorEvent::StepCompleted {
                                plan_id: "react".to_string(),
                                step_id: format!("react_step_{}", step_count),
                                title: format!("Action: {}", tool_name),
                                result: observation.clone(),
                            };
                            self.events.push(obs_event.clone());
                            self.event_bus
                                .push(crate::event::EventBusEvent::ObservationCreated {
                                    observation_type: "ReActObservation".to_string(),
                                    summary: format!(
                                        "Tool '{}' result: {}",
                                        tool_name, observation
                                    ),
                                });
                            events.push(obs_event);

                            // Phase 2: Update the Act step with observation result
                            if let Some(trace_id) = trace_id {
                                if let Some(ref mut collab) = self.collaboration {
                                    collab.reasoning_bank.record_observe(
                                        trace_id,
                                        format!("Step {}: {}", step_count, tool_name),
                                        observation.clone(),
                                    );
                                }
                            }

                            let _ = Self::observe_and_continue_static(
                                guard.get_mut(),
                                request_text,
                                &observation,
                            )
                            .await;
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
                    self.event_bus
                        .push(crate::event::EventBusEvent::ObservationCreated {
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

                    // Phase 2: Complete the ReasoningTrace as failed
                    if let Some(trace_id) = trace_id {
                        if let Some(ref mut collab) = self.collaboration {
                            collab.reasoning_bank.bank_mut().mark_trace_complete(
                                trace_id,
                                false,
                                vec!["ReAct".into(), "Failed".into()],
                            );
                        }
                    }

                    self.react_agent = guard.into_inner();
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

        // Phase 2: Complete the ReasoningTrace as max steps reached
        if let Some(trace_id) = trace_id {
            if let Some(ref mut collab) = self.collaboration {
                collab.reasoning_bank.bank_mut().mark_trace_complete(
                    trace_id,
                    false,
                    vec!["ReAct".into(), "MaxSteps".into()],
                );
            }
        }

        self.react_agent = guard.into_inner();
        Ok(events)
    }
}
