//! Real-time Communication Bridge
//!
//! Integrates WebSocket transport with SkillSystem and SceneAgent for
//! bidirectional real-time game operations. Handles skill dispatch from
//! the Multica server, scene event streaming back to the server, and
//! connection-aware message queuing.

use crate::error::{BridgeError, Result};
use crate::message_handler::{MessageHandleResult, MessageHandler};
use crate::multica_db::SharedMulticaDb;
use crate::scene_agent::SceneAgent;
use crate::scene_context::SharedSceneContext;
use crate::skill_system::{SkillExecResult, SkillExecutionContext, SkillSystem};
use crate::types::{BridgeConfig, Message};
use crate::ws_client::{ConnectionMonitor, MessageQueue, MulticaWebSocketClient};
use log::{debug, info, warn};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

// ========================================
// Event Type Constants
// ========================================

pub const EVENT_SKILL_DISPATCH: &str = "skill:dispatch";
pub const EVENT_SKILL_RESULT: &str = "skill:result";
pub const EVENT_SKILL_LIST_REQUEST: &str = "skill:list_request";
pub const EVENT_SKILL_LIST_RESPONSE: &str = "skill:list_response";

pub const EVENT_SCENE_SYNC_REQUEST: &str = "scene:sync_request";
pub const EVENT_SCENE_SYNC_RESPONSE: &str = "scene:sync_response";
pub const EVENT_SCENE_EVENT: &str = "scene:event";

pub const EVENT_ENTITY_EVENT: &str = "entity:event";

pub const EVENT_REALTIME_STATUS: &str = "realtime:status";
pub const EVENT_REALTIME_ERROR: &str = "realtime:error";

// ========================================
// Payload Types
// ========================================

/// Skill dispatch request from the server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillDispatchPayload {
    pub request_id: String,
    pub skill_name: String,
    pub parameters: serde_json::Value,
    pub scene_id: Option<String>,
    pub entity_ids: Vec<u64>,
    pub timeout_ms: Option<u64>,
}

/// Skill execution result sent back to the server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillResultPayload {
    pub request_id: String,
    pub skill_name: String,
    pub success: bool,
    pub output: Option<String>,
    pub error: Option<String>,
    pub data: Option<serde_json::Value>,
    pub entity_ids_affected: Vec<u64>,
    pub duration_ms: u64,
}

impl From<SkillExecResult> for SkillResultPayload {
    fn from(r: SkillExecResult) -> Self {
        Self {
            request_id: String::new(),
            skill_name: r.skill_name,
            success: r.success,
            output: r.output,
            error: r.error,
            data: r.data,
            entity_ids_affected: r.entity_ids_affected,
            duration_ms: r.duration_ms,
        }
    }
}

/// Scene event to broadcast to the server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneEventPayload {
    pub event_type: String,
    pub scene_id: String,
    pub entity_id: Option<u64>,
    pub timestamp: String,
    pub data: serde_json::Value,
}

/// Entity event (create/update/delete) for real-time sync
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityEventPayload {
    pub action: String,
    pub scene_id: String,
    pub entity_id: u64,
    pub entity_name: String,
    pub component_types: Vec<String>,
    pub position: Option<[f64; 3]>,
    pub timestamp: String,
}

/// Scene sync request payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneSyncRequestPayload {
    pub request_id: String,
    pub scene_id: String,
    pub full_sync: bool,
    pub since_version: Option<u64>,
}

/// Scene sync response payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneSyncResponsePayload {
    pub request_id: String,
    pub scene_id: String,
    pub entity_count: usize,
    pub entities: serde_json::Value,
    pub version: u64,
    pub timestamp: String,
}

/// Realtime bridge status
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RealtimeStatus {
    Disconnected,
    Connecting,
    Connected,
    Syncing,
    Error(String),
}

/// Realtime bridge statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RealtimeStats {
    pub skills_dispatched: u64,
    pub skills_completed: u64,
    pub skills_failed: u64,
    pub scene_events_sent: u64,
    pub entity_events_sent: u64,
    pub syncs_performed: u64,
    pub messages_sent: u64,
    pub messages_received: u64,
    pub last_activity: Option<String>,
    pub avg_skill_duration_ms: f64,
}

