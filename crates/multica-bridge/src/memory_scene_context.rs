//! Memory Scene Context
//!
//! 将 multica-db 中的场景数据集成到 agent-core 的四层记忆系统
//! 支持场景快照保存、恢复、差异记录和事件触发记忆更新

use crate::error::{BridgeError, Result};
use crate::scene_context::{SceneDiff, SceneEntity, SceneSnapshot};
use log::{debug, info, warn};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// 场景记忆条目
/// 用于记录场景的快照状态，支持 Working Memory 和 Episodic Memory
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneMemoryEntry {
    /// 场景唯一 ID
    pub scene_id: String,
    /// 场景名称
    pub scene_name: String,
    /// 场景快照
    pub snapshot: SceneSnapshot,
    /// 时间戳
    pub timestamp: u64,
    /// 上下文标签
    pub context_tags: Vec<String>,
    /// 关联任务 ID 列表
    pub related_task_ids: Vec<String>,
}

impl SceneMemoryEntry {
    /// 创建新的场景记忆条目
    pub fn new(scene_id: String, scene_name: String, snapshot: SceneSnapshot) -> Self {
        let timestamp = chrono::Utc::now().timestamp() as u64;
        Self {
            scene_id,
            scene_name,
            snapshot,
            timestamp,
            context_tags: Vec::new(),
            related_task_ids: Vec::new(),
        }
    }

    /// 添加上下文标签
    pub fn add_context_tag(&mut self, tag: &str) {
        if !self.context_tags.contains(&tag.to_string()) {
            self.context_tags.push(tag.to_string());
        }
    }

    /// 添加关联任务
    pub fn add_related_task(&mut self, task_id: &str) {
        if !self.related_task_ids.contains(&task_id.to_string()) {
            self.related_task_ids.push(task_id.to_string());
        }
    }

    /// 获取实体数量
    pub fn entity_count(&self) -> usize {
        self.snapshot.entities.len()
    }

    /// 查找实体
    pub fn find_entity(&self, entity_id: u64) -> Option<&SceneEntity> {
        self.snapshot.entities.iter().find(|e| e.id == entity_id)
    }

    /// 查找实体按名称
    pub fn find_entity_by_name(&self, name: &str) -> Option<&SceneEntity> {
        self.snapshot.entities.iter().find(|e| e.name == name)
    }
}

/// 场景变更记录
/// 用于记录场景的变更历史，支持 Episodic Memory 和差异对比
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneChangeEvent {
    /// 变更唯一 ID
    pub change_id: String,
    /// 场景 ID
    pub scene_id: String,
    /// 场景差异
    pub diff: SceneDiff,
    /// 影响的任务 ID 列表
    pub affected_tasks: Vec<String>,
    /// 时间戳
    pub timestamp: u64,
    /// 变更描述
    pub description: Option<String>,
}

impl SceneChangeEvent {
    /// 创建新的场景变更事件
    pub fn new(scene_id: String, diff: SceneDiff) -> Self {
        let change_id = format!("change_{}", uuid::Uuid::new_v4().as_simple());
        let timestamp = chrono::Utc::now().timestamp() as u64;
        Self {
            change_id,
            scene_id,
            diff,
            affected_tasks: Vec::new(),
            timestamp,
            description: None,
        }
    }

    /// 设置变更描述
    pub fn with_description(mut self, description: String) -> Self {
        self.description = Some(description);
        self
    }

    /// 添加影响的任务
    pub fn add_affected_task(&mut self, task_id: &str) {
        if !self.affected_tasks.contains(&task_id.to_string()) {
            self.affected_tasks.push(task_id.to_string());
        }
    }

    /// 获取变更摘要
    pub fn summary(&self) -> String {
        format!(
            "Scene {}: +{} added, -{} removed, ~{} updated",
            self.scene_id,
            self.diff.added.len(),
            self.diff.removed.len(),
            self.diff.modified.len()
        )
    }
}

/// 场景关系知识图谱节点
/// 用于 Semantic Memory，记录场景、实体、任务之间的关系
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneRelationNode {
    /// 节点 ID
    pub node_id: String,
    /// 节点类型
    pub node_type: SceneNodeType,
    /// 节点名称
    pub name: String,
    /// 节点属性
    pub properties: HashMap<String, String>,
}

/// 节点类型枚举
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SceneNodeType {
    /// 场景节点
    Scene,
    /// 实体节点
    Entity,
    /// 任务节点
    Task,
    /// 资源节点
    Resource,
}

