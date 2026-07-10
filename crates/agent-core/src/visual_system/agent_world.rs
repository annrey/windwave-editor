use super::*;
use crate::types::current_timestamp;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// 第 3 层：融合感知 + Agent 世界模型
// ============================================================================

/// 场景变化摘要
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneChangeSummary {
    pub timestamp: u64,
    pub entities_created: Vec<String>,
    pub entities_deleted: Vec<String>,
    pub entities_modified: Vec<String>,
    pub components_changed: Vec<ComponentChange>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentChange {
    pub entity_name: String,
    pub component_type: String,
    pub property: String,
    pub old_value: Option<serde_json::Value>,
    pub new_value: Option<serde_json::Value>,
}

/// Agent 世界视图 - Agent 的"眼睛"
pub struct AgentWorldView {
    /// 结构化数据
    pub snapshot: WorldSnapshot,
    /// 视觉分析结果
    pub visual_observation: Option<VisualObservation>,
    /// 上次操作后的变化
    pub last_change: Option<SceneChangeSummary>,
    /// 世界状态摘要（供 LLM 使用）
    pub summary: String,
    /// 截图历史
    pub screenshot_history: Vec<ScreenshotArtifact>,
    /// 最大截图历史数
    max_screenshot_history: usize,
}

impl AgentWorldView {
    pub fn new() -> Self {
        Self {
            snapshot: WorldSnapshot::new(),
            visual_observation: None,
            last_change: None,
            summary: String::new(),
            screenshot_history: Vec::new(),
            max_screenshot_history: 10,
        }
    }

    /// 更新世界视图（结构化数据）
    pub fn update_snapshot(&mut self, snapshot: WorldSnapshot) {
        // 检测变化
        if let Some(change) = self.detect_change(&snapshot) {
            self.last_change = Some(change);
        }

        self.snapshot = snapshot;
        self.summary = self.generate_summary();
    }

    /// 设置视觉分析结果
    pub fn set_visual_observation(&mut self, observation: VisualObservation) {
        self.visual_observation = Some(observation);
        self.summary = self.generate_summary();
    }

    /// 添加截图
    pub fn add_screenshot(&mut self, screenshot: ScreenshotArtifact) {
        self.screenshot_history.push(screenshot);
        // 限制历史数量
        if self.screenshot_history.len() > self.max_screenshot_history {
            self.screenshot_history.remove(0);
        }
    }

    /// 检测变化
    fn detect_change(&self, new: &WorldSnapshot) -> Option<SceneChangeSummary> {
        let old = &self.snapshot;

        let mut changes = SceneChangeSummary {
            timestamp: current_timestamp(),
            entities_created: Vec::new(),
            entities_deleted: Vec::new(),
            entities_modified: Vec::new(),
            components_changed: Vec::new(),
        };

        // 检测创建/删除
        let old_names: std::collections::HashSet<_> =
            old.entities.iter().map(|e| &e.name).collect();
        let new_names: std::collections::HashSet<_> =
            new.entities.iter().map(|e| &e.name).collect();

        for name in new_names.difference(&old_names) {
            changes.entities_created.push((*name).clone());
        }

        for name in old_names.difference(&new_names) {
            changes.entities_deleted.push((*name).clone());
        }

        // 检测修改（简化版）
        changes.entities_modified = new
            .entities
            .iter()
            .filter(|e| {
                old.entities.iter().any(|old_e| {
                    old_e.name == e.name && old_e.components.len() != e.components.len()
                })
            })
            .map(|e| e.name.clone())
            .collect();

        if changes.entities_created.is_empty()
            && changes.entities_deleted.is_empty()
            && changes.entities_modified.is_empty()
        {
            return None;
        }

        Some(changes)
    }

    /// 生成摘要
    fn generate_summary(&self) -> String {
        let mut parts = Vec::new();

        // 结构化数据摘要
        parts.push(self.snapshot.describe_for_llm(DetailLevel::Brief));

        // 视觉分析摘要
        if let Some(visual) = &self.visual_observation {
            parts.push(format!(
                "\n【视觉分析】检测到 {} 个实体，{} 个异常 (置信度: {:.0}%)",
                visual.visible_entities.len(),
                visual.anomalies.len(),
                visual.confidence * 100.0
            ));
        }

        // 最近变化
        if let Some(change) = &self.last_change {
            parts.push(format!(
                "\n【最近变化】创建 {} 个，删除 {} 个，修改 {} 个",
                change.entities_created.len(),
                change.entities_deleted.len(),
                change.entities_modified.len()
            ));
        }

        parts.join("\n")
    }

    /// 生成 LLM 可读的完整世界描述
    pub fn describe(&self) -> String {
        format!(
            "【当前场景】({})\n{}\n\n【视觉分析】\n{}\n\n【最近变化】\n{}",
            chrono::Utc::now().format("%Y-%m-%d %H:%M:%S"),
            self.snapshot.describe_for_llm(DetailLevel::Normal),
            self.visual_description(),
            self.change_description()
        )
    }

    /// 视觉描述
    fn visual_description(&self) -> String {
        match &self.visual_observation {
            Some(obs) => {
                let mut parts = Vec::new();
                for entity in &obs.visible_entities {
                    parts.push(format!(
                        "- {}: {} (置信度: {:.0}%)",
                        entity.name,
                        entity.detected_type,
                        entity.confidence * 100.0
                    ));
                }
                for anomaly in &obs.anomalies {
                    parts.push(format!(
                        "- 异常: {} ({:?})",
                        anomaly.description, anomaly.severity
                    ));
                }
                if parts.is_empty() {
                    "(无视觉分析)".into()
                } else {
                    parts.join("\n")
                }
            }
            None => "(未进行视觉分析)".into(),
        }
    }

    /// 变化描述
    fn change_description(&self) -> String {
        match &self.last_change {
            Some(change) => {
                let mut parts = Vec::new();
                for name in &change.entities_created {
                    parts.push(format!("- 创建: {}", name));
                }
                for name in &change.entities_deleted {
                    parts.push(format!("- 删除: {}", name));
                }
                for name in &change.entities_modified {
                    parts.push(format!("- 修改: {}", name));
                }
                if parts.is_empty() {
                    "(无变化)".into()
                } else {
                    parts.join("\n")
                }
            }
            None => "(无变化记录)".into(),
        }
    }

    /// 比较操作前后的状态差异
    pub fn diff_since(&self, after: &AgentWorldView) -> SceneDiff {
        SceneDiff::compute(self, after)
    }
}

impl Default for AgentWorldView {
    fn default() -> Self {
        Self::new()
    }
}

/// 场景差异
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneDiff {
    pub entities_added: Vec<EntityDetail>,
    pub entities_removed: Vec<EntityDetail>,
    pub entities_modified: Vec<EntityModification>,
    pub component_changes: Vec<ComponentChange>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityModification {
    pub entity: EntityDetail,
    pub changed_components: Vec<String>,
}

impl SceneDiff {
    pub fn compute(before: &AgentWorldView, after: &AgentWorldView) -> Self {
        let mut diff = Self {
            entities_added: Vec::new(),
            entities_removed: Vec::new(),
            entities_modified: Vec::new(),
            component_changes: Vec::new(),
        };

        let before_map: HashMap<_, _> = before
            .snapshot
            .entities
            .iter()
            .map(|e| (e.name.clone(), e.clone()))
            .collect();
        let after_map: HashMap<_, _> = after
            .snapshot
            .entities
            .iter()
            .map(|e| (e.name.clone(), e.clone()))
            .collect();

        // 添加的实体
        for (name, entity) in &after_map {
            if !before_map.contains_key(name) {
                diff.entities_added.push(entity.clone());
            }
        }

        // 删除的实体
        for (name, entity) in &before_map {
            if !after_map.contains_key(name) {
                diff.entities_removed.push(entity.clone());
            }
        }

        // 修改的实体
        for (name, after_entity) in &after_map {
            if let Some(before_entity) = before_map.get(name) {
                if before_entity.components != after_entity.components {
                    let changed_components: Vec<String> = after_entity
                        .components
                        .iter()
                        .filter(|c| {
                            !before_entity.components.iter().any(|bc| {
                                bc.type_name == c.type_name && bc.properties == c.properties
                            })
                        })
                        .map(|c| c.type_name.clone())
                        .collect();

                    if !changed_components.is_empty() {
                        diff.entities_modified.push(EntityModification {
                            entity: after_entity.clone(),
                            changed_components,
                        });
                    }
                }
            }
        }

        diff
    }
}