// ========================================
// Skill Dispatch Handler
// ========================================

/// Message handler for skill dispatch messages
pub struct SkillDispatchHandler {
    skill_system: Arc<SkillSystem>,
}

impl SkillDispatchHandler {
    pub fn new(skill_system: Arc<SkillSystem>) -> Self {
        Self { skill_system }
    }
}

impl MessageHandler for SkillDispatchHandler {
    fn handle_message(&self, message: &Message) -> Result<MessageHandleResult> {
        let payload: SkillDispatchPayload = serde_json::from_value(message.payload.clone())
            .map_err(BridgeError::SerializationError)?;

        info!(
            "Dispatching skill '{}' (request: {})",
            payload.skill_name, payload.request_id
        );

        let ctx = SkillExecutionContext {
            scene_id: payload.scene_id,
            entity_ids: payload.entity_ids,
            ..Default::default()
        };

        let result =
            self.skill_system
                .execute(&payload.skill_name, &payload.parameters, Some(&ctx))?;

        let mut metadata = HashMap::new();
        metadata.insert("request_id".to_string(), payload.request_id);
        metadata.insert("skill_name".to_string(), payload.skill_name);
        metadata.insert("success".to_string(), result.success.to_string());

        Ok(MessageHandleResult {
            success: result.success,
            message_type: message.message_type.clone(),
            error: result.error,
            processed_at: chrono::Utc::now().to_rfc3339(),
            metadata,
        })
    }

    fn handler_name(&self) -> &str {
        "skill_dispatch"
    }
}

// ========================================
// Scene Event Streamer
// ========================================

/// Streams scene events to the Multica server in real-time
#[allow(dead_code)]
pub struct SceneEventStreamer {
    db: SharedMulticaDb,
    scene_context: SharedSceneContext,
    pending_events: Arc<Mutex<Vec<SceneEventPayload>>>,
    is_streaming: Arc<AtomicBool>,
    max_batch_size: usize,
}

impl SceneEventStreamer {
    pub fn new(db: SharedMulticaDb, scene_context: SharedSceneContext) -> Self {
        Self {
            db,
            scene_context,
            pending_events: Arc::new(Mutex::new(Vec::new())),
            is_streaming: Arc::new(AtomicBool::new(false)),
            max_batch_size: 50,
        }
    }

    /// Record a scene event for streaming
    pub fn record_scene_event(
        &self,
        event_type: &str,
        scene_id: &str,
        entity_id: Option<u64>,
        data: serde_json::Value,
    ) {
        let event = SceneEventPayload {
            event_type: event_type.to_string(),
            scene_id: scene_id.to_string(),
            entity_id,
            timestamp: chrono::Utc::now().to_rfc3339(),
            data,
        };

        let mut events = self.pending_events.lock().expect("mutex poisoned");
        events.push(event);

        if events.len() > self.max_batch_size * 2 {
            events.drain(0..self.max_batch_size);
            warn!("Scene event buffer overflow, dropping oldest events");
        }

        debug!(
            "Recorded scene event: {} for scene {}",
            event_type, scene_id
        );
    }

    /// Record an entity event
    pub fn record_entity_event(
        &self,
        action: &str,
        scene_id: &str,
        entity_id: u64,
        entity_name: &str,
    ) {
        let payload = EntityEventPayload {
            action: action.to_string(),
            scene_id: scene_id.to_string(),
            entity_id,
            entity_name: entity_name.to_string(),
            component_types: Vec::new(),
            position: None,
            timestamp: chrono::Utc::now().to_rfc3339(),
        };

        self.record_scene_event(
            &format!("entity:{}", action),
            scene_id,
            Some(entity_id),
            serde_json::to_value(&payload).unwrap_or_default(),
        );
    }

    /// Take pending events for sending
    pub fn take_pending_events(&self) -> Vec<SceneEventPayload> {
        let mut events = self.pending_events.lock().expect("mutex poisoned");
        if events.is_empty() {
            return Vec::new();
        }

        std::mem::take(&mut *events)
    }

