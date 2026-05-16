//! Persistent Memory — long-term learned data (legacy).
//!
//! Kept for backward compatibility with agent-ui's memory_persistence module.
//! New code should use `crate::memory::*` instead.

use crate::types::EntityId;
use std::collections::HashMap;
use serde::{Deserialize, Serialize};

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
