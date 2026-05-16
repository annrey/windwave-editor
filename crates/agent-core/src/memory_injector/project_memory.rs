use crate::types::{EntityId, current_timestamp};
use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::fs;
use crate::memory_injector::MemoryError;

/// 项目元数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectManifest {
    pub name: String,
    pub engine: String,
    pub engine_version: String,
    pub created_at: u64,
    pub last_modified: u64,
}

impl Default for ProjectManifest {
    fn default() -> Self {
        Self {
            name: "Untitled".into(),
            engine: "bevy".into(),
            engine_version: "0.17".into(),
            created_at: current_timestamp(),
            last_modified: current_timestamp(),
        }
    }
}

/// 实体知识库 - 记住每个实体的详细信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityKnowledge {
    pub name: String,
    pub entity_id: EntityId,
    pub purpose: String,
    pub systems: Vec<String>,
    pub components: Vec<String>,
    pub related_entities: Vec<String>,
    pub common_operations: Vec<String>,
    pub last_modified: u64,
    pub notes: String,
}

impl EntityKnowledge {
    pub fn new(name: String, entity_id: EntityId) -> Self {
        Self {
            name,
            entity_id,
            purpose: String::new(),
            systems: Vec::new(),
            components: Vec::new(),
            related_entities: Vec::new(),
            common_operations: Vec::new(),
            last_modified: current_timestamp(),
            notes: String::new(),
        }
    }

    pub fn update(&mut self, purpose: Option<&str>, components: Option<&Vec<String>>) {
        if let Some(p) = purpose {
            self.purpose = p.to_string();
        }
        if let Some(c) = components {
            self.components = c.clone();
        }
        self.last_modified = current_timestamp();
    }

    pub fn record_operation(&mut self, operation: &str) {
        if !self.common_operations.contains(&operation.to_string()) {
            self.common_operations.push(operation.to_string());
        }
        self.last_modified = current_timestamp();
    }
}

/// 系统知识库
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemKnowledge {
    pub name: String,
    pub file_path: String,
    pub description: String,
    pub entities_affected: Vec<String>,
    pub components_used: Vec<String>,
    pub dependencies: Vec<String>,
}

/// 项目约定
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Convention {
    pub category: ConventionCategory,
    pub description: String,
    pub example: String,
    pub confidence: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConventionCategory {
    Naming,
    Style,
    Pattern,
}

/// 项目工作流模板
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowTemplate {
    pub name: String,
    pub trigger_keywords: Vec<String>,
    pub steps: Vec<String>,
    pub success_rate: f32,
}

/// 项目变更历史
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectChange {
    pub timestamp: u64,
    pub description: String,
    pub entity_affected: Option<String>,
    pub change_type: ChangeType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChangeType {
    Created,
    Modified,
    Deleted,
    Moved,
}

impl ProjectChange {
    pub fn change_type_as_string(&self) -> &'static str {
        match self.change_type {
            ChangeType::Created => "创建",
            ChangeType::Modified => "修改",
            ChangeType::Deleted => "删除",
            ChangeType::Moved => "移动",
        }
    }
}

/// 项目记忆 - 跨会话记住项目的所有重要信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectMemory {
    pub manifest: ProjectManifest,
    pub entity_knowledge: HashMap<String, EntityKnowledge>,
    pub system_knowledge: HashMap<String, SystemKnowledge>,
    pub conventions: Vec<Convention>,
    pub workflows: Vec<WorkflowTemplate>,
    pub change_log: Vec<ProjectChange>,
    pub user_preferences: HashMap<String, String>,
}

impl ProjectMemory {
    pub fn new(manifest: ProjectManifest) -> Self {
        Self {
            manifest,
            entity_knowledge: HashMap::new(),
            system_knowledge: HashMap::new(),
            conventions: Vec::new(),
            workflows: Vec::new(),
            change_log: Vec::new(),
            user_preferences: HashMap::new(),
        }
    }

    pub fn add_entity(&mut self, knowledge: EntityKnowledge) {
        self.entity_knowledge.insert(knowledge.name.clone(), knowledge);
        self.manifest.last_modified = current_timestamp();
    }

    pub fn get_entity(&self, name: &str) -> Option<&EntityKnowledge> {
        self.entity_knowledge.get(name)
    }

    pub fn get_entity_mut(&mut self, name: &str) -> Option<&mut EntityKnowledge> {
        self.entity_knowledge.get_mut(name)
    }

    pub fn record_change(&mut self, description: String, entity: Option<String>, change_type: ChangeType) {
        self.change_log.push(ProjectChange {
            timestamp: current_timestamp(),
            description,
            entity_affected: entity,
            change_type,
        });
        if self.change_log.len() > 100 {
            self.change_log.drain(0..self.change_log.len() - 100);
        }
        self.manifest.last_modified = current_timestamp();
    }

    pub fn set_preference(&mut self, key: &str, value: &str) {
        self.user_preferences.insert(key.to_string(), value.to_string());
    }

    pub fn get_preference(&self, key: &str) -> Option<&String> {
        self.user_preferences.get(key)
    }

    pub fn save(&self, path: &Path) -> Result<(), MemoryError> {
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| MemoryError::Serialization(e.to_string()))?;
        fs::write(path, json).map_err(|e| MemoryError::Io(e.to_string()))?;
        Ok(())
    }

    pub fn load(path: &Path) -> Result<Self, MemoryError> {
        if !path.exists() {
            return Ok(Self::new(ProjectManifest::default()));
        }
        let json = fs::read_to_string(path)
            .map_err(|e| MemoryError::Io(e.to_string()))?;
        serde_json::from_str(&json).map_err(|e| MemoryError::Serialization(e.to_string()))
    }

    /// 生成实体描述（用于 LLM 上下文）
    pub fn describe_entities(&self, names: &[String]) -> String {
        if names.is_empty() {
            return "(no entities)".into();
        }
        let mut parts = Vec::new();
        for name in names {
            if let Some(entity) = self.entity_knowledge.get(name) {
                parts.push(format!(
                    "- {}: {} (组件: {}, 关联系统: {})",
                    entity.name,
                    entity.purpose,
                    entity.components.join(", "),
                    entity.systems.join(", ")
                ));
            }
        }
        parts.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_project_memory_entity_knowledge() {
        let mut memory = ProjectMemory::new(ProjectManifest::default());
        let entity = EntityKnowledge::new("Player".into(), EntityId(1));
        memory.add_entity(entity);
        assert!(memory.get_entity("Player").is_some());
        assert_eq!(memory.get_entity("Player").unwrap().name, "Player");
    }
}