    /// Build a scene sync response
    pub fn build_sync_response(&self, scene_id: &str) -> Result<SceneSyncResponsePayload> {
        let db = self.db.lock().expect("mutex poisoned");
        let entities = db.get_scene_entities(scene_id);

        Ok(SceneSyncResponsePayload {
            request_id: uuid::Uuid::new_v4().to_string(),
            scene_id: scene_id.to_string(),
            entity_count: entities.len(),
            entities: serde_json::to_value(&entities).unwrap_or_default(),
            version: 0,
            timestamp: chrono::Utc::now().to_rfc3339(),
        })
    }

    pub fn pending_count(&self) -> usize {
        self.pending_events.lock().expect("mutex poisoned").len()
    }

    pub fn set_streaming(&self, active: bool) {
        self.is_streaming.store(active, Ordering::SeqCst);
    }

    pub fn is_streaming(&self) -> bool {
        self.is_streaming.load(Ordering::SeqCst)
    }
}

// ========================================
// Realtime Bridge
// ========================================

/// Central real-time communication bridge
#[allow(dead_code)]
pub struct RealtimeBridge {
    config: BridgeConfig,
    ws_client: Option<MulticaWebSocketClient>,
    skill_system: Arc<SkillSystem>,
    scene_agent: Option<Arc<SceneAgent>>,
    event_streamer: Arc<SceneEventStreamer>,
    status: RealtimeStatus,
    stats: RealtimeStats,
    is_running: Arc<AtomicBool>,
    message_queue: Arc<MessageQueue>,
    connection_monitor: Arc<ConnectionMonitor>,
    sync_interval: Duration,
    last_sync: Instant,
}

impl RealtimeBridge {
    pub fn new(
        config: BridgeConfig,
        skill_system: SkillSystem,
        event_streamer: SceneEventStreamer,
    ) -> Self {
        let skill_system = Arc::new(skill_system);
        let event_streamer = Arc::new(event_streamer);
        let message_queue = Arc::new(MessageQueue::new(1000));
        let connection_monitor = Arc::new(ConnectionMonitor::new(10));

        Self {
            config: config.clone(),
            ws_client: Some(MulticaWebSocketClient::new(config)),
            skill_system,
            scene_agent: None,
            event_streamer,
            status: RealtimeStatus::Disconnected,
            stats: RealtimeStats::default(),
            is_running: Arc::new(AtomicBool::new(false)),
            message_queue,
            connection_monitor,
            sync_interval: Duration::from_secs(5),
            last_sync: Instant::now(),
        }
    }

    pub fn with_scene_agent(mut self, agent: SceneAgent) -> Self {
        self.scene_agent = Some(Arc::new(agent));
        self
    }

    /// Start the real-time bridge
    pub async fn start(&mut self) -> Result<()> {
        info!("Starting real-time bridge...");
        self.is_running.store(true, Ordering::SeqCst);
        self.status = RealtimeStatus::Connecting;

        if let Some(ref mut client) = self.ws_client {
            match client.connect().await {
                Ok(()) => {
                    self.status = RealtimeStatus::Connected;
                    self.event_streamer.set_streaming(true);
                    info!("Real-time bridge connected");

                    client.register_daemon().await?;

                    let events = self.event_streamer.take_pending_events();
                    let count = events.len();
                    for event in &events {
                        let msg = Message {
                            message_type: EVENT_SCENE_EVENT.to_string(),
                            payload: serde_json::to_value(event)?,
                        };
                        client.send_message(msg).await?;
                    }
                    self.stats.scene_events_sent += count as u64;
                }
                Err(e) => {
                    self.status = RealtimeStatus::Error(e.to_string());
                    warn!("Real-time bridge connection failed: {}", e);
                }
            }
        }

        Ok(())
    }

    /// Stop the real-time bridge
    pub async fn stop(&mut self) -> Result<()> {
        info!("Stopping real-time bridge...");
        self.is_running.store(false, Ordering::SeqCst);
        self.event_streamer.set_streaming(false);

        if let Some(ref mut client) = self.ws_client {
            client.disconnect().await?;
        }

        self.status = RealtimeStatus::Disconnected;
        Ok(())
    }

