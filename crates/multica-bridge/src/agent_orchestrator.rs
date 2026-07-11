//! Agent Orchestrator — Unified agent execution pipeline
//!
//! Ties together SceneAgent, AgentProxy, SkillSystem, TaskBridge,
//! MemoryInjector and RealtimeBridge into a single cohesive interface.
//!
//! Provides:
//! - Agent dispatching with scene context injection
//! - Skill execution routed through SceneAgent when available
//! - Auto memory injection on scene changes
//! - Task lifecycle management linked to agent execution
//! - Pipeline execution statistics and monitoring

use crate::agent_proxy::AgentProxy;
use crate::error::{BridgeError, Result};
use crate::memory_injector::{InjectedEntry, MulticaMemoryInjector};
use crate::multica_db::SharedMulticaDb;
use crate::realtime_bridge::RealtimeBridge;
use crate::scene_agent::{SceneAgent, SceneAgentCapability, SceneAgentConfig, SceneAgentResult};
use crate::scene_context::SharedSceneContext;
use crate::skill_system::{SkillExecutionContext, SkillSystem};
use crate::task_bridge::{TaskBridge, UnifiedTask, UnifiedTaskStatus};
#[cfg(test)]
use crate::task_sync::TaskSync;
use crate::types::BridgeConfig;
use log::{debug, error, info, warn};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use std::time::Instant;

// ========================================
// Pipeline Status
// ========================================

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PipelineStatus {
    Idle,
    Initializing,
    Running,
    Paused,
    Stopping,
    Stopped,
    Error(String),
}

// ========================================
// Execution Mode
// ========================================

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExecutionMode {
    LocalOnly,
    RemoteOnly,
    Hybrid,
}

// ========================================
// Pipeline Stats
// ========================================

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PipelineStats {
    pub agent_tasks_dispatched: u64,
    pub agent_tasks_completed: u64,
    pub agent_tasks_failed: u64,
    pub skills_executed: u64,
    pub skill_success_rate: f64,
    pub memory_injections: u64,
    pub scene_events_processed: u64,
    pub total_execution_time_ms: u128,
    pub avg_execution_time_ms: f64,
    pub last_pipeline_run: Option<String>,
}

impl PipelineStats {
    pub fn record_agent_success(&mut self, duration_ms: u64) {
        self.agent_tasks_dispatched += 1;
        self.agent_tasks_completed += 1;
        self.total_execution_time_ms += duration_ms as u128;
        self.avg_execution_time_ms =
            self.total_execution_time_ms as f64 / self.agent_tasks_completed.max(1) as f64;
    }

    pub fn record_agent_failure(&mut self) {
        self.agent_tasks_dispatched += 1;
        self.agent_tasks_failed += 1;
    }

    pub fn record_skill_execution(&mut self, success: bool) {
        self.skills_executed += 1;
        let total = self.skills_executed as f64;
        let successes = if success {
            self.skill_success_rate = ((self.skill_success_rate * (total - 1.0)) + 1.0) / total;
            0
        } else {
            self.skill_success_rate = (self.skill_success_rate * (total - 1.0)) / total;
            0
        };
        let _ = successes;
    }

    pub fn record_memory_injection(&mut self) {
        self.memory_injections += 1;
    }
}

// ========================================
// Agent Traits for integration with agent-core
// ========================================

/// Abstract agent executor — bridge to agent-core's Agent trait
pub trait AgentExecutor: Send + Sync {
    fn execute(&self, prompt: &str, context: &str) -> std::result::Result<String, String>;
    fn name(&self) -> &str;
    fn capabilities(&self) -> Vec<String>;
}

/// Abstract skill provider — bridge to agent-core's ToolRegistry
pub trait SkillProvider: Send + Sync {
    fn available_skills(&self) -> Vec<String>;
    fn execute_skill(
        &self,
        name: &str,
        params: &serde_json::Value,
    ) -> std::result::Result<serde_json::Value, String>;
}