/// 场景关系边
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneRelationEdge {
    /// 起始节点 ID
    pub from_node_id: String,
    /// 目标节点 ID
    pub to_node_id: String,
    /// 关系类型
    pub relation_type: String,
    /// 关系属性
    pub properties: HashMap<String, String>,
}

/// 场景上下文记忆管理器
/// 管理场景相关的记忆条目、变更事件和关系图谱
pub struct SceneContextMemory {
    /// 当前活跃的场景记忆
    pub active_scene: Option<SceneMemoryEntry>,
    /// 场景历史记忆 (按场景 ID 索引)
    pub scene_history: HashMap<String, Vec<SceneMemoryEntry>>,
    /// 场景变更事件历史
    pub change_events: Vec<SceneChangeEvent>,
    /// 场景关系图谱节点
    pub relation_nodes: HashMap<String, SceneRelationNode>,
    /// 场景关系图谱边
    pub relation_edges: Vec<SceneRelationEdge>,
}

impl SceneContextMemory {
    /// 创建新的场景上下文记忆管理器
    pub fn new() -> Self {
        Self {
            active_scene: None,
            scene_history: HashMap::new(),
            change_events: Vec::new(),
            relation_nodes: HashMap::new(),
            relation_edges: Vec::new(),
        }
    }

    /// 保存场景快照到记忆
    pub fn save_scene_snapshot(
        &mut self,
        scene_id: String,
        scene_name: String,
        snapshot: SceneSnapshot,
    ) -> Result<()> {
        let entry = SceneMemoryEntry::new(scene_id.clone(), scene_name, snapshot.clone());

        // 更新活跃场景
        self.active_scene = Some(entry.clone());

        // 添加到历史记录
        self.scene_history
            .entry(scene_id.clone())
            .or_default()
            .push(entry);

        info!(
            "Scene snapshot saved to memory: {} ({} entities)",
            scene_id,
            snapshot.entities.len()
        );

        Ok(())
    }

    /// 恢复场景快照
    pub fn restore_scene_snapshot(&self, scene_id: &str) -> Option<SceneSnapshot> {
        self.scene_history
            .get(scene_id)
            .and_then(|entries| entries.last())
            .map(|entry| entry.snapshot.clone())
    }

    /// 获取最近的场景快照
    pub fn get_latest_scene(&self, scene_id: &str) -> Option<&SceneMemoryEntry> {
        self.scene_history
            .get(scene_id)
            .and_then(|entries| entries.last())
    }

    /// 记录场景变更
    pub fn record_scene_change(&mut self, diff: SceneDiff, scene_id: String) -> Result<String> {
        let change_event = SceneChangeEvent::new(scene_id, diff);
        let summary = change_event.summary();

        debug!("Recording scene change: {}", summary);

        self.change_events.push(change_event.clone());

        // 更新活跃场景的版本号
        if let Some(active) = &mut self.active_scene {
            if active.scene_id == change_event.scene_id {
                // Note: We would need to apply the diff to update the snapshot
                // For now, we just record the change event
            }
        }

        Ok(change_event.change_id)
    }

    /// 获取场景变更历史
    pub fn get_change_history(&self, scene_id: &str) -> Vec<&SceneChangeEvent> {
        self.change_events
            .iter()
            .filter(|event| event.scene_id == scene_id)
            .collect()
    }

    /// 获取最近的变更事件
    pub fn recent_changes(&self, count: usize) -> &[SceneChangeEvent] {
        let start = self.change_events.len().saturating_sub(count);
        &self.change_events[start..]
    }

    /// 添加场景关系节点
    pub fn add_relation_node(
        &mut self,
        node_type: SceneNodeType,
        name: String,
        properties: HashMap<String, String>,
    ) -> String {
        let node_id = format!("node_{}", uuid::Uuid::new_v4().as_simple());
        let node = SceneRelationNode {
            node_id: node_id.clone(),
            node_type,
            name,
            properties,
        };
        self.relation_nodes.insert(node_id.clone(), node);
        node_id
    }

    /// 添加场景关系边
    pub fn add_relation_edge(
        &mut self,
        from_node_id: String,
        to_node_id: String,
        relation_type: String,
    ) {
        let edge = SceneRelationEdge {
            from_node_id,
            to_node_id,
            relation_type,
            properties: HashMap::new(),
        };
        self.relation_edges.push(edge);
    }