    /// Dispatch a skill execution request from the server
    pub fn dispatch_skill(&self, payload: SkillDispatchPayload) -> SkillExecResult {
        let ctx = SkillExecutionContext {
            scene_id: payload.scene_id.clone(),
            entity_ids: payload.entity_ids.clone(),
            ..Default::default()
        };

        match self
            .skill_system
            .execute(&payload.skill_name, &payload.parameters, Some(&ctx))
        {
            Ok(result) => result,
            Err(e) => SkillExecResult {
                skill_name: payload.skill_name.clone(),
                success: false,
                output: None,
                error: Some(e.to_string()),
                data: None,
                entity_ids_affected: Vec::new(),
                category: "error".to_string(),
                duration_ms: 0,
            },
        }
    }

    /// Execute a skill and send the result back to the server
    pub async fn execute_and_respond(&mut self, payload: SkillDispatchPayload) -> Result<()> {
        let result = self.dispatch_skill(payload.clone());

        let mut result_payload = SkillResultPayload::from(result);
        result_payload.request_id = payload.request_id;

        let message = Message {
            message_type: EVENT_SKILL_RESULT.to_string(),
            payload: serde_json::to_value(&result_payload)?,
        };

        if let Some(ref mut client) = self.ws_client {
            client.send_message(message).await?;
        }

        self.stats.skills_dispatched += 1;
        if result_payload.success {
            self.stats.skills_completed += 1;
        } else {
            self.stats.skills_failed += 1;
        }
        self.stats.last_activity = Some(chrono::Utc::now().to_rfc3339());

        Ok(())
    }

    /// Send a skill list response
    pub async fn send_skill_list(&mut self, request_id: &str) -> Result<()> {
        let skills = self.skill_system.list_skills();

        let message = Message {
            message_type: EVENT_SKILL_LIST_RESPONSE.to_string(),
            payload: serde_json::json!({
                "request_id": request_id,
                "skills": skills,
                "count": skills.len()
            }),
        };

        if let Some(ref mut client) = self.ws_client {
            client.send_message(message).await?;
        }

        Ok(())
    }

    /// Send a scene sync response
    pub async fn send_scene_sync(&mut self, request: SceneSyncRequestPayload) -> Result<()> {
        let response = self.event_streamer.build_sync_response(&request.scene_id)?;

        let message = Message {
            message_type: EVENT_SCENE_SYNC_RESPONSE.to_string(),
            payload: serde_json::to_value(&response)?,
        };

        if let Some(ref mut client) = self.ws_client {
            client.send_message(message).await?;
        }

        self.stats.syncs_performed += 1;
        Ok(())
    }

    /// Record an entity creation event for streaming
    pub fn on_entity_created(&self, scene_id: &str, entity_id: u64, entity_name: &str) {
        self.event_streamer
            .record_entity_event("created", scene_id, entity_id, entity_name);
    }

    /// Record an entity update event for streaming
    pub fn on_entity_updated(&self, scene_id: &str, entity_id: u64, entity_name: &str) {
        self.event_streamer
            .record_entity_event("updated", scene_id, entity_id, entity_name);
    }

    /// Record an entity deletion event for streaming
    pub fn on_entity_deleted(&self, scene_id: &str, entity_id: u64, entity_name: &str) {
        self.event_streamer
            .record_entity_event("deleted", scene_id, entity_id, entity_name);
    }

    /// Record a scene event for streaming
    pub fn on_scene_event(&self, event_type: &str, scene_id: &str, data: serde_json::Value) {
        self.event_streamer
            .record_scene_event(event_type, scene_id, None, data);
    }

