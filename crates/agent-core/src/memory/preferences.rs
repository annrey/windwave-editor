//! User Preferences - Episodic memory for user habits and preferences
//!
//! Stores and retrieves user preferences learned from past interactions.
//! Examples: "User prefers red color", "User likes simple UI", "User often creates enemies"
//!
//! This module implements L2.5 UserPreferences as an extension to Episodic Memory.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserPreference {
    pub key: String,
    pub value: serde_json::Value,
    pub category: PreferenceCategory,
    pub confidence: f32,
    pub source_event: String,
    pub created_at: u64,
    pub updated_at: u64,
    pub usage_count: u32,
}

impl UserPreference {
    pub fn new(
        key: impl Into<String>,
        value: serde_json::Value,
        category: PreferenceCategory,
        source: impl Into<String>,
    ) -> Self {
        let now = timestamp();
        Self {
            key: key.into(),
            value,
            category,
            confidence: 0.5,
            source_event: source.into(),
            created_at: now,
            updated_at: now,
            usage_count: 0,
        }
    }

    pub fn with_confidence(mut self, confidence: f32) -> Self {
        self.confidence = confidence.clamp(0.0, 1.0);
        self
    }

    pub fn touch(&mut self) {
        self.usage_count += 1;
        self.updated_at = timestamp();
        self.confidence = (self.confidence + 0.1).min(1.0);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PreferenceCategory {
    Color,
    Style,
    Layout,
    Workflow,
    Naming,
    Component,
    EntityType,
    Other,
}

impl PreferenceCategory {
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "color" => Self::Color,
            "style" => Self::Style,
            "layout" => Self::Layout,
            "workflow" => Self::Workflow,
            "naming" => Self::Naming,
            "component" => Self::Component,
            "entity_type" | "entitytype" | "entity-type" => Self::EntityType,
            _ => Self::Other,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Color => "Color",
            Self::Style => "Style",
            Self::Layout => "Layout",
            Self::Workflow => "Workflow",
            Self::Naming => "Naming",
            Self::Component => "Component",
            Self::EntityType => "EntityType",
            Self::Other => "Other",
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UserPreferences {
    preferences: HashMap<String, UserPreference>,
    category_index: HashMap<PreferenceCategory, Vec<String>>,
    recent_interactions: Vec<PreferenceInteraction>,
}

impl UserPreferences {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set(
        &mut self,
        key: impl Into<String>,
        value: serde_json::Value,
        category: PreferenceCategory,
        source: impl Into<String>,
    ) {
        let key = key.into();
        let preference = UserPreference::new(&key, value.clone(), category, source);

        if !self.preferences.contains_key(&key) {
            self.category_index
                .entry(category)
                .or_default()
                .push(key.clone());
        }

        self.preferences.insert(key, preference);
    }

    pub fn get(&self, key: &str) -> Option<&UserPreference> {
        self.preferences.get(key)
    }

    pub fn get_mut(&mut self, key: &str) -> Option<&mut UserPreference> {
        if let Some(pref) = self.preferences.get_mut(key) {
            pref.touch();
        }
        self.preferences.get_mut(key)
    }

    pub fn remove(&mut self, key: &str) -> Option<UserPreference> {
        if let Some(pref) = self.preferences.remove(key) {
            if let Some(keys) = self.category_index.get_mut(&pref.category) {
                keys.retain(|k| k != key);
            }
            return Some(pref);
        }
        None
    }

    pub fn by_category(&self, category: PreferenceCategory) -> Vec<&UserPreference> {
        self.category_index
            .get(&category)
            .map(|keys| {
                keys.iter()
                    .filter_map(|k| self.preferences.get(k))
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn search(&self, query: &str) -> Vec<&UserPreference> {
        let query_lower = query.to_lowercase();
        self.preferences
            .values()
            .filter(|p| {
                p.key.to_lowercase().contains(&query_lower)
                    || format!("{:?}", p.value)
                        .to_lowercase()
                        .contains(&query_lower)
                    || p.category.as_str().to_lowercase().contains(&query_lower)
            })
            .collect()
    }

    pub fn top_preferences(&self, limit: usize) -> Vec<&UserPreference> {
        let mut prefs: Vec<_> = self.preferences.values().collect();
        prefs.sort_by(|a, b| {
            b.confidence
                .partial_cmp(&a.confidence)
                .unwrap()
                .then_with(|| b.usage_count.cmp(&a.usage_count))
        });
        prefs.into_iter().take(limit).collect()
    }

    pub fn add_interaction(&mut self, interaction: PreferenceInteraction) {
        self.recent_interactions.push(interaction.clone());
        if self.recent_interactions.len() > 100 {
            self.recent_interactions.remove(0);
        }
    }

    pub fn recent_interactions(&self) -> &[PreferenceInteraction] {
        &self.recent_interactions
    }

    pub fn count(&self) -> usize {
        self.preferences.len()
    }

    pub fn is_empty(&self) -> bool {
        self.preferences.is_empty()
    }

    pub fn build_context(&self, query: Option<&str>) -> String {
        let relevant: Vec<&UserPreference> = if let Some(q) = query {
            self.search(q)
        } else {
            self.top_preferences(10)
        };

        if relevant.is_empty() {
            return String::new();
        }

        let mut parts = vec!["## User Preferences".to_string()];

        for pref in &relevant {
            parts.push(format!(
                "- {}: {} (confidence: {:.0}%, used {} times)",
                pref.key,
                pref.value,
                pref.confidence * 100.0,
                pref.usage_count
            ));
        }

        parts.join("\n")
    }

    pub fn infer_preference(&self, context: &str) -> Option<String> {
        let context_lower = context.to_lowercase();

        let patterns: Vec<(&str, &str)> = vec![
            ("color", "color"),
            ("style", "style"),
            ("layout", "layout"),
            ("偏好", "preference"),
            ("喜欢", "likes"),
            ("上次", "last time"),
        ];

        for (pattern, _) in patterns {
            if context_lower.contains(pattern) {
                if let Some(pref) = self.search(pattern).first() {
                    return Some(format!("{}: {}", pref.key, pref.value));
                }
            }
        }

        self.top_preferences(3)
            .first()
            .map(|p| format!("{}: {}", p.key, p.value))
    }

    /// Learn from a user correction.
    ///
    /// When the user corrects the agent's action (e.g., "no, use red"),
    /// this method extracts the intent and stores it as a preference.
    pub fn learn_from_correction(
        &mut self,
        correction: &str,
        original_action: &str,
    ) -> Option<String> {
        let corr_lower = correction.to_lowercase();

        let color_patterns = [
            ("red", "red", "#FF0000"),
            ("blue", "blue", "#0000FF"),
            ("green", "green", "#00FF00"),
            ("yellow", "yellow", "#FFFF00"),
            ("purple", "purple", "#800080"),
            ("orange", "orange", "#FFA500"),
            ("black", "black", "#000000"),
            ("white", "white", "#FFFFFF"),
            ("pink", "pink", "#FFC0CB"),
            ("red", "红色", "#FF0000"),
            ("blue", "蓝色", "#0000FF"),
            ("green", "绿色", "#00FF00"),
            ("yellow", "黄色", "#FFFF00"),
            ("purple", "紫色", "#800080"),
            ("orange", "橙色", "#FFA500"),
            ("black", "黑色", "#000000"),
            ("white", "白色", "#FFFFFF"),
            ("pink", "粉色", "#FFC0CB"),
        ];

        for (en_name, cn_name, hex) in color_patterns {
            if corr_lower.contains(en_name) || corr_lower.contains(cn_name) {
                let key = "favorite_color";
                let value = serde_json::json!({"name": en_name, "hex": hex});
                self.set(key, value, PreferenceCategory::Color, correction);
                let existing = self.get(key);
                if let Some(pref) = existing {
                    return Some(format!(
                        "Learned: user prefers {} ({}) - confidence {:.0}%",
                        en_name,
                        hex,
                        pref.confidence * 100.0
                    ));
                }
            }
        }

        let style_patterns = [
            ("simple", PreferenceCategory::Style),
            ("minimal", PreferenceCategory::Style),
            ("complex", PreferenceCategory::Style),
            ("large", PreferenceCategory::Layout),
            ("small", PreferenceCategory::Layout),
            ("grid", PreferenceCategory::Layout),
        ];

        for (word, category) in style_patterns {
            if corr_lower.contains(word) {
                let key = format!("style_{}", word);
                let value = serde_json::json!(word);
                self.set(key, value, category, correction);
            }
        }

        self.add_interaction(PreferenceInteraction {
            user_action: correction.to_string(),
            entities_affected: vec![original_action.to_string()],
            timestamp: timestamp(),
            outcome: InteractionOutcome::Reverted,
        });

        None
    }

    /// Record a failure interaction.
    ///
    /// When a plan step fails, record the failure for future learning.
    /// Repeated failures of the same type increase the system's awareness
    /// of problematic patterns.
    pub fn record_failure(&mut self, action: &str, entities: Vec<String>, error_message: &str) {
        let key = format!("failure_pattern_{}", action);
        let existing = self.preferences.get(&key);

        let severity = if let Some(pref) = existing {
            pref.confidence + 0.2
        } else {
            0.3
        };

        let value = serde_json::json!({
            "action": action,
            "error": error_message,
            "occurrences": existing.map(|p| {
                p.value.get("occurrences").and_then(|v| v.as_u64()).unwrap_or(1) + 1
            }).unwrap_or(1),
        });

        self.set(
            key,
            value,
            PreferenceCategory::Workflow,
            format!("failure: {}", error_message),
        );

        if let Some(pref) = self.get_mut(&format!("failure_pattern_{}", action)) {
            pref.confidence = severity.min(1.0);
        }

        self.add_interaction(PreferenceInteraction {
            user_action: action.to_string(),
            entities_affected: entities,
            timestamp: timestamp(),
            outcome: InteractionOutcome::Failed,
        });
    }

    /// Get failure patterns for a given action type.
    ///
    /// Returns known failure patterns so the agent can avoid repeating
    /// the same mistakes.
    pub fn known_failure_patterns(&self, action_hint: &str) -> Vec<&UserPreference> {
        let key = format!("failure_pattern_{}", action_hint);
        let mut results: Vec<&UserPreference> = Vec::new();

        if let Some(pref) = self.get(&key) {
            results.push(pref);
        }

        for pref in self.by_category(PreferenceCategory::Workflow) {
            if pref.key.starts_with("failure_pattern_") {
                results.push(pref);
            }
        }

        results.sort_by(|a, b| {
            b.confidence
                .partial_cmp(&a.confidence)
                .unwrap()
                .then_with(|| b.usage_count.cmp(&a.usage_count))
        });

        results.truncate(5);
        results
    }

    /// Detect repeated failures and suggest a different approach.
    pub fn suggest_alternative(&self, action: &str) -> Option<String> {
        let patterns = self.known_failure_patterns(action);
        if patterns.is_empty() {
            return None;
        }

        let total_occurrences: u64 = patterns
            .iter()
            .filter_map(|p| p.value.get("occurrences").and_then(|v| v.as_u64()))
            .sum();

        if total_occurrences >= 2 {
            let suggestions = format!(
                "Note: '{}' has failed {} times before. Consider a different approach.",
                action, total_occurrences
            );
            return Some(suggestions);
        }

        None
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreferenceInteraction {
    pub user_action: String,
    pub entities_affected: Vec<String>,
    pub timestamp: u64,
    pub outcome: InteractionOutcome,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InteractionOutcome {
    Success,
    PartialSuccess,
    Reverted,
    Failed,
}

fn timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_set_and_get_preference() {
        let mut prefs = UserPreferences::new();
        prefs.set(
            "favorite_color",
            serde_json::json!("#FF0000"),
            PreferenceCategory::Color,
            "user said red",
        );

        assert!(prefs.get("favorite_color").is_some());
        assert_eq!(
            prefs.get("favorite_color").unwrap().value,
            serde_json::json!("#FF0000")
        );
    }

    #[test]
    fn test_category_filtering() {
        let mut prefs = UserPreferences::new();
        prefs.set(
            "bg_color",
            serde_json::json!("dark"),
            PreferenceCategory::Color,
            "ui",
        );
        prefs.set(
            "layout",
            serde_json::json!("grid"),
            PreferenceCategory::Layout,
            "ui",
        );

        let colors: Vec<_> = prefs.by_category(PreferenceCategory::Color);
        assert_eq!(colors.len(), 1);
        assert_eq!(colors[0].key, "bg_color");
    }

    #[test]
    fn test_confidence_increase() {
        let mut prefs = UserPreferences::new();
        prefs.set(
            "test",
            serde_json::json!("value"),
            PreferenceCategory::Other,
            "init",
        );

        let initial = prefs.get("test").unwrap().confidence;
        prefs.get_mut("test");
        let after = prefs.get("test").unwrap().confidence;

        assert!(after > initial);
    }

    #[test]
    fn test_context_building() {
        let mut prefs = UserPreferences::new();
        prefs.set(
            "red_enemy",
            serde_json::json!("RGB(255,0,0)"),
            PreferenceCategory::Color,
            "user said red enemy",
        );
        prefs.set(
            "blue_player",
            serde_json::json!("RGB(0,0,255)"),
            PreferenceCategory::Color,
            "user said blue player",
        );

        let context = prefs.build_context(Some("color"));
        assert!(context.contains("red_enemy"));
        assert!(context.contains("blue_player"));
    }
}