// ========================================
// Pipeline Task
// ========================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineTask {
    pub id: String,
    pub title: String,
    pub description: String,
    pub scene_id: Option<String>,
    pub entity_ids: Vec<u64>,
    pub skill_name: Option<String>,
    pub skill_params: Option<serde_json::Value>,
    pub agent_prompt: Option<String>,
    pub mode: ExecutionMode,
    pub priority: u8,
    pub created_at: String,
    pub status: PipelineTaskStatus,
    pub result: Option<serde_json::Value>,
    pub error: Option<String>,
    pub duration_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PipelineTaskStatus {
    Queued,
    Running,
    Completed,
    Failed,
    Cancelled,
}

impl PipelineTask {
    pub fn new(title: &str, description: &str) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            title: title.to_string(),
            description: description.to_string(),
            scene_id: None,
            entity_ids: Vec::new(),
            skill_name: None,
            skill_params: None,
            agent_prompt: None,
            mode: ExecutionMode::Hybrid,
            priority: 5,
            created_at: chrono::Utc::now().to_rfc3339(),
            status: PipelineTaskStatus::Queued,
            result: None,
            error: None,
            duration_ms: None,
        }
    }

    pub fn with_scene(mut self, scene_id: &str) -> Self {
        self.scene_id = Some(scene_id.to_string());
        self
    }

    pub fn with_skill(mut self, name: &str, params: serde_json::Value) -> Self {
        self.skill_name = Some(name.to_string());
        self.skill_params = Some(params);
        self
    }

    pub fn with_agent_prompt(mut self, prompt: &str) -> Self {
        self.agent_prompt = Some(prompt.to_string());
        self
    }

    pub fn with_mode(mut self, mode: ExecutionMode) -> Self {
        self.mode = mode;
        self
    }

    pub fn with_entities(mut self, entity_ids: Vec<u64>) -> Self {
        self.entity_ids = entity_ids;
        self
    }
}

// ========================================
// Agent Orchestrator
// ========================================

#[allow(dead_code)]
pub struct AgentOrchestrator {
    config: BridgeConfig,
    db: SharedMulticaDb,
    scene_context: SharedSceneContext,
    scene_agent: Option<Arc<SceneAgent>>,
    skill_system: Option<Arc<SkillSystem>>,
    agent_proxy: Option<AgentProxy>,
    task_bridge: Option<Arc<Mutex<TaskBridge>>>,
    memory_injector: Option<Arc<Mutex<MulticaMemoryInjector>>>,
    realtime_bridge: Option<RealtimeBridge>,
    agent_executor: Option<Box<dyn AgentExecutor>>,
    skill_provider: Option<Box<dyn SkillProvider>>,
    status: PipelineStatus,
    stats: PipelineStats,
    task_queue: Vec<PipelineTask>,
    max_concurrent_tasks: usize,
    execution_mode: ExecutionMode,
}

impl AgentOrchestrator {
    pub fn new(
        config: BridgeConfig,
        db: SharedMulticaDb,
        scene_context: SharedSceneContext,
    ) -> Self {
        Self {
            config,
            db,
            scene_context,
            scene_agent: None,
            skill_system: None,
            agent_proxy: None,
            task_bridge: None,
            memory_injector: None,
            realtime_bridge: None,
            agent_executor: None,
            skill_provider: None,
            status: PipelineStatus::Idle,
            stats: PipelineStats::default(),
            task_queue: Vec::new(),
            max_concurrent_tasks: 5,
            execution_mode: ExecutionMode::Hybrid,
        }
    }

    // ----- Builder Methods -----

    pub fn with_scene_agent(mut self, agent: SceneAgent) -> Self {
        self.scene_agent = Some(Arc::new(agent));
        self
    }

    pub fn with_skill_system(mut self, system: SkillSystem) -> Self {
        self.skill_system = Some(Arc::new(system));
        self
    }

    pub fn with_agent_proxy(mut self, proxy: AgentProxy) -> Self {
        self.agent_proxy = Some(proxy);
        self
    }

    pub fn with_task_bridge(mut self, bridge: TaskBridge) -> Self {
        self.task_bridge = Some(Arc::new(Mutex::new(bridge)));
        self
    }

    pub fn with_memory_injector(mut self, injector: Arc<Mutex<MulticaMemoryInjector>>) -> Self {
        self.memory_injector = Some(injector);
        self
    }

    pub fn with_realtime_bridge(mut self, bridge: RealtimeBridge) -> Self {
        self.realtime_bridge = Some(bridge);
        self
    }

