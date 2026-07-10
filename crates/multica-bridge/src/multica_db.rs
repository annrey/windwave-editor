//! Multica DB - 统一数据访问层
//!
//! 内存存储原型，支持游戏场景、实体、资源的存储和查询
//! 与 Multica 表结构兼容，为未来 PostgreSQL 实现铺路

use crate::error::{BridgeError, Result};
use crate::scene_context::{ComponentData, SceneSnapshot};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

// ============================================================================
// 数据结构定义
// ============================================================================

/// 游戏场景记录
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SceneRecord {
    /// 场景唯一 ID
    pub scene_id: String,
    /// 场景名称
    pub name: String,
    /// 场景描述
    pub description: Option<String>,
    /// 创建时间戳
    pub created_at: String,
    /// 更新时间戳
    pub updated_at: String,
    /// 是否活跃
    pub is_active: bool,
}

/// 游戏实体记录
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EntityRecord {
    /// 实体唯一 ID
    pub entity_id: u64,
    /// 所属场景 ID
    pub scene_id: String,
    /// 实体名称
    pub name: String,
    /// 实体类型标签
    pub entity_type: String,
    /// 组件数据
    pub components: Vec<ComponentData>,
    /// 位置信息
    pub position: Option<[f64; 3]>,
    /// 创建时间戳
    pub created_at: String,
    /// 更新时间戳
    pub updated_at: String,
}

/// 游戏资源记录
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ResourceRecord {
    /// 资源唯一 ID
    pub resource_id: String,
    /// 资源类型
    pub resource_type: String,
    /// 资源名称
    pub name: String,
    /// 资源路径
    pub path: Option<String>,
    /// 资源元数据
    pub metadata: HashMap<String, String>,
    /// 创建时间戳
    pub created_at: String,
    /// 更新时间戳
    pub updated_at: String,
}

/// 任务-场景关联记录
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TaskSceneRelation {
    /// 关联 ID
    pub relation_id: String,
    /// 任务 ID
    pub task_id: String,
    /// 场景 ID
    pub scene_id: String,
    /// 关联类型
    pub relation_type: TaskSceneRelationType,
    /// 创建时间戳
    pub created_at: String,
}

/// 任务-场景关联类型
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskSceneRelationType {
    /// 任务在该场景中执行
    ExecutesIn,
    /// 任务修改该场景
    Modifies,
    /// 任务引用该场景
    References,
}

/// 查询结果 - 任务+场景+实体信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskSceneEntityQueryResult {
    /// 任务信息
    pub task: TaskRecord,
    /// 关联场景信息
    pub scene: SceneRecord,
    /// 场景中的实体列表
    pub entities: Vec<EntityRecord>,
    /// 相关资源列表
    pub resources: Vec<ResourceRecord>,
}

/// 任务记录（简化版，用于展示）
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TaskRecord {
    /// 任务 ID
    pub task_id: String,
    /// 任务标题
    pub title: String,
    /// 任务描述
    pub description: Option<String>,
    /// 任务状态
    pub status: String,
    /// 创建时间
    pub created_at: String,
}

// ============================================================================
// Multica DB 主结构体
// ============================================================================

/// Multica 数据库（内存原型）
pub struct MulticaDb {
    /// 场景存储
    scenes: HashMap<String, SceneRecord>,
    /// 实体存储
    entities: HashMap<u64, EntityRecord>,
    /// 场景 -> 实体索引
    scene_entities: HashMap<String, HashSet<u64>>,
    /// 资源存储
    resources: HashMap<String, ResourceRecord>,
    /// 任务-场景关联存储
    task_scene_relations: HashMap<String, TaskSceneRelation>,
    /// 任务 -> 场景索引
    task_scene_index: HashMap<String, HashSet<String>>,
    /// 下一个实体 ID
    next_entity_id: u64,
}

impl MulticaDb {
    /// 创建新的 Multica 数据库
    pub fn new() -> Self {
        Self {
            scenes: HashMap::new(),
            entities: HashMap::new(),
            scene_entities: HashMap::new(),
            resources: HashMap::new(),
            task_scene_relations: HashMap::new(),
            task_scene_index: HashMap::new(),
            next_entity_id: 1,
        }
    }

    // ========================================================================
    // 场景操作
    // ========================================================================

    /// 创建场景
    pub fn create_scene(&mut self, name: String, description: Option<String>) -> Result<String> {
        let scene_id = format!("scene_{}", uuid::Uuid::new_v4().as_simple());
        let now = chrono::Utc::now().to_rfc3339();

        let scene = SceneRecord {
            scene_id: scene_id.clone(),
            name,
            description,
            created_at: now.clone(),
            updated_at: now,
            is_active: true,
        };

        self.scenes.insert(scene_id.clone(), scene);
        self.scene_entities.insert(scene_id.clone(), HashSet::new());

        Ok(scene_id)
    }