    /// Handle an incoming message from the server
    pub async fn handle_incoming(&mut self, message: &Message) -> Result<Option<Message>> {
        self.stats.messages_received += 1;
        self.stats.last_activity = Some(chrono::Utc::now().to_rfc3339());

        match message.message_type.as_str() {
            EVENT_SKILL_DISPATCH => {
                let payload: SkillDispatchPayload =
                    serde_json::from_value(message.payload.clone())?;
                self.execute_and_respond(payload).await?;
                Ok(None)
            }
            EVENT_SKILL_LIST_REQUEST => {
                let request_id = message.payload["request_id"].as_str().unwrap_or("unknown");
                self.send_skill_list(request_id).await?;
                Ok(None)
            }
            EVENT_SCENE_SYNC_REQUEST => {
                let request: SceneSyncRequestPayload =
                    serde_json::from_value(message.payload.clone())?;
                self.send_scene_sync(request).await?;
                Ok(None)
            }
            _ => {
                debug!("Unhandled message type: {}", message.message_type);
                Ok(Some(message.clone()))
            }
        }
    }

    /// Periodic sync of scene events
    pub async fn maybe_sync(&mut self) {
        if self.last_sync.elapsed() < self.sync_interval {
            return;
        }

        self.last_sync = Instant::now();

        let events = self.event_streamer.take_pending_events();
        if events.is_empty() {
            return;
        }

        if let Some(ref mut client) = self.ws_client {
            if client.is_connected() {
                let count = events.len();
                for event in &events {
                    let msg = Message {
                        message_type: EVENT_SCENE_EVENT.to_string(),
                        payload: serde_json::to_value(event).unwrap_or_default(),
                    };
                    if let Err(e) = client.send_message(msg).await {
                        warn!("Failed to send scene event: {}", e);
                    }
                }
                self.stats.scene_events_sent += count as u64;
            }
        }
    }

    /// Check if connected
    pub fn is_connected(&self) -> bool {
        self.ws_client
            .as_ref()
            .map(|c| c.is_connected())
            .unwrap_or(false)
    }

    pub fn status(&self) -> &RealtimeStatus {
        &self.status
    }

    pub fn stats(&self) -> &RealtimeStats {
        &self.stats
    }

    pub fn skill_system(&self) -> &Arc<SkillSystem> {
        &self.skill_system
    }

    pub fn scene_agent(&self) -> Option<&Arc<SceneAgent>> {
        self.scene_agent.as_ref()
    }

    pub fn event_streamer(&self) -> &Arc<SceneEventStreamer> {
        &self.event_streamer
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

    fn setup() -> (RealtimeBridge, Arc<SkillSystem>) {
        let db = create_shared_multica_db();
        let ctx = create_shared_scene_context();
        let skill_system = SkillSystem::new(db.clone(), ctx.clone());
        let event_streamer = SceneEventStreamer::new(db, ctx.clone());
        let config = BridgeConfig {
            auto_reconnect: false,
            ..BridgeConfig::default()
        };

        let bridge = RealtimeBridge::new(config, skill_system, event_streamer);
        let skill_system = bridge.skill_system().clone();
        (bridge, skill_system)
    }

    #[test]
    fn test_bridge_initial_state() {
        let (bridge, _) = setup();
        assert_eq!(bridge.status, RealtimeStatus::Disconnected);
        assert!(!bridge.is_connected());
        assert_eq!(bridge.stats.skills_dispatched, 0);
    }

    #[test]
    fn test_skill_dispatch_payload_serde() {
        let payload = SkillDispatchPayload {
            request_id: "req-1".into(),
            skill_name: "entity.create".into(),
            parameters: serde_json::json!({"name": "Test"}),
            scene_id: Some("scene-1".into()),
            entity_ids: vec![1, 2],
            timeout_ms: Some(5000),
        };

        let json = serde_json::to_string(&payload).unwrap();
        let parsed: SkillDispatchPayload = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.request_id, "req-1");
        assert_eq!(parsed.skill_name, "entity.create");
    }

    #[test]
    fn test_skill_result_from_exec_result() {
        let result = SkillExecResult {
            skill_name: "test".into(),
            success: true,
            output: Some("ok".into()),
            error: None,
            data: Some(serde_json::json!({"key": "val"})),
            entity_ids_affected: vec![1, 2],
            category: "entity".into(),
            duration_ms: 42,
        };

        let payload = SkillResultPayload::from(result);
        assert!(payload.success);
        assert_eq!(payload.skill_name, "test");
        assert_eq!(payload.entity_ids_affected, vec![1, 2]);
        assert_eq!(payload.duration_ms, 42);
    }