    pub fn with_agent_executor(mut self, executor: Box<dyn AgentExecutor>) -> Self {
        self.agent_executor = Some(executor);
        self
    }

    pub fn with_skill_provider(mut self, provider: Box<dyn SkillProvider>) -> Self {
        self.skill_provider = Some(provider);
        self
    }

    pub fn with_mode(mut self, mode: ExecutionMode) -> Self {
        self.execution_mode = mode;
        self
    }

    // ----- Initialize -----

    pub fn initialize(&mut self) -> Result<()> {
        self.status = PipelineStatus::Initializing;
        info!("Initializing agent orchestrator pipeline...");

        if self.scene_agent.is_none() {
            let db = self.db.lock().expect("mutex poisoned");
            let scene_id = db.get_all_scenes().first().map(|s| s.scene_id.clone());
            drop(db);

            let config = SceneAgentConfig {
                scene_id,
                capabilities: vec![
                    SceneAgentCapability::QueryScene,
                    SceneAgentCapability::CreateEntity,
                    SceneAgentCapability::DeleteEntity,
                    SceneAgentCapability::ModifyEntity,
                    SceneAgentCapability::GenerateTerrain,
                    SceneAgentCapability::ValidateScene,
                    SceneAgentCapability::OptimizeScene,
                    SceneAgentCapability::ApplySkill,
                ],
                system_prompt: Some("You are a game editor scene agent. Translate instructions into scene manipulations.".into()),
                ..Default::default()
            };
            let agent = SceneAgent::new(config, self.db.clone(), self.scene_context.clone());
            self.scene_agent = Some(Arc::new(agent));
            info!("Created default SceneAgent");
        }

        self.status = PipelineStatus::Running;
        info!("Agent orchestrator pipeline initialized");
        Ok(())
    }

    // ----- Agent Execution -----

    /// Execute an agent prompt with scene context
    pub fn run_agent(&mut self, prompt: &str, _task_id: Option<&str>) -> Result<SceneAgentResult> {
        let start = Instant::now();

        let result = if let Some(ref agent) = self.scene_agent {
            agent.execute_with_scene_context(prompt)
        } else {
            return Err(BridgeError::Other("No SceneAgent configured".into()));
        };

        let duration_ms = start.elapsed().as_millis() as u64;

        if result.success {
            self.stats.record_agent_success(duration_ms);
            self.stats.last_pipeline_run = Some(chrono::Utc::now().to_rfc3339());

            if let Some(ref bridge) = self.task_bridge {
                let bridge = bridge.lock().expect("mutex poisoned");
                let mut found_id: Option<u64> = None;
                for task in bridge.get_all_tasks() {
                    if task.title.contains("Pipeline") && task.status != UnifiedTaskStatus::Done {
                        found_id = Some(task.id.bridge_id);
                        break;
                    }
                }
                if let Some(id) = found_id {
                    let _ = bridge.update_task_status(id, UnifiedTaskStatus::Done);
                }
            }
        } else {
            self.stats.record_agent_failure();
        }

        Ok(result)
    }

