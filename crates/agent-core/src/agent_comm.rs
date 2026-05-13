//! Agent Communication Module - Inter-agent messaging and context sharing
//!
//! Provides message passing and shared context for multi-agent collaboration.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// Unique message identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MessageId(pub u64);

/// Agent message for inter-agent communication
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMessage {
    pub id: MessageId,
    pub from: crate::registry::AgentId,
    pub to: Option<crate::registry::AgentId>, // None = broadcast
    pub msg_type: MessageType,
    pub payload: MessagePayload,
    pub timestamp: f64,
    pub correlation_id: Option<String>, // For request/response pairing
}

/// Message types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageType {
    Request,
    Response,
    Notification,
    Command,
    Event,
}

/// Message payload content
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessagePayload {
    /// Text content
    Text { content: String },
    /// Structured data
    Data { data: serde_json::Value },
    /// Task assignment
    Task { task_id: String, description: String, priority: TaskPriority },
    /// Result of task execution
    Result { task_id: String, success: bool, output: serde_json::Value },
    /// Shared context update
    ContextUpdate { key: String, value: serde_json::Value },
    /// Query for information
    Query { query_type: String, parameters: serde_json::Value },
}

/// Task priority levels
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum TaskPriority {
    Critical = 0,
    High = 1,
    Normal = 2,
    Low = 3,
}

impl Default for TaskPriority {
    fn default() -> Self {
        TaskPriority::Normal
    }
}

/// Shared context for agent collaboration
#[derive(Debug, Clone)]
pub struct SharedContext {
    /// Context entries with timestamps
    entries: Arc<Mutex<HashMap<String, ContextEntry>>>,
    /// Maximum entries to prevent unbounded growth
    max_entries: usize,
}

/// Single context entry
#[derive(Debug, Clone)]
struct ContextEntry {
    value: serde_json::Value,
    updated_at: Instant,
    updated_by: crate::registry::AgentId,
}

impl SharedContext {
    pub fn new() -> Self {
        Self {
            entries: Arc::new(Mutex::new(HashMap::new())),
            max_entries: 1000,
        }
    }

    /// Set a context value
    pub fn set(
        &self,
        key: impl Into<String>,
        value: impl Serialize,
        agent_id: crate::registry::AgentId,
    ) -> Result<(), AgentCommError> {
        let key = key.into();
        let value = serde_json::to_value(value)
            .map_err(|e| AgentCommError::SerializeError(e.to_string()))?;

        let mut entries = self.entries.lock()
            .map_err(|_| AgentCommError::LockError)?;

        // Prevent unbounded growth
        if entries.len() >= self.max_entries && !entries.contains_key(&key) {
            // Remove oldest entry
            let oldest = entries.iter()
                .min_by_key(|(_, v)| v.updated_at)
                .map(|(k, _)| k.clone());
            if let Some(k) = oldest {
                entries.remove(&k);
            }
        }

        entries.insert(key, ContextEntry {
            value,
            updated_at: Instant::now(),
            updated_by: agent_id,
        });

        Ok(())
    }

    /// Get a context value
    pub fn get(&self, key: &str) -> Option<serde_json::Value> {
        let entries = self.entries.lock().ok()?;
        entries.get(key).map(|e| e.value.clone())
    }

    /// Get value with metadata
    pub fn get_with_meta(&self, key: &str) -> Option<(serde_json::Value, Instant, crate::registry::AgentId)> {
        let entries = self.entries.lock().ok()?;
        entries.get(key).map(|e| (
            e.value.clone(),
            e.updated_at,
            e.updated_by,
        ))
    }

    /// Remove a context value
    pub fn remove(&self, key: &str) -> Option<serde_json::Value> {
        let mut entries = self.entries.lock().ok()?;
        entries.remove(key).map(|e| e.value)
    }

    /// List all context keys
    pub fn keys(&self) -> Vec<String> {
        let entries = self.entries.lock()
            .map(|e| e.keys().cloned().collect())
            .unwrap_or_default();
        entries
    }

