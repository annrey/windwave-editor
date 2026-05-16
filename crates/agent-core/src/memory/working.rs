//! Working Memory (L3) - Short-term immediate context
//!
//! Holds temporary data the Agent needs during execution:
//! - Active conversation turns (limited by context window)
//! - Current entity selections and references
//! - Computed values and intermediate results
//! - Active tool calls and their states

use crate::types::{Message, EntityId};
use crate::memory::MemoryMetadata;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Types of entries in working memory
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EntryType {
    ConversationTurn,
    EntityReference,
    ComputedValue,
    ToolState,
    UserIntent,
    ContextHint,
}

/// A single entry in working memory
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkingMemoryEntry {
    pub entry_type: EntryType,
    pub content: String,
    pub metadata: MemoryMetadata,
    /// For conversation turns: the original message
    pub source_message: Option<Message>,
    /// For entity references: the entity ID
    pub entity_id: Option<EntityId>,
    /// For computed values: the computed result
    pub value_json: Option<serde_json::Value>,
    /// TTL in seconds (0 = no expiry)
    pub ttl_seconds: u64,
}

impl WorkingMemoryEntry {
    pub fn conversation(message: Message, id: u64) -> Self {
        Self {
            entry_type: EntryType::ConversationTurn,
            content: message.content.clone(),
            metadata: MemoryMetadata::new(id, crate::memory::MemoryTier::Working),
            source_message: Some(message),
            entity_id: None,
            value_json: None,
            ttl_seconds: 0,
        }
    }

    pub fn entity_reference(name: &str, id: EntityId, entry_id: u64) -> Self {
        Self {
            entry_type: EntryType::EntityReference,
            content: name.to_string(),
            metadata: MemoryMetadata::new(entry_id, crate::memory::MemoryTier::Working),
            source_message: None,
            entity_id: Some(id),
            value_json: None,
            ttl_seconds: 3600, // 1 hour TTL
        }
    }

    pub fn computed_value(key: &str, value: serde_json::Value, entry_id: u64) -> Self {
        Self {
            entry_type: EntryType::ComputedValue,
            content: key.to_string(),
            metadata: MemoryMetadata::new(entry_id, crate::memory::MemoryTier::Working),
            source_message: None,
            entity_id: None,
            value_json: Some(value),
            ttl_seconds: 1800, // 30 min TTL
        }
    }

    pub fn user_intent(intent: &str, entry_id: u64) -> Self {
        Self {
            entry_type: EntryType::UserIntent,
            content: intent.to_string(),
            metadata: MemoryMetadata::new(entry_id, crate::memory::MemoryTier::Working),
            source_message: None,
            entity_id: None,
            value_json: None,
            ttl_seconds: 0,
        }
    }

    pub fn context_hint(hint: &str, entry_id: u64) -> Self {
        Self {
            entry_type: EntryType::ContextHint,
            content: hint.to_string(),
            metadata: MemoryMetadata::new(entry_id, crate::memory::MemoryTier::Working),
            source_message: None,
            entity_id: None,
            value_json: None,
            ttl_seconds: 7200, // 2 hours TTL
        }
    }

    pub fn is_expired(&self) -> bool {
        if self.ttl_seconds == 0 {
            return false;
        }
        let now = crate::types::current_timestamp();
        now > self.metadata.created_at + self.ttl_seconds
    }
}

/// Working Memory - L3 immediate context
///
/// Design principles:
/// - Limited capacity (configurable, default 50 entries)
/// - FIFO eviction when full (except pinned entries)
/// - Automatic TTL-based cleanup
/// - Fast O(1) access by type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkingMemory {
    entries: Vec<WorkingMemoryEntry>,
    max_entries: usize,
    next_id: u64,
    /// Pinned entry IDs that should not be evicted
    pinned_ids: Vec<u64>,
    /// Quick lookup by entry type
    type_index: HashMap<EntryType, Vec<usize>>,
}