    /// Execute a pipeline task (skill + agent in sequence)
    pub fn execute_pipeline_task(&mut self, task: &mut PipelineTask) -> Result<()> {
        let start = Instant::now();
        task.status = PipelineTaskStatus::Running;
        info!("Executing pipeline task: {}", task.title);

        let mut results = Vec::new();

        // Step 1: Inject scene memory if scene_id is set
        if let (Some(ref scene_id), Some(ref injector)) = (&task.scene_id, &self.memory_injector) {
            let mut inj = injector.lock().expect("mutex poisoned");
            match inj.inject_scene(scene_id) {
                Ok(entries) => {
                    self.stats.record_memory_injection();
                    debug!(
                        "Injected {} memory entries for pipeline task",
                        entries.len()
                    );
                }
                Err(e) => warn!("Memory injection failed for pipeline task: {}", e),
            }
        }

        // Step 2: Execute skill if specified
        if let (Some(ref name), Some(ref params)) = (&task.skill_name, &task.skill_params) {
            if let Some(ref system) = self.skill_system {
                let ctx = SkillExecutionContext {
                    scene_id: task.scene_id.clone(),
                    entity_ids: task.entity_ids.clone(),
                    ..Default::default()
                };
                match system.execute(name, params, Some(&ctx)) {
                    Ok(result) => {
                        self.stats.record_skill_execution(result.success);
                        results.push(serde_json::json!({
                            "type": "skill",
                            "name": name,
                            "success": result.success,
                            "data": result.data,
                            "error": result.error,
                        }));
                    }
                    Err(e) => {
                        warn!("Skill execution failed in pipeline: {}", e);
                    }
                }
            } else if let Some(ref provider) = self.skill_provider {
                match provider.execute_skill(name, params) {
                    Ok(data) => {
                        self.stats.record_skill_execution(true);
                        results.push(serde_json::json!({
                            "type": "skill",
                            "name": name,
                            "success": true,
                            "data": data,
                        }));
                    }
                    Err(e) => {
                        self.stats.record_skill_execution(false);
                        results.push(serde_json::json!({
                            "type": "skill",
                            "name": name,
                            "success": false,
                            "error": e,
                        }));
                    }
                }
            }
        }

        // Step 3: Execute agent prompt if specified
        if let Some(ref prompt) = task.agent_prompt {
            match self.run_agent(prompt, Some(&task.id)) {
                Ok(result) => {
                    results.push(serde_json::json!({
                        "type": "agent",
                        "success": result.success,
                        "output": result.output,
                        "capability_used": result.capability_used,
                        "entity_ids_affected": result.entity_ids_affected,
                    }));
                }
                Err(e) => {
                    results.push(serde_json::json!({
                        "type": "agent",
                        "success": false,
                        "error": e.to_string(),
                    }));
                }
            }
        }

        // Step 4: Create unified task in task bridge
        if let Some(ref bridge) = self.task_bridge {
            let bridge = bridge.lock().expect("mutex poisoned");
            let unified_task = UnifiedTask::new(task.title.clone(), task.description.clone());
            let mut t = unified_task;
            let scene_id_clone = task.scene_id.clone();
            t.set_scene(scene_id_clone.unwrap_or_default(), None);
            t.entity_ids = task.entity_ids.iter().map(|id| id.to_string()).collect();
            let _ = bridge.register_task(t);
        }

        let duration_ms = start.elapsed().as_millis() as u64;
        task.duration_ms = Some(duration_ms);
        task.status = PipelineTaskStatus::Completed;
        task.result = Some(serde_json::json!({
            "steps": results,
            "total_duration_ms": duration_ms,
        }));

        self.stats.last_pipeline_run = Some(chrono::Utc::now().to_rfc3339());
        info!("Pipeline task completed: {} ({})", task.title, duration_ms);

        Ok(())
    }

    // ----- Task Queue Management -----

    /// Enqueue a pipeline task
    pub fn enqueue(&mut self, task: PipelineTask) {
        info!("Enqueueing pipeline task: {}", task.title);
        self.task_queue.push(task);
        // Sort by priority (higher = more urgent)
        self.task_queue
            .sort_by_key(|a| std::cmp::Reverse(a.priority));
    }

    /// Process the task queue, executing up to max_concurrent_tasks
    pub fn process_queue(&mut self) -> Result<usize> {
        if self.task_queue.is_empty() {
            return Ok(0);
        }

        let to_process: Vec<usize> = self
            .task_queue
            .iter()
            .enumerate()
            .filter(|(_, t)| t.status == PipelineTaskStatus::Queued)
            .take(self.max_concurrent_tasks)
            .map(|(i, _)| i)
            .collect();

        let processed = to_process.len();
        for idx in to_process.into_iter().rev() {
            let mut task = self.task_queue.remove(idx);
            if let Err(e) = self.execute_pipeline_task(&mut task) {
                error!("Pipeline task failed: {} - {}", task.title, e);
                task.status = PipelineTaskStatus::Failed;
                task.error = Some(e.to_string());
            }
            self.task_queue.push(task);
        }

        Ok(processed)
    }

    /// Cancel a queued task
    pub fn cancel_task(&mut self, task_id: &str) -> bool {
        if let Some(task) = self.task_queue.iter_mut().find(|t| t.id == task_id) {
            if task.status == PipelineTaskStatus::Queued {
                task.status = PipelineTaskStatus::Cancelled;
                return true;
            }
        }
        false
    }

