use super::super::types::{DirectorRuntime, DirectorTraceEntry, EditorEvent};
use crate::strategy::ReActStep;
use crate::types::now_millis;

impl DirectorRuntime {
    /// Sprint 1: Push a ReAct step as a real-time EditorEvent and EventBus event.
    pub(crate) fn push_react_step_event_static(
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
                    summary: format!(
                        "Step {} Thought: {} | Reasoning: {}",
                        step_idx, content, reasoning
                    ),
                });
                trace_entries.push(DirectorTraceEntry {
                    timestamp_ms: now_millis(),
                    actor: "ReActAgent".into(),
                    summary: format!("Step {} Thought: {}", step_idx, content),
                });
            }
            ReActStep::Action {
                tool_name,
                parameters,
            } => {
                events.push(EditorEvent::StepStarted {
                    plan_id: "react".to_string(),
                    step_id: format!("react_step_{}", step_idx),
                    title: format!("Act: {}", tool_name),
                });
                event_bus.push(crate::event::EventBusEvent::ObservationCreated {
                    observation_type: "ReActAction".to_string(),
                    summary: format!(
                        "Step {} Action: {} params={:?}",
                        step_idx, tool_name, parameters
                    ),
                });
                trace_entries.push(DirectorTraceEntry {
                    timestamp_ms: now_millis(),
                    actor: "ReActAgent".into(),
                    summary: format!(
                        "Step {} Action: {} params={:?}",
                        step_idx, tool_name, parameters
                    ),
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
                    summary: format!(
                        "Step {} Observation (success={}): {}",
                        step_idx, success, content
                    ),
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
    pub(crate) fn react_step_to_editor_event(
        _request_text: &str,
        step: &ReActStep,
        step_idx: usize,
    ) -> EditorEvent {
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
}
