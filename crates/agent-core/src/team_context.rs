//! Team context — per-agent isolated context with shared knowledge pool.
//!
//! Each agent maintains its own `ConversationMemory` for focused task execution,
//! while shared knowledge (project info, team roster, common patterns) lives in
//! the `CommunicationHub::SharedContext`. When HR onboards a new agent, the agent
//! receives a bootstrapped context from shared knowledge.

use crate::agent_comm::CommunicationHub;
use crate::message_buffer::MessageBuffer;
use crate::registry::AgentId;
use serde_json::json;
use std::collections::HashMap;

fn uuid_str() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    format!("ctx_{}", COUNTER.fetch_add(1, Ordering::Relaxed))
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// A per-agent isolated context with access to shared team knowledge.
#[derive(Debug)]
pub struct TeamAgentContext {
    /// This agent's private message buffer.
    pub memory: MessageBuffer,
    /// Shared team knowledge (reference to CommunicationHub's SharedContext).
    /// Wrapped in AgentId for ownership tracking.
    pub agent_id: AgentId,
    /// Whether this agent's context has been initialized from team knowledge.
    pub initialized: bool,
}

impl TeamAgentContext {
    pub fn new(agent_id: AgentId, max_messages: usize) -> Self {
        Self {
            memory: MessageBuffer::new(max_messages),
            agent_id,
            initialized: false,
        }
    }

    /// Bootstrap this agent's context from shared team knowledge.
    pub fn bootstrap(&mut self, hub: &CommunicationHub) {
        if self.initialized { return; }

        let project_info = hub.context.get("project:info");
        let team_roster = hub.context.get("team:roster");
        let patterns = hub.context.get("team:patterns");

        let mut bootstrap = String::from("[Team Bootstrap] ");
        if let Some(ref info) = project_info {
            bootstrap.push_str(&format!("Project: {}. ", info));
        }
        if let Some(ref roster) = team_roster {
            bootstrap.push_str(&format!("Team: {}. ", roster));
        }
        if let Some(ref patterns) = patterns {
            bootstrap.push_str(&format!("Patterns: {}. ", patterns));
        }

        let msg = crate::types::Message {
            id: uuid_str(),
            message_type: crate::types::MessageType::System,
            content: bootstrap,
            timestamp: now_secs(),
            metadata: std::collections::HashMap::new(),
        };
        self.memory.add_message(msg);
        self.initialized = true;
    }

    /// Add a message to this agent's private memory.
    pub fn memorize(&mut self, message: crate::types::Message) {
        self.memory.add_message(message);
    }
}

/// Registry of all agent contexts on the team.
#[derive(Debug)]
pub struct TeamContextRegistry {
    contexts: HashMap<u64, TeamAgentContext>,
    max_messages_per_agent: usize,
}

impl TeamContextRegistry {
    pub fn new(max_messages: usize) -> Self {
        Self { contexts: HashMap::new(), max_messages_per_agent: max_messages }
    }

    /// Register a new agent's context.
    pub fn register(&mut self, agent_id: AgentId, hub: &Option<CommunicationHub>) {
        let mut ctx = TeamAgentContext::new(agent_id, self.max_messages_per_agent);
        if let Some(ref hub) = hub {
            ctx.bootstrap(hub);
        }
        self.contexts.insert(agent_id.0, ctx);
    }

    /// Remove an agent's context.
    pub fn unregister(&mut self, agent_id: AgentId) {
        self.contexts.remove(&agent_id.0);
    }

    /// Get mutable reference to an agent's context.
    pub fn get_mut(&mut self, agent_id: AgentId) -> Option<&mut TeamAgentContext> {
        self.contexts.get_mut(&agent_id.0)
    }

    /// Get an agent's context.
    pub fn get(&self, agent_id: AgentId) -> Option<&TeamAgentContext> {
        self.contexts.get(&agent_id.0)
    }

    /// Memorize a message for a specific agent only.
    pub fn memorize(&mut self, agent_id: AgentId, message: crate::types::Message) -> bool {
        if let Some(ctx) = self.contexts.get_mut(&agent_id.0) {
            ctx.memorize(message);
            true
        } else {
            false
        }
    }

    /// Number of registered contexts.
    pub fn len(&self) -> usize {
        self.contexts.len()
    }

    pub fn is_empty(&self) -> bool {
        self.contexts.is_empty()
    }
}

impl Default for TeamContextRegistry {
    fn default() -> Self {
        Self::new(100)
    }
}

// ---------------------------------------------------------------------------
// CommunicationHub extensions for team context
// ---------------------------------------------------------------------------

impl CommunicationHub {
    /// Publish shared team context that all agents can access.
    pub fn publish_team_context(&self, key: &str, value: serde_json::Value) {
        let _ = self.share_context(
            crate::registry::AgentId::default(),
            key,
            value,
        );
    }

    /// Initialize team knowledge in shared context.
    pub fn init_team_knowledge(
        &self,
        project_name: &str,
        project_path: &str,
    ) {
        self.publish_team_context("project:info", json!({
            "name": project_name,
            "path": project_path,
            "language": "Rust",
            "engine": "Bevy 0.17",
        }));

        self.publish_team_context("team:roster", json!({
            "agents": [],
            "default_roles": ["director", "planner", "executor", "reviewer", "hr"],
        }));

        self.publish_team_context("team:patterns", json!({
            "common_operations": [
                "create entity",
                "delete entity",
                "move entity",
                "change color",
                "modify component",
            ],
        }));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_context_bootstrap() {
        let mut ctx = TeamAgentContext::new(AgentId(1), 50);
        assert!(!ctx.initialized);

        // Without hub, bootstrap is a no-op
        let hub = CommunicationHub::new();
        hub.init_team_knowledge("TestProject", "/tmp/test");
        ctx.bootstrap(&hub);
        assert!(ctx.initialized);
    }

    #[test]
    fn test_team_context_registry() {
        let mut registry = TeamContextRegistry::new(100);
        registry.register(AgentId(1), &None);
        registry.register(AgentId(2), &None);

        assert_eq!(registry.len(), 2);

        let msg = crate::types::Message::new_user("test".to_string());
        assert!(registry.memorize(AgentId(1), msg));

        registry.unregister(AgentId(1));
        assert_eq!(registry.len(), 1);
    }
}
