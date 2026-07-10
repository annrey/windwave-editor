//! Multica Daemon
//!
//! 后台守护进程，负责与 Multica 服务器的持续连接和任务同步
//! 实现心跳机制、状态报告、自动重连、消息队列和消息处理

use crate::error::{BridgeError, Result};
use crate::message_handler::{
    create_shared_message_handler_with_sync, DefaultMessageHandler, MessageHandler,
};
use crate::realtime_bridge::{
    RealtimeBridge, SceneEventStreamer, SkillDispatchPayload, EVENT_SCENE_EVENT,
    EVENT_SCENE_SYNC_REQUEST, EVENT_SKILL_DISPATCH, EVENT_SKILL_LIST_REQUEST,
};
use crate::task_sync::TaskSync;
use crate::task_sync_module::TaskSynchronizer;
use crate::types::{
    BridgeConfig, DaemonHeartbeatRequestPayload, DaemonRegisterPayload, Message, RuntimeInfo,
    EVENT_DAEMON_HEARTBEAT, EVENT_DAEMON_REGISTER,
};
use crate::ws_client::MulticaWebSocketClient;
use log::{debug, error, info, warn};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::mpsc;

/// 守护进程状态
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DaemonStatus {
    /// 初始化中
    Initializing,
    /// 连接中
    Connecting,
    /// 已连接
    Connected,
    /// 断开连接
    Disconnected,
    /// 错误状态
    Error(String),
}

/// 守护进程统计信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonStats {
    /// 启动时间
    pub start_time: String,
    /// 总心跳数
    pub heartbeat_count: u64,
    /// 总重连次数
    pub reconnect_count: u64,
    /// 总消息发送数
    pub messages_sent: u64,
    /// 总消息接收数
    pub messages_received: u64,
    /// 最后心跳时间
    pub last_heartbeat: Option<String>,
    /// 最后错误信息
    pub last_error: Option<String>,
}

impl Default for DaemonStats {
    fn default() -> Self {
        Self {
            start_time: chrono::Utc::now().to_rfc3339(),
            heartbeat_count: 0,
            reconnect_count: 0,
            messages_sent: 0,
            messages_received: 0,
            last_heartbeat: None,
            last_error: None,
        }
    }
}

/// Multica 守护进程
pub struct MulticaDaemon {
    /// 配置
    pub config: BridgeConfig,
    /// WebSocket 客户端
    pub ws_client: MulticaWebSocketClient,
    /// 任务同步管理器
    pub task_sync: TaskSync,
    /// 消息处理器
    pub message_handler: Arc<DefaultMessageHandler>,
    /// 任务同步器
    pub task_synchronizer: Arc<TaskSynchronizer>,
    /// 实时通信桥（可选）
    pub realtime_bridge: Option<RealtimeBridge>,
    /// 场景事件流发送器
    pub event_streamer: Option<Arc<SceneEventStreamer>>,
    /// 守护进程状态
    pub status: DaemonStatus,
    /// 统计信息
    pub stats: DaemonStats,
    /// 是否正在运行
    pub is_running: bool,
    /// 心跳间隔
    pub heartbeat_interval: Duration,
    /// 场景事件同步间隔
    pub scene_sync_interval: Duration,
    /// 运行标志（用于控制循环）
    pub running_flag: Arc<AtomicBool>,
    /// 消息接收通道
    message_tx: Option<mpsc::Sender<Message>>,
    message_rx: Option<mpsc::Receiver<Message>>,
}

impl MulticaDaemon {
    /// 创建新的守护进程
    pub fn new(config: BridgeConfig) -> Self {
        let ws_client = MulticaWebSocketClient::new(config.clone());
        let task_sync = TaskSync::new(config.clone());
        let task_synchronizer = Arc::new(crate::task_sync_module::TaskSynchronizer::new(
            crate::task_sync_module::TaskSyncConfig::default(),
        ));
        let message_handler = create_shared_message_handler_with_sync(task_synchronizer.clone());

        let (tx, rx) = mpsc::channel(1000);

        Self {
            config,
            ws_client,
            task_sync,
            message_handler,
            task_synchronizer,
            realtime_bridge: None,
            event_streamer: None,
            status: DaemonStatus::Initializing,
            stats: DaemonStats::default(),
            is_running: false,
            heartbeat_interval: Duration::from_secs(30),
            scene_sync_interval: Duration::from_secs(5),
            running_flag: Arc::new(AtomicBool::new(false)),
            message_tx: Some(tx),
            message_rx: Some(rx),
        }
    }

