//! Memory System - Three-layer memory architecture (Legacy)
//!
//! This file contains the original memory implementation.
//! The new four-layer memory system is in the `memory/` directory.
//!
//! This module is kept for backward compatibility.
//! New code should use `crate::memory::MemorySystem` instead.

use crate::types::{Message, MessageType, EntityId, ContextTag, current_timestamp};
use std::collections::HashMap;
use serde::{Deserialize, Serialize};

/// Memory tier classification used by the lifecycle manager.
/// Corresponds to the L0-L3 layered context from UI-TARS-desktop.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MemoryTier {
    Working,
    Episodic,
    Semantic,
    Procedural,
}

/// Conversation Memory - Short-term message history
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationMemory {
    messages: Vec<Message>,
    max_messages: usize,
}

impl ConversationMemory {
    pub fn new(max_messages: usize) -> Self {
        Self {
            messages: Vec::new(),
            max_messages,
        }
    }

    pub fn add_message(&mut self, message: Message) {
        self.messages.push(message);
        if self.messages.len() > self.max_messages {
            let to_remove = self.messages.len() - self.max_messages;
            self.messages.drain(0..to_remove);
        }
    }

    pub fn recent_messages(&self, n: usize) -> Vec<Message> {
        self.messages.iter().rev().take(n).cloned().collect::<Vec<_>>().into_iter().rev().collect()
    }

    pub fn all_messages(&self) -> &[Message] {
        &self.messages
    }

    pub fn message_count(&self) -> usize {
        self.messages.len()
    }

    pub fn clear(&mut self) {
        self.messages.clear();
    }

    pub fn clear_recent(&mut self, n: usize) {
        let start = self.messages.len().saturating_sub(n);
        self.messages.truncate(start);
    }

    pub fn messages_by_type(&self, msg_type: MessageType) -> Vec<Message> {
        self.messages.iter()
            .filter(|m| std::mem::discriminant(&m.message_type) == std::mem::discriminant(&msg_type))
            .cloned()
            .collect()
    }

    pub fn search(&self, query: &str) -> Vec<Message> {
        self.messages.iter()
            .filter(|m| m.content.to_lowercase().contains(&query.to_lowercase()))
            .cloned()
            .collect()
    }

    pub fn detect_cycle(&self, window_size: usize) -> bool {
        if self.messages.len() < window_size * 2 {
            return false;
        }
        let recent = &self.messages[self.messages.len() - window_size..];
        let previous = &self.messages[self.messages.len() - window_size * 2..self.messages.len() - window_size];
        recent.iter().zip(previous.iter())
            .filter(|(a, b)| a.content == b.content)
            .count() >= window_size / 2
    }

    pub fn build_context(&self) -> String {
        self.messages.iter()
            .map(|m| format!("[{:?}] {}", m.message_type, m.content))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// Memory Value - Values stored in working memory
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MemoryValue {
    String(String),
    Number(f64),
    Bool(bool),
    Entity(EntityId),
    List(Vec<MemoryValue>),
    Json(serde_json::Value),
}

/// Working Memory - Temporary variables and data
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorkingMemory {
    variables: HashMap<String, MemoryValue>,
    entity_references: HashMap<String, EntityId>,
    last_results: Vec<String>,
}

impl WorkingMemory {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set(&mut self, key: impl Into<String>, value: MemoryValue) {
        self.variables.insert(key.into(), value);
    }

    pub fn get(&self, key: &str) -> Option<&MemoryValue> {
        self.variables.get(key)
    }

    pub fn remove(&mut self, key: &str) -> Option<MemoryValue> {
        self.variables.remove(key)
    }

    pub fn register_entity(&mut self, name: impl Into<String>, id: EntityId) {
        self.entity_references.insert(name.into(), id);
    }

    pub fn lookup_entity(&self, name: &str) -> Option<EntityId> {
        self.entity_references.get(name).copied()
    }

    pub fn push_result(&mut self, result: impl Into<String>) {
        self.last_results.push(result.into());
        if self.last_results.len() > 10 {
            self.last_results.remove(0);
        }
    }

    pub fn recent_results(&self, n: usize) -> Vec<&String> {
        self.last_results.iter().rev().take(n).collect()
    }

    pub fn clear(&mut self) {
        self.variables.clear();
        self.entity_references.clear();
        self.last_results.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.variables.is_empty() && self.entity_references.is_empty()
    }
}

/// Selection Context - Current focus of the Agent
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SelectionContext {
    pub selected_entities: Vec<EntityId>,
    pub active_components: Vec<String>,
    pub tags: Vec<ContextTag>,
    pub workspace_context: String,
}

impl SelectionContext {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_entity(&mut self, id: EntityId) {
        if !self.selected_entities.contains(&id) {
            self.selected_entities.push(id);
        }
    }

    pub fn remove_entity(&mut self, id: EntityId) {
        self.selected_entities.retain(|&e| e != id);
    }

    pub fn clear_selection(&mut self) {
        self.selected_entities.clear();
        self.active_components.clear();
    }

    pub fn add_tag(&mut self, tag: ContextTag) {
        self.tags.retain(|t| !(t.tag_type == tag.tag_type && t.value == tag.value));
        self.tags.push(tag);
    }

    pub fn tags_by_type(&self, tag_type: crate::types::TagType) -> Vec<&ContextTag> {
        self.tags.iter()
            .filter(|t| std::mem::discriminant(&t.tag_type) == std::mem::discriminant(&tag_type))
            .collect()
    }

    pub fn set_workspace(&mut self, context: impl Into<String>) {
        self.workspace_context = context.into();
    }

    pub fn format(&self) -> String {
        let entities = self.selected_entities.iter()
            .map(|e| format!("{:?}", e))
            .collect::<Vec<_>>()
            .join(", ");
        let components = self.active_components.join(", ");
        format!("Selection: [{}] | Components: [{}] | Context: {}", entities, components, self.workspace_context)
    }
}

/// Log of an action performed by the Agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionLog {
    pub timestamp: u64,
    pub action_type: String,
    pub description: String,
    pub success: bool,
}

/// Session Memory - Medium-term memory for a work session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMemory {
    pub working_memory: WorkingMemory,
    pub selection_context: SelectionContext,
    pub recent_actions: Vec<ActionLog>,
    pub session_start: u64,
    pub project_context: String,
}

