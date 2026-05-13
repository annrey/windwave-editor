//! Agent Context Builder — Unified context assembly for Agent LLM calls.
//!
//! Gathers: editor selection, scene state, memory, project config, available tools.
//! Outputs a structured prompt context that feeds into LLM prompts.

use std::collections::HashMap;
use crate::scene_bridge::EntityListItem;
use crate::memory_legacy::WorkingMemory;

// ---------------------------------------------------------------------------
// AgentContext
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct AgentContext {
    /// Currently selected entity
    pub selected_entity: Option<EntityContext>,
    /// Scene summary (entity count, key entities)
    pub scene_summary: Option<SceneContext>,
    /// Recent memory entries
    pub recent_memory: Vec<String>,
    /// Project metadata
    pub project_name: String,
    /// Available tool names
    pub available_tools: Vec<String>,
    /// Active constraints (permissions, limits)
    pub constraints: Vec<String>,
    /// Custom key-value extra context
    pub extra: HashMap<String, String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EntityContext {
    pub name: String,
    pub components: Vec<String>,
    pub transform: Option<TransformInfo>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TransformInfo {
    pub translation: [f32; 3],
    pub scale: [f32; 3],
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SceneContext {
    pub entity_count: usize,
    pub key_entities: Vec<String>,
}

// ---------------------------------------------------------------------------
// Builder
// ---------------------------------------------------------------------------

pub struct AgentContextBuilder {
    ctx: AgentContext,
}

impl AgentContextBuilder {
    pub fn new() -> Self {
        Self { ctx: AgentContext::default() }
    }

    pub fn with_project(mut self, name: impl Into<String>) -> Self {
        self.ctx.project_name = name.into();
        self
    }

    pub fn with_selection(mut self, entity: Option<EntityListItem>) -> Self {
        if let Some(e) = entity {
            self.ctx.selected_entity = Some(EntityContext {
                name: e.name.clone(),
                components: e.components.clone(),
                transform: None,
            });
        }
        self
    }

    pub fn with_selection_name(mut self, name: String, components: Vec<String>) -> Self {
        self.ctx.selected_entity = Some(EntityContext {
            name, components, transform: None,
        });
        self
    }

    pub fn with_scene(mut self, entities: &[EntityListItem]) -> Self {
        self.ctx.scene_summary = Some(SceneContext {
            entity_count: entities.len(),
            key_entities: entities.iter()
                .filter(|e| !e.name.is_empty() && !e.name.starts_with("entity_"))
                .map(|e| e.name.clone())
                .take(20)
                .collect(),
        });
        self
    }

    pub fn with_memory(mut self, memory: &WorkingMemory, limit: usize) -> Self {
        self.ctx.recent_memory = memory.recent_results(limit)
            .iter()
            .map(|s| s.to_string())
            .collect();
        self
    }

    pub fn with_tools(mut self, tool_names: Vec<String>) -> Self {
        self.ctx.available_tools = tool_names;
        self
    }

    pub fn with_constraint(mut self, constraint: impl Into<String>) -> Self {
        self.ctx.constraints.push(constraint.into());
        self
    }

    pub fn with_extra(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.ctx.extra.insert(key.into(), value.into());
        self
    }

    pub fn build(self) -> AgentContext {
        self.ctx
    }
}

// ---------------------------------------------------------------------------
// Prompt formatting
// ---------------------------------------------------------------------------

impl AgentContext {
    /// Format as structured prompt text for LLM injection
    pub fn to_prompt_text(&self) -> String {
        let mut parts = Vec::new();

        if !self.project_name.is_empty() {
            parts.push(format!("Project: {}", self.project_name));
        }

        if let Some(ref sel) = self.selected_entity {
            let comps = sel.components.join(", ");
            parts.push(format!("Selected entity: {} [{}]", sel.name, comps));
        }

        if let Some(ref scene) = self.scene_summary {
            parts.push(format!("Scene has {} entities. Key: {}",
                scene.entity_count,
                scene.key_entities.join(", "),
            ));
        }

        if !self.recent_memory.is_empty() {
            parts.push("Recent context:".into());
            for entry in &self.recent_memory {
                parts.push(format!("  {}", entry));
            }
        }

        if !self.available_tools.is_empty() {
            parts.push(format!("Available tools: {}", self.available_tools.join(", ")));
        }

        if !self.constraints.is_empty() {
            parts.push(format!("Constraints: {}", self.constraints.join("; ")));
        }

        parts.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builder_empty() {
        let ctx = AgentContextBuilder::new().build();
        assert!(ctx.selected_entity.is_none());
        assert!(ctx.scene_summary.is_none());
    }

    #[test]
    fn test_builder_full() {
        let entities = vec![
            EntityListItem { name: "Player".into(), id: 1, components: vec![] },
            EntityListItem { name: "Enemy".into(), id: 2, components: vec![] },
        ];

        let ctx = AgentContextBuilder::new()
            .with_project("MyGame")
            .with_selection_name("Player".into(), vec!["Transform".into(), "Sprite".into()])
            .with_scene(&entities)
            .with_tools(vec!["search_code".into(), "query_scene".into()])
            .with_constraint("max steps: 5")
            .build();

        assert_eq!(ctx.project_name, "MyGame");
        assert!(ctx.selected_entity.is_some());
        assert_eq!(ctx.scene_summary.as_ref().unwrap().entity_count, 2);
        assert_eq!(ctx.available_tools.len(), 2);

        let prompt = ctx.to_prompt_text();
        assert!(prompt.contains("MyGame"));
        assert!(prompt.contains("Player"));
        assert!(prompt.contains("2 entities"));
        assert!(prompt.contains("search_code"));
    }
}
