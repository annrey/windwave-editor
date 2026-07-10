use crate::error::{BridgeError, Result};
use crate::game_skill_bridge::SkillHandler;
use log::info;
use std::collections::HashMap;

/// Local skill definition
pub struct LocalSkill {
    pub id: String,
    pub name: String,
    pub description: String,
    pub tags: Vec<String>,
    pub handler: SkillHandler,
}

/// Skill adapter
pub struct SkillAdapter {
    local_skills: HashMap<String, LocalSkill>,
}

impl SkillAdapter {
    /// Create new skill adapter
    pub fn new() -> Self {
        Self {
            local_skills: HashMap::new(),
        }
    }

    /// Register a local skill
    pub fn register_skill<F>(
        &mut self,
        name: &str,
        description: &str,
        tags: Vec<String>,
        handler: F,
    ) where
        F: Fn(&serde_json::Value) -> Result<serde_json::Value> + Send + Sync + 'static,
    {
        let skill = LocalSkill {
            id: uuid::Uuid::new_v4().to_string(),
            name: name.to_string(),
            description: description.to_string(),
            tags,
            handler: Box::new(handler),
        };

        info!("Registering local skill: {}", name);
        self.local_skills.insert(skill.id.clone(), skill);
    }

    /// Execute a local skill
    pub async fn execute_local_skill(
        &self,
        skill_id: &str,
        params: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let skill = self
            .local_skills
            .get(skill_id)
            .ok_or_else(|| BridgeError::Other(format!("Skill not found: {}", skill_id)))?;

        info!("Executing local skill: {}", skill.name);

        (skill.handler)(params)
    }

    /// List local skills
    pub fn list_local_skills(&self) -> Vec<&LocalSkill> {
        self.local_skills.values().collect()
    }
}

impl Default for SkillAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_skill_adapter_new() {
        let adapter = SkillAdapter::new();
        assert!(adapter.list_local_skills().is_empty());
    }

    #[test]
    fn test_register_and_list_skills() {
        let mut adapter = SkillAdapter::new();
        adapter.register_skill(
            "test_skill",
            "A test skill",
            vec!["test".to_string()],
            |_params| Ok(serde_json::json!({"result": "success"})),
        );

        let skills = adapter.list_local_skills();
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].name, "test_skill");
    }

    #[tokio::test]
    async fn test_execute_skill() {
        let mut adapter = SkillAdapter::new();
        adapter.register_skill(
            "add_numbers",
            "Add two numbers",
            vec!["math".to_string()],
            |params| {
                let a = params["a"].as_i64().unwrap_or(0);
                let b = params["b"].as_i64().unwrap_or(0);
                Ok(serde_json::json!({"sum": a + b}))
            },
        );

        let skills = adapter.list_local_skills();
        let skill_id = skills[0].id.clone();

        let result = adapter
            .execute_local_skill(&skill_id, &serde_json::json!({"a": 5, "b": 3}))
            .await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap()["sum"], 8);
    }

    #[tokio::test]
    async fn test_execute_nonexistent_skill() {
        let adapter = SkillAdapter::new();
        let result = adapter
            .execute_local_skill("nonexistent", &serde_json::json!({}))
            .await;
        assert!(result.is_err());
    }
}
