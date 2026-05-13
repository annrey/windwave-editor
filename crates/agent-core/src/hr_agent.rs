//! HR Agent — manages agent team membership at runtime.

use crate::registry::{AgentId, AgentRequest, AgentResponse, AgentResultKind, AgentError, Agent};
use crate::team_structure::{TeamRole, TeamRoster};
use serde_json::json;

pub struct HrAgent {
    id: AgentId,
    name: String,
    roster: TeamRoster,
}

impl HrAgent {
    pub fn new(id: AgentId, roster: TeamRoster) -> Self {
        Self { id, name: "HR".into(), roster }
    }

    pub fn team_size(&self) -> usize { self.roster.members.len() }
}

#[async_trait::async_trait]
impl Agent for HrAgent {
    fn id(&self) -> AgentId { self.id }
    fn name(&self) -> &str { &self.name }
    fn role(&self) -> &str { "hr" }
    fn capabilities(&self) -> &[crate::registry::CapabilityKind] {
        &[crate::registry::CapabilityKind::Orchestrate, crate::registry::CapabilityKind::SceneWrite]
    }

    async fn handle(&mut self, request: AgentRequest) -> Result<AgentResponse, AgentError> {
        let ins = request.instruction.to_lowercase();

        if ins.contains("add") || ins.contains("hire") || ins.contains("create") || ins.contains("添加") {
            let role_str = request.context.get("role").and_then(|v| v.as_str()).unwrap_or("executor");
            let name = request.context.get("name").and_then(|v| v.as_str()).unwrap_or("NewAgent");
            let role = match role_str {
                "director" | "调度" => TeamRole::Director,
                "planner" | "规划" => TeamRole::Planner,
                "executor" | "执行" => TeamRole::Executor,
                "reviewer" | "审查" => TeamRole::Reviewer,
                "hr" | "人事" => TeamRole::Hr,
                _ => TeamRole::Executor,
            };
            Ok(AgentResponse { agent_id: self.id, agent_name: self.name.clone(), result: AgentResultKind::NeedUserInput { question: format!("Add agent '{}' as {:?}?", name, role) }, events: vec![] })
        } else if ins.contains("remove") || ins.contains("fire") || ins.contains("移除") {
            let target = request.context.get("agent_id").and_then(|v| v.as_u64()).unwrap_or(0);
            // Actually remove the agent from the roster
            if self.roster.remove(target) {
                Ok(AgentResponse { agent_id: self.id, agent_name: self.name.clone(), result: AgentResultKind::Success { summary: format!("Agent {} removed", target), output: json!({"removed_id": target}) }, events: vec![] })
            } else {
                Ok(AgentResponse { agent_id: self.id, agent_name: self.name.clone(), result: AgentResultKind::Failed { reason: format!("Agent with ID {} not found", target) }, events: vec![] })
            }
        } else if ins.contains("list") || ins.contains("team") || ins.contains("列表") {
            let members: Vec<_> = self.roster.members.iter().map(|m| json!({ "id": m.agent_id, "name": m.name, "role": m.role.name(), "online": m.online })).collect();
            Ok(AgentResponse { agent_id: self.id, agent_name: self.name.clone(), result: AgentResultKind::Success { summary: format!("{} members", members.len()), output: json!({ "members": members }) }, events: vec![] })
        } else {
            Ok(AgentResponse { agent_id: self.id, agent_name: self.name.clone(), result: AgentResultKind::Failed { reason: "Unknown HR command. Try: add <role> <name>, remove <id>, list team".into() }, events: vec![] })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::AgentId;
    use serde_json::json;

    #[tokio::test]
    async fn test_hr_list_team() {
        let mut roster = TeamRoster::new();
        roster.add("D", TeamRole::Director, vec![]);
        let mut hr = HrAgent::new(AgentId(200), roster);
        let req = AgentRequest { task_id: Some("h1".into()), instruction: "list team".into(), context: json!({}) };
        let resp = hr.handle(req).await.unwrap();
        assert!(matches!(resp.result, AgentResultKind::Success { .. }));
    }

    #[tokio::test]
    async fn test_hr_add_needs_confirm() {
        let roster = TeamRoster::new();
        let mut hr = HrAgent::new(AgentId(200), roster);
        let req = AgentRequest { task_id: Some("h2".into()), instruction: "add executor".into(), context: json!({"role": "executor", "name": "CB"}) };
        let resp = hr.handle(req).await.unwrap();
        assert!(matches!(resp.result, AgentResultKind::NeedUserInput { .. }));
    }

    #[tokio::test]
    async fn test_hr_remove() {
        let mut roster = TeamRoster::new();
        let aid = roster.add("X", TeamRole::Executor, vec![]);
        let mut hr = HrAgent::new(AgentId(200), roster);
        let req = AgentRequest { task_id: Some("h3".into()), instruction: "remove".into(), context: json!({"agent_id": aid}) };
        let resp = hr.handle(req).await.unwrap();
        assert!(matches!(resp.result, AgentResultKind::Success { .. }));
        assert!(hr.team_size() == 0);
    }
}
