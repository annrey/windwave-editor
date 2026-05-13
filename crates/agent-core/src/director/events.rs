//! Event management methods for DirectorRuntime.

use crate::event::EventBus;
use super::types::{DirectorRuntime, EditorEvent, DirectorTraceEntry};

impl DirectorRuntime {
    #[allow(dead_code)] // Capacity limits reserved for future event/trace management
    const MAX_EVENTS: usize = 1000;
    #[allow(dead_code)]
    const MAX_TRACE_ENTRIES: usize = 500;

    /// Get the most recent `n` events from the event log.
    ///
    /// Returns fewer than `n` if fewer events exist.
    ///
    /// # Arguments
    ///
    /// * `n` - Maximum number of events to return.
    pub fn recent_events(&self, n: usize) -> Vec<EditorEvent> {
        self.recent_events_internal(n)
    }

    /// Get the full execution trace for debugging / audit.
    pub fn trace(&self) -> Vec<DirectorTraceEntry> {
        self.trace_entries.clone()
    }

    /// Returns the current trace log for debugging purposes.
    pub fn trace_log(&self) -> &[DirectorTraceEntry] {
        &self.trace_entries
    }

    /// Access the EventBus for subscribing from UI / engine layers.
    pub fn event_bus(&self) -> &EventBus {
        &self.event_bus
    }

    /// Mutable access to EventBus (for dispatching events to subscribers).
    pub fn event_bus_mut(&mut self) -> &mut EventBus {
        &mut self.event_bus
    }

    // ------------------------------------------------------------------
    // Internal helpers
    // ------------------------------------------------------------------

    /// Internal helper: return the last `n` events from the event log.
    pub(crate) fn recent_events_internal(&self, n: usize) -> Vec<EditorEvent> {
        let len = self.events.len();
        let count = n.min(len);
        let start = len.saturating_sub(count);
        self.events[start..].to_vec()
    }

    /// Internal helper: add event with capacity limit
    #[allow(dead_code)]
    pub(crate) fn add_event(&mut self, event: EditorEvent) {
        self.events.push(event);
        if self.events.len() > Self::MAX_EVENTS {
            self.events.drain(0..self.events.len() - Self::MAX_EVENTS);
        }
    }

    /// Internal helper: add trace entry with capacity limit
    #[allow(dead_code)]
    pub(crate) fn add_trace_entry(&mut self, entry: DirectorTraceEntry) {
        self.trace_entries.push(entry);
        if self.trace_entries.len() > Self::MAX_TRACE_ENTRIES {
            self.trace_entries.drain(0..self.trace_entries.len() - Self::MAX_TRACE_ENTRIES);
        }
    }
}
