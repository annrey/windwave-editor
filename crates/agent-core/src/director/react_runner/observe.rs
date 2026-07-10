use super::super::types::{DirectorRuntime, DirectorTraceEntry};
use crate::types::now_millis;

impl DirectorRuntime {
    /// Sprint 1: Observation feedback loop — feed tool execution result back into ReActAgent.
    #[allow(dead_code)]
    pub(crate) async fn observe_and_continue(
        &mut self,
        _request_text: &str,
        observation: &str,
    ) -> Result<String, String> {
        if let Some(revision) =
            self.dynamic_planner
                .analyze_observation(observation, 0, "react_loop")
        {
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
            react
                .run(&observation_prompt)
                .await
                .map_err(|e| e.to_string())
        } else {
            Err("ReActAgent not available".to_string())
        }
    }

    /// Static version of observe_and_continue for use when the agent is taken out of self.
    pub(crate) async fn observe_and_continue_static(
        react: &mut crate::strategy::ReActAgent,
        _request_text: &str,
        observation: &str,
    ) -> Result<String, String> {
        let observation_prompt = format!(
            "Observation: {}\n\nBased on this observation, what is your next thought and action?",
            observation
        );
        react
            .run(&observation_prompt)
            .await
            .map_err(|e| e.to_string())
    }
}
