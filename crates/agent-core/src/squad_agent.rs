//! Squad Agent — orchestrates a team of specialized agents.
//!
//! This Agent acts as a unified interface for assigning work to a Squad.
//! You can @mention a Squad (e.g., @FrontendTeam) and it will automatically
//! route the task to the right member.

use crate::registry::{
    Agent, AgentError, AgentId, AgentRequest, AgentResponse, AgentResultKind, CapabilityKind,
};
use crate::squad::{RoutingPolicy, Squad, SquadId, SquadRegistry, SquadTask};
use serde_json::json;

/// Agent that manages a Squad.
pub struct SquadAgent {
    id: AgentId,
    name: String,
    squad_id: SquadId,
    registry: SquadRegistry,
}

impl SquadAgent {
    pub fn new(id: AgentId, name: String, squad_id: SquadId, registry: SquadRegistry) -> Self {
        Self {
            id,
            name,
            squad_id,
            registry,
        }
    }

    /// Get a reference to the registry.
    pub fn registry(&self) -> &SquadRegistry {
        &self.registry
    }

    /// Get a mutable reference to the registry.
    pub fn registry_mut(&mut self) -> &mut SquadRegistry {
        &mut self.registry
    }

    /// Get a reference to the Squad managed by this Agent.
    pub fn squad(&self) -> Option<&Squad> {
        self.registry.get(self.squad_id)
    }

    /// Get a mutable reference to the Squad managed by this Agent.
    pub fn squad_mut(&mut self) -> Option<&mut Squad> {
        self.registry.get_mut(self.squad_id)
    }
}

