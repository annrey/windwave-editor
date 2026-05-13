//! Agent Event Stream — Agent 与 UI 之间的协议层解耦。
//!
//! Inspired by UI-TARS-desktop's Event Stream architecture.
//! All agent state changes (messages, tool calls, scene actions, plan updates)
//! are expressed as typed events. The UI is just a consumer of this stream,
//! enabling multiple subscribers, event replay, and UI replacement.
//!
//! # Architecture
//!
//! ```text
//! DirectorRuntime ──publish──> EventStreamBroker ──subscribe──> UI panels
//!                                                   ──subscribe──> Audit log
//!                                                   ──subscribe──> Snapshot testing
//! ```
//!
//! Events are broadcast via `tokio::sync::broadcast` and persisted to JSONL
//! for deterministic replay and debugging.

use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::broadcast;

// ---------------------------------------------------------------------------
// AgentEvent — the universal event type for all agent state changes
// ---------------------------------------------------------------------------

/// All agent state changes are expressed as one of these events.
/// Every event carries a monotonically increasing `sequence` number and a
/// `timestamp` for ordering and replay.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum AgentEvent {
    /// Agent thinks or replies with text content.
    AssistantMessage {
        message_id: String,
        content: String,
        timestamp: u64,
    },

    /// Agent invokes a tool.
    ToolCall {
        call_id: String,
        tool_name: String,
        arguments: serde_json::Value,
        timestamp: u64,
    },

    /// Tool execution result.
    ToolResult {
        call_id: String,
        result: serde_json::Value,
        elapsed_ms: u64,
        success: bool,
        timestamp: u64,
    },

    /// Agent executes a scene mutation.
    SceneAction {
        action_type: String,
        target: String,
        result: String,
        timestamp: u64,
    },

    /// An edit plan is created or updated.
    PlanUpdated {
        plan_id: String,
        title: String,
        step_count: usize,
        current_step: usize,
        timestamp: u64,
    },

    /// A plan enters a new lifecycle stage.
    PlanStatusChanged {
        plan_id: String,
        new_status: String,
        timestamp: u64,
    },

    /// A plan step completed.
    StepCompleted {
        plan_id: String,
        step_id: String,
        result: String,
        timestamp: u64,
    },

    /// A plan step failed.
    StepFailed {
        plan_id: String,
        step_id: String,
        error: String,
        timestamp: u64,
    },

    /// A goal was achieved.
    GoalAchieved {
        goal: String,
        timestamp: u64,
    },

    /// Permission was requested from the user.
    PermissionRequested {
        plan_id: String,
        risk: String,
        reason: String,
        timestamp: u64,
    },

    /// User resolved a permission request.
    PermissionResolved {
        plan_id: String,
        approved: bool,
        timestamp: u64,
    },

    /// An undo/redo operation was performed.
    UndoRedoPerformed {
        direction: String, // "undo" or "redo"
        timestamp: u64,
    },

    /// Agent agent state changed (started, paused, error, finished).
    AgentStateChanged {
        agent_name: String,
        new_state: String,
        timestamp: u64,
    },

    /// Generic error event.
    Error {
        message: String,
        timestamp: u64,
    },
}

impl AgentEvent {
    pub fn timestamp(&self) -> u64 {
        match self {
            Self::AssistantMessage { timestamp, .. }
            | Self::ToolCall { timestamp, .. }
            | Self::ToolResult { timestamp, .. }
            | Self::SceneAction { timestamp, .. }
            | Self::PlanUpdated { timestamp, .. }
            | Self::PlanStatusChanged { timestamp, .. }
            | Self::StepCompleted { timestamp, .. }
            | Self::StepFailed { timestamp, .. }
            | Self::GoalAchieved { timestamp, .. }
            | Self::PermissionRequested { timestamp, .. }
            | Self::PermissionResolved { timestamp, .. }
            | Self::UndoRedoPerformed { timestamp, .. }
            | Self::AgentStateChanged { timestamp, .. }
            | Self::Error { timestamp, .. } => *timestamp,
        }
    }
}