    /// Attach a realtime bridge for skill dispatch and scene event streaming
    pub fn with_realtime_bridge(
        mut self,
        bridge: RealtimeBridge,
        streamer: Arc<SceneEventStreamer>,
    ) -> Self {
        self.realtime_bridge = Some(bridge);
        self.event_streamer = Some(streamer);
        self
    }

    /// 启动守护进程
    pub async fn start(&mut self) -> Result<()> {
        info!("Starting Multica daemon...");
        self.is_running = true;
        self.running_flag.store(true, Ordering::SeqCst);
        self.status = DaemonStatus::Connecting;

        // 尝试连接服务器
        match self.ws_client.connect().await {
            Ok(_) => {
                info!("Connected to Multica server");
                self.status = DaemonStatus::Connected;

                // 注册守护进程
                self.register_daemon().await?;

                // 启动实时通信桥
                if let Some(ref mut bridge) = self.realtime_bridge {
                    if let Err(e) = bridge.start().await {
                        warn!("Realtime bridge start failed: {}", e);
                    } else {
                        info!("Realtime bridge started");
                    }
                }

                // 启动消息接收循环
                self.start_message_loop();

                // 启动心跳循环
                self.start_heartbeat().await;

                // 启动场景事件同步循环
                self.start_scene_event_loop();

                Ok(())
            }
            Err(e) => {
                error!("Failed to connect to Multica server: {}", e);
                self.status = DaemonStatus::Error(e.to_string());
                self.stats.last_error = Some(e.to_string());

                // 尝试自动重连
                if self.config.auto_reconnect {
                    self.handle_reconnect().await
                } else {
                    Err(BridgeError::Other(
                        "Connection failed and auto-reconnect is disabled".to_string(),
                    ))
                }
            }
        }
    }

    /// 注册守护进程到服务器
    async fn register_daemon(&mut self) -> Result<()> {
        let payload = DaemonRegisterPayload {
            daemon_id: self.config.daemon_id.clone(),
            agent_id: self.config.agent_id.clone(),
            runtimes: vec![RuntimeInfo {
                runtime_type: "WindWave".to_string(),
                version: "0.1.0".to_string(),
                status: "ready".to_string(),
            }],
        };

        let message = Message {
            message_type: EVENT_DAEMON_REGISTER.to_string(),
            payload: serde_json::to_value(payload)?,
        };

        self.ws_client.send_message(message).await?;
        self.stats.messages_sent += 1;
        info!("Daemon registered successfully");

        Ok(())
    }

    /// 启动心跳循环
    ///
    /// Spawns a background tokio task that periodically sends heartbeat messages
    /// and updates the daemon stats.
    async fn start_heartbeat(&mut self) {
        info!(
            "Starting heartbeat loop (interval: {:?})",
            self.heartbeat_interval
        );
        let running_flag = self.running_flag.clone();
        let heartbeat_interval = self.heartbeat_interval;
        let message_tx = self.message_tx.clone();

        tokio::spawn(async move {
            info!("Heartbeat loop task started");
            while running_flag.load(Ordering::SeqCst) {
                tokio::time::sleep(heartbeat_interval).await;

                if !running_flag.load(Ordering::SeqCst) {
                    break;
                }

                let payload = DaemonHeartbeatRequestPayload {
                    runtime_id: String::new(),
                    supports_batch_import: Some(true),
                };

                let heartbeat_msg = Message {
                    message_type: EVENT_DAEMON_HEARTBEAT.to_string(),
                    payload: serde_json::to_value(&payload).unwrap_or_default(),
                };

                if let Some(ref tx) = message_tx {
                    if tx.send(heartbeat_msg).await.is_err() {
                        warn!("Heartbeat channel closed, stopping heartbeat loop");
                        break;
                    }
                }
            }
            info!("Heartbeat loop task stopped");
        });
    }