#[async_trait::async_trait]
impl Agent for SquadAgent {
    fn id(&self) -> AgentId {
        self.id
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn role(&self) -> &str {
        "squad_leader"
    }

    fn capabilities(&self) -> &[CapabilityKind] {
        &[
            CapabilityKind::Orchestrate,
            CapabilityKind::SceneRead,
            CapabilityKind::SceneWrite,
        ]
    }

    async fn handle(&mut self, request: AgentRequest) -> Result<AgentResponse, AgentError> {
        let ins = request.instruction.to_lowercase();

        if crate::keyword_matcher::KeywordMatcher::is_create_operation(&ins) {
            let name = request
                .context
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("NewSquad");
            let leader_id_val = request
                .context
                .get("leader_id")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            let leader_id = AgentId(leader_id_val);

            let policy_str = request
                .context
                .get("policy")
                .and_then(|v| v.as_str())
                .unwrap_or("skill_based");
            let policy = match policy_str {
                "leader_decides" => RoutingPolicy::LeaderDecides,
                "round_robin" => RoutingPolicy::RoundRobin,
                "load_balance" => RoutingPolicy::LoadBalance,
                _ => RoutingPolicy::SkillBased,
            };

            let squad_id = self
                .registry
                .create_squad(name.to_string(), leader_id, policy);

            Ok(AgentResponse {
                agent_id: self.id,
                agent_name: self.name.clone(),
                result: AgentResultKind::Success {
                    summary: format!("Created Squad '{}' with ID {}", name, squad_id.0),
                    output: json!({
                        "squad_id": squad_id.0,
                        "name": name,
                        "leader_id": leader_id.0,
                    }),
                },
                events: vec![],
            })
        } else if ins.contains("add member") || ins.contains("加入") {
            let squad_id_val = request
                .context
                .get("squad_id")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            let agent_id_val = request
                .context
                .get("agent_id")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);

            if let Some(squad) = self.registry.get_mut(SquadId(squad_id_val)) {
                squad.add_member(AgentId(agent_id_val));
                Ok(AgentResponse {
                    agent_id: self.id,
                    agent_name: self.name.clone(),
                    result: AgentResultKind::Success {
                        summary: format!("Added agent {} to Squad", agent_id_val),
                        output: json!({
                            "squad_id": squad_id_val,
                            "agent_id": agent_id_val,
                        }),
                    },
                    events: vec![],
                })
            } else {
                Ok(AgentResponse {
                    agent_id: self.id,
                    agent_name: self.name.clone(),
                    result: AgentResultKind::Failed {
                        reason: format!("Squad {} not found", squad_id_val),
                    },
                    events: vec![],
                })
            }
        } else if ins.contains("submit task")
            || ins.contains("assign task")
            || ins.contains("提交任务")
        {
            let squad_id_val = request
                .context
                .get("squad_id")
                .and_then(|v| v.as_u64())
                .unwrap_or(self.squad_id.0);
            let title = request
                .context
                .get("title")
                .and_then(|v| v.as_str())
                .unwrap_or("Unnamed task")
                .to_string();
            let description = request
                .context
                .get("description")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            if let Some((squad, task_id_val)) =
                self.registry.get_mut_with_task_id(SquadId(squad_id_val))
            {
                let task = SquadTask::new(task_id_val, title, description, vec![]);
                squad.submit_task(task);

                Ok(AgentResponse {
                    agent_id: self.id,
                    agent_name: self.name.clone(),
                    result: AgentResultKind::Success {
                        summary: format!("Submitted task to Squad {}", squad_id_val),
                        output: json!({
                            "squad_id": squad_id_val,
                            "task_id": task_id_val.0,
                        }),
                    },
                    events: vec![],
                })
            } else {
                Ok(AgentResponse {
                    agent_id: self.id,
                    agent_name: self.name.clone(),
                    result: AgentResultKind::Failed {
                        reason: format!("Squad {} not found", squad_id_val),
                    },
                    events: vec![],
                })
            }
        } else if ins.contains("list") || ins.contains("list squads") || ins.contains("列表") {
            let squads: Vec<_> = self
                .registry
                .list()
                .into_iter()
                .map(|(id, s)| {
                    json!({
                        "id": id.0,
                        "name": s.name,
                        "leader": s.leader.0,
                        "member_count": s.members.len(),
                        "queued_tasks": s.task_queue.len(),
                        "active_tasks": s.active_tasks.len(),
                    })
                })
                .collect();

            Ok(AgentResponse {
                agent_id: self.id,
                agent_name: self.name.clone(),
                result: AgentResultKind::Success {
                    summary: format!("{} Squads available", squads.len()),
                    output: json!({ "squads": squads }),
                },
                events: vec![],
            })
        } else if ins.contains("status") || ins.contains("状态") {
            if let Some(squad) = self.squad() {
                let active_task_count = squad.active_tasks.len();
                let queue_task_count = squad.task_queue.len();
                let member_count = squad.members.len();

                Ok(AgentResponse {
                    agent_id: self.id,
                    agent_name: self.name.clone(),
                    result: AgentResultKind::Success {
                        summary: format!(
                            "Squad '{}': {} members, {} active tasks, {} queued",
                            squad.name, member_count, active_task_count, queue_task_count
                        ),
                        output: json!({
                            "squad_name": squad.name,
                            "squad_id": squad.id.0,
                            "leader_id": squad.leader.0,
                            "members": member_count,
                            "active_tasks": active_task_count,
                            "queued_tasks": queue_task_count,
                        }),
                    },
                    events: vec![],
                })
            } else {
                Ok(AgentResponse {
                    agent_id: self.id,
                    agent_name: self.name.clone(),
                    result: AgentResultKind::Failed {
                        reason: "Squad not found".to_string(),
                    },
                    events: vec![],
                })
            }
        } else {
            Ok(AgentResponse {
                agent_id: self.id,
                agent_name: self.name.clone(),
                result: AgentResultKind::Failed {
                    reason: "Unknown Squad command. Try: create <name> <leader_id> [policy], add <agent_id> to <squad_id>, submit <title> [description] to <squad_id>, list, status".to_string(),
                },
                events: vec![],
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::squad::SquadRegistry;

    #[tokio::test]
    async fn test_create_squad_via_agent() {
        let registry = SquadRegistry::new();
        let mut agent = SquadAgent::new(AgentId(100), "SquadManager".into(), SquadId(0), registry);

        let req = AgentRequest {
            task_id: Some("test".into()),
            instruction: "create squad".into(),
            context: json!({
                "name": "GameDevTeam",
                "leader_id": 1,
                "policy": "skill_based"
            }),
        };

        let res = agent.handle(req).await.unwrap();
        assert!(matches!(res.result, AgentResultKind::Success { .. }));
    }
}
