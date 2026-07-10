//! Livelog — structured streaming log for agent execution.
//!
//! Provides a ring-buffer of timestamped log entries with support for
//! streaming callbacks (e.g. WebSocket, SSE) and tail-based retrieval.
//!
//! ## Example
//!
//! ```ignore
//! let mut log = Livelog::new(500);
//! log.subscribe_stream(Box::new(|entry| {
//!     println!("[{}] {}: {}", entry.level, entry.source, entry.message);
//!     true // continue streaming
//! }));
//! log.info("scene_agent", "Created enemy entity", None);
//! ```

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

// ---------------------------------------------------------------------------
// Entry
// ---------------------------------------------------------------------------

/// A single log entry produced during agent execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LivelogEntry {
    /// When the entry was created (UTC)
    pub timestamp: DateTime<Utc>,
    /// Severity level
    pub level: LivelogLevel,
    /// Source component (e.g. "scene_agent", "skill_executor", "tool.create_entity")
    pub source: String,
    /// Human-readable message
    pub message: String,
    /// Optional structured data (tool params, results, etc.)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

// ---------------------------------------------------------------------------
// Level
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LivelogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

impl std::fmt::Display for LivelogLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LivelogLevel::Debug => write!(f, "DEBUG"),
            LivelogLevel::Info => write!(f, "INFO"),
            LivelogLevel::Warn => write!(f, "WARN"),
            LivelogLevel::Error => write!(f, "ERROR"),
        }
    }
}

// ---------------------------------------------------------------------------
// Callback type
// ---------------------------------------------------------------------------

/// Callback for streaming log entries.
/// Return `true` to continue, `false` to unsubscribe.
pub type LivelogCallback = Box<dyn FnMut(&LivelogEntry) -> bool + Send + 'static>;

// ---------------------------------------------------------------------------
// Livelog
// ---------------------------------------------------------------------------

/// Thread-safe ring-buffer log with streaming callbacks.
pub struct Livelog {
    entries: VecDeque<LivelogEntry>,
    capacity: usize,
    callbacks: Vec<LivelogCallback>,
    /// Monotonic ID for each entry (useful for gated polling)
    next_id: u64,
    /// ID of the last emitted entry
    last_emitted_id: u64,
}

impl Livelog {
    /// Create a new livelog with the given capacity for the ring buffer.
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: VecDeque::with_capacity(capacity),
            capacity,
            callbacks: Vec::new(),
            next_id: 0,
            last_emitted_id: 0,
        }
    }

    /// Log an entry at `Info` level.
    pub fn info(&mut self, source: &str, message: &str, data: Option<Value>) {
        self.push(LivelogLevel::Info, source, message, data);
    }

    /// Log an entry at `Warn` level.
    pub fn warn(&mut self, source: &str, message: &str, data: Option<Value>) {
        self.push(LivelogLevel::Warn, source, message, data);
    }

    /// Log an entry at `Error` level.
    pub fn error(&mut self, source: &str, message: &str, data: Option<Value>) {
        self.push(LivelogLevel::Error, source, message, data);
    }

    /// Log an entry at `Debug` level.
    pub fn debug(&mut self, source: &str, message: &str, data: Option<Value>) {
        self.push(LivelogLevel::Debug, source, message, data);
    }

    /// Push a new entry onto the ring buffer and notify callbacks.
    fn push(&mut self, level: LivelogLevel, source: &str, message: &str, data: Option<Value>) {
        let entry = LivelogEntry {
            timestamp: Utc::now(),
            level,
            source: source.to_string(),
            message: message.to_string(),
            data,
        };

        if self.entries.len() >= self.capacity {
            self.entries.pop_front();
        }
        self.entries.push_back(entry.clone());
        self.next_id += 1;

        // Notify streaming callbacks
        self.callbacks.retain_mut(|cb| cb(&entry));
    }

    // --- Retrieval ---

    /// Return the most recent `n` entries (newest last).
    pub fn tail(&self, n: usize) -> Vec<&LivelogEntry> {
        let start = if n >= self.entries.len() {
            0
        } else {
            self.entries.len() - n
        };
        self.entries.iter().skip(start).collect()
    }

    /// Return all entries currently in the buffer.
    pub fn all(&self) -> Vec<&LivelogEntry> {
        self.entries.iter().collect()
    }

    /// Return entries since a given monotonic `since_id` (inclusive).
    /// `since_id` should be `last_emitted_id` from a previous call.
    pub fn poll_since(&mut self, since_id: u64) -> Vec<&LivelogEntry> {
        let count = self.next_id - since_id;
        if count == 0 {
            return vec![];
        }
        let start = self.entries.len().saturating_sub(count as usize);
        self.last_emitted_id = self.next_id;
        self.entries.iter().skip(start).collect()
    }

    /// Get the current monotonic ID (useful as baseline for `poll_since`).
    pub fn current_id(&self) -> u64 {
        self.next_id
    }

    // --- Streaming ---

    /// Subscribe a callback that receives every new entry pushed to the log.
    /// The callback returns `true` to stay subscribed, `false` to unsubscribe.
    pub fn subscribe_stream(&mut self, callback: LivelogCallback) {
        self.callbacks.push(callback);
    }

    /// Number of active streaming subscriptions.
    pub fn subscriber_count(&self) -> usize {
        self.callbacks.len()
    }

    /// Clear all streaming subscriptions.
    pub fn unsubscribe_all(&mut self) {
        self.callbacks.clear();
    }

    // --- Misc ---

    /// Number of entries in the buffer.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Clear all entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