    /// 获取场景的关联节点
    pub fn get_scene_related_nodes(&self, scene_id: &str) -> Vec<&SceneRelationNode> {
        self.relation_edges
            .iter()
            .filter(|edge| {
                self.relation_nodes
                    .get(&edge.from_node_id)
                    .is_some_and(|node| {
                        node.properties
                            .get("scene_id")
                            .is_some_and(|id| id == scene_id)
                    })
                    || self
                        .relation_nodes
                        .get(&edge.to_node_id)
                        .is_some_and(|node| {
                            node.properties
                                .get("scene_id")
                                .is_some_and(|id| id == scene_id)
                        })
            })
            .flat_map(|edge| {
                vec![
                    self.relation_nodes.get(&edge.from_node_id),
                    self.relation_nodes.get(&edge.to_node_id),
                ]
                .into_iter()
                .flatten()
            })
            .collect()
    }

    /// 获取统计信息
    pub fn get_statistics(&self) -> SceneMemoryStats {
        SceneMemoryStats {
            active_scene_id: self.active_scene.as_ref().map(|s| s.scene_id.clone()),
            total_scene_entries: self.scene_history.values().map(|v| v.len()).sum(),
            total_change_events: self.change_events.len(),
            total_relation_nodes: self.relation_nodes.len(),
            total_relation_edges: self.relation_edges.len(),
        }
    }
}

impl Default for SceneContextMemory {
    fn default() -> Self {
        Self::new()
    }
}

/// 场景记忆统计信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneMemoryStats {
    pub active_scene_id: Option<String>,
    pub total_scene_entries: usize,
    pub total_change_events: usize,
    pub total_relation_nodes: usize,
    pub total_relation_edges: usize,
}

/// 线程安全的场景上下文记忆
pub type SharedSceneContextMemory = Arc<Mutex<SceneContextMemory>>;

/// 创建线程安全的场景上下文记忆
pub fn create_shared_scene_context_memory() -> SharedSceneContextMemory {
    Arc::new(Mutex::new(SceneContextMemory::new()))
}

/// 场景记忆注入器
/// 将场景数据注入到 agent-core 的记忆系统中
pub struct SceneMemoryInjector {
    scene_memory: SharedSceneContextMemory,
}

impl SceneMemoryInjector {
    /// 创建新的记忆注入器
    pub fn new(scene_memory: SharedSceneContextMemory) -> Self {
        Self { scene_memory }
    }

    /// 注入场景快照到 Working Memory (L3)
    pub fn inject_to_working_memory(&self, scene_id: &str) -> Result<String> {
        let memory = self.scene_memory.lock().expect("mutex poisoned");
        if let Some(entry) = memory.get_latest_scene(scene_id) {
            // In a real implementation, this would inject into agent-core's WorkingMemory
            // For now, we just log it
            info!(
                "Injecting scene {} to Working Memory (L3): {} entities",
                scene_id,
                entry.entity_count()
            );
            Ok(format!("Injected {} entities", entry.entity_count()))
        } else {
            Err(BridgeError::Other(format!("Scene not found: {}", scene_id)))
        }
    }

    /// 注入场景变更到 Episodic Memory (L2)
    pub fn inject_changes_to_episodic_memory(&self, scene_id: &str) -> Result<Vec<String>> {
        let memory = self.scene_memory.lock().expect("mutex poisoned");
        let changes = memory.get_change_history(scene_id);

        let summaries: Vec<String> = changes.iter().map(|change| change.summary()).collect();

        info!(
            "Injecting {} scene changes to Episodic Memory (L2): {}",
            summaries.len(),
            scene_id
        );

        Ok(summaries)
    }

    /// 注入场景关系到 Semantic Memory (L1)
    pub fn inject_relations_to_semantic_memory(&self, scene_id: &str) -> Result<usize> {
        let memory = self.scene_memory.lock().expect("mutex poisoned");
        let related_nodes = memory.get_scene_related_nodes(scene_id);

        info!(
            "Injecting {} relations to Semantic Memory (L1): {}",
            related_nodes.len(),
            scene_id
        );

        Ok(related_nodes.len())
    }