    /// Clear all context
    pub fn clear(&self) {
        if let Ok(mut entries) = self.entries.lock() {
            entries.clear();
        }
    }
}

impl Default for SharedContext {
    fn default() -> Self {
        Self::new()
    }
}

/// Message broker for agent communication
#[derive(Debug)]
pub struct MessageBroker {
    /// Message history
    messages: Arc<Mutex<Vec<AgentMessage>>>,
    /// Message counter for ID generation
    next_id: Arc<Mutex<u64>>,
    /// Subscribers by agent ID
    subscribers: Arc<Mutex<HashMap<crate::registry::AgentId, Vec<crossbeam::channel::Sender<AgentMessage>>>>>,
    /// Maximum message history
    max_history: usize,
}

impl MessageBroker {
    pub fn new() -> Self {
        Self {
            messages: Arc::new(Mutex::new(Vec::new())),
            next_id: Arc::new(Mutex::new(1)),
            subscribers: Arc::new(Mutex::new(HashMap::new())),
            max_history: 1000,
        }
    }

    /// Generate next message ID
    fn next_message_id(&self) -> MessageId {
        let mut id = self.next_id.lock().unwrap();
        let current = *id;
        *id += 1;
        MessageId(current)
    }

    /// Send a message
    pub fn send(&self, mut message: AgentMessage) -> Result<MessageId, AgentCommError> {
        // Assign ID if not set
        if message.id.0 == 0 {
            message.id = self.next_message_id();
        }
        message.timestamp = Instant::now().elapsed().as_secs_f64();

        // Store in history
        {
            let mut messages = self.messages.lock()
                .map_err(|_| AgentCommError::LockError)?;
            messages.push(message.clone());
            
            // Trim history
            if messages.len() > self.max_history {
                messages.remove(0);
            }
        }

        // Deliver to subscribers
        let subscribers = self.subscribers.lock()
            .map_err(|_| AgentCommError::LockError)?;

        // Direct message
        if let Some(target) = message.to {
            if let Some(subs) = subscribers.get(&target) {
                for sub in subs {
                    let _ = sub.try_send(message.clone());
                }
            }
        }

        // Broadcast to all subscribers
        if message.to.is_none() {
            for subs in subscribers.values() {
                for sub in subs {
                    let _ = sub.try_send(message.clone());
                }
            }
        }

        Ok(message.id)
    }

    /// Subscribe to messages for a specific agent
    pub fn subscribe(&self, agent_id: crate::registry::AgentId) -> crossbeam::channel::Receiver<AgentMessage> {
        let (tx, rx) = crossbeam::channel::unbounded();
        
        if let Ok(mut subscribers) = self.subscribers.lock() {
            subscribers.entry(agent_id)
                .or_insert_with(Vec::new)
                .push(tx);
        }
        
        rx
    }

    /// Get message history
    pub fn history(&self) -> Vec<AgentMessage> {
        self.messages.lock()
            .map(|m| m.clone())
            .unwrap_or_default()
    }

