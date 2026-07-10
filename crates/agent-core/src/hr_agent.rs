//! HR Agent — provides runtime team management: add/remove/list agents.
//! Used by the Director to manage agent roster dynamically.
//!

use crate::registry::{Agent, AgentError, AgentId, AgentRequest, AgentResponse, AgentResultKind};
use crate::team_structure::{TeamRole, TeamRoster};
use serde_json::json;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PendingHrAction {
    Add {
        name: String,
        role: TeamRole,
        capabilities: Vec<String>,
    },
    Remove {
        agent_id: u64,
        name: String,
    },
}

pub struct HrAgent {
    id: AgentId,
    name: String,
    roster: TeamRoster,
    pending_action: Option<PendingHrAction>,
}

impl HrAgent {
    pub fn new(id: AgentId, roster: TeamRoster) -> Self {
        Self {
            id,
            name: "HR".into(),
            roster,
            pending_action: None,
        }
    }

    pub fn team_size(&self) -> usize {
        self.roster.members.len()
    }

    pub fn pending_action(&self) -> Option<&PendingHrAction> {
        self.pending_action.as_ref()
    }

    pub fn confirm_pending(&mut self) -> Result<AgentResponse, AgentError> {
        let action = self
            .pending_action
            .take()
            .ok_or_else(|| AgentError::ExecutionFailed("No pending HR action to confirm".into()))?;

        let (summary, output) = match action {
            PendingHrAction::Add {
                name,
                role,
                capabilities,
            } => {
                let agent_id = self.roster.add(name.clone(), role, capabilities);
                (
                    format!("Added agent '{}' (id {})", name, agent_id),
                    json!({ "action": "add", "agent_id": agent_id }),
                )
            }
            PendingHrAction::Remove { agent_id, name } => {
                if self.roster.remove(agent_id) {
                    (
                        format!("Removed agent '{}' (id {})", name, agent_id),
                        json!({ "action": "remove", "agent_id": agent_id }),
                    )
                } else {
                    (
                        format!("Agent '{}' (id {}) was already absent", name, agent_id),
                        json!({ "action": "remove", "agent_id": agent_id, "already_absent": true }),
                    )
                }
            }
        };

        Ok(AgentResponse {
            agent_id: self.id,
            agent_name: self.name.clone(),
            result: AgentResultKind::Success { summary, output },
            events: vec![],
        })
    }

    pub fn reject_pending(&mut self) -> bool {
        self.pending_action.take().is_some()
    }

    fn parse_role(role_str: &str) -> TeamRole {
        match role_str {
            "director" | "调度" | "planner" | "规划" => TeamRole::ProjectManager,
            "executor" | "执行" => TeamRole::Executor,
            "reviewer" | "审查" => TeamRole::Reviewer,
            "hr" | "人事" => TeamRole::Hr,
            _ => TeamRole::Executor,
        }
    }

    fn parse_capabilities(request: &AgentRequest) -> Vec<String> {
        request
            .context
            .get("capabilities")
            .and_then(|v| v.as_array())
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| item.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default()
    }
}

#[async_trait::async_trait]
impl Agent for HrAgent {
    fn id(&self) -> AgentId {
        self.id
    }
    fn name(&self) -> &str {
        &self.name
    }
    fn role(&self) -> &str {
        "hr"
    }
    fn capabilities(&self) -> &[crate::registry::CapabilityKind] {
        &[
            crate::registry::CapabilityKind::Orchestrate,
            crate::registry::CapabilityKind::SceneWrite,
        ]
    }

    async fn handle(&mut self, request: AgentRequest) -> Result<AgentResponse, AgentError> {
        let ins = request.instruction.to_lowercase();

        if ins.contains("add")
            || ins.contains("hire")
            || ins.contains("create")
            || ins.contains("添加")
        {
            let role_str = request
                .context
                .get("role")
                .and_then(|v| v.as_str())
                .unwrap_or("executor");
            let name = request
                .context
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("NewAgent");
            let role = Self::parse_role(role_str);
            self.pending_action = Some(PendingHrAction::Add {
                name: name.to_string(),
                role,
                capabilities: Self::parse_capabilities(&request),
            });
            Ok(AgentResponse {
                agent_id: self.id,
                agent_name: self.name.clone(),
                result: AgentResultKind::NeedUserInput {
                    question: format!("Add agent '{}' as {:?}?", name, role),
                },
                events: vec![],
            })
        } else if ins.contains("remove") || ins.contains("fire") || ins.contains("移除") {
            let target = request
                .context
                .get("agent_id")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            // Removing a team member is destructive — require explicit confirmation
            // before mutating the roster. The actual removal is applied later, once
            // the Director receives the user's confirmation (pending-approval flow).
            match self.roster.find(target) {
                Some(member) => {
                    self.pending_action = Some(PendingHrAction::Remove {
                        agent_id: target,
                        name: member.name.clone(),
                    });
                    Ok(AgentResponse {
                        agent_id: self.id,
                        agent_name: self.name.clone(),
                        result: AgentResultKind::NeedUserInput {
                            question: format!("Remove agent '{}' (id {})?", member.name, target),
                        },
                        events: vec![],
                    })
                }
                None => Ok(AgentResponse {
                    agent_id: self.id,
                    agent_name: self.name.clone(),
                    result: AgentResultKind::Failed {
                        reason: format!("Agent with ID {} not found", target),
                    },
                    events: vec![],
                }),
            }
        } else if ins.contains("list") || ins.contains("team") || ins.contains("列表") {
            let members: Vec<_> = self.roster.members.iter().map(|m| json!({ "id": m.agent_id, "name": m.name, "role": m.role.name(), "online": m.online })).collect();
            Ok(AgentResponse {
                agent_id: self.id,
                agent_name: self.name.clone(),
                result: AgentResultKind::Success {
                    summary: format!("{} members", members.len()),
                    output: json!({ "members": members }),
                },
                events: vec![],
            })
        } else {
            Ok(AgentResponse {
                agent_id: self.id,
                agent_name: self.name.clone(),
                result: AgentResultKind::Failed {
                    reason: "Unknown HR command. Try: add <role> <name>, remove <id>, list team"
                        .into(),
                },
                events: vec![],
            })
        }
    }

