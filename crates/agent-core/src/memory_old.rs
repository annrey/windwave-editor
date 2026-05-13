//! Memory System - Three-layer memory architecture
//!
//! 1. ConversationMemory (short-term): Current conversation messages
//! 2. SessionMemory (medium-term): Working memory + selection context
//! 3. PersistentMemory (long-term): Learned patterns, user preferences (future)

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
/// 
/// Stores the current conversation with the user.
/// Limited by context window size, old messages are pruned.
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
    
    /// Add a message to the conversation
    pub fn add_message(&mut self, message: Message) {
        self.messages.push(message);
        
        // Prune old messages if exceeding limit
        if self.messages.len() > self.max_messages {
            let to_remove = self.messages.len() - self.max_messages;
            self.messages.drain(0..to_remove);
        }
    }
    
    /// Get recent messages
    pub fn recent_messages(&self, n: usize) -> Vec<Message> {
        self.messages.iter()
            .rev()
            .take(n)
            .cloned()
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect()
    }
    
    /// Get all messages
    pub fn all_messages(&self) -> &[Message] {
        &self.messages
    }
    
    /// Get message count
    pub fn message_count(&self) -> usize {
        self.messages.len()
    }
    
    /// Clear all messages
    pub fn clear(&mut self) {
        self.messages.clear();
    }
    
    /// Clear recent n messages (for stuck recovery)
    pub fn clear_recent(&mut self, n: usize) {
        let start = self.messages.len().saturating_sub(n);
        self.messages.truncate(start);
    }
    
    /// Get messages by type
    pub fn messages_by_type(&self, msg_type: MessageType) -> Vec<Message> {
        self.messages.iter()
            .filter(|m| std::mem::discriminant(&m.message_type) == std::mem::discriminant(&msg_type))
            .cloned()
            .collect()
    }
    
    /// Search messages by content (simple substring match)
    pub fn search(&self, query: &str) -> Vec<Message> {
        self.messages.iter()
            .filter(|m| m.content.to_lowercase().contains(&query.to_lowercase()))
            .cloned()
            .collect()
    }
    
    /// Check for cycle in recent messages (for stuck detection)
    pub fn detect_cycle(&self, window_size: usize) -> bool {
        if self.messages.len() < window_size * 2 {
            return false;
        }
        
        let recent = &self.messages[self.messages.len() - window_size..];
        let previous = &self.messages[self.messages.len() - window_size * 2..self.messages.len() - window_size];
        
        // Compare content of messages
        recent.iter().zip(previous.iter())
            .filter(|(a, b)| a.content == b.content)
            .count() >= window_size / 2
    }
    
    /// Build context string for LLM prompt
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
/// 
/// Holds temporary data the Agent needs during execution,
/// like extracted entity names, computed values, etc.
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
    
    /// Set a variable
    pub fn set(&mut self, key: impl Into<String>, value: MemoryValue) {
        self.variables.insert(key.into(), value);
    }
    
    /// Get a variable
    pub fn get(&self, key: &str) -> Option<&MemoryValue> {
        self.variables.get(key)
    }
    
    /// Remove a variable
    pub fn remove(&mut self, key: &str) -> Option<MemoryValue> {
        self.variables.remove(key)
    }
    
    /// Register an entity reference
    pub fn register_entity(&mut self, name: impl Into<String>, id: EntityId) {
        self.entity_references.insert(name.into(), id);
    }
    
    /// Lookup entity by name
    pub fn lookup_entity(&self, name: &str) -> Option<EntityId> {
        self.entity_references.get(name).copied()
    }
    
    /// Store a result
    pub fn push_result(&mut self, result: impl Into<String>) {
        self.last_results.push(result.into());
        // Keep only last 10 results
        if self.last_results.len() > 10 {
            self.last_results.remove(0);
        }
    }
    
    /// Get recent results
    pub fn recent_results(&self, n: usize) -> Vec<&String> {
        self.last_results.iter().rev().take(n).collect()
    }
    
    /// Clear all working memory
    pub fn clear(&mut self) {
        self.variables.clear();
        self.entity_references.clear();
        self.last_results.clear();
    }
    
    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.variables.is_empty() && self.entity_references.is_empty()
    }
}

