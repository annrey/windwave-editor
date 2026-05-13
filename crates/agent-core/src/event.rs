//! Event system types — the communication backbone between agents, the Director,
//! and the UI layer. Events are pushed onto a lightweight in-process bus and
//! consumed by interested subscribers.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::permission::{OperationRisk, PermissionDecision};

/// Every significant action in the editor produces an `EditorEvent`. The event
/// carries enough context so that subscribers (UI, logger, other agents) can
/// react without polling.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EventBusEvent {
    /// A raw user request has been received (text form).
    UserRequestReceived { request: String },

    /// The Planner has produced a new edit plan. The plan is stored as a JSON
    /// value to avoid a hard dependency on the `plan` module.
    EditPlanCreated { plan: serde_json::Value },

    /// An agent is requesting user permission for a potentially risky operation.
    PermissionRequested {
        plan_id: String,
        risk: OperationRisk,
        reason: String,
    },

    /// A permission request was resolved (approved, denied, or forbidden).
    PermissionResolved {
        plan_id: String,
        decision: PermissionDecision,
    },

    /// A new trace entry has been appended to an execution trace.
    ExecutionTraceUpdated {
        trace_id: String,
        entry: serde_json::Value,
    },

    /// A new transaction has been opened for a plan step.
    TransactionStarted {
        transaction_id: String,
        step_id: String,
        task_id: u64,
    },

    /// A transaction was successfully committed.
    TransactionCommitted { transaction_id: String },

    /// A transaction was rolled back (before or after commit).
    TransactionRolledBack { transaction_id: String },

    /// A low-level engine command was applied (success or failure).
    EngineCommandApplied {
        transaction_id: String,
        success: bool,
        message: String,
    },

    /// An agent has made an observation about the current scene / project state.
    ObservationCreated {
        observation_type: String,
        summary: String,
    },

    /// The Reviewer has checked a task goal against the current state.
    GoalChecked {
        task_id: u64,
        all_matched: bool,
        summary: String,
    },

    /// The Reviewer has completed its review cycle.
    ReviewCompleted {
        decision: String,
        summary: String,
    },
}

/// Who (or what) emitted the event. Useful for filtering and audit trails.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EventSource {
    User,
    Director,
    /// An agent identified by its human-readable name (e.g. `"planner"`).
    Agent(String),
    System,
}

// ---------------------------------------------------------------------------
// EventBus — lightweight in-process event queue
// ---------------------------------------------------------------------------

/// A simple publish/subscribe event bus scoped to a single process.
///
/// Events are stored with an auto-incrementing sequence number so consumers
/// can replay or catch-up from a known position.
#[derive(Debug, Clone)]
pub struct EventBus {
    /// Sequence number -> event pairs.
    events: Vec<(u64, EventBusEvent)>,

    /// Map: event variant name -> set of subscriber IDs.
    subscribers: HashMap<String, Vec<u64>>,

    /// Monotonically increasing counter for event sequence numbers.
    next_seq: u64,

    /// Monotonically increasing counter for subscriber IDs.
    next_sub_id: u64,
}

impl EventBus {
    /// Create an empty event bus.
    pub fn new() -> Self {
        Self {
            events: Vec::new(),
            subscribers: HashMap::new(),
            next_seq: 0,
            next_sub_id: 0,
        }
    }

    /// Push an event onto the bus and return its sequence number.
    pub fn push(&mut self, event: EventBusEvent) -> u64 {
        let seq = self.next_seq;
        self.next_seq += 1;
        self.events.push((seq, event));
        seq
    }

    /// Register interest in a particular event variant.
    ///
    /// `event_type` should be a string matching the variant name (e.g.
    /// `"PermissionRequested"`). Returns a subscriber ID that can be used to
    /// correlate events later (full filtering is left to consumers).
    pub fn subscribe(&mut self, event_type: &str) -> u64 {
        let id = self.next_sub_id;
        self.next_sub_id += 1;
        self.subscribers
            .entry(event_type.to_string())
            .or_default()
            .push(id);
        id
    }

    /// Return the total number of events ever pushed onto the bus.
    pub fn len(&self) -> usize {
        self.events.len()
    }

    /// Returns `true` when no events have been pushed.
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    /// Get a reference to all events (for inspection / replay).
    pub fn events(&self) -> &[(u64, EventBusEvent)] {
        &self.events
    }

    /// Get the subscriber IDs for a given event variant.
    pub fn subscribers_for(&self, event_type: &str) -> Option<&[u64]> {
        self.subscribers.get(event_type).map(|v| v.as_slice())
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_bus_push_and_len() {
        let mut bus = EventBus::new();
        assert!(bus.is_empty());
        assert_eq!(bus.len(), 0);

        bus.push(EventBusEvent::UserRequestReceived {
            request: "create enemy".into(),
        });
        assert_eq!(bus.len(), 1);

        bus.push(EventBusEvent::TransactionCommitted {
            transaction_id: "txn_1".into(),
        });
        assert_eq!(bus.len(), 2);
    }

    #[test]
    fn test_event_bus_sequence_numbers() {
        let mut bus = EventBus::new();
        let seq0 = bus.push(EventBusEvent::TransactionStarted {
            transaction_id: "txn_a".into(),
            step_id: "s1".into(),
            task_id: 10,
        });
        let seq1 = bus.push(EventBusEvent::TransactionCommitted {
            transaction_id: "txn_a".into(),
        });
        assert_eq!(seq0, 0);
        assert_eq!(seq1, 1);
    }

    #[test]
    fn test_event_bus_subscribe() {
        let mut bus = EventBus::new();
        let sub_id = bus.subscribe("TransactionStarted");
        assert_eq!(sub_id, 0);

        let sub_id2 = bus.subscribe("GoalChecked");
        assert_eq!(sub_id2, 1);

        let subs = bus.subscribers_for("TransactionStarted");
        assert!(subs.is_some());
        assert_eq!(subs.unwrap().len(), 1);
    }

    #[test]
    fn test_event_bus_events_view() {
        let mut bus = EventBus::new();
        bus.push(EventBusEvent::GoalChecked {
            task_id: 1,
            all_matched: true,
            summary: "all good".into(),
        });
        bus.push(EventBusEvent::ReviewCompleted {
            decision: "approved".into(),
            summary: "looks good".into(),
        });

        let events = bus.events();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].0, 0);
        assert_eq!(events[1].0, 1);
    }

    #[test]
    fn test_event_bus_subscribers_for_missing_type() {
        let bus = EventBus::new();
        assert!(bus.subscribers_for("NonexistentEvent").is_none());
    }

    #[test]
    fn test_event_source_variants() {
        let user = EventSource::User;
        let agent = EventSource::Agent("planner".into());
        let system = EventSource::System;

        assert!(matches!(user, EventSource::User));
        assert!(matches!(agent, EventSource::Agent(_)));
        assert!(matches!(system, EventSource::System));
    }
}