    fn confirm_pending_user_input(&mut self) -> Result<Option<AgentResponse>, AgentError> {
        self.confirm_pending().map(Some)
    }

    fn reject_pending_user_input(&mut self) -> bool {
        self.reject_pending()
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
        roster.add("PM", TeamRole::ProjectManager, vec![]);
        let mut hr = HrAgent::new(AgentId(200), roster);
        let req = AgentRequest {
            task_id: Some("h1".into()),
            instruction: "list team".into(),
            context: json!({}),
        };
        let resp = hr.handle(req).await.unwrap();
        assert!(matches!(resp.result, AgentResultKind::Success { .. }));
    }

    #[tokio::test]
    async fn test_hr_add_needs_confirm() {
        let roster = TeamRoster::new();
        let mut hr = HrAgent::new(AgentId(200), roster);
        let req = AgentRequest {
            task_id: Some("h2".into()),
            instruction: "add executor".into(),
            context: json!({"role": "executor", "name": "CB"}),
        };
        let resp = hr.handle(req).await.unwrap();
        assert!(matches!(resp.result, AgentResultKind::NeedUserInput { .. }));
        assert!(
            matches!(hr.pending_action(), Some(PendingHrAction::Add { name, role, .. }) if name == "CB" && *role == TeamRole::Executor)
        );
        assert_eq!(hr.team_size(), 0);
    }

    #[tokio::test]
    async fn test_hr_confirm_add_applies_roster_change() {
        let roster = TeamRoster::new();
        let mut hr = HrAgent::new(AgentId(200), roster);
        let req = AgentRequest {
            task_id: Some("h2-confirm".into()),
            instruction: "add executor".into(),
            context: json!({"role": "executor", "name": "CB", "capabilities": ["scene_write"]}),
        };

        let resp = hr.handle(req).await.unwrap();
        assert!(matches!(resp.result, AgentResultKind::NeedUserInput { .. }));
        let applied = hr.confirm_pending().unwrap();

        assert!(matches!(applied.result, AgentResultKind::Success { .. }));
        assert_eq!(hr.team_size(), 1);
        assert!(hr.pending_action().is_none());
    }

    #[tokio::test]
    async fn test_hr_remove_needs_confirm() {
        let mut roster = TeamRoster::new();
        let aid = roster.add("X", TeamRole::Executor, vec![]);
        let mut hr = HrAgent::new(AgentId(200), roster);
        let req = AgentRequest {
            task_id: Some("h3".into()),
            instruction: "remove".into(),
            context: json!({"agent_id": aid}),
        };
        let resp = hr.handle(req).await.unwrap();
        // Removal must ask for confirmation and must NOT mutate the roster yet.
        assert!(matches!(resp.result, AgentResultKind::NeedUserInput { .. }));
        assert!(
            matches!(hr.pending_action(), Some(PendingHrAction::Remove { agent_id, name }) if *agent_id == aid && name == "X")
        );
        assert!(hr.team_size() == 1);
    }

    #[tokio::test]
    async fn test_hr_confirm_remove_applies_roster_change() {
        let mut roster = TeamRoster::new();
        let aid = roster.add("X", TeamRole::Executor, vec![]);
        let mut hr = HrAgent::new(AgentId(200), roster);
        let req = AgentRequest {
            task_id: Some("h3-confirm".into()),
            instruction: "remove".into(),
            context: json!({"agent_id": aid}),
        };

        let resp = hr.handle(req).await.unwrap();
        assert!(matches!(resp.result, AgentResultKind::NeedUserInput { .. }));
        let applied = hr.confirm_pending().unwrap();

        assert!(matches!(applied.result, AgentResultKind::Success { .. }));
        assert_eq!(hr.team_size(), 0);
        assert!(hr.pending_action().is_none());
    }

    #[test]
    fn test_hr_reject_pending_clears_without_mutation() {
        let mut roster = TeamRoster::new();
        let aid = roster.add("X", TeamRole::Executor, vec![]);
        let mut hr = HrAgent::new(AgentId(200), roster);
        hr.pending_action = Some(PendingHrAction::Remove {
            agent_id: aid,
            name: "X".into(),
        });

        assert!(hr.reject_pending());
        assert_eq!(hr.team_size(), 1);
        assert!(hr.pending_action().is_none());
    }

    #[tokio::test]
    async fn test_hr_remove_unknown_fails() {
        let roster = TeamRoster::new();
        let mut hr = HrAgent::new(AgentId(200), roster);
        let req = AgentRequest {
            task_id: Some("h4".into()),
            instruction: "remove".into(),
            context: json!({"agent_id": 999}),
        };
        let resp = hr.handle(req).await.unwrap();
        assert!(matches!(resp.result, AgentResultKind::Failed { .. }));
        assert!(hr.pending_action().is_none());
    }
}