    /// 注入场景操作模式到 Procedural Memory (L0)
    pub fn inject_patterns_to_procedural_memory(&self, scene_id: &str) -> Result<String> {
        // Analyze scene changes to find patterns
        let memory = self.scene_memory.lock().expect("mutex poisoned");
        let changes = memory.get_change_history(scene_id);

        if changes.is_empty() {
            return Err(BridgeError::Other(format!(
                "No changes found for scene: {}",
                scene_id
            )));
        }

        // Simple pattern: count entity additions vs removals
        let additions: usize = changes.iter().map(|c| c.diff.added.len()).sum();
        let removals: usize = changes.iter().map(|c| c.diff.removed.len()).sum();

        let pattern = format!(
            "Scene {} pattern: +{} entities, -{} entities over {} changes",
            scene_id,
            additions,
            removals,
            changes.len()
        );

        info!("Injecting pattern to Procedural Memory (L0): {}", pattern);
        Ok(pattern)
    }
}

/// 场景变更事件监听器
/// 与 scene_event_bus 集成，自动记录场景变更事件
pub struct SceneChangeListener {
    scene_memory: SharedSceneContextMemory,
}

impl SceneChangeListener {
    /// 创建新的场景变更监听器
    pub fn new(scene_memory: SharedSceneContextMemory) -> Self {
        Self { scene_memory }
    }

    /// 处理场景实体创建事件
    pub fn on_entity_created(&self, scene_id: &str, entity: &SceneEntity) {
        info!(
            "Entity created in scene {}: {} (id={})",
            scene_id, entity.name, entity.id
        );
        // In real implementation, this would trigger memory update
    }

    /// 处理场景实体更新事件
    pub fn on_entity_updated(&self, scene_id: &str, entity: &SceneEntity) {
        debug!(
            "Entity updated in scene {}: {} (id={})",
            scene_id, entity.name, entity.id
        );
    }

    /// 处理场景实体删除事件
    pub fn on_entity_deleted(&self, scene_id: &str, entity_id: u64) {
        info!("Entity deleted in scene {}: id={}", scene_id, entity_id);
    }