impl SessionMemory {
    pub fn new() -> Self {
        Self {
            working_memory: WorkingMemory::new(),
            selection_context: SelectionContext::new(),
            recent_actions: Vec::with_capacity(100),
            session_start: current_timestamp(),
            project_context: String::new(),
        }
    }

    pub fn log_action(&mut self, action_type: impl Into<String>, description: impl Into<String>, success: bool) {
        self.recent_actions.push(ActionLog {
            timestamp: current_timestamp(),
            action_type: action_type.into(),
            description: description.into(),
            success,
        });
        if self.recent_actions.len() > 100 {
            self.recent_actions.remove(0);
        }
    }

    pub fn recent_actions(&self, n: usize) -> Vec<&ActionLog> {
        self.recent_actions.iter().rev().take(n).collect()
    }

    pub fn set_project_context(&mut self, context: impl Into<String>) {
        self.project_context = context.into();
    }

    pub fn clear(&mut self) {
        self.working_memory.clear();
        self.selection_context.clear_selection();
        self.recent_actions.clear();
    }
}

/// Confirmation Level
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub enum ConfirmationLevel {
    Always,
    #[default]
    Destructive,
    Never,
}

/// User Preferences
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UserPreferences {
    pub preferred_engine: String,
    pub code_style: String,
    pub confirmation_level: ConfirmationLevel,
    pub theme: String,
}

/// Learned Pattern
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearnedPattern {
    pub name: String,
    pub description: String,
    pub trigger_keywords: Vec<String>,
    pub template: String,
}

/// Entity Knowledge
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityKnowledge {
    pub entity_id: EntityId,
    pub common_operations: Vec<String>,
    pub related_entities: Vec<EntityId>,
    pub notes: String,
}

/// Persistent Memory - Long-term learned data
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PersistentMemory {
    pub user_preferences: UserPreferences,
    pub learned_patterns: Vec<LearnedPattern>,
    pub entity_knowledge: HashMap<EntityId, EntityKnowledge>,
}

impl PersistentMemory {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn seed_with_defaults(&mut self) {
        self.seed_code_patterns();
        self.seed_user_preferences();
    }

    fn seed_code_patterns(&mut self) {
        let patterns = vec![
            LearnedPattern {
                name: "simple_component".into(),
                description: "A basic Bevy component with derive macros".into(),
                trigger_keywords: vec!["component".into(), "struct".into()],
                template: r#"use bevy::prelude::*;

#[derive(Component, Debug, Clone)]
pub struct {name} {
    pub value: f32,
}"#.into(),
            },
            LearnedPattern {
                name: "player_movement_2d".into(),
                description: "WASD-style 2D player movement system".into(),
                trigger_keywords: vec!["move".into(), "player".into(), "wasd".into(), "input".into()],
                template: r#"use bevy::prelude::*;

const PLAYER_SPEED: f32 = 300.0;

pub fn player_movement(
    input: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    mut query: Query<&mut Transform, With<Player>>,
) {
    if let Ok(mut transform) = query.get_single_mut() {
        let mut direction = Vec3::ZERO;
        if input.pressed(KeyCode::KeyW) { direction.y += 1.0; }
        if input.pressed(KeyCode::KeyS) { direction.y -= 1.0; }
        if input.pressed(KeyCode::KeyA) { direction.x -= 1.0; }
        if input.pressed(KeyCode::KeyD) { direction.x += 1.0; }
        if direction != Vec3::ZERO {
            direction = direction.normalize();
            transform.translation += direction * PLAYER_SPEED * time.delta_secs();
        }
    }
}"#.into(),
            },
        ];
        self.learned_patterns = patterns;
    }

    fn seed_user_preferences(&mut self) {
        self.user_preferences = UserPreferences {
            preferred_engine: "Bevy".into(),
            code_style: "simple".into(),
            confirmation_level: ConfirmationLevel::Destructive,
            theme: "dark".into(),
        };
    }

    pub fn find_pattern(&self, keywords: &[&str]) -> Option<&LearnedPattern> {
        self.learned_patterns.iter()
            .find(|pattern| {
                keywords.iter().any(|kw| pattern.trigger_keywords.iter().any(|tk| tk.contains(*kw)))
            })
    }

    pub fn add_entity_knowledge(&mut self, entity_id: EntityId, common_ops: Vec<String>, notes: &str) {
        self.entity_knowledge.insert(entity_id, EntityKnowledge {
            entity_id,
            common_operations: common_ops,
            related_entities: vec![],
            notes: notes.into(),
        });
    }

    pub fn get_entity_knowledge(&self, entity_id: EntityId) -> Option<&EntityKnowledge> {
        self.entity_knowledge.get(&entity_id)
    }

    pub fn save(&self, path: &std::path::Path) -> Result<(), std::io::Error> {
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;
        std::fs::write(path, json)
    }

    pub fn load(path: &std::path::Path) -> Result<Self, std::io::Error> {
        if !path.exists() {
            return Ok(Self::new());
        }
        let json = std::fs::read_to_string(path)?;
        serde_json::from_str(&json)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))
    }
}