/// Selection Context - Current focus of the Agent
/// 
/// Tracks what entities and components are currently selected,
/// along with any active context tags.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SelectionContext {
    /// Currently selected entities
    pub selected_entities: Vec<EntityId>,
    /// Active components being edited
    pub active_components: Vec<String>,
    /// Context tags (e.g., @Player, #Physics)
    pub tags: Vec<ContextTag>,
    /// Current workspace/view context
    pub workspace_context: String,
}

impl SelectionContext {
    pub fn new() -> Self {
        Self::default()
    }
    
    /// Add entity to selection
    pub fn add_entity(&mut self, id: EntityId) {
        if !self.selected_entities.contains(&id) {
            self.selected_entities.push(id);
        }
    }
    
    /// Remove entity from selection
    pub fn remove_entity(&mut self, id: EntityId) {
        self.selected_entities.retain(|&e| e != id);
    }
    
    /// Clear selection
    pub fn clear_selection(&mut self) {
        self.selected_entities.clear();
        self.active_components.clear();
    }
    
    /// Add a context tag
    pub fn add_tag(&mut self, tag: ContextTag) {
        // Remove existing tag of same type/value if exists
        self.tags.retain(|t| !(t.tag_type == tag.tag_type && t.value == tag.value));
        self.tags.push(tag);
    }
    
    /// Get active tags by type
    pub fn tags_by_type(&self, tag_type: crate::types::TagType) -> Vec<&ContextTag> {
        self.tags.iter()
            .filter(|t| std::mem::discriminant(&t.tag_type) == std::mem::discriminant(&tag_type))
            .collect()
    }
    
    /// Set workspace context
    pub fn set_workspace(&mut self, context: impl Into<String>) {
        self.workspace_context = context.into();
    }
    
    /// Format for display
    pub fn format(&self) -> String {
        let entities = self.selected_entities.iter()
            .map(|e| format!("{:?}", e))
            .collect::<Vec<_>>()
            .join(", ");
        
        let components = self.active_components.join(", ");
        
        format!(
            "Selection: [{}] | Components: [{}] | Context: {}",
            entities, components, self.workspace_context
        )
    }
}

/// Session Memory - Medium-term memory for a work session
/// 
/// Combines working memory, selection context, and recent actions.
/// Persisted for the duration of the editor session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMemory {
    pub working_memory: WorkingMemory,
    pub selection_context: SelectionContext,
    pub recent_actions: Vec<ActionLog>,
    pub session_start: u64,
    pub project_context: String,
}

/// Log of an action performed by the Agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionLog {
    pub timestamp: u64,
    pub action_type: String,
    pub description: String,
    pub success: bool,
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
    
    /// Log an action
    pub fn log_action(&mut self, action_type: impl Into<String>, description: impl Into<String>, success: bool) {
        self.recent_actions.push(ActionLog {
            timestamp: current_timestamp(),
            action_type: action_type.into(),
            description: description.into(),
            success,
        });
        
        // Prune old actions
        if self.recent_actions.len() > 100 {
            self.recent_actions.remove(0);
        }
    }
    
    /// Get recent actions
    pub fn recent_actions(&self, n: usize) -> Vec<&ActionLog> {
        self.recent_actions.iter().rev().take(n).collect()
    }
    
    /// Set project context
    pub fn set_project_context(&mut self, context: impl Into<String>) {
        self.project_context = context.into();
    }
    
    /// Clear session (but keep project context)
    pub fn clear(&mut self) {
        self.working_memory.clear();
        self.selection_context.clear_selection();
        self.recent_actions.clear();
    }
}

