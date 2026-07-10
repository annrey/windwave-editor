//! Multica Compatibility Layer
//!
//! Optional bridge between WindWave's internal task/game systems and the
//! external Multica server protocol. Supports:
//! - Task import/export between WindWave and Multica formats
//! - Protocol conversion for WebSocket messages
//! - Compatibility mode configuration (Local / Remote / Hybrid)
//! - Scene-aware task format translation

use crate::error::{BridgeError, Result};
use crate::multica_db::SharedMulticaDb;
use crate::task_bridge::{TaskBridge, UnifiedTask, UnifiedTaskStatus};
use crate::task_sync::{LocalTask, TaskId, TaskStatus};
use crate::types::*;
use log::{info, warn};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ========================================
// Compat Mode
// ========================================

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum CompatMode {
    /// Only use WindWave's internal task system
    #[default]
    LocalOnly,
    /// Connect to external Multica server for all task management
    RemoteOnly,
    /// Keep tasks locally but optionally sync to remote
    Hybrid {
        /// Sync local tasks to remote on creation
        push_to_remote: bool,
        /// Pull remote tasks into local system
        pull_from_remote: bool,
    },
}

impl CompatMode {
    pub fn is_local_enabled(&self) -> bool {
        matches!(self, Self::LocalOnly | Self::Hybrid { .. })
    }

    pub fn is_remote_enabled(&self) -> bool {
        matches!(self, Self::RemoteOnly | Self::Hybrid { .. })
    }
}

// ========================================
// Task Export / Import
// ========================================

/// Exported task in Multica-compatible format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportedTask {
    pub multica_task: MulticaTask,
    /// Original WindWave-specific fields preserved as metadata
    pub windwave_metadata: WindWaveTaskMetadata,
}

/// WindWave-specific metadata attached to exported tasks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindWaveTaskMetadata {
    pub scene_id: Option<String>,
    pub entity_ids: Vec<String>,
    pub resource_ids: Vec<String>,
    pub scene_snapshot: Option<serde_json::Value>,
}

/// Result of an import operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportResult {
    pub tasks_imported: usize,
    pub tasks_skipped: usize,
    pub tasks_failed: usize,
    pub errors: Vec<String>,
    pub imported_ids: Vec<String>,
}

impl ImportResult {
    pub fn new() -> Self {
        Self {
            tasks_imported: 0,
            tasks_skipped: 0,
            tasks_failed: 0,
            errors: Vec::new(),
            imported_ids: Vec::new(),
        }
    }
}

impl Default for ImportResult {
    fn default() -> Self {
        Self::new()
    }
}

/// Export a local task to Multica format
pub fn export_local_task(task: &LocalTask) -> ExportedTask {
    let multica_status = match task.status {
        TaskStatus::Draft => TaskLifecycleStatus::Created,
        TaskStatus::Planning => TaskLifecycleStatus::Created,
        TaskStatus::InProgress => TaskLifecycleStatus::Executing,
        TaskStatus::Done => TaskLifecycleStatus::Completed,
        TaskStatus::Failed => TaskLifecycleStatus::Failed,
        TaskStatus::Cancelled => TaskLifecycleStatus::Cancelled,
    };

    ExportedTask {
        multica_task: MulticaTask {
            task_id: task.id.to_string(),
            issue_id: None,
            title: task.title.clone(),
            description: Some(task.description.clone()),
            status: multica_status,
            agent_type: None,
            model: None,
            output: None,
            error: None,
            duration_ms: None,
            created_at: chrono::Utc::now().to_rfc3339(),
            updated_at: chrono::Utc::now().to_rfc3339(),
        },
        windwave_metadata: WindWaveTaskMetadata {
            scene_id: task.scene_id.clone(),
            entity_ids: task.entity_ids.clone(),
            resource_ids: task.resource_ids.clone(),
            scene_snapshot: task.scene_snapshot.clone(),
        },
    }
}