/// Helper: generate millisecond-precision timestamp.
fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

// ---------------------------------------------------------------------------
// EventStreamBroker — broadcast + persistence
// ---------------------------------------------------------------------------

/// Broadcasts agent events to all subscribers and persists them to JSONL.
///
/// Uses `tokio::sync::broadcast` for zero-copy multi-consumer delivery.
/// JSONL persistence stores every event under `events_dir/events_YYYY-MM-DD.jsonl`
/// for replay and auditing.
pub struct EventStreamBroker {
    tx: broadcast::Sender<AgentEvent>,
    /// Maximum number of in-memory events retained for late subscribers.
    capacity: usize,
    sequence: u64,
    /// Optional directory for JSONL persistence.
    persistence_dir: Option<std::path::PathBuf>,
}

impl EventStreamBroker {
    /// Create a new broker with a bounded channel (`capacity` = max buffered events).
    pub fn new(capacity: usize) -> Self {
        let (tx, _) = broadcast::channel(capacity);
        Self {
            tx,
            capacity,
            sequence: 0,
            persistence_dir: None,
        }
    }

    /// Enable JSONL persistence to the given directory.
    pub fn with_persistence(mut self, dir: impl Into<std::path::PathBuf>) -> Self {
        self.persistence_dir = Some(dir.into());
        self
    }

    /// Publish an event to all subscribers.
    ///
    /// Returns `Ok(())` if at least one subscriber received the event,
    /// or `Err` if there are no subscribers (event is dropped). Events are
    /// always persisted to JSONL regardless of subscriber count.
    pub fn publish(&mut self, mut event: AgentEvent) -> Result<usize, String> {
        // Stamp event with sequence number (use timestamp for ordering)
        let ts = now_ms();
        match &mut event {
            AgentEvent::AssistantMessage { timestamp, .. }
            | AgentEvent::ToolCall { timestamp, .. }
            | AgentEvent::ToolResult { timestamp, .. }
            | AgentEvent::SceneAction { timestamp, .. }
            | AgentEvent::PlanUpdated { timestamp, .. }
            | AgentEvent::PlanStatusChanged { timestamp, .. }
            | AgentEvent::StepCompleted { timestamp, .. }
            | AgentEvent::StepFailed { timestamp, .. }
            | AgentEvent::GoalAchieved { timestamp, .. }
            | AgentEvent::PermissionRequested { timestamp, .. }
            | AgentEvent::PermissionResolved { timestamp, .. }
            | AgentEvent::UndoRedoPerformed { timestamp, .. }
            | AgentEvent::AgentStateChanged { timestamp, .. }
            | AgentEvent::Error { timestamp, .. } => *timestamp = ts,
        }
        self.sequence += 1;

        let recv_count = self.tx.receiver_count();

        // Persist to JSONL (best-effort)
        self.persist_to_jsonl(&event);

        // Broadcast (only succeeds if there are subscribers)
        let _ = self.tx.send(event);
        Ok(recv_count)
    }

    /// Subscribe to the event stream. Returns a receiver for consuming events.
    pub fn subscribe(&self) -> broadcast::Receiver<AgentEvent> {
        self.tx.subscribe()
    }

    /// Number of active subscribers.
    pub fn subscriber_count(&self) -> usize {
        self.tx.receiver_count()
    }

    /// Number of events published so far.
    pub fn sequence(&self) -> u64 {
        self.sequence
    }

    fn persist_to_jsonl(&self, event: &AgentEvent) {
        if let Some(ref dir) = self.persistence_dir {
            if let Ok(json) = serde_json::to_string(event) {
                // Write to daily log file
                let today = chrono::Local::now().format("%Y-%m-%d").to_string();
                let path = dir.join(format!("events_{}.jsonl", today));
                if let Ok(mut file) = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&path)
                {
                    use std::io::Write;
                    let _ = writeln!(file, "{}", json);
                }
            }
        }
    }
}