    /// Get queue statistics
    pub fn queue_stats(&self) -> (usize, usize, usize, usize) {
        let queued = self
            .task_queue
            .iter()
            .filter(|t| t.status == PipelineTaskStatus::Queued)
            .count();
        let running = self
            .task_queue
            .iter()
            .filter(|t| t.status == PipelineTaskStatus::Running)
            .count();
        let completed = self
            .task_queue
            .iter()
            .filter(|t| t.status == PipelineTaskStatus::Completed)
            .count();
        let failed = self
            .task_queue
            .iter()
            .filter(|t| t.status == PipelineTaskStatus::Failed)
            .count();
        (queued, running, completed, failed)
    }

    // ----- Memory Injection -----

    /// Trigger memory injection for a scene
    pub fn inject_scene_memory(&mut self, scene_id: &str) -> Result<Vec<InjectedEntry>> {
        if let Some(ref injector) = self.memory_injector {
            let mut inj = injector.lock().expect("mutex poisoned");
            let entries = inj.inject_scene(scene_id)?;
            self.stats.record_memory_injection();
            Ok(entries)
        } else {
            Err(BridgeError::Other("No memory injector configured".into()))
        }
    }

    /// Inject all scenes into memory
    pub fn inject_all_memory(&mut self) -> Result<Vec<InjectedEntry>> {
        if let Some(ref injector) = self.memory_injector {
            let mut inj = injector.lock().expect("mutex poisoned");
            let entries = inj.inject_all_scenes()?;
            self.stats.memory_injections += 1;
            Ok(entries)
        } else {
            Err(BridgeError::Other("No memory injector configured".into()))
        }
    }

    // ----- Scene Event Handling -----

    /// Handle a scene entity created event
    pub fn on_entity_created(&mut self, scene_id: &str, entity_id: u64, entity_name: &str) {
        if let Some(ref realtime) = self.realtime_bridge {
            realtime.on_entity_created(scene_id, entity_id, entity_name);
        }
        if let Some(ref injector) = self.memory_injector {
            let mut inj = injector.lock().expect("mutex poisoned");
            let entries = inj.inject_scene(scene_id).unwrap_or_default();
            if !entries.is_empty() {
                self.stats.record_memory_injection();
            }
        }
        self.stats.scene_events_processed += 1;
    }

    /// Handle a scene entity updated event
    pub fn on_entity_updated(&mut self, scene_id: &str, entity_id: u64, entity_name: &str) {
        if let Some(ref realtime) = self.realtime_bridge {
            realtime.on_entity_updated(scene_id, entity_id, entity_name);
        }
        self.stats.scene_events_processed += 1;
    }

    /// Handle a scene entity deleted event
    pub fn on_entity_deleted(&mut self, scene_id: &str, entity_id: u64, entity_name: &str) {
        if let Some(ref realtime) = self.realtime_bridge {
            realtime.on_entity_deleted(scene_id, entity_id, entity_name);
        }
        self.stats.scene_events_processed += 1;
    }

    // ----- Status and Stats -----

    pub fn status(&self) -> &PipelineStatus {
        &self.status
    }

    pub fn stats(&self) -> &PipelineStats {
        &self.stats
    }

    pub fn is_initialized(&self) -> bool {
        self.status == PipelineStatus::Running
    }

    pub fn set_mode(&mut self, mode: ExecutionMode) {
        self.execution_mode = mode;
    }

    pub fn mode(&self) -> &ExecutionMode {
        &self.execution_mode
    }

    pub fn scene_agent(&self) -> Option<&Arc<SceneAgent>> {
        self.scene_agent.as_ref()
    }

    pub fn skill_system(&self) -> Option<&Arc<SkillSystem>> {
        self.skill_system.as_ref()
    }
}