/// Export a unified task to Multica format
pub fn export_unified_task(task: &UnifiedTask) -> ExportedTask {
    let multica_status = match task.status {
        UnifiedTaskStatus::Draft => TaskLifecycleStatus::Created,
        UnifiedTaskStatus::Planning => TaskLifecycleStatus::Created,
        UnifiedTaskStatus::Queued => TaskLifecycleStatus::Claimed,
        UnifiedTaskStatus::Dispatched => TaskLifecycleStatus::Executing,
        UnifiedTaskStatus::Running => TaskLifecycleStatus::Executing,
        UnifiedTaskStatus::WaitingForUser => TaskLifecycleStatus::Executing,
        UnifiedTaskStatus::Blocked => TaskLifecycleStatus::Executing,
        UnifiedTaskStatus::Done => TaskLifecycleStatus::Completed,
        UnifiedTaskStatus::Failed => TaskLifecycleStatus::Failed,
        UnifiedTaskStatus::Cancelled => TaskLifecycleStatus::Cancelled,
    };

    ExportedTask {
        multica_task: MulticaTask {
            task_id: task.id.bridge_id.to_string(),
            issue_id: task.id.multica_id.map(|id| id.to_string()),
            title: task.title.clone(),
            description: Some(task.description.clone()),
            status: multica_status,
            agent_type: None,
            model: None,
            output: None,
            error: None,
            duration_ms: None,
            created_at: task.created_at.clone(),
            updated_at: task.updated_at.clone(),
        },
        windwave_metadata: WindWaveTaskMetadata {
            scene_id: task.scene_id.clone(),
            entity_ids: task.entity_ids.clone(),
            resource_ids: task.resource_ids.clone(),
            scene_snapshot: task.scene_snapshot.clone(),
        },
    }
}

/// Import a Multica task into a local task
pub fn import_to_local_task(exported: &ExportedTask, local_id: u64) -> LocalTask {
    let status = match exported.multica_task.status {
        TaskLifecycleStatus::Created => TaskStatus::Planning,
        TaskLifecycleStatus::Claimed => TaskStatus::InProgress,
        TaskLifecycleStatus::Executing => TaskStatus::InProgress,
        TaskLifecycleStatus::Completed => TaskStatus::Done,
        TaskLifecycleStatus::Failed => TaskStatus::Failed,
        TaskLifecycleStatus::Cancelled => TaskStatus::Cancelled,
    };

    LocalTask {
        id: TaskId(local_id),
        title: exported.multica_task.title.clone(),
        description: exported
            .multica_task
            .description
            .clone()
            .unwrap_or_default(),
        status,
        scene_id: exported.windwave_metadata.scene_id.clone(),
        entity_ids: exported.windwave_metadata.entity_ids.clone(),
        resource_ids: exported.windwave_metadata.resource_ids.clone(),
        scene_snapshot: exported.windwave_metadata.scene_snapshot.clone(),
    }
}

/// Import a Multica task into a unified task
pub fn import_to_unified_task(exported: &ExportedTask) -> UnifiedTask {
    let status = match exported.multica_task.status {
        TaskLifecycleStatus::Created => UnifiedTaskStatus::Draft,
        TaskLifecycleStatus::Claimed => UnifiedTaskStatus::Queued,
        TaskLifecycleStatus::Executing => UnifiedTaskStatus::Running,
        TaskLifecycleStatus::Completed => UnifiedTaskStatus::Done,
        TaskLifecycleStatus::Failed => UnifiedTaskStatus::Failed,
        TaskLifecycleStatus::Cancelled => UnifiedTaskStatus::Cancelled,
    };

    let mut task = UnifiedTask::new(
        exported.multica_task.title.clone(),
        exported
            .multica_task
            .description
            .clone()
            .unwrap_or_default(),
    );
    task.status = status;
    task.scene_id = exported.windwave_metadata.scene_id.clone();
    task.entity_ids = exported.windwave_metadata.entity_ids.clone();
    task.resource_ids = exported.windwave_metadata.resource_ids.clone();
    task.scene_snapshot = exported.windwave_metadata.scene_snapshot.clone();
    task
}

// ========================================
// Protocol Conversion
// ========================================

/// Convert a WindWave event to a Multica-compatible message
pub fn windwave_event_to_multica_message(event_type: &str, payload: &serde_json::Value) -> Message {
    Message {
        message_type: event_type.to_string(),
        payload: payload.clone(),
    }
}

/// Create a daemon register payload from local config
pub fn create_daemon_register_payload(
    daemon_id: &str,
    agent_id: &str,
    _supports_batch: bool,
) -> DaemonRegisterPayload {
    DaemonRegisterPayload {
        daemon_id: daemon_id.to_string(),
        agent_id: agent_id.to_string(),
        runtimes: vec![RuntimeInfo {
            runtime_type: "windwave-editor".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            status: "online".to_string(),
        }],
    }
}

/// Create a task claim payload
pub fn create_task_claim_payload(
    runtime_id: &str,
    daemon_id: &str,
    task_ids: Vec<String>,
) -> TaskClaimPayload {
    TaskClaimPayload {
        runtime_id: runtime_id.to_string(),
        tasks: task_ids,
        daemon_id: daemon_id.to_string(),
    }
}

