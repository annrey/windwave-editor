//! Agent dispatch — routes requests to specialist agents in Team mode.

use super::types::{DirectorRuntime, DirectorTraceEntry, EditorEvent};
use crate::keyword_matcher::KeywordMatcher;
use crate::types::now_millis;

impl DirectorRuntime {
    /// Dispatch a request through the runtime-owned agent registry.
    ///
    /// This is the path UI systems should use: pending approvals created by the
    /// dispatch stay attached to the same registry that later approve/reject
    /// calls will confirm against.
    pub fn dispatch_to_registered_agent(&mut self, request_text: &str) -> Vec<EditorEvent> {
        let Some(mut registry) = self.agent_registry.take() else {
            return self.handle_user_request(request_text);
        };

        let events = self.dispatch_to_agent(request_text, &mut registry);
        self.agent_registry = Some(registry);
        events
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
        let is_team_request = KeywordMatcher::is_team_request(request_text);

        let (candidates, matched_capability) = if is_team_request {
            (
                registry.find_by_capability(&crate::registry::CapabilityKind::Orchestrate),
                "Orchestrate",
            )
        } else {
            match KeywordMatcher::classify_capability(request_text) {
                Some(crate::registry::CapabilityKind::CodeWrite) => (
                    registry.find_by_capability(&crate::registry::CapabilityKind::CodeWrite),
                    "CodeWrite",
                ),
                Some(crate::registry::CapabilityKind::RuleCheck) => (
                    registry.find_by_capability(&crate::registry::CapabilityKind::RuleCheck),
                    "RuleCheck",
                ),
                Some(crate::registry::CapabilityKind::SceneWrite) => (
                    registry.find_by_capability(&crate::registry::CapabilityKind::SceneWrite),
                    "SceneWrite",
                ),
                Some(crate::registry::CapabilityKind::Orchestrate) => (
                    registry.find_by_capability(&crate::registry::CapabilityKind::Orchestrate),
                    "Orchestrate",
                ),
                _ => (vec![], ""),
            }
        };

        if !candidates.is_empty() {
            let selected = if is_team_request {
                let is_hr = KeywordMatcher::is_hr_request(request_text);
                if is_hr {
                    candidates
                        .iter()
                        .copied()
                        .find(|agent| agent.role() == "hr")
                        .unwrap_or(candidates[0])
                } else {
                    candidates
                        .iter()
                        .copied()
                        .find(|agent| agent.role() == "squad_leader")
                        .unwrap_or(candidates[0])
                }
            } else {
                candidates[0]
            };
            let agent_name = selected.name().to_string();
            let agent_id = selected.id();
            drop(candidates);
            self.trace_entries.push(DirectorTraceEntry {
                timestamp_ms: now_millis(),
                actor: "AgentDispatch".into(),
                summary: format!(
                    "Dispatching to agent '{}' (cap: {})",
                    agent_name, matched_capability
                ),
            });

            let agent_req = crate::registry::AgentRequest {
                task_id: None,
                instruction: request_text.to_string(),
                context: serde_json::json!({"capability": matched_capability}),
            };

            match registry.dispatch_sync(agent_req, Some(agent_id)) {
                Ok(response) => {
                    let mut events = Vec::new();
                    match response.result {
                        crate::registry::AgentResultKind::NeedUserInput { question } => {
                            let approval_id =
                                format!("agent_{}_approval_{}", agent_id.0, now_millis());
                            self.agent_pending_approvals
                                .insert(approval_id.clone(), agent_id);
                            events.push(EditorEvent::PermissionRequested {
                                plan_id: approval_id,
                                risk: "HighRisk".to_string(),
                                reason: question,
                            });
                        }
                        other => {
                            events.push(EditorEvent::StepCompleted {
                                plan_id: "agent_dispatch".to_string(),
                                step_id: format!("{:?}", agent_id),
                                title: agent_name.clone(),
                                result: format!("{:?}", other),
                            });
                        }
                    }
                    self.events.extend(events.clone());
                    return events;
                }
                Err(e) => {
                    self.trace_entries.push(DirectorTraceEntry {
                        timestamp_ms: now_millis(),
                        actor: "AgentDispatch".into(),
                        summary: format!(
                            "Agent '{}' dispatch failed: {:?}, falling back",
                            agent_name, e
                        ),
                    });
                }
            }
        }

        self.trace_entries.push(DirectorTraceEntry {
            timestamp_ms: now_millis(),
            actor: "AgentDispatch".into(),
            summary: "No specialist agent matched; using default pipeline".into(),
        });
        self.handle_user_request(request_text)
    }
}