// ========================================
// Tests
// ========================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::multica_db::create_shared_multica_db;
    use crate::scene_context::create_shared_scene_context;
    use crate::task_bridge::TaskBridge;

    fn setup() -> AgentOrchestrator {
        let config = BridgeConfig::default();
        let db = create_shared_multica_db();
        let ctx = create_shared_scene_context();
        AgentOrchestrator::new(config, db, ctx)
    }

    fn setup_with_bridge() -> AgentOrchestrator {
        let mut orch = setup();
        let bridge = TaskBridge::new(TaskSync::new(BridgeConfig::default()));
        orch = orch.with_task_bridge(bridge);
        orch
    }

    #[test]
    fn test_orchestrator_initial_state() {
        let orch = setup();
        assert_eq!(orch.status, PipelineStatus::Idle);
        assert_eq!(orch.max_concurrent_tasks, 5);
        assert_eq!(orch.stats.agent_tasks_dispatched, 0);
        assert!(orch.scene_agent.is_none());
    }

    #[test]
    fn test_initialize_creates_default_scene_agent() {
        let mut orch = setup();
        orch.initialize().unwrap();
        assert_eq!(orch.status, PipelineStatus::Running);
        assert!(orch.scene_agent.is_some());
    }

    #[test]
    fn test_builder_methods() {
        let orch = setup();
        let scene_agent_config = SceneAgentConfig {
            scene_id: None,
            capabilities: vec![],
            system_prompt: None,
            ..Default::default()
        };
        let agent = SceneAgent::new(
            scene_agent_config,
            create_shared_multica_db(),
            create_shared_scene_context(),
        );
        let system = SkillSystem::new(create_shared_multica_db(), create_shared_scene_context());
        let proxy = AgentProxy::new(BridgeConfig::default(), "TestAgent".into());
        let bridge = TaskBridge::new(TaskSync::new(BridgeConfig::default()));

        let orch = orch
            .with_scene_agent(agent)
            .with_skill_system(system)
            .with_agent_proxy(proxy)
            .with_task_bridge(bridge)
            .with_mode(ExecutionMode::LocalOnly);

        assert!(orch.scene_agent.is_some());
        assert!(orch.skill_system.is_some());
        assert!(orch.agent_proxy.is_some());
        assert!(orch.task_bridge.is_some());
        assert_eq!(orch.execution_mode, ExecutionMode::LocalOnly);
    }

    #[test]
    fn test_pipeline_task_creation() {
        let task = PipelineTask::new("Test Task", "Test Description");
        assert_eq!(task.title, "Test Task");
        assert_eq!(task.status, PipelineTaskStatus::Queued);
        assert!(task.skill_name.is_none());
    }

    #[test]
    fn test_pipeline_task_with_skill() {
        let task = PipelineTask::new("Skill Task", "Run skill")
            .with_skill("entity.create", serde_json::json!({"name": "Test"}))
            .with_agent_prompt("Create a player entity");

        assert_eq!(task.skill_name, Some("entity.create".into()));
        assert!(task.agent_prompt.is_some());
    }

    #[test]
    fn test_pipeline_task_with_scene() {
        let task = PipelineTask::new("Scene Task", "Scene operation")
            .with_scene("scene-1")
            .with_entities(vec![1, 2, 3]);

        assert_eq!(task.scene_id, Some("scene-1".into()));
        assert_eq!(task.entity_ids, vec![1, 2, 3]);
    }

    #[test]
    fn test_enqueue_and_queue_stats() {
        let mut orch = setup();
        orch.enqueue(PipelineTask::new("Task A", "Desc A"));
        orch.enqueue(PipelineTask::new("Task B", "Desc B"));

        let (queued, running, completed, failed) = orch.queue_stats();
        assert_eq!(queued, 2);
        assert_eq!(running, 0);
        assert_eq!(completed, 0);
        assert_eq!(failed, 0);
    }

    #[test]
    fn test_cancel_task() {
        let mut orch = setup();
        let task = PipelineTask::new("Cancellable", "Test");
        let id = task.id.clone();
        orch.enqueue(task);

        assert!(orch.cancel_task(&id));
        let (queued, _, _, _) = orch.queue_stats();
        assert_eq!(queued, 0);
    }

    #[test]
    fn test_execute_skill_pipeline_task() {
        let mut orch = setup();
        let skill_system =
            SkillSystem::new(create_shared_multica_db(), create_shared_scene_context());
        orch = orch.with_skill_system(skill_system);

        let mut task = PipelineTask::new("Skill Pipeline", "Execute entity query")
            .with_skill("entity.query", serde_json::json!({}));

        let result = orch.execute_pipeline_task(&mut task);
        assert!(result.is_ok());
        assert_eq!(task.status, PipelineTaskStatus::Completed);
        assert!(task.duration_ms.is_some());
    }

    #[test]
    fn test_execute_agent_pipeline_task() {
        let mut orch = setup();
        orch.initialize().unwrap();

        let mut task = PipelineTask::new("Agent Pipeline", "Scene query")
            .with_agent_prompt("List all entities in the current scene");

        let result = orch.execute_pipeline_task(&mut task);
        assert!(result.is_ok());
        assert_eq!(task.status, PipelineTaskStatus::Completed);
    }

    #[test]
    fn test_execute_combined_pipeline_task() {
        let mut orch = setup_with_bridge();
        orch.initialize().unwrap();

        let skill_system =
            SkillSystem::new(create_shared_multica_db(), create_shared_scene_context());
        orch = orch.with_skill_system(skill_system);

        let mut task = PipelineTask::new("Combined Pipeline", "Skill + Agent")
            .with_skill("entity.query", serde_json::json!({}))
            .with_agent_prompt("Analyze the entities in the scene");

        let result = orch.execute_pipeline_task(&mut task);
        assert!(result.is_ok());
        assert_eq!(task.status, PipelineTaskStatus::Completed);

        let data = task.result.as_ref().unwrap();
        let steps = data["steps"].as_array().unwrap();
        assert_eq!(steps.len(), 2);
        assert_eq!(steps[0]["type"], "skill");
        assert_eq!(steps[1]["type"], "agent");
    }

    #[test]
    fn test_process_queue() {
        let mut orch = setup();
        let skill_system =
            SkillSystem::new(create_shared_multica_db(), create_shared_scene_context());
        orch = orch.with_skill_system(skill_system);

        orch.enqueue(
            PipelineTask::new("Queue Task 1", "First")
                .with_skill("entity.query", serde_json::json!({})),
        );
        orch.enqueue(
            PipelineTask::new("Queue Task 2", "Second")
                .with_skill("entity.query", serde_json::json!({})),
        );

        let processed = orch.process_queue().unwrap();
        assert_eq!(processed, 2);

        let (queued, _, completed, failed) = orch.queue_stats();
        assert_eq!(queued, 0);
        assert_eq!(completed, 2);
        assert_eq!(failed, 0);
    }

    #[test]
    fn test_stats_tracking() {
        let mut orch = setup();
        let skill_system =
            SkillSystem::new(create_shared_multica_db(), create_shared_scene_context());
        orch = orch.with_skill_system(skill_system);

        let mut task = PipelineTask::new("Stats Test", "Track me")
            .with_skill("entity.query", serde_json::json!({}));
        orch.execute_pipeline_task(&mut task).unwrap();

        assert!(orch.stats.skills_executed >= 1);
        assert!(orch.stats.last_pipeline_run.is_some());
    }

    #[test]
    fn test_on_entity_events() {
        let mut orch = setup();

        orch.on_entity_created("scene-1", 1, "Player");
        orch.on_entity_updated("scene-1", 2, "Enemy");
        orch.on_entity_deleted("scene-1", 3, "Item");

        assert_eq!(orch.stats.scene_events_processed, 3);
    }

    #[test]
    fn test_execution_mode() {
        let mut orch = setup();
        assert_eq!(orch.mode(), &ExecutionMode::Hybrid);

        orch.set_mode(ExecutionMode::LocalOnly);
        assert_eq!(orch.mode(), &ExecutionMode::LocalOnly);
    }

    #[test]
    fn test_priority_sorting() {
        let mut orch = setup();
        orch.enqueue(PipelineTask::new("Low Priority", "low"));

        let mut mid = PipelineTask::new("Mid Priority", "mid");
        mid.priority = 5;
        orch.enqueue(mid);

        let mut high = PipelineTask::new("High Priority", "high");
        high.priority = 10;
        orch.enqueue(high);

        assert_eq!(orch.task_queue[0].priority, 10);
        assert_eq!(orch.task_queue[1].priority, 5);
        assert_eq!(orch.task_queue[2].priority, 5);
    }

    #[test]
    fn test_inject_scene_memory_no_injector() {
        let mut orch = setup();
        let result = orch.inject_scene_memory("test-scene");
        assert!(result.is_err());
    }
}