// ---------------------------------------------------------------------------
// Thread-safe wrapper
// ---------------------------------------------------------------------------

/// Thread-safe (Arc<Mutex<>>) livelog for sharing across agents.
pub type SharedLivelog = Arc<Mutex<Livelog>>;

/// Create a new shared livelog.
pub fn create_shared_livelog(capacity: usize) -> SharedLivelog {
    Arc::new(Mutex::new(Livelog::new(capacity)))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_push_and_tail() {
        let mut log = Livelog::new(10);
        log.info("test", "msg1", None);
        log.info("test", "msg2", None);
        log.info("test", "msg3", None);

        let tail = log.tail(2);
        assert_eq!(tail.len(), 2);
        assert_eq!(tail[0].message, "msg2");
        assert_eq!(tail[1].message, "msg3");
    }

    #[test]
    fn test_ring_buffer_overflow() {
        let mut log = Livelog::new(3);
        log.info("test", "a", None);
        log.info("test", "b", None);
        log.info("test", "c", None);
        log.info("test", "d", None);

        assert_eq!(log.len(), 3);
        let all = log.all();
        assert_eq!(all[0].message, "b");
        assert_eq!(all[2].message, "d");
    }

    #[test]
    fn test_level_ordering() {
        assert!(LivelogLevel::Debug < LivelogLevel::Info);
        assert!(LivelogLevel::Warn < LivelogLevel::Error);
    }

    #[test]
    fn test_stream_callback() {
        let mut log = Livelog::new(10);
        let collected = Arc::new(Mutex::new(Vec::new()));
        let c2 = collected.clone();

        log.subscribe_stream(Box::new(move |entry| {
            c2.lock().unwrap().push(entry.message.clone());
            true
        }));

        log.info("test", "hello", None);
        log.warn("test", "world", None);

        let msgs = collected.lock().unwrap();
        assert_eq!(msgs.len(), 2);
        assert_eq!(msgs[0], "hello");
        assert_eq!(msgs[1], "world");
    }

    #[test]
    fn test_stream_unsubscribe() {
        let mut log = Livelog::new(10);
        let mut called = 0;
        log.subscribe_stream(Box::new(move |_entry| {
            called += 1;
            called < 2 // unsubscribe after 2 calls
        }));

        log.info("test", "a", None);
        log.info("test", "b", None);
        log.info("test", "c", None);

        assert_eq!(
            log.subscriber_count(),
            0,
            "Callback should have been removed"
        );
    }

    #[test]
    fn test_poll_since() {
        let mut log = Livelog::new(10);
        let baseline = log.current_id();
        log.info("test", "msg1", None);
        log.info("test", "msg2", None);

        let new_entries = log.poll_since(baseline);
        assert_eq!(new_entries.len(), 2);
    }

    #[test]
    fn test_shared_livelog() {
        let log = create_shared_livelog(10);
        {
            let mut guard = log.lock().unwrap();
            guard.info("shared", "test_msg", None);
        }
        let guard = log.lock().unwrap();
        let entries = guard.all();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].message, "test_msg");
    }
}