/// Persistent Memory - Long-term learned data (placeholder for future)
/// 
/// This would include:
/// - User preferences and habits
/// - Learned code patterns
/// - Entity relationship knowledge
/// - Project-specific conventions
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PersistentMemory {
    pub user_preferences: UserPreferences,
    pub learned_patterns: Vec<LearnedPattern>,
    pub entity_knowledge: HashMap<EntityId, EntityKnowledge>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UserPreferences {
    pub preferred_engine: String,
    pub code_style: String,
    pub confirmation_level: ConfirmationLevel,
    pub theme: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub enum ConfirmationLevel {
    Always,      // Always ask for confirmation
    #[default]
    Destructive, // Only for destructive actions
    Never,       // Never ask (expert mode)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearnedPattern {
    pub name: String,
    pub description: String,
    pub trigger_keywords: Vec<String>,
    pub template: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityKnowledge {
    pub entity_id: EntityId,
    pub common_operations: Vec<String>,
    pub related_entities: Vec<EntityId>,
    pub notes: String,
}

impl PersistentMemory {
    pub fn new() -> Self {
        Self::default()
    }

    /// Seed knowledge bases with common Bevy / game-editing patterns.
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
}"#
                .into(),
            },
            LearnedPattern {
                name: "simple_system".into(),
                description: "A query-based Bevy system that iterates entities".into(),
                trigger_keywords: vec!["system".into(), "query".into(), "update".into()],
                template: r#"use bevy::prelude::*;

pub fn {name}(query: Query<&{component}>, time: Res<Time>) {
    for component in query.iter() {
        // Apply game logic here
    }
}"#
                .into(),
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
}"#
                .into(),
            },
            LearnedPattern {
                name: "camera_follow_2d".into(),
                description: "Camera follows a target entity in 2D".into(),
                trigger_keywords: vec!["camera".into(), "follow".into(), "target".into()],
                template: r#"use bevy::prelude::*;

pub fn camera_follow(
    target: Query<&Transform, (With<Player>, Without<Camera>)>,
    mut camera: Query<&mut Transform, (With<Camera>, Without<Player>)>,
) {
    if let (Ok(target_transform), Ok(mut camera_transform)) = 
        (target.get_single(), camera.get_single_mut()) 
    {
        camera_transform.translation = target_transform.translation;
    }
}"#
                .into(),
            },
            LearnedPattern {
                name: "spawn_on_click".into(),
                description: "Spawn an entity when mouse button is clicked".into(),
                trigger_keywords: vec!["spawn".into(), "click".into(), "mouse".into(), "create".into()],
                template: r#"use bevy::prelude::*;

pub fn spawn_on_click(
    input: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    camera: Query<(&Camera, &GlobalTransform)>,
    mut commands: Commands,
) {
    if input.just_pressed(MouseButton::Left) {
        let window = windows.single();
        if let Some(pos) = window.cursor_position() {
            if let Ok((camera, camera_transform)) = camera.get_single() {
                if let Ok(world_pos) = camera.viewport_to_world_2d(camera_transform, pos) {
                    commands.spawn((
                        Sprite::default(),
                        Transform::from_translation(world_pos.extend(10.0)),
                    ));
                }
            }
        }
    }
}"#
                .into(),
            },
            LearnedPattern {
                name: "endless_scrolling_bg".into(),
                description: "Seamlessly scrolling background texture".into(),
                trigger_keywords: vec!["scroll".into(), "background".into(), "endless".into(), "parallax".into()],
                template: r#"use bevy::prelude::*;

pub fn scroll_background(
    time: Res<Time>,
    mut query: Query<&mut Transform, With<Background>>,
) {
    for mut transform in query.iter_mut() {
        transform.translation.x -= 50.0 * time.delta_secs();
        if transform.translation.x < -1280.0 {
            transform.translation.x += 2560.0;
        }
    }
}"#
                .into(),
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

    /// Find a code pattern matching the given keywords.
    pub fn find_pattern(&self, keywords: &[&str]) -> Option<&LearnedPattern> {
        self.learned_patterns
            .iter()
            .find(|pattern| {
                keywords
                    .iter()
                    .any(|kw| pattern.trigger_keywords.iter().any(|tk| tk.contains(*kw)))
            })
    }

    /// Add entity knowledge for an entity.
    pub fn add_entity_knowledge(
        &mut self,
        entity_id: EntityId,
        common_ops: Vec<String>,
        notes: &str,
    ) {
        self.entity_knowledge.insert(
            entity_id,
            EntityKnowledge {
                entity_id,
                common_operations: common_ops,
                related_entities: vec![],
                notes: notes.into(),
            },
        );
    }

    /// Query entity knowledge by ID.
    pub fn get_entity_knowledge(&self, entity_id: EntityId) -> Option<&EntityKnowledge> {
        self.entity_knowledge.get(&entity_id)
    }

    /// Save persistent memory to disk as JSON.
    pub fn save(&self, path: &std::path::Path) -> Result<(), std::io::Error> {
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;
        std::fs::write(path, json)
    }

    /// Load persistent memory from a JSON file.
    /// Returns a new default if the file does not exist.
    pub fn load(path: &std::path::Path) -> Result<Self, std::io::Error> {
        if !path.exists() {
            return Ok(Self::new());
        }
        let json = std::fs::read_to_string(path)?;
        serde_json::from_str(&json)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_conversation_memory() {
        let mut mem = ConversationMemory::new(5);
        
        mem.add_message(Message {
            id: "1".to_string(),
            message_type: MessageType::User,
            content: "Hello".to_string(),
            timestamp: 1,
            metadata: HashMap::new(),
        });
        
        assert_eq!(mem.message_count(), 1);
        assert_eq!(mem.recent_messages(1)[0].content, "Hello");
    }
    
    #[test]
    fn test_working_memory() {
        let mut mem = WorkingMemory::new();
        
        mem.set("player_name", MemoryValue::String("Hero".to_string()));
        mem.register_entity("Player", EntityId(1));
        
        assert!(matches!(mem.get("player_name"), Some(MemoryValue::String(_))));
        assert_eq!(mem.lookup_entity("Player"), Some(EntityId(1)));
    }
    
    #[test]
    fn test_selection_context() {
        let mut ctx = SelectionContext::new();
        
        ctx.add_entity(EntityId(1));
        ctx.add_entity(EntityId(2));
        ctx.active_components.push("Transform".to_string());
        
        assert_eq!(ctx.selected_entities.len(), 2);
        assert!(ctx.format().contains("Transform"));
    }

    #[test]
    fn test_persistent_memory_save_and_load() {
        let mut mem = PersistentMemory::new();
        mem.user_preferences.preferred_engine = "bevy".to_string();
        mem.learned_patterns.push(LearnedPattern {
            name: "spawn_enemy".into(),
            description: "Spawn a red enemy".into(),
            trigger_keywords: vec!["敌人".into(), "enemy".into()],
            template: "create_entity".into(),
        });

        let dir = std::env::temp_dir();
        let path = dir.join("test_agent_memory.json");

        mem.save(&path).expect("save should succeed");
        let loaded = PersistentMemory::load(&path).expect("load should succeed");

        assert_eq!(loaded.user_preferences.preferred_engine, "bevy");
        assert_eq!(loaded.learned_patterns.len(), 1);
        assert_eq!(loaded.learned_patterns[0].name, "spawn_enemy");

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_persistent_memory_load_missing_file() {
        let dir = std::env::temp_dir();
        let path = dir.join("nonexistent_memory.json");
        let loaded = PersistentMemory::load(&path).expect("load should succeed for missing file");
        assert!(loaded.learned_patterns.is_empty());
        assert_eq!(loaded.user_preferences.confirmation_level, ConfirmationLevel::Destructive);
    }

    #[test]
    fn test_session_memory_log_and_retrieve() {
        let mut session = SessionMemory::new();
        session.log_action("create_entity", "Created enemy_03", true);
        session.log_action("set_color", "Failed to set color", false);

        let recent = session.recent_actions(2);
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].action_type, "set_color");
        assert_eq!(recent[1].action_type, "create_entity");
    }
}