    /// 处理场景快照更新
    pub fn on_snapshot_updated(&self, scene_id: &str, scene_name: &str, snapshot: &SceneSnapshot) {
        let mut memory = self.scene_memory.lock().expect("mutex poisoned");
        if let Err(e) = memory.save_scene_snapshot(
            scene_id.to_string(),
            scene_name.to_string(),
            snapshot.clone(),
        ) {
            warn!("Failed to save scene snapshot: {}", e);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scene_memory_entry_creation() {
        let snapshot = SceneSnapshot {
            version: 1,
            timestamp: "2024-01-01T00:00:00Z".to_string(),
            entities: vec![SceneEntity {
                id: 1,
                name: "Player".to_string(),
                components: vec![],
                position: Some([0.0, 0.0, 0.0]),
            }],
        };

        let entry = SceneMemoryEntry::new("scene-1".to_string(), "MainScene".to_string(), snapshot);

        assert_eq!(entry.scene_id, "scene-1");
        assert_eq!(entry.scene_name, "MainScene");
        assert_eq!(entry.entity_count(), 1);
    }

    #[test]
    fn test_scene_memory_entry_tags() {
        let snapshot = SceneSnapshot {
            version: 1,
            timestamp: "2024-01-01T00:00:00Z".to_string(),
            entities: vec![],
        };

        let mut entry =
            SceneMemoryEntry::new("scene-1".to_string(), "TestScene".to_string(), snapshot);

        entry.add_context_tag("combat");
        entry.add_context_tag("exploration");
        entry.add_related_task("task-123");

        assert_eq!(entry.context_tags.len(), 2);
        assert_eq!(entry.related_task_ids.len(), 1);
    }

    #[test]
    fn test_scene_memory_entry_find_entity() {
        let snapshot = SceneSnapshot {
            version: 1,
            timestamp: "2024-01-01T00:00:00Z".to_string(),
            entities: vec![SceneEntity {
                id: 42,
                name: "Enemy".to_string(),
                components: vec![],
                position: Some([10.0, 0.0, 5.0]),
            }],
        };

        let entry = SceneMemoryEntry::new("scene-1".to_string(), "TestScene".to_string(), snapshot);

        let found = entry.find_entity(42);
        assert!(found.is_some());
        assert_eq!(found.unwrap().name, "Enemy");

        let not_found = entry.find_entity(999);
        assert!(not_found.is_none());

        let by_name = entry.find_entity_by_name("Enemy");
        assert!(by_name.is_some());
    }

    #[test]
    fn test_scene_change_event() {
        let diff = SceneDiff {
            added: vec![SceneEntity {
                id: 1,
                name: "NewEntity".to_string(),
                components: vec![],
                position: None,
            }],
            removed: vec![],
            modified: vec![],
        };

        let event = SceneChangeEvent::new("scene-1".to_string(), diff);
        let summary = event.summary();

        assert!(summary.contains("+1 added"));
        assert!(summary.contains("0 removed"));
        assert!(summary.contains("0 updated"));
    }

    #[test]
    fn test_scene_context_memory_save_and_restore() {
        let mut memory = SceneContextMemory::new();

        let snapshot = SceneSnapshot {
            version: 1,
            timestamp: "2024-01-01T00:00:00Z".to_string(),
            entities: vec![SceneEntity {
                id: 1,
                name: "Player".to_string(),
                components: vec![],
                position: Some([0.0, 0.0, 0.0]),
            }],
        };

        memory
            .save_scene_snapshot(
                "scene-1".to_string(),
                "MainScene".to_string(),
                snapshot.clone(),
            )
            .unwrap();

        let restored = memory.restore_scene_snapshot("scene-1");
        assert!(restored.is_some());
        assert_eq!(restored.unwrap().entities.len(), 1);
    }

    #[test]
    fn test_scene_context_memory_change_record() {
        let mut memory = SceneContextMemory::new();

        let diff = SceneDiff {
            added: vec![SceneEntity {
                id: 1,
                name: "Entity".to_string(),
                components: vec![],
                position: None,
            }],
            removed: vec![],
            modified: vec![],
        };

        let change_id = memory
            .record_scene_change(diff, "scene-1".to_string())
            .unwrap();

        assert!(!change_id.is_empty());
        assert_eq!(memory.change_events.len(), 1);

        let history = memory.get_change_history("scene-1");
        assert_eq!(history.len(), 1);
    }

    #[test]
    fn test_scene_context_memory_relation_nodes() {
        let mut memory = SceneContextMemory::new();

        let mut props = HashMap::new();
        props.insert("scene_id".to_string(), "scene-1".to_string());

        let node_id = memory.add_relation_node(SceneNodeType::Entity, "Player".to_string(), props);

        assert!(!node_id.is_empty());
        assert_eq!(memory.relation_nodes.len(), 1);
    }

    #[test]
    fn test_memory_injector() {
        let shared_memory = create_shared_scene_context_memory();

        {
            let mut memory = shared_memory.lock().expect("mutex poisoned");
            let snapshot = SceneSnapshot {
                version: 1,
                timestamp: "2024-01-01T00:00:00Z".to_string(),
                entities: vec![SceneEntity {
                    id: 1,
                    name: "Player".to_string(),
                    components: vec![],
                    position: None,
                }],
            };

            memory
                .save_scene_snapshot("scene-1".to_string(), "MainScene".to_string(), snapshot)
                .unwrap();
        }

        let injector = SceneMemoryInjector::new(shared_memory);
        let result = injector.inject_to_working_memory("scene-1");
        assert!(result.is_ok());
        assert!(result.unwrap().contains("1 entities"));
    }

    #[test]
    fn test_memory_injector_not_found() {
        let shared_memory = create_shared_scene_context_memory();
        let injector = SceneMemoryInjector::new(shared_memory);

        let result = injector.inject_to_working_memory("nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn test_scene_change_listener() {
        let shared_memory = create_shared_scene_context_memory();
        let listener = SceneChangeListener::new(shared_memory.clone());

        let entity = SceneEntity {
            id: 1,
            name: "TestEntity".to_string(),
            components: vec![],
            position: Some([0.0, 0.0, 0.0]),
        };

        listener.on_entity_created("scene-1", &entity);
        listener.on_entity_updated("scene-1", &entity);
        listener.on_entity_deleted("scene-1", 1);

        // Verify no crashes - the listener should handle all events
        assert!(shared_memory
            .lock()
            .expect("mutex poisoned")
            .active_scene
            .is_none());
    }

    #[test]
    fn test_scene_change_listener_snapshot_update() {
        let shared_memory = create_shared_scene_context_memory();
        let listener = SceneChangeListener::new(shared_memory.clone());

        let snapshot = SceneSnapshot {
            version: 1,
            timestamp: "2024-01-01T00:00:00Z".to_string(),
            entities: vec![SceneEntity {
                id: 1,
                name: "Player".to_string(),
                components: vec![],
                position: None,
            }],
        };

        listener.on_snapshot_updated("scene-1", "MainScene", &snapshot);

        let memory = shared_memory.lock().expect("mutex poisoned");
        assert!(memory.active_scene.is_some());
        assert_eq!(memory.active_scene.as_ref().unwrap().scene_id, "scene-1");
    }
}