/// Create a task completed payload
pub fn create_task_completed_payload(
    task_id: &str,
    output: Option<String>,
) -> TaskCompletedPayload {
    TaskCompletedPayload {
        task_id: task_id.to_string(),
        pr_url: None,
        output,
    }
}

/// Create a task progress payload
pub fn create_task_progress_payload(
    task_id: &str,
    summary: &str,
    step: Option<i32>,
    total: Option<i32>,
) -> TaskProgressPayload {
    TaskProgressPayload {
        task_id: task_id.to_string(),
        summary: summary.to_string(),
        step,
        total,
    }
}

// ========================================
// Compat Layer
// ========================================

/// Central compatibility layer managing WindWave ↔ Multica interop
pub struct MulticaCompatLayer {
    pub mode: CompatMode,
    #[allow(dead_code)]
    db: SharedMulticaDb,
    /// Exported tasks cache indexed by Multica task_id
    exported_cache: HashMap<String, ExportedTask>,
    /// Import batch tracking
    last_import: Option<ImportResult>,
}

impl MulticaCompatLayer {
    pub fn new(mode: CompatMode, db: SharedMulticaDb) -> Self {
        Self {
            mode,
            db,
            exported_cache: HashMap::new(),
            last_import: None,
        }
    }

    /// Set the compatibility mode
    pub fn set_mode(&mut self, mode: CompatMode) {
        info!("Compat mode changed to: {:?}", mode);
        self.mode = mode;
    }

    /// Export all local tasks to Multica format
    pub fn export_all_local(
        &mut self,
        task_sync: &crate::task_sync::TaskSync,
    ) -> Vec<ExportedTask> {
        let tasks = task_sync.get_all_tasks();
        let mut exported = Vec::with_capacity(tasks.len());

        for task in tasks {
            let exp = export_local_task(task);
            self.exported_cache
                .insert(exp.multica_task.task_id.clone(), exp.clone());
            exported.push(exp);
        }

        info!("Exported {} tasks to Multica format", exported.len());
        exported
    }

    /// Export all unified tasks to Multica format
    pub fn export_all_unified(&mut self, task_bridge: &TaskBridge) -> Vec<ExportedTask> {
        let tasks = task_bridge.get_all_tasks();
        let mut exported = Vec::with_capacity(tasks.len());

        for task in tasks {
            let exp = export_unified_task(&task);
            self.exported_cache
                .insert(exp.multica_task.task_id.clone(), exp.clone());
            exported.push(exp);
        }

        info!(
            "Exported {} unified tasks to Multica format",
            exported.len()
        );
        exported
    }

    /// Export tasks as JSON
    pub fn export_json(&mut self, task_bridge: &TaskBridge) -> Result<String> {
        let exported = self.export_all_unified(task_bridge);
        serde_json::to_string_pretty(&exported).map_err(BridgeError::SerializationError)
    }

    /// Import tasks from JSON
    pub fn import_json(&mut self, json: &str, task_bridge: &TaskBridge) -> ImportResult {
        let mut result = ImportResult::new();

        let exported: Vec<ExportedTask> = match serde_json::from_str(json) {
            Ok(tasks) => tasks,
            Err(e) => {
                result.errors.push(format!("JSON parse error: {}", e));
                result.tasks_failed += 1;
                return result;
            }
        };

        for exp in exported {
            let task_id = exp.multica_task.task_id.clone();
            let existing = task_bridge
                .get_all_tasks()
                .iter()
                .any(|t| t.id.bridge_id.to_string() == task_id);

            if existing {
                result.tasks_skipped += 1;
                warn!("Task {} already exists, skipping", task_id);
                continue;
            }

            let unified = import_to_unified_task(&exp);
            match task_bridge.register_task(unified) {
                Ok(registered) => {
                    result.tasks_imported += 1;
                    result
                        .imported_ids
                        .push(registered.id.bridge_id.to_string());
                    self.exported_cache.insert(task_id.clone(), exp);
                }
                Err(e) => {
                    result.tasks_failed += 1;
                    result.errors.push(format!("Import {}: {}", task_id, e));
                }
            }
        }

        self.last_import = Some(result.clone());
        info!(
            "Import complete: {} imported, {} skipped, {} failed",
            result.tasks_imported, result.tasks_skipped, result.tasks_failed
        );

        result
    }

    /// Get last import result
    pub fn last_import_result(&self) -> Option<&ImportResult> {
        self.last_import.as_ref()
    }

    /// Get cached export by task ID
    pub fn get_exported(&self, task_id: &str) -> Option<&ExportedTask> {
        self.exported_cache.get(task_id)
    }

