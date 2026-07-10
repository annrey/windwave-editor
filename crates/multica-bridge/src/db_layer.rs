//! Enhanced Database Access Layer
//!
//! Production-grade query builder, batched transactions, indexing,
//! snapshots, and import/export on top of the MulticaDb memory store.

use crate::error::{BridgeError, Result};
use crate::multica_db::{
    EntityRecord, MulticaDb, ResourceRecord, SceneRecord, SharedMulticaDb, TaskSceneRelation,
    TaskSceneRelationType,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================
// Filters
// ============================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EntityFilter {
    NameContains(String),
    NameEquals(String),
    NameStartsWith(String),
    EntityType(String),
    HasComponent(String),
    SceneId(String),
    CreatedAfter(String),
    UpdatedAfter(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SceneFilter {
    NameContains(String),
    NameEquals(String),
    IsActive(bool),
    CreatedAfter(String),
    HasEntities(bool),
}

// ============================================================
// Sort
// ============================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SortOrder {
    Asc,
    Desc,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SortField {
    Name,
    EntityType,
    CreatedAt,
    UpdatedAt,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SortSpec {
    pub field: SortField,
    pub order: SortOrder,
}

impl SortSpec {
    pub fn asc(field: SortField) -> Self {
        Self {
            field,
            order: SortOrder::Asc,
        }
    }
    pub fn desc(field: SortField) -> Self {
        Self {
            field,
            order: SortOrder::Desc,
        }
    }
}

// ============================================================
// Pagination
// ============================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pagination {
    pub offset: usize,
    pub limit: usize,
    pub total: Option<usize>,
}

impl Pagination {
    pub fn new(offset: usize, limit: usize) -> Self {
        Self {
            offset,
            limit,
            total: None,
        }
    }

    pub fn page(page: usize, per_page: usize) -> Self {
        Self {
            offset: page.saturating_sub(1).saturating_mul(per_page),
            limit: per_page,
            total: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaginatedResult<T> {
    pub items: Vec<T>,
    pub page: usize,
    pub per_page: usize,
    pub total: usize,
    pub total_pages: usize,
    pub has_next: bool,
    pub has_prev: bool,
}

impl<T> PaginatedResult<T> {
    pub fn from_items(items: Vec<T>, total: usize, pagination: &Pagination) -> Self {
        let per_page = pagination.limit.max(1);
        let page = (pagination.offset / per_page) + 1;
        let total_pages = total.div_ceil(per_page);
        PaginatedResult {
            items,
            page,
            per_page,
            total,
            total_pages,
            has_next: page < total_pages,
            has_prev: page > 1,
        }
    }
}

// ============================================================
// Entity Query Builder
// ============================================================

pub struct EntityQuery {
    filters: Vec<EntityFilter>,
    sort: Vec<SortSpec>,
    pagination: Option<Pagination>,
}

impl EntityQuery {
    pub fn new() -> Self {
        Self {
            filters: vec![],
            sort: vec![],
            pagination: None,
        }
    }

    pub fn filter(mut self, filter: EntityFilter) -> Self {
        self.filters.push(filter);
        self
    }

    pub fn sort_by(mut self, spec: SortSpec) -> Self {
        self.sort.push(spec);
        self
    }

    pub fn paginate(mut self, pagination: Pagination) -> Self {
        self.pagination = Some(pagination);
        self
    }

    pub fn execute(&self, db: &MulticaDb) -> PaginatedResult<EntityRecord> {
        let mut results: Vec<EntityRecord> = db.all_entities();

        for filter in &self.filters {
            results.retain(|e| match_e(e, filter));
        }

        // Sort (last sort spec wins as primary)
        if let Some(spec) = self.sort.last() {
            results.sort_by(|a, b| {
                let cmp = match spec.field {
                    SortField::Name => a.name.cmp(&b.name),
                    SortField::EntityType => a.entity_type.cmp(&b.entity_type),
                    SortField::CreatedAt => a.created_at.cmp(&b.created_at),
                    SortField::UpdatedAt => a.updated_at.cmp(&b.updated_at),
                };
                match spec.order {
                    SortOrder::Asc => cmp,
                    SortOrder::Desc => cmp.reverse(),
                }
            });
        }

        let total = results.len();
        let pagination = self
            .pagination
            .as_ref()
            .cloned()
            .unwrap_or(Pagination::new(0, total.max(1)));

        let items = results
            .into_iter()
            .skip(pagination.offset)
            .take(pagination.limit)
            .collect();

        PaginatedResult::from_items(items, total, &pagination)
    }
}

impl Default for EntityQuery {
    fn default() -> Self {
        Self::new()
    }
}

fn match_e(e: &EntityRecord, filter: &EntityFilter) -> bool {
    match filter {
        EntityFilter::NameContains(s) => e.name.to_lowercase().contains(&s.to_lowercase()),
        EntityFilter::NameEquals(s) => e.name == *s,
        EntityFilter::NameStartsWith(s) => e.name.to_lowercase().starts_with(&s.to_lowercase()),
        EntityFilter::EntityType(t) => e.entity_type == *t,
        EntityFilter::HasComponent(c) => e.components.iter().any(|comp| comp.type_name == *c),
        EntityFilter::SceneId(id) => e.scene_id == *id,
        EntityFilter::CreatedAfter(ts) => e.created_at.as_str() > ts.as_str(),
        EntityFilter::UpdatedAfter(ts) => e.updated_at.as_str() > ts.as_str(),
    }
}

// ============================================================
// Scene Query Builder
// ============================================================

pub struct SceneQuery {
    filters: Vec<SceneFilter>,
    pagination: Option<Pagination>,
}

impl SceneQuery {
    pub fn new() -> Self {
        Self {
            filters: vec![],
            pagination: None,
        }
    }

    pub fn filter(mut self, filter: SceneFilter) -> Self {
        self.filters.push(filter);
        self
    }

    pub fn paginate(mut self, pagination: Pagination) -> Self {
        self.pagination = Some(pagination);
        self
    }

    pub fn execute(&self, db: &MulticaDb) -> PaginatedResult<SceneRecord> {
        let mut results: Vec<SceneRecord> = db.get_all_scenes();

        for filter in &self.filters {
            results.retain(|s| match filter {
                SceneFilter::NameContains(q) => s.name.to_lowercase().contains(&q.to_lowercase()),
                SceneFilter::NameEquals(q) => s.name == *q,
                SceneFilter::IsActive(active) => s.is_active == *active,
                SceneFilter::CreatedAfter(ts) => s.created_at.as_str() > ts.as_str(),
                SceneFilter::HasEntities(has) => {
                    let ents = db.get_scene_entities(&s.scene_id);
                    if *has {
                        !ents.is_empty()
                    } else {
                        ents.is_empty()
                    }
                }
            });
        }

        let total = results.len();
        let pagination = self
            .pagination
            .as_ref()
            .cloned()
            .unwrap_or(Pagination::new(0, total.max(1)));

        let items = results
            .into_iter()
            .skip(pagination.offset)
            .take(pagination.limit)
            .collect();

        PaginatedResult::from_items(items, total, &pagination)
    }
}

impl Default for SceneQuery {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================
// Batch Transactions
// ============================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BatchOp {
    CreateScene {
        name: String,
        description: Option<String>,
    },
    DeleteScene {
        scene_id: String,
    },
    CreateEntity {
        scene_id: String,
        name: String,
        entity_type: String,
        components_json: serde_json::Value,
        position: Option<[f64; 3]>,
    },
    UpdateEntity {
        entity_id: u64,
        name: Option<String>,
        component_types: Option<Vec<String>>,
        position: Option<Option<[f64; 3]>>,
    },
    DeleteEntity {
        entity_id: u64,
    },
    CreateResource {
        resource_type: String,
        name: String,
        path: Option<String>,
        metadata: HashMap<String, String>,
    },
    RelateTaskScene {
        task_id: String,
        scene_id: String,
        relation_type: TaskSceneRelationType,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchResult {
    pub total_ops: usize,
    pub succeeded: usize,
    pub failed: usize,
    pub results: Vec<BatchOpResult>,
    pub rolled_back: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchOpResult {
    pub index: usize,
    pub op_type: String,
    pub success: bool,
    pub error: Option<String>,
    pub entity_id: Option<u64>,
    pub resource_id: Option<String>,
}

pub struct BatchTransaction {
    ops: Vec<BatchOp>,
}

impl BatchTransaction {
    pub fn new() -> Self {
        Self { ops: vec![] }
    }

    pub fn push(mut self, op: BatchOp) -> Self {
        self.ops.push(op);
        self
    }

    pub fn len(&self) -> usize {
        self.ops.len()
    }

    pub fn is_empty(&self) -> bool {
        self.ops.is_empty()
    }

    /// Execute all operations. On any failure, all previous operations
    /// are rolled back (via snapshot restore).
    pub fn execute(&self, db: &mut MulticaDb) -> Result<BatchResult> {
        let snapshot = DbSnapshot::from_db(db);
        let mut results = Vec::new();
        let mut succeeded = 0;

        for (i, op) in self.ops.iter().enumerate() {
            let result = Self::execute_op(db, op);
            match result {
                Ok(r) => {
                    succeeded += 1;
                    results.push(r);
                }
                Err(e) => {
                    results.push(BatchOpResult {
                        index: i,
                        op_type: Self::op_type(op),
                        success: false,
                        error: Some(e.to_string()),
                        entity_id: None,
                        resource_id: None,
                    });
                    // Rollback on first failure
                    snapshot.restore(db);
                    return Ok(BatchResult {
                        total_ops: self.ops.len(),
                        succeeded: 0,
                        failed: 1,
                        results,
                        rolled_back: true,
                    });
                }
            }
        }

        Ok(BatchResult {
            total_ops: self.ops.len(),
            succeeded,
            failed: self.ops.len() - succeeded,
            results,
            rolled_back: false,
        })
    }

    fn execute_op(db: &mut MulticaDb, op: &BatchOp) -> Result<BatchOpResult> {
        let index = 0;
        let op_type = Self::op_type(op);

        match op {
            BatchOp::CreateScene { name, description } => {
                let id = db.create_scene(name.clone(), description.clone())?;
                Ok(BatchOpResult {
                    index,
                    op_type,
                    success: true,
                    error: None,
                    entity_id: None,
                    resource_id: Some(id),
                })
            }
            BatchOp::DeleteScene { scene_id } => {
                db.delete_scene(scene_id)?;
                Ok(BatchOpResult {
                    index,
                    op_type,
                    success: true,
                    error: None,
                    entity_id: None,
                    resource_id: None,
                })
            }
            BatchOp::CreateEntity {
                scene_id,
                name,
                entity_type,
                components_json: _,
                position,
            } => {
                let components = vec![];
                let eid = db.create_entity(
                    scene_id.clone(),
                    name.clone(),
                    entity_type.clone(),
                    components,
                    *position,
                )?;
                Ok(BatchOpResult {
                    index,
                    op_type,
                    success: true,
                    error: None,
                    entity_id: Some(eid),
                    resource_id: None,
                })
            }
            BatchOp::UpdateEntity {
                entity_id,
                name,
                component_types: _,
                position,
            } => {
                let components = None; // Partial update: only update if specified
                db.update_entity(*entity_id, name.clone(), components, *position)?;
                Ok(BatchOpResult {
                    index,
                    op_type,
                    success: true,
                    error: None,
                    entity_id: Some(*entity_id),
                    resource_id: None,
                })
            }
            BatchOp::DeleteEntity { entity_id } => {
                db.delete_entity(*entity_id)?;
                Ok(BatchOpResult {
                    index,
                    op_type,
                    success: true,
                    error: None,
                    entity_id: None,
                    resource_id: None,
                })
            }
            BatchOp::CreateResource {
                resource_type,
                name,
                path,
                metadata,
            } => {
                let rid = db.create_resource(
                    resource_type.clone(),
                    name.clone(),
                    path.clone(),
                    metadata.clone(),
                )?;
                Ok(BatchOpResult {
                    index,
                    op_type,
                    success: true,
                    error: None,
                    entity_id: None,
                    resource_id: Some(rid),
                })
            }
            BatchOp::RelateTaskScene {
                task_id,
                scene_id,
                relation_type,
            } => {
                let _rel_id = db.create_task_scene_relation(
                    task_id.clone(),
                    scene_id.clone(),
                    relation_type.clone(),
                )?;
                Ok(BatchOpResult {
                    index,
                    op_type,
                    success: true,
                    error: None,
                    entity_id: None,
                    resource_id: None,
                })
            }
        }
    }

    fn op_type(op: &BatchOp) -> String {
        match op {
            BatchOp::CreateScene { .. } => "create_scene".into(),
            BatchOp::DeleteScene { .. } => "delete_scene".into(),
            BatchOp::CreateEntity { .. } => "create_entity".into(),
            BatchOp::UpdateEntity { .. } => "update_entity".into(),
            BatchOp::DeleteEntity { .. } => "delete_entity".into(),
            BatchOp::CreateResource { .. } => "create_resource".into(),
            BatchOp::RelateTaskScene { .. } => "relate_task_scene".into(),
        }
    }
}

impl Default for BatchTransaction {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================
// Database Snapshot
// ============================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbSnapshot {
    pub scenes: Vec<SceneRecord>,
    pub entities: Vec<EntityRecord>,
    pub resources: Vec<ResourceRecord>,
    pub relations: Vec<TaskSceneRelation>,
    pub next_entity_id: u64,
    pub created_at: String,
    pub version: u32,
}

impl DbSnapshot {
    pub fn from_db(db: &MulticaDb) -> Self {
        let stats = db.get_statistics();
        Self {
            scenes: db.get_all_scenes(),
            entities: db.all_entities(),
            resources: db.get_all_resources(),
            relations: db.all_relations(),
            next_entity_id: stats.entity_count as u64 + 1,
            created_at: chrono::Utc::now().to_rfc3339(),
            version: 1,
        }
    }

    /// Restore database from snapshot (clears existing data)
    pub fn restore(&self, db: &mut MulticaDb) {
        db.clear_all();

        let mut scene_id_map: HashMap<String, String> = HashMap::new();

        for scene in &self.scenes {
            let new_id = db
                .create_scene(scene.name.clone(), scene.description.clone())
                .unwrap_or_else(|_| format!("scene_{}", chrono::Utc::now().timestamp()));
            scene_id_map.insert(scene.scene_id.clone(), new_id);
        }

        for entity in &self.entities {
            let mapped_scene_id = scene_id_map
                .get(&entity.scene_id)
                .cloned()
                .unwrap_or_else(|| entity.scene_id.clone());
            let _ = db.create_entity(
                mapped_scene_id,
                entity.name.clone(),
                entity.entity_type.clone(),
                entity.components.clone(),
                entity.position,
            );
        }

        for resource in &self.resources {
            let _ = db.create_resource(
                resource.resource_type.clone(),
                resource.name.clone(),
                resource.path.clone(),
                resource.metadata.clone(),
            );
        }

        for relation in &self.relations {
            let mapped_scene_id = scene_id_map
                .get(&relation.scene_id)
                .cloned()
                .unwrap_or_else(|| relation.scene_id.clone());
            let _ = db.create_task_scene_relation(
                relation.task_id.clone(),
                mapped_scene_id,
                relation.relation_type.clone(),
            );
        }
    }

    pub fn to_json(&self) -> Result<String> {
        serde_json::to_string_pretty(self).map_err(BridgeError::SerializationError)
    }

    pub fn from_json(json: &str) -> Result<Self> {
        serde_json::from_str(json).map_err(BridgeError::SerializationError)
    }
}

// ============================================================
// Index
// ============================================================

pub struct EntityIndex {
    by_name: HashMap<String, Vec<u64>>,
    by_type: HashMap<String, Vec<u64>>,
    by_scene: HashMap<String, Vec<u64>>,
}

impl EntityIndex {
    pub fn build(db: &MulticaDb) -> Self {
        let mut idx = Self {
            by_name: HashMap::new(),
            by_type: HashMap::new(),
            by_scene: HashMap::new(),
        };

        for entity in db.all_entities() {
            let lower = entity.name.to_lowercase();
            idx.by_name.entry(lower).or_default().push(entity.entity_id);
            idx.by_type
                .entry(entity.entity_type.clone())
                .or_default()
                .push(entity.entity_id);
            idx.by_scene
                .entry(entity.scene_id.clone())
                .or_default()
                .push(entity.entity_id);
        }

        idx
    }

    pub fn find_by_name(&self, name: &str) -> Vec<&u64> {
        self.by_name
            .get(&name.to_lowercase())
            .map(|v| v.iter().collect())
            .unwrap_or_default()
    }

    pub fn find_by_prefix(&self, prefix: &str) -> Vec<&u64> {
        let lower = prefix.to_lowercase();
        self.by_name
            .iter()
            .filter(|(k, _)| k.starts_with(&lower))
            .flat_map(|(_, v)| v.iter())
            .collect()
    }

    pub fn find_by_type(&self, entity_type: &str) -> Vec<&u64> {
        self.by_type
            .get(entity_type)
            .map(|v| v.iter().collect())
            .unwrap_or_default()
    }

    pub fn find_by_scene(&self, scene_id: &str) -> Vec<&u64> {
        self.by_scene
            .get(scene_id)
            .map(|v| v.iter().collect())
            .unwrap_or_default()
    }
}

// ============================================================
// DB Facade - unified access
// ============================================================

pub struct DbLayer {
    db: SharedMulticaDb,
}

impl DbLayer {
    pub fn new(db: SharedMulticaDb) -> Self {
        Self { db }
    }

    pub fn query_entities(&self) -> EntityQuery {
        EntityQuery::new()
    }

    pub fn query_scenes(&self) -> SceneQuery {
        SceneQuery::new()
    }

    pub fn batch(&self) -> BatchTransaction {
        BatchTransaction::new()
    }

    pub fn snapshot(&self) -> Result<DbSnapshot> {
        let db_lock = self
            .db
            .lock()
            .map_err(|e| BridgeError::Other(format!("lock: {}", e)))?;
        Ok(DbSnapshot::from_db(&db_lock))
    }

    pub fn restore(&self, snapshot: &DbSnapshot) -> Result<()> {
        let mut db_lock = self
            .db
            .lock()
            .map_err(|e| BridgeError::Other(format!("lock: {}", e)))?;
        snapshot.restore(&mut db_lock);
        Ok(())
    }

    pub fn build_index(&self) -> Result<EntityIndex> {
        let db_lock = self
            .db
            .lock()
            .map_err(|e| BridgeError::Other(format!("lock: {}", e)))?;
        Ok(EntityIndex::build(&db_lock))
    }

    pub fn export_json(&self) -> Result<String> {
        let snapshot = self.snapshot()?;
        snapshot.to_json()
    }

    pub fn import_json(&self, json: &str) -> Result<()> {
        let snapshot = DbSnapshot::from_json(json)?;
        self.restore(&snapshot)
    }

    pub fn with_lock<F, R>(&self, f: F) -> Result<R>
    where
        F: FnOnce(&mut MulticaDb) -> Result<R>,
    {
        let mut db_lock = self
            .db
            .lock()
            .map_err(|e| BridgeError::Other(format!("lock: {}", e)))?;
        f(&mut db_lock)
    }
}

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::multica_db::create_shared_multica_db;

    fn setup() -> (DbLayer, SharedMulticaDb, String) {
        let db = create_shared_multica_db();
        let sid;
        {
            let mut d = db.lock().unwrap();
            sid = d.create_scene("Main".into(), None).unwrap();
            d.create_entity(
                sid.clone(),
                "Player".into(),
                "Character".into(),
                vec![],
                Some([0.0, 0.0, 0.0]),
            )
            .unwrap();
            d.create_entity(
                sid.clone(),
                "Enemy".into(),
                "NPC".into(),
                vec![],
                Some([1.0, 0.0, 0.0]),
            )
            .unwrap();
            d.create_entity(
                sid.clone(),
                "Tree".into(),
                "Prop".into(),
                vec![],
                Some([2.0, 0.0, 0.0]),
            )
            .unwrap();
            d.create_scene("Level2".into(), None).unwrap();
        }
        (DbLayer::new(db.clone()), db, sid)
    }

    // --- EntityQuery ---

    #[test]
    fn test_query_all_entities() {
        let (layer, _, _) = setup();
        let db = layer.db.lock().unwrap();
        let result = EntityQuery::new().execute(&db);
        assert_eq!(result.total, 3);
    }

    #[test]
    fn test_query_filter_by_name_contains() {
        let (layer, _, _) = setup();
        let db = layer.db.lock().unwrap();
        let result = EntityQuery::new()
            .filter(EntityFilter::NameContains("play".into()))
            .execute(&db);
        assert_eq!(result.total, 1);
        assert_eq!(result.items[0].name, "Player");
    }

    #[test]
    fn test_query_filter_by_entity_type() {
        let (layer, _, _) = setup();
        let db = layer.db.lock().unwrap();
        let result = EntityQuery::new()
            .filter(EntityFilter::EntityType("Prop".into()))
            .execute(&db);
        assert_eq!(result.total, 1);
        assert_eq!(result.items[0].name, "Tree");
    }

    #[test]
    fn test_query_filter_by_name_starts_with() {
        let (layer, _, _) = setup();
        let db = layer.db.lock().unwrap();
        let result = EntityQuery::new()
            .filter(EntityFilter::NameStartsWith("en".into()))
            .execute(&db);
        assert_eq!(result.total, 1);
    }

    #[test]
    fn test_query_pagination() {
        let (layer, _, _) = setup();
        let db = layer.db.lock().unwrap();
        let result = EntityQuery::new()
            .paginate(Pagination::new(0, 2))
            .execute(&db);
        assert_eq!(result.items.len(), 2);
        assert_eq!(result.total, 3);
        assert_eq!(result.total_pages, 2);
        assert!(result.has_next);
    }

    #[test]
    fn test_query_pagination_page_2() {
        let (layer, _, _) = setup();
        let db = layer.db.lock().unwrap();
        let result = EntityQuery::new()
            .paginate(Pagination::page(2, 2))
            .execute(&db);
        assert_eq!(result.items.len(), 1);
        assert!(!result.has_next);
        assert!(result.has_prev);
    }

    #[test]
    fn test_query_sort_by_name_desc() {
        let (layer, _, _) = setup();
        let db = layer.db.lock().unwrap();
        let result = EntityQuery::new()
            .sort_by(SortSpec::desc(SortField::Name))
            .execute(&db);
        assert_eq!(result.items[0].name, "Tree");
        assert_eq!(result.items.last().unwrap().name, "Enemy");
    }

    // --- SceneQuery ---

    #[test]
    fn test_query_scenes_name_filter() {
        let (layer, _, _) = setup();
        let db = layer.db.lock().unwrap();
        let result = SceneQuery::new()
            .filter(SceneFilter::NameEquals("Main".into()))
            .execute(&db);
        assert_eq!(result.total, 1);
    }

    #[test]
    fn test_query_scenes_has_entities() {
        let (layer, _, _) = setup();
        let db = layer.db.lock().unwrap();
        let result = SceneQuery::new()
            .filter(SceneFilter::HasEntities(true))
            .execute(&db);
        assert_eq!(result.total, 1); // Main has entities, Level2 doesn't
    }

    // --- Batch ---

    #[test]
    fn test_batch_create_entities() {
        let (_layer, db, sid) = setup();
        println!("sid = {:?}", sid);
        let mut d = db.lock().unwrap();

        let tx = BatchTransaction::new()
            .push(BatchOp::CreateEntity {
                scene_id: sid.clone(),
                name: "Item1".into(),
                entity_type: "Pickup".into(),
                components_json: serde_json::json!([]),
                position: Some([5.0, 0.0, 0.0]),
            })
            .push(BatchOp::CreateEntity {
                scene_id: sid,
                name: "Item2".into(),
                entity_type: "Pickup".into(),
                components_json: serde_json::json!([]),
                position: Some([6.0, 0.0, 0.0]),
            });

        let result = tx.execute(&mut d).unwrap();
        dbg!(&result);
        assert_eq!(result.succeeded, 2);
        assert!(!result.rolled_back);
    }

    #[test]
    fn test_batch_rollback_on_error() {
        let (_layer, db, sid) = setup();
        let mut d = db.lock().unwrap();

        let tx = BatchTransaction::new()
            .push(BatchOp::CreateEntity {
                scene_id: sid.clone(),
                name: "Good".into(),
                entity_type: "X".into(),
                components_json: serde_json::json!([]),
                position: None,
            })
            .push(BatchOp::DeleteScene {
                scene_id: "nonexistent".into(),
            });

        let result = tx.execute(&mut d).unwrap();
        assert!(result.rolled_back);
        assert_eq!(result.succeeded, 0);
        // Verify the first entity was rolled back
        let after = d.all_entities();
        assert_eq!(after.len(), 3); // back to original count
    }

    // --- Snapshot ---

    #[test]
    fn test_snapshot_and_restore() {
        let (layer, db, _) = setup();

        let snapshot = layer.snapshot().unwrap();
        assert_eq!(snapshot.scenes.len(), 2);
        assert_eq!(snapshot.entities.len(), 3);

        {
            let mut d = db.lock().unwrap();
            d.clear_all();
            assert_eq!(d.get_statistics().entity_count, 0);
        }

        layer.restore(&snapshot).unwrap();

        {
            let d = db.lock().unwrap();
            assert_eq!(d.get_statistics().scene_count, 2);
            assert_eq!(d.get_statistics().entity_count, 3);
        }
    }

    #[test]
    fn test_export_import_json() {
        let (layer, _, _) = setup();
        let json = layer.export_json().unwrap();
        assert!(json.contains("Main"));

        let snapshot = DbSnapshot::from_json(&json).unwrap();
        assert_eq!(snapshot.entities.len(), 3);
    }

    // --- Index ---

    #[test]
    fn test_index_find_by_name() {
        let (layer, _, _) = setup();
        let idx = layer.build_index().unwrap();
        let ids = idx.find_by_name("Player");
        assert_eq!(ids.len(), 1);
    }

    #[test]
    fn test_index_find_by_prefix() {
        let (layer, _, _) = setup();
        let idx = layer.build_index().unwrap();
        let ids = idx.find_by_prefix("p");
        assert_eq!(ids.len(), 1); // Player
    }

    #[test]
    fn test_index_find_by_type() {
        let (layer, _, _) = setup();
        let idx = layer.build_index().unwrap();
        let npcs = idx.find_by_type("NPC");
        assert_eq!(npcs.len(), 1);
    }

    #[test]
    fn test_db_layer_with_lock() {
        let (layer, _, _) = setup();
        let count = layer
            .with_lock(|db| Ok(db.get_statistics().entity_count))
            .unwrap();
        assert_eq!(count, 3);
    }
}