    /// Get messages for a specific agent
    pub fn messages_for(&self, agent_id: crate::registry::AgentId) -> Vec<AgentMessage> {
        self.messages.lock()
            .map(|m| {
                m.iter()
                    .filter(|msg| {
                        msg.to.map(|to| to == agent_id).unwrap_or(true) ||
                        msg.from == agent_id
                    })
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }
}

impl Default for MessageBroker {
    fn default() -> Self {
        Self::new()
    }
}

/// Communication errors
#[derive(Debug, thiserror::Error)]
pub enum AgentCommError {
    #[error("Serialization error: {0}")]
    SerializeError(String),
    
    #[error("Lock error")]
    LockError,
    
    #[error("Send error: {0}")]
    SendError(String),
}

/// Communication hub that combines broker and context
#[derive(Debug, Default)]
pub struct CommunicationHub {
    pub broker: MessageBroker,
    pub context: SharedContext,
}

impl CommunicationHub {
    pub fn new() -> Self {
        Self {
            broker: MessageBroker::new(),
            context: SharedContext::new(),
        }
    }

    /// Send a simple text message
    pub fn send_text(
        &self,
        from: crate::registry::AgentId,
        to: Option<crate::registry::AgentId>,
        content: impl Into<String>,
    ) -> Result<MessageId, AgentCommError> {
        let message = AgentMessage {
            id: MessageId(0),
            from,
            to,
            msg_type: MessageType::Notification,
            payload: MessagePayload::Text { content: content.into() },
            timestamp: 0.0,
            correlation_id: None,
        };
        self.broker.send(message)
    }

    /// Request task execution
    pub fn request_task(
        &self,
        from: crate::registry::AgentId,
        to: crate::registry::AgentId,
        task_id: impl Into<String>,
        description: impl Into<String>,
        priority: TaskPriority,
    ) -> Result<MessageId, AgentCommError> {
        let message = AgentMessage {
            id: MessageId(0),
            from,
            to: Some(to),
            msg_type: MessageType::Request,
            payload: MessagePayload::Task {
                task_id: task_id.into(),
                description: description.into(),
                priority,
            },
            timestamp: 0.0,
            correlation_id: None,
        };
        self.broker.send(message)
    }

    /// Share context with other agents
    pub fn share_context(
        &self,
        from: crate::registry::AgentId,
        key: impl Into<String>,
        value: impl Serialize,
    ) -> Result<(), AgentCommError> {
        let key_str: String = key.into();
        let value_json = serde_json::to_value(&value)
            .map_err(|e| AgentCommError::SerializeError(e.to_string()))?;
        
        // Set in context (value needs to be re-serialized or cloned)
        self.context.set(key_str.clone(), &value_json, from)?;
        
        // Also broadcast as message
        
        let message = AgentMessage {
            id: MessageId(0),
            from,
            to: None, // Broadcast
            msg_type: MessageType::Notification,
            payload: MessagePayload::ContextUpdate {
                key: key_str,
                value: value_json,
            },
            timestamp: 0.0,
            correlation_id: None,
        };
        let _ = self.broker.send(message);
        
        Ok(())
    }

    /// Subscribe an agent to receive messages of a specific type.
    /// Returns a receiver for the agent to poll.
    pub fn subscribe(&self, agent_id: crate::registry::AgentId) -> crossbeam::channel::Receiver<AgentMessage> {
        self.broker.subscribe(agent_id)
    }

    /// Broadcast a task to all agents matching a capability (for team dispatch)
    pub fn broadcast_to_capable(
        &self,
        from: crate::registry::AgentId,
        task_id: impl Into<String>,
        description: impl Into<String>,
        priority: TaskPriority,
    ) -> Result<MessageId, AgentCommError> {
        let message = AgentMessage {
            id: MessageId(0),
            from,
            to: None, // broadcast
            msg_type: MessageType::Request,
            payload: MessagePayload::Task {
                task_id: task_id.into(),
                description: description.into(),
                priority,
            },
            timestamp: 0.0,
            correlation_id: None,
        };
        self.broker.send(message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_shared_context() {
        let ctx = SharedContext::new();
        let agent_id = crate::registry::AgentId::default();
        
        ctx.set("test_key", "test_value", agent_id).unwrap();
        assert_eq!(
            ctx.get("test_key"),
            Some(serde_json::Value::String("test_value".to_string()))
        );
    }
    
    #[test]
    fn test_message_broker() {
        let broker = MessageBroker::new();
        let agent_id = crate::registry::AgentId::default();
        
        let message = AgentMessage {
            id: MessageId(0),
            from: agent_id,
            to: None,
            msg_type: MessageType::Notification,
            payload: MessagePayload::Text { content: "Hello".to_string() },
            timestamp: 0.0,
            correlation_id: None,
        };
        
        let id = broker.send(message).unwrap();
        assert_ne!(id.0, 0);
        
        let history = broker.history();
        assert_eq!(history.len(), 1);
    }
}