    /// Convert a local task directly to a Multica message for WebSocket sending
    pub fn local_task_to_message(&self, task: &LocalTask) -> Message {
        let exported = export_local_task(task);
        Message {
            message_type: "task:exported".to_string(),
            payload: serde_json::to_value(&exported).unwrap_or_default(),
        }
    }

    /// Convert a unified task directly to a Multica message
    pub fn unified_task_to_message(&self, task: &UnifiedTask) -> Message {
        let exported = export_unified_task(task);
        Message {
            message_type: "task:exported".to_string(),
            payload: serde_json::to_value(&exported).unwrap_or_default(),
        }
    }

    /// Check if a task ID exists in the export cache
    pub fn is_cached(&self, task_id: &str) -> bool {
        self.exported_cache.contains_key(task_id)
    }

    /// Clear the export cache
    pub fn clear_cache(&mut self) {
        self.exported_cache.clear();
        info!("Export cache cleared");
    }

    /// Get cache statistics
    pub fn cache_stats(&self) -> (usize, Option<&ImportResult>) {
        (self.exported_cache.len(), self.last_import.as_ref())
    }
}

// ========================================
// Tests
// ========================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::multica_db::create_shared_multica_db;
    use crate::task_bridge::TaskBridge;
    use crate::task_sync::TaskSync;
    use crate::types::BridgeConfig;

    fn make_local_task(id: u64, title: &str) -> LocalTask {
        let mut task = LocalTask::new(id, title.to_string(), format!("desc_{}", id));
        task.scene_id = Some("scene_test".to_string());
        task.entity_ids = vec!["entity_1".to_string(), "entity_2".to_string()];
        task
    }

    #[test]
    fn test_compat_mode_defaults() {
        let mode = CompatMode::default();
        assert_eq!(mode, CompatMode::LocalOnly);
        assert!(mode.is_local_enabled());
        assert!(!mode.is_remote_enabled());
    }

    #[test]
    fn test_compat_mode_local_only() {
        let mode = CompatMode::LocalOnly;
        assert!(mode.is_local_enabled());
        assert!(!mode.is_remote_enabled());
    }

    #[test]
    fn test_compat_mode_remote_only() {
        let mode = CompatMode::RemoteOnly;
        assert!(!mode.is_local_enabled());
        assert!(mode.is_remote_enabled());
    }

    #[test]
    fn test_compat_mode_hybrid() {
        let mode = CompatMode::Hybrid {
            push_to_remote: true,
            pull_from_remote: false,
        };
        assert!(mode.is_local_enabled());
        assert!(mode.is_remote_enabled());
    }

    #[test]
    fn test_export_local_task() {
        let task = make_local_task(1, "Test task");
        let exported = export_local_task(&task);

        assert_eq!(exported.multica_task.title, "Test task");
        assert_eq!(exported.multica_task.status, TaskLifecycleStatus::Executing);
        assert_eq!(
            exported.windwave_metadata.scene_id,
            Some("scene_test".to_string())
        );
        assert_eq!(exported.windwave_metadata.entity_ids.len(), 2);
    }

    #[test]
    fn test_export_local_task_status_mapping() {
        let status_cases = vec![
            (TaskStatus::Draft, TaskLifecycleStatus::Created),
            (TaskStatus::Planning, TaskLifecycleStatus::Created),
            (TaskStatus::InProgress, TaskLifecycleStatus::Executing),
            (TaskStatus::Done, TaskLifecycleStatus::Completed),
            (TaskStatus::Failed, TaskLifecycleStatus::Failed),
            (TaskStatus::Cancelled, TaskLifecycleStatus::Cancelled),
        ];

        for (local, expected) in status_cases {
            let mut task = make_local_task(1, "test");
            task.status = local;
            let exported = export_local_task(&task);
            assert_eq!(
                exported.multica_task.status, expected,
                "Status {:?} should map to {:?}",
                local, expected
            );
        }
    }

    #[test]
    fn test_import_to_local_task() {
        let mut task = make_local_task(42, "Original");
        task.status = TaskStatus::Done;
        let exported = export_local_task(&task);

        let imported = import_to_local_task(&exported, 42);
        assert_eq!(imported.title, "Original");
        assert_eq!(imported.status, TaskStatus::Done);
        assert_eq!(imported.scene_id, Some("scene_test".to_string()));
    }

    #[test]
    fn test_export_then_import_roundtrip() {
        let original = make_local_task(99, "Roundtrip test");
        let exported = export_local_task(&original);
        let imported = import_to_local_task(&exported, 99);

        assert_eq!(imported.title, original.title);
        assert_eq!(imported.status, original.status);
        assert_eq!(imported.scene_id, original.scene_id);
        assert_eq!(imported.entity_ids, original.entity_ids);
    }

    #[test]
    fn test_export_unified_task() {
        let mut task = UnifiedTask::new(
            "Unified export".to_string(),
            "Testing unified export".to_string(),
        );
        task.scene_id = Some("scene_u1".to_string());
        task.status = UnifiedTaskStatus::Running;

        let exported = export_unified_task(&task);
        assert_eq!(exported.multica_task.status, TaskLifecycleStatus::Executing);
        assert_eq!(
            exported.windwave_metadata.scene_id,
            Some("scene_u1".to_string())
        );
    }

    #[test]
    fn test_compat_layer_new() {
        let db = create_shared_multica_db();
        let layer = MulticaCompatLayer::new(
            CompatMode::Hybrid {
                push_to_remote: true,
                pull_from_remote: true,
            },
            db,
        );

        assert!(layer.mode.is_local_enabled());
        assert!(layer.mode.is_remote_enabled());
        assert!(layer.last_import_result().is_none());
        assert!(layer.exported_cache.is_empty());
    }

    #[test]
    fn test_compat_layer_export_all_local() {
        let db = create_shared_multica_db();
        let mut layer = MulticaCompatLayer::new(CompatMode::default(), db);

        let config = BridgeConfig::default();
        let task_sync = TaskSync::new(config);
        let exported = layer.export_all_local(&task_sync);

        // TaskSync starts empty
        assert_eq!(exported.len(), 0);
    }

    #[test]
    fn test_compat_layer_export_all_unified() {
        let db = create_shared_multica_db();
        let mut layer = MulticaCompatLayer::new(CompatMode::default(), db);

        let config = BridgeConfig::default();
        let task_sync = TaskSync::new(config);
        let task_bridge = TaskBridge::new(task_sync);

        let exported = layer.export_all_unified(&task_bridge);
        assert_eq!(exported.len(), 0);
    }

    #[test]
    fn test_compat_layer_export_json() {
        let db = create_shared_multica_db();
        let mut layer = MulticaCompatLayer::new(CompatMode::default(), db);

        let config = BridgeConfig::default();
        let task_sync = TaskSync::new(config);
        let task_bridge = TaskBridge::new(task_sync);

        let json = layer.export_json(&task_bridge).unwrap();
        assert_eq!(json.trim(), "[]");
    }

    #[test]
    fn test_compat_layer_import_json() {
        let db = create_shared_multica_db();
        let mut layer = MulticaCompatLayer::new(CompatMode::default(), db);

        let config = BridgeConfig::default();
        let task_sync = TaskSync::new(config);
        let task_bridge = TaskBridge::new(task_sync);

        let result = layer.import_json("[]", &task_bridge);
        assert_eq!(result.tasks_imported, 0);
        assert_eq!(result.tasks_failed, 0);
    }

    #[test]
    fn test_compat_layer_cache() {
        let db = create_shared_multica_db();
        let mut layer = MulticaCompatLayer::new(CompatMode::default(), db);

        assert_eq!(layer.cache_stats().0, 0);
        assert!(!layer.is_cached("nonexistent"));

        layer.clear_cache();
        assert_eq!(layer.cache_stats().0, 0);
    }

    #[test]
    fn test_set_mode() {
        let db = create_shared_multica_db();
        let mut layer = MulticaCompatLayer::new(CompatMode::default(), db);

        assert_eq!(layer.mode, CompatMode::LocalOnly);
        layer.set_mode(CompatMode::RemoteOnly);
        assert_eq!(layer.mode, CompatMode::RemoteOnly);
    }

    #[test]
    fn test_payload_creators() {
        let register = create_daemon_register_payload("daemon_1", "agent_1", true);
        assert_eq!(register.daemon_id, "daemon_1");
        assert_eq!(register.agent_id, "agent_1");
        assert_eq!(register.runtimes.len(), 1);
        assert_eq!(register.runtimes[0].runtime_type, "windwave-editor");

        let claim = create_task_claim_payload("rt_1", "daemon_1", vec!["t1".to_string()]);
        assert_eq!(claim.tasks.len(), 1);

        let completed = create_task_completed_payload("t1", Some("done".to_string()));
        assert_eq!(completed.task_id, "t1");
        assert_eq!(completed.output, Some("done".to_string()));

        let progress = create_task_progress_payload("t1", "working", Some(1), Some(5));
        assert_eq!(progress.summary, "working");
    }

    #[test]
    fn test_windwave_event_to_message() {
        let payload = serde_json::json!({"key": "value"});
        let msg = windwave_event_to_multica_message("entity:created", &payload);
        assert_eq!(msg.message_type, "entity:created");
        assert_eq!(msg.payload["key"], "value");
    }
}
