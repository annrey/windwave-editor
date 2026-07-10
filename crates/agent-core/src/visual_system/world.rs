use crate::types::{current_timestamp, EntityId};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// 第 1 层：结构化世界视图
// ============================================================================

/// 组件摘要
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ComponentSummary {
    pub type_name: String,
    pub properties: HashMap<String, serde_json::Value>,
}

/// 实体详情 - 完整的实体信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityDetail {
    pub id: EntityId,
    pub name: String,
    pub components: Vec<ComponentSummary>,
    pub children: Vec<EntityId>,
    pub parent: Option<EntityId>,
}

/// 层级关系
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParentChildRelation {
    pub parent_id: EntityId,
    pub parent_name: String,
    pub child_id: EntityId,
    pub child_name: String,
}

/// 场景度量
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneMetrics {
    pub total_entities: usize,
    pub component_types: Vec<String>,
    pub avg_depth: f32,
    pub max_depth: usize,
}

/// 世界快照 - 完整的场景状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldSnapshot {
    pub entities: Vec<EntityDetail>,
    pub hierarchy: Vec<ParentChildRelation>,
    pub timestamp: u64,
    pub metrics: SceneMetrics,
}

impl WorldSnapshot {
    pub fn new() -> Self {
        Self {
            entities: Vec::new(),
            hierarchy: Vec::new(),
            timestamp: current_timestamp(),
            metrics: SceneMetrics {
                total_entities: 0,
                component_types: Vec::new(),
                avg_depth: 0.0,
                max_depth: 0,
            },
        }
    }

    /// 生成 LLM 可读的世界描述
    pub fn describe_for_llm(&self, detail_level: DetailLevel) -> String {
        match detail_level {
            DetailLevel::Brief => self.brief_summary(),
            DetailLevel::Normal => self.full_description(),
            DetailLevel::Detailed => self.detailed_with_components(),
        }
    }

    /// 简要摘要
    fn brief_summary(&self) -> String {
        format!(
            "场景包含 {} 个实体，{} 种组件类型",
            self.metrics.total_entities,
            self.metrics.component_types.len()
        )
    }

    /// 完整描述
    fn full_description(&self) -> String {
        let mut parts = Vec::new();
        parts.push(format!(
            "## 场景概览 ({} 实体)\n",
            self.metrics.total_entities
        ));

        for entity in &self.entities {
            parts.push(format!(
                "- {} (ID: {}): {} 个组件",
                entity.name,
                entity.id.0,
                entity.components.len()
            ));
        }

        parts.join("\n")
    }

    /// 详细组件描述
    fn detailed_with_components(&self) -> String {
        let mut parts = Vec::new();
        parts.push("## 完整场景状态\n".into());

        for entity in &self.entities {
            parts.push(format!("### {} (ID: {})\n", entity.name, entity.id.0));

            for comp in &entity.components {
                parts.push(format!("- {}: {:?}", comp.type_name, comp.properties));
            }

            if !entity.children.is_empty() {
                parts.push(format!("  子实体: {:?}", entity.children));
            }

            parts.push("\n".into());
        }

        parts.join("\n")
    }

    /// 转为 JSON 格式（供 Agent 推理）
    pub fn to_structured_json(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or(serde_json::json!({}))
    }
}

impl Default for WorldSnapshot {
    fn default() -> Self {
        Self::new()
    }
}

/// 详细程度
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetailLevel {
    Brief,    // 仅实体数量和类型
    Normal,   // 实体列表和组件数量
    Detailed, // 完整组件属性
}