    #[test]
    fn test_event_streamer_record_and_take() {
        let db = create_shared_multica_db();
        let ctx = create_shared_scene_context();
        let streamer = SceneEventStreamer::new(db, ctx);

        assert_eq!(streamer.pending_count(), 0);

        streamer.record_scene_event("test", "scene-1", Some(1), serde_json::json!({"a": 1}));
        streamer.record_scene_event("test", "scene-1", Some(2), serde_json::json!({"b": 2}));
        assert_eq!(streamer.pending_count(), 2);

        let events = streamer.take_pending_events();
        assert_eq!(events.len(), 2);
        assert_eq!(streamer.pending_count(), 0);
    }

    #[test]
    fn test_event_streamer_entity_event() {
        let db = create_shared_multica_db();
        let ctx = create_shared_scene_context();
        let streamer = SceneEventStreamer::new(db, ctx);

        streamer.record_entity_event("created", "scene-1", 42, "Player");
        assert_eq!(streamer.pending_count(), 1);

        let events = streamer.take_pending_events();
        assert_eq!(events[0].event_type, "entity:created");
        assert_eq!(events[0].entity_id, Some(42));
    }

    #[test]
    fn test_event_streamer_streaming_flag() {
        let db = create_shared_multica_db();
        let ctx = create_shared_scene_context();
        let streamer = SceneEventStreamer::new(db, ctx);

        assert!(!streamer.is_streaming());
        streamer.set_streaming(true);
        assert!(streamer.is_streaming());
    }

    #[test]
    fn test_dispatch_skill_locally() {
        let (bridge, _) = setup();
        let payload = SkillDispatchPayload {
            request_id: "test-req".into(),
            skill_name: "entity.query".into(),
            parameters: serde_json::json!({}),
            scene_id: None,
            entity_ids: vec![],
            timeout_ms: None,
        };

        let result = bridge.dispatch_skill(payload);
        assert!(result.success);
        assert_eq!(result.skill_name, "entity.query");
    }

    #[test]
    fn test_dispatch_skill_create_entity() {
        let (bridge, _) = setup();
        let payload = SkillDispatchPayload {
            request_id: "test-req".into(),
            skill_name: "entity.create".into(),
            parameters: serde_json::json!({"name": "TestSprite", "position": [1.0, 2.0, 3.0]}),
            scene_id: None,
            entity_ids: vec![],
            timeout_ms: None,
        };

        let result = bridge.dispatch_skill(payload);
        assert!(result.success);
        assert!(result.data.is_some());
        let data = result.data.unwrap();
        assert_eq!(data["name"], "TestSprite");
    }

    #[test]
    fn test_event_constants() {
        assert_eq!(EVENT_SKILL_DISPATCH, "skill:dispatch");
        assert_eq!(EVENT_SKILL_RESULT, "skill:result");
        assert_eq!(EVENT_SKILL_LIST_REQUEST, "skill:list_request");
        assert_eq!(EVENT_SKILL_LIST_RESPONSE, "skill:list_response");
        assert_eq!(EVENT_SCENE_SYNC_REQUEST, "scene:sync_request");
        assert_eq!(EVENT_SCENE_SYNC_RESPONSE, "scene:sync_response");
        assert_eq!(EVENT_SCENE_EVENT, "scene:event");
        assert_eq!(EVENT_ENTITY_EVENT, "entity:event");
    }

    #[test]
    fn test_scene_sync_request_serde() {
        let request = SceneSyncRequestPayload {
            request_id: "sync-1".into(),
            scene_id: "scene-a".into(),
            full_sync: true,
            since_version: None,
        };

        let json = serde_json::to_string(&request).unwrap();
        let parsed: SceneSyncRequestPayload = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.scene_id, "scene-a");
        assert!(parsed.full_sync);
    }
}
