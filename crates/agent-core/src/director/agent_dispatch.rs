//! Agent dispatch — routes requests to specialist agents in Team mode.

use crate::types::now_millis;
use super::types::{DirectorRuntime, EditorEvent, DirectorTraceEntry};

impl DirectorRuntime {
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

        self.trace_entries.push(DirectorTraceEntry {
            timestamp_ms: now_millis(),
            actor: "AgentDispatch".into(),
            summary: "No specialist agent matched; using default pipeline".into(),
        });
        self.handle_user_request(request_text)
    }
}