    /// 启动消息接收循环
    ///
    /// Spawns a background tokio task that receives messages from the channel
    /// and dispatches them to the message handler for processing.
    fn start_message_loop(&mut self) {
        info!("Starting message receive loop");
        let running_flag = self.running_flag.clone();
        let message_rx = self.message_rx.take();
        let handler = self.message_handler.clone();

        if let Some(mut rx) = message_rx {
            let handler_clone = handler.clone();
            tokio::spawn(async move {
                info!("Message receive loop task started");
                while running_flag.load(Ordering::SeqCst) {
                    match rx.recv().await {
                        Some(message) => {
                            debug!("Received message: type={}", message.message_type);
                            match message.message_type.as_str() {
                                EVENT_SKILL_DISPATCH
                                | EVENT_SKILL_LIST_REQUEST
                                | EVENT_SCENE_SYNC_REQUEST => {
                                    debug!("Routing skill/scene message to handler");
                                    let _ = handler_clone.handle_message(&message);
                                }
                                _ => {
                                    let _ = handler_clone.handle_message(&message);
                                }
                            }
                        }
                        None => {
                            info!("Message channel closed, stopping receive loop");
                            break;
                        }
                    }
                }
                info!("Message receive loop task stopped");
            });
        } else {
            warn!("No message receiver available, skipping message loop");
        }
    }

    /// 启动场景事件同步循环
    ///
    /// Spawns a background tokio task that periodically flushes pending
    /// scene events to the Multica server.
    fn start_scene_event_loop(&mut self) {
        let streamer = self.event_streamer.clone();
        let message_tx = self.message_tx.clone();
        let running_flag = self.running_flag.clone();
        let sync_interval = self.scene_sync_interval;

        if let Some(ref streamer) = streamer {
            let streamer = streamer.clone();
            tokio::spawn(async move {
                info!(
                    "Scene event sync loop started (interval: {:?})",
                    sync_interval
                );
                while running_flag.load(Ordering::SeqCst) {
                    tokio::time::sleep(sync_interval).await;

                    if !running_flag.load(Ordering::SeqCst) {
                        break;
                    }

                    let events = streamer.take_pending_events();
                    if events.is_empty() {
                        continue;
                    }

                    debug!("Syncing {} scene events", events.len());
                    for event in events {
                        let msg = Message {
                            message_type: EVENT_SCENE_EVENT.to_string(),
                            payload: serde_json::to_value(&event).unwrap_or_default(),
                        };
                        if let Some(ref tx) = message_tx {
                            if tx.send(msg).await.is_err() {
                                warn!("Scene event channel closed");
                                break;
                            }
                        }
                    }
                }
                info!("Scene event sync loop stopped");
            });
        } else {
            debug!("No event streamer configured, skipping scene event loop");
        }
    }

    /// Handle a skill dispatch message via the realtime bridge
    pub async fn handle_skill_dispatch(&mut self, payload: SkillDispatchPayload) -> Result<()> {
        if let Some(ref mut bridge) = self.realtime_bridge {
            bridge.execute_and_respond(payload).await?;
            self.stats.messages_sent += 1;
        } else {
            warn!("Skill dispatch received but no realtime bridge configured");
        }
        Ok(())
    }

    /// Record a scene event for streaming
    pub fn record_scene_event(&self, event_type: &str, scene_id: &str, data: serde_json::Value) {
        if let Some(ref streamer) = self.event_streamer {
            streamer.record_scene_event(event_type, scene_id, None, data);
        }
    }

    /// Record an entity event for streaming
    pub fn record_entity_event(
        &self,
        action: &str,
        scene_id: &str,
        entity_id: u64,
        entity_name: &str,
    ) {
        if let Some(ref streamer) = self.event_streamer {
            streamer.record_entity_event(action, scene_id, entity_id, entity_name);
        }
    }