    /// 获取场景
    pub fn get_scene(&self, scene_id: &str) -> Option<SceneRecord> {
        self.scenes.get(scene_id).cloned()
    }

    /// 获取所有场景
    pub fn get_all_scenes(&self) -> Vec<SceneRecord> {
        self.scenes.values().cloned().collect()
    }

    /// 更新场景
    pub fn update_scene(
        &mut self,
        scene_id: &str,
        name: Option<String>,
        description: Option<String>,
    ) -> Result<()> {
        let scene = self
            .scenes
            .get_mut(scene_id)
            .ok_or_else(|| BridgeError::Other(format!("Scene not found: {}", scene_id)))?;

        if let Some(n) = name {
            scene.name = n;
        }
        if let Some(d) = description {
            scene.description = Some(d);
        }
        scene.updated_at = chrono::Utc::now().to_rfc3339();

        Ok(())
    }

    /// 删除场景
    pub fn delete_scene(&mut self, scene_id: &str) -> Result<()> {
        if let Some(entity_ids) = self.scene_entities.remove(scene_id) {
            for entity_id in entity_ids {
                self.entities.remove(&entity_id);
            }
        }
        self.scenes
            .remove(scene_id)
            .ok_or_else(|| BridgeError::Other(format!("Scene not found: {}", scene_id)))?;

        Ok(())
    }

    // ========================================================================
    // 实体操作
    // ========================================================================

    /// 创建实体
    pub fn create_entity(
        &mut self,
        scene_id: String,
        name: String,
        entity_type: String,
        components: Vec<ComponentData>,
        position: Option<[f64; 3]>,
    ) -> Result<u64> {
        if !self.scenes.contains_key(&scene_id) {
            return Err(BridgeError::Other(format!("Scene not found: {}", scene_id)));
        }

        let entity_id = self.next_entity_id;
        self.next_entity_id += 1;

        let now = chrono::Utc::now().to_rfc3339();
        let entity = EntityRecord {
            entity_id,
            scene_id: scene_id.clone(),
            name,
            entity_type,
            components,
            position,
            created_at: now.clone(),
            updated_at: now,
        };

        self.entities.insert(entity_id, entity.clone());
        self.scene_entities
            .get_mut(&scene_id)
            .expect("scene_entities not initialized for scene")
            .insert(entity_id);

        Ok(entity_id)
    }

    /// 获取实体
    pub fn get_entity(&self, entity_id: u64) -> Option<EntityRecord> {
        self.entities.get(&entity_id).cloned()
    }