impl WorkingMemory {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Vec::with_capacity(max_entries),
            max_entries,
            next_id: 1,
            pinned_ids: Vec::new(),
            type_index: HashMap::new(),
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self::new(capacity)
    }

    /// Add an entry to working memory
    pub fn push(&mut self, entry: WorkingMemoryEntry) {
        // Remove expired entries first
        self.cleanup_expired();

        // If at capacity, evict oldest non-pinned entry
        if self.entries.len() >= self.max_entries {
            self.evict_oldest();
        }

        let entry_type = entry.entry_type.clone();
        let idx = self.entries.len();
        self.entries.push(entry);

        // Update type index
        self.type_index.entry(entry_type).or_default().push(idx);
    }

    /// Add a conversation message
    pub fn add_message(&mut self, message: Message) {
        let id = self.next_id();
        let entry = WorkingMemoryEntry::conversation(message, id);
        self.push(entry);
    }

    /// Register an entity reference
    pub fn register_entity(&mut self, name: &str, id: EntityId) {
        let entry_id = self.next_id();
        let entry = WorkingMemoryEntry::entity_reference(name, id, entry_id);
        self.push(entry);
    }

    /// Store a computed value
    pub fn set_value(&mut self, key: &str, value: serde_json::Value) {
        let entry_id = self.next_id();
        let entry = WorkingMemoryEntry::computed_value(key, value, entry_id);
        self.push(entry);
    }

    /// Store user intent
    pub fn set_intent(&mut self, intent: &str) {
        let entry_id = self.next_id();
        let entry = WorkingMemoryEntry::user_intent(intent, entry_id);
        self.push(entry);
    }

    /// Add context hint
    pub fn add_hint(&mut self, hint: &str) {
        let entry_id = self.next_id();
        let entry = WorkingMemoryEntry::context_hint(hint, entry_id);
        self.push(entry);
    }

    /// Get recent conversation messages
    pub fn recent_messages(&mut self, n: usize) -> Vec<&Message> {
        self.cleanup_expired();
        let indices = self.type_index.get(&EntryType::ConversationTurn).cloned().unwrap_or_default();
        indices.iter()
            .rev()
            .take(n)
            .filter_map(|&idx| self.entries.get(idx))
            .filter_map(|e| e.source_message.as_ref())
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect()
    }

    /// Get all conversation messages as context string
    pub fn conversation_context(&mut self) -> String {
        self.cleanup_expired();
        let indices = self.type_index.get(&EntryType::ConversationTurn).cloned().unwrap_or_default();
        indices.iter()
            .filter_map(|&idx| self.entries.get(idx))
            .filter_map(|e| e.source_message.as_ref())
            .map(|m| format!("[{:?}] {}", m.message_type, m.content))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Lookup entity by name
    pub fn lookup_entity(&mut self, name: &str) -> Option<EntityId> {
        self.cleanup_expired();
        let indices = self.type_index.get(&EntryType::EntityReference).cloned().unwrap_or_default();
        indices.iter()
            .rev()
            .filter_map(|&idx| self.entries.get(idx))
            .find(|e| e.content == name)
            .and_then(|e| e.entity_id)
    }

    /// Get computed value
    pub fn get_value(&mut self, key: &str) -> Option<&serde_json::Value> {
        self.cleanup_expired();
        let indices = self.type_index.get(&EntryType::ComputedValue).cloned().unwrap_or_default();
        indices.iter()
            .rev()
            .filter_map(|&idx| self.entries.get(idx))
            .find(|e| e.content == key)
            .and_then(|e| e.value_json.as_ref())
    }

    /// Get current user intent
    pub fn current_intent(&mut self) -> Option<String> {
        self.cleanup_expired();
        let indices = self.type_index.get(&EntryType::UserIntent).cloned().unwrap_or_default();
        indices.iter()
            .rev()
            .filter_map(|&idx| self.entries.get(idx))
            .next()
            .map(|e| e.content.clone())
    }

    /// Get all context hints
    pub fn context_hints(&mut self) -> Vec<String> {
        self.cleanup_expired();
        let indices = self.type_index.get(&EntryType::ContextHint).cloned().unwrap_or_default();
        indices.iter()
            .filter_map(|&idx| self.entries.get(idx))
            .map(|e| e.content.clone())
            .collect()
    }

    /// Pin an entry so it won't be evicted
    pub fn pin(&mut self, entry_id: u64) {
        if !self.pinned_ids.contains(&entry_id) {
            self.pinned_ids.push(entry_id);
        }
    }

    /// Unpin an entry
    pub fn unpin(&mut self, entry_id: u64) {
        self.pinned_ids.retain(|&id| id != entry_id);
    }

    /// Clear all working memory
    pub fn clear(&mut self) {
        self.entries.clear();
        self.type_index.clear();
        self.pinned_ids.clear();
    }

    /// Get entry count
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Get all entries for inspection
    pub fn all_entries(&self) -> &[WorkingMemoryEntry] {
        &self.entries
    }

    /// Build a summary for LLM context injection
    pub fn build_summary(&mut self) -> String {
        self.cleanup_expired();

        let mut parts = Vec::new();

        // Current intent
        if let Some(intent) = self.current_intent() {
            parts.push(format!("Current Intent: {}", intent));
        }

        // Context hints
        let hints = self.context_hints();
        if !hints.is_empty() {
            parts.push(format!("Context Hints: {}", hints.join(", ")));
        }

        // Entity references
        let entity_indices = self.type_index.get(&EntryType::EntityReference).cloned().unwrap_or_default();
        let entities: Vec<String> = entity_indices.iter()
            .filter_map(|&idx| self.entries.get(idx))
            .map(|e| format!("{}({:?})", e.content, e.entity_id.unwrap_or(crate::types::EntityId(0))))
            .collect();
        if !entities.is_empty() {
            parts.push(format!("Active Entities: {}", entities.join(", ")));
        }

        // Recent conversation (last 3 turns)
        let msg_indices = self.type_index.get(&EntryType::ConversationTurn).cloned().unwrap_or_default();
        let recent_msgs: Vec<String> = msg_indices.iter()
            .rev()
            .take(6) // 3 turns = 6 messages (user + agent)
            .filter_map(|&idx| self.entries.get(idx))
            .filter_map(|e| e.source_message.as_ref())
            .map(|m| format!("[{:?}] {}", m.message_type, m.content))
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        if !recent_msgs.is_empty() {
            parts.push(format!("Recent Conversation:\n{}", recent_msgs.join("\n")));
        }

        parts.join("\n")
    }

    fn next_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    pub fn cleanup_expired(&mut self) {
        let mut to_remove = Vec::new();
        for (idx, entry) in self.entries.iter().enumerate() {
            if entry.is_expired() {
                to_remove.push(idx);
            }
        }
        // Remove from end to preserve indices
        for idx in to_remove.into_iter().rev() {
            self.entries.remove(idx);
        }
        // Rebuild type index
        self.rebuild_index();
    }

    fn evict_oldest(&mut self) {
        if self.entries.is_empty() {
            return;
        }

        // Find oldest non-pinned entry
        let mut oldest_idx = None;
        for (idx, entry) in self.entries.iter().enumerate() {
            if !self.pinned_ids.contains(&entry.metadata.id.0) {
                if oldest_idx.is_none() {
                    oldest_idx = Some(idx);
                }
            }
        }

        if let Some(idx) = oldest_idx {
            self.entries.remove(idx);
            self.rebuild_index();
        }
    }

    fn rebuild_index(&mut self) {
        self.type_index.clear();
        for (idx, entry) in self.entries.iter().enumerate() {
            self.type_index.entry(entry.entry_type.clone()).or_default().push(idx);
        }
    }

    // =================================================================
    // Persistence Operations
    // =================================================================

    /// Get all entries as JSON values for serialization
    pub fn get_entries_for_persistence(&self) -> Vec<serde_json::Value> {
        self.entries.iter()
            .map(|e| serde_json::to_value(e).unwrap_or(serde_json::Value::Null))
            .collect()
    }

    /// Restore an entry from JSON value
    pub fn restore_entry(&mut self, entry: serde_json::Value) {
        if let Ok(deserialized) = serde_json::from_value::<WorkingMemoryEntry>(entry) {
            let entry_type = deserialized.entry_type.clone();
            let idx = self.entries.len();
            self.entries.push(deserialized);
            self.type_index.entry(entry_type).or_default().push(idx);
            if self.next_id <= self.entries.len() as u64 {
                self.next_id = self.entries.len() as u64 + 1;
            }
        }
    }
}

impl Default for WorkingMemory {
    fn default() -> Self {
        Self::new(50)
    }
}