    /// 发送心跳
    #[allow(dead_code)]
    async fn send_heartbeat(&mut self) -> Result<()> {
        let payload = DaemonHeartbeatRequestPayload {
            runtime_id: self.config.daemon_id.clone(),
            supports_batch_import: Some(true),
        };

        let message = Message {
            message_type: EVENT_DAEMON_HEARTBEAT.to_string(),
            payload: serde_json::to_value(payload)?,
        };

        self.ws_client.send_message(message).await?;
        self.stats.messages_sent += 1;

        Ok(())
    }

    /// 处理重连
    async fn handle_reconnect(&mut self) -> Result<()> {
        info!("Attempting to reconnect...");
        self.stats.reconnect_count += 1;

        // 等待重连间隔
        tokio::time::sleep(std::time::Duration::from_secs(self.config.reconnect_delay)).await;

        match self.ws_client.connect().await {
            Ok(_) => {
                info!("Reconnected successfully");
                self.status = DaemonStatus::Connected;
                self.register_daemon().await
            }
            Err(e) => {
                error!("Reconnection failed: {}", e);
                self.status = DaemonStatus::Error(e.to_string());
                self.stats.last_error = Some(e.to_string());
                Err(e)
            }
        }
    }

    /// 停止守护进程
    pub async fn stop(&mut self) -> Result<()> {
        info!("Stopping Multica daemon...");
        self.is_running = false;
        self.status = DaemonStatus::Disconnected;

        if let Some(ref mut bridge) = self.realtime_bridge {
            if let Err(e) = bridge.stop().await {
                warn!("Realtime bridge stop failed: {}", e);
            }
        }

        self.ws_client.disconnect().await?;
        info!("Daemon stopped successfully");

        Ok(())
    }

    /// 获取守护进程状态
    pub fn get_status(&self) -> &DaemonStatus {
        &self.status
    }

    /// 获取统计信息
    pub fn get_stats(&self) -> &DaemonStats {
        &self.stats
    }

    /// 检查是否正在运行
    pub fn is_running(&self) -> bool {
        self.is_running
    }
}

/// 线程安全的守护进程
pub type SharedMulticaDaemon = Arc<Mutex<MulticaDaemon>>;

/// 创建线程安全的守护进程
pub fn create_shared_daemon(config: BridgeConfig) -> SharedMulticaDaemon {
    Arc::new(Mutex::new(MulticaDaemon::new(config)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_daemon_creation() {
        let config = BridgeConfig::default();
        let daemon = MulticaDaemon::new(config.clone());

        assert_eq!(daemon.config.daemon_id, config.daemon_id);
        assert!(!daemon.is_running);
        assert_eq!(daemon.status, DaemonStatus::Initializing);
    }

    #[test]
    fn test_daemon_stats() {
        let config = BridgeConfig::default();
        let daemon = MulticaDaemon::new(config);

        let stats = daemon.get_stats();
        assert_eq!(stats.heartbeat_count, 0);
        assert_eq!(stats.reconnect_count, 0);
        assert_eq!(stats.messages_sent, 0);
        assert!(stats.last_heartbeat.is_none());
    }

    #[test]
    fn test_daemon_status_transitions() {
        let config = BridgeConfig::default();
        let mut daemon = MulticaDaemon::new(config);

        assert_eq!(daemon.status, DaemonStatus::Initializing);
        daemon.status = DaemonStatus::Connecting;
        assert_eq!(daemon.status, DaemonStatus::Connecting);
        daemon.status = DaemonStatus::Connected;
        assert_eq!(daemon.status, DaemonStatus::Connected);
        daemon.status = DaemonStatus::Disconnected;
        assert_eq!(daemon.status, DaemonStatus::Disconnected);
    }

    #[test]
    fn test_daemon_error_status() {
        let config = BridgeConfig::default();
        let mut daemon = MulticaDaemon::new(config);

        daemon.status = DaemonStatus::Error("test error".to_string());
        assert_eq!(daemon.status, DaemonStatus::Error("test error".to_string()));
    }
}