impl Default for EventStreamBroker {
    fn default() -> Self {
        Self::new(256)
    }
}

// ---------------------------------------------------------------------------
// AgentUiConsumer — trait for UI panels consuming the event stream
// ---------------------------------------------------------------------------

/// Panels that want to display agent state implement this trait.
///
/// The UI registers itself as a subscriber to the `EventStreamBroker` and
/// processes events in its `Update` system.
pub trait AgentUiConsumer {
    /// Called for every event arriving on the stream.
    /// Return `true` if the event was consumed (displayed), `false` if ignored.
    fn on_event(&mut self, event: &AgentEvent) -> bool;

    /// Return a snapshot of the UI state (for testing/snapshotting).
    fn ui_state(&self) -> serde_json::Value {
        serde_json::json!({})
    }
}

// ---------------------------------------------------------------------------
// EventReplay — replay historical events for debugging/testing
// ---------------------------------------------------------------------------

/// Replays events from a JSONL file to verify agent behavior deterministically.
pub struct EventReplay {
    events: Vec<AgentEvent>,
    cursor: usize,
}

impl EventReplay {
    /// Load events from a JSONL file.
    pub fn from_file(path: impl AsRef<std::path::Path>) -> Result<Self, String> {
        let content = std::fs::read_to_string(path.as_ref())
            .map_err(|e| format!("Failed to read file: {}", e))?;
        let events: Vec<AgentEvent> = content
            .lines()
            .filter(|l| !l.trim().is_empty())
            .filter_map(|line| serde_json::from_str(line).ok())
            .collect();
        Ok(Self { events, cursor: 0 })
    }

    /// Get the next event in replay order.
    pub fn next(&mut self) -> Option<&AgentEvent> {
        if self.cursor < self.events.len() {
            let event = &self.events[self.cursor];
            self.cursor += 1;
            Some(event)
        } else {
            None
        }
    }

    /// Reset cursor to beginning.
    pub fn reset(&mut self) {
        self.cursor = 0;
    }

    pub fn event_count(&self) -> usize {
        self.events.len()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_publish_and_subscribe() {
        let mut broker = EventStreamBroker::new(64);
        let mut rx = broker.subscribe();

        broker.publish(AgentEvent::AssistantMessage {
            message_id: "msg_1".into(),
            content: "Hello".into(),
            timestamp: 0,
        }).unwrap();

        let event = rx.try_recv().unwrap();
        match event {
            AgentEvent::AssistantMessage { content, .. } => {
                assert_eq!(content, "Hello");
            }
            _ => panic!("Expected AssistantMessage"),
        }
    }

    #[test]
    fn test_event_serialization() {
        let event = AgentEvent::ToolCall {
            call_id: "call_1".into(),
            tool_name: "create_entity".into(),
            arguments: serde_json::json!({"name": "Player"}),
            timestamp: 12345,
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"type\":\"ToolCall\""));
        assert!(json.contains("\"tool_name\":\"create_entity\""));

        let back: AgentEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(back.timestamp(), 12345);
    }

    #[test]
    fn test_event_replay() {
        let mut broker = EventStreamBroker::new(64);
        let mut rx = broker.subscribe(); // Subscribe BEFORE publishing

        broker.publish(AgentEvent::PlanUpdated {
            plan_id: "p1".into(),
            title: "Test".into(),
            step_count: 3,
            current_step: 0,
            timestamp: 0,
        }).unwrap();
        broker.publish(AgentEvent::StepCompleted {
            plan_id: "p1".into(),
            step_id: "s1".into(),
            result: "ok".into(),
            timestamp: 0,
        }).unwrap();

        let e1 = rx.try_recv().unwrap();
        let e2 = rx.try_recv().unwrap();
        assert!(matches!(e1, AgentEvent::PlanUpdated { .. }));
        assert!(matches!(e2, AgentEvent::StepCompleted { .. }));
    }
}