    /// 获取场景中的所有实体
    pub fn get_scene_entities(&self, scene_id: &str) -> Vec<EntityRecord> {
        self.scene_entities
            .get(scene_id)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| self.entities.get(id))
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }

    /// 更新实体
    pub fn update_entity(
        &mut self,
        entity_id: u64,
        name: Option<String>,
        components: Option<Vec<ComponentData>>,
        position: Option<Option<[f64; 3]>>,
    ) -> Result<()> {
        let entity = self
            .entities
            .get_mut(&entity_id)
            .ok_or_else(|| BridgeError::Other(format!("Entity not found: {}", entity_id)))?;

        if let Some(n) = name {
            entity.name = n;
        }
        if let Some(c) = components {
            entity.components = c;
        }
        if let Some(p) = position {
            entity.position = p;
        }
        entity.updated_at = chrono::Utc::now().to_rfc3339();

        Ok(())
    }

    /// 删除实体
    pub fn delete_entity(&mut self, entity_id: u64) -> Result<()> {
        let entity = self
            .entities
            .remove(&entity_id)
            .ok_or_else(|| BridgeError::Other(format!("Entity not found: {}", entity_id)))?;

        if let Some(entities) = self.scene_entities.get_mut(&entity.scene_id) {
            entities.remove(&entity_id);
        }

        Ok(())
    }

    /// 从场景快照同步实体
    pub fn sync_from_scene_snapshot(
        &mut self,
        scene_id: &str,
        snapshot: SceneSnapshot,
    ) -> Result<()> {
        if !self.scenes.contains_key(scene_id) {
            return Err(BridgeError::Other(format!("Scene not found: {}", scene_id)));
        }

        let existing_ids: HashSet<u64> = self
            .scene_entities
            .get(scene_id)
            .cloned()
            .unwrap_or_default();

        let mut new_ids = HashSet::new();

        for scene_entity in snapshot.entities {
            if scene_entity.id == 0 {
                let new_id = self.create_entity(
                    scene_id.to_string(),
                    scene_entity.name,
                    "GameObject".to_string(),
                    scene_entity.components,
                    scene_entity.position,
                )?;
                new_ids.insert(new_id);
            } else {
                if existing_ids.contains(&scene_entity.id) {
                    self.update_entity(
                        scene_entity.id,
                        Some(scene_entity.name),
                        Some(scene_entity.components),
                        Some(scene_entity.position),
                    )?;
                } else {
                    let entity = EntityRecord {
                        entity_id: scene_entity.id,
                        scene_id: scene_id.to_string(),
                        name: scene_entity.name,
                        entity_type: "GameObject".to_string(),
                        components: scene_entity.components,
                        position: scene_entity.position,
                        created_at: chrono::Utc::now().to_rfc3339(),
                        updated_at: chrono::Utc::now().to_rfc3339(),
                    };
                    self.entities.insert(scene_entity.id, entity);
                    self.scene_entities
                        .get_mut(scene_id)
                        .expect("scene_entities not initialized for synced scene")
                        .insert(scene_entity.id);
                }
                new_ids.insert(scene_entity.id);
            }
        }

        for old_id in existing_ids.difference(&new_ids) {
            self.delete_entity(*old_id)?;
        }

        Ok(())
    }

    // ========================================================================
    // 资源操作
    // ========================================================================

    /// 创建资源
    pub fn create_resource(
        &mut self,
        resource_type: String,
        name: String,
        path: Option<String>,
        metadata: HashMap<String, String>,
    ) -> Result<String> {
        let resource_id = format!("res_{}", uuid::Uuid::new_v4().as_simple());
        let now = chrono::Utc::now().to_rfc3339();

        let resource = ResourceRecord {
            resource_id: resource_id.clone(),
            resource_type,
            name,
            path,
            metadata,
            created_at: now.clone(),
            updated_at: now,
        };

        self.resources.insert(resource_id.clone(), resource);
        Ok(resource_id)
    }

    /// 获取资源
    pub fn get_resource(&self, resource_id: &str) -> Option<ResourceRecord> {
        self.resources.get(resource_id).cloned()
    }

    /// 获取所有资源
    pub fn get_all_resources(&self) -> Vec<ResourceRecord> {
        self.resources.values().cloned().collect()
    }

    // ========================================================================
    // 任务-场景关联操作
    // ========================================================================

    /// 创建任务-场景关联
    pub fn create_task_scene_relation(
        &mut self,
        task_id: String,
        scene_id: String,
        relation_type: TaskSceneRelationType,
    ) -> Result<String> {
        if !self.scenes.contains_key(&scene_id) {
            return Err(BridgeError::Other(format!("Scene not found: {}", scene_id)));
        }

        let relation_id = format!("rel_{}", uuid::Uuid::new_v4().as_simple());
        let now = chrono::Utc::now().to_rfc3339();

        let relation = TaskSceneRelation {
            relation_id: relation_id.clone(),
            task_id: task_id.clone(),
            scene_id: scene_id.clone(),
            relation_type,
            created_at: now,
        };

        self.task_scene_relations
            .insert(relation_id.clone(), relation);
        self.task_scene_index
            .entry(task_id)
            .or_default()
            .insert(scene_id);

        Ok(relation_id)
    }

    /// 获取任务关联的场景
    pub fn get_task_scenes(&self, task_id: &str) -> Vec<SceneRecord> {
        self.task_scene_index
            .get(task_id)
            .map(|scene_ids| {
                scene_ids
                    .iter()
                    .filter_map(|id| self.scenes.get(id))
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }

    /// 获取场景关联的任务
    pub fn get_scene_tasks(&self, scene_id: &str) -> Vec<TaskRecord> {
        self.task_scene_relations
            .values()
            .filter(|r| r.scene_id == scene_id)
            .map(|r| TaskRecord {
                task_id: r.task_id.clone(),
                title: format!("Task for {}", scene_id),
                description: None,
                status: "pending".to_string(),
                created_at: r.created_at.clone(),
            })
            .collect()
    }

    // ========================================================================
    // 组合查询操作
    // ========================================================================

    /// 完整查询：任务+场景+实体+资源
    pub fn query_task_scene_entities(&self, task_id: &str) -> Vec<TaskSceneEntityQueryResult> {
        let scenes = self.get_task_scenes(task_id);

        scenes
            .into_iter()
            .map(|scene| {
                let entities = self.get_scene_entities(&scene.scene_id);
                let resources = self.get_all_resources();

                let task_record = TaskRecord {
                    task_id: task_id.to_string(),
                    title: format!("Task related to {}", scene.name),
                    description: scene.description.clone(),
                    status: "pending".to_string(),
                    created_at: scene.created_at.clone(),
                };

                TaskSceneEntityQueryResult {
                    task: task_record,
                    scene,
                    entities,
                    resources,
                }
            })
            .collect()
    }

    // ========================================================================
    // 统计信息
    // ========================================================================

    /// 获取数据库统计信息
    pub fn get_statistics(&self) -> DbStatistics {
        DbStatistics {
            scene_count: self.scenes.len(),
            entity_count: self.entities.len(),
            resource_count: self.resources.len(),
            relation_count: self.task_scene_relations.len(),
        }
    }

    // ========================================================================
    // 数据库层增强方法
    // ========================================================================

    /// 获取所有实体（不分场景）
    pub fn all_entities(&self) -> Vec<EntityRecord> {
        self.entities.values().cloned().collect()
    }

    /// 获取所有任务-场景关联
    pub fn all_relations(&self) -> Vec<TaskSceneRelation> {
        self.task_scene_relations.values().cloned().collect()
    }

    /// 清空所有数据
    pub fn clear_all(&mut self) {
        self.scenes.clear();
        self.entities.clear();
        self.scene_entities.clear();
        self.resources.clear();
        self.task_scene_relations.clear();
        self.task_scene_index.clear();
        self.next_entity_id = 1;
    }
}

impl Default for MulticaDb {
    fn default() -> Self {
        Self::new()
    }
}

/// 数据库统计信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbStatistics {
    pub scene_count: usize,
    pub entity_count: usize,
    pub resource_count: usize,
    pub relation_count: usize,
}

// ============================================================================
// 线程安全包装
// ============================================================================

/// 线程安全的 Multica DB
pub type SharedMulticaDb = Arc<Mutex<MulticaDb>>;

/// 创建线程安全的 Multica DB
pub fn create_shared_multica_db() -> SharedMulticaDb {
    Arc::new(Mutex::new(MulticaDb::new()))
}

// ============================================================================
// 测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_and_get_scene() {
        let mut db = MulticaDb::new();

        let scene_id = db
            .create_scene("TestScene".to_string(), Some("Description".to_string()))
            .unwrap();

        let scene = db.get_scene(&scene_id).unwrap();
        assert_eq!(scene.name, "TestScene");
        assert_eq!(scene.description, Some("Description".to_string()));
    }

    #[test]
    fn test_create_and_get_entity() {
        let mut db = MulticaDb::new();

        let scene_id = db.create_scene("TestScene".to_string(), None).unwrap();
        let entity_id = db
            .create_entity(
                scene_id.clone(),
                "Player".to_string(),
                "GameObject".to_string(),
                vec![],
                Some([0.0, 0.0, 0.0]),
            )
            .unwrap();

        let entity = db.get_entity(entity_id).unwrap();
        assert_eq!(entity.name, "Player");
        assert_eq!(entity.scene_id, scene_id);
    }

    #[test]
    fn test_task_scene_relation() {
        let mut db = MulticaDb::new();

        let scene_id = db.create_scene("MainScene".to_string(), None).unwrap();

        db.create_task_scene_relation(
            "task_123".to_string(),
            scene_id.clone(),
            TaskSceneRelationType::ExecutesIn,
        )
        .unwrap();

        let scenes = db.get_task_scenes("task_123");
        assert_eq!(scenes.len(), 1);
        assert_eq!(scenes[0].scene_id, scene_id);
    }

    #[test]
    fn test_query_task_scene_entities() {
        let mut db = MulticaDb::new();

        let scene_id = db.create_scene("GameScene".to_string(), None).unwrap();
        db.create_entity(
            scene_id.clone(),
            "Enemy".to_string(),
            "GameObject".to_string(),
            vec![],
            None,
        )
        .unwrap();

        db.create_task_scene_relation(
            "task_456".to_string(),
            scene_id.clone(),
            TaskSceneRelationType::Modifies,
        )
        .unwrap();

        let results = db.query_task_scene_entities("task_456");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].entities.len(), 1);
    }

    #[test]
    fn test_statistics() {
        let mut db = MulticaDb::new();

        db.create_scene("Scene1".to_string(), None).unwrap();
        let scene_id = db.create_scene("Scene2".to_string(), None).unwrap();
        db.create_entity(
            scene_id,
            "Entity1".to_string(),
            "Type".to_string(),
            vec![],
            None,
        )
        .unwrap();

        let stats = db.get_statistics();
        assert_eq!(stats.scene_count, 2);
        assert_eq!(stats.entity_count, 1);
    }
}
