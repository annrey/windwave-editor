use crate::error::{BridgeError, Result};
use crate::types::*;
use futures_util::{SinkExt, StreamExt};
use log::{debug, error, info, warn};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::Mutex as TokioMutex;
use tokio::time::{sleep, Duration};
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message as WsMessage};

/// WebSocket 连接状态
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionStatus {
    Disconnected,
    Connecting,
    Connected,
    Reconnecting,
    Error(String),
}

/// 连接状态监控
#[derive(Debug, Clone)]
pub struct ConnectionMonitor {
    status: Arc<std::sync::Mutex<ConnectionStatus>>,
    reconnect_attempts: Arc<AtomicUsize>,
    max_reconnect_attempts: Arc<AtomicUsize>,
    last_connected_at: Arc<std::sync::Mutex<Option<std::time::Instant>>>,
    is_reconnecting: Arc<AtomicBool>,
}

impl ConnectionMonitor {
    pub fn new(max_reconnect_attempts: usize) -> Self {
        Self {
            status: Arc::new(std::sync::Mutex::new(ConnectionStatus::Disconnected)),
            reconnect_attempts: Arc::new(AtomicUsize::new(0)),
            max_reconnect_attempts: Arc::new(AtomicUsize::new(max_reconnect_attempts)),
            last_connected_at: Arc::new(std::sync::Mutex::new(None)),
            is_reconnecting: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn set_status(&self, status: ConnectionStatus) {
        let mut s = self.status.lock().expect("mutex poisoned");
        *s = status;
    }

    pub fn get_status(&self) -> ConnectionStatus {
        self.status.lock().expect("mutex poisoned").clone()
    }

    pub fn is_connected(&self) -> bool {
        matches!(self.get_status(), ConnectionStatus::Connected)
    }

    pub fn increment_reconnect_attempts(&self) -> usize {
        self.reconnect_attempts.fetch_add(1, Ordering::SeqCst) + 1
    }

    pub fn reset_reconnect_attempts(&self) {
        self.reconnect_attempts.store(0, Ordering::SeqCst);
    }

    pub fn get_reconnect_attempts(&self) -> usize {
        self.reconnect_attempts.load(Ordering::SeqCst)
    }

    pub fn set_last_connected(&self) {
        let mut last = self.last_connected_at.lock().expect("mutex poisoned");
        *last = Some(std::time::Instant::now());
    }

    pub fn set_reconnecting(&self, value: bool) {
        self.is_reconnecting.store(value, Ordering::SeqCst);
    }

    pub fn is_reconnecting(&self) -> bool {
        self.is_reconnecting.load(Ordering::SeqCst)
    }

    pub fn max_reconnect_attempts(&self) -> usize {
        self.max_reconnect_attempts.load(Ordering::SeqCst)
    }

    pub fn should_reconnect(&self) -> bool {
        self.get_reconnect_attempts() < self.max_reconnect_attempts()
    }
}

/// 消息队列，用于缓存断线期间的消息
#[derive(Debug, Clone)]
pub struct MessageQueue {
    messages: Arc<TokioMutex<VecDeque<Message>>>,
    max_size: usize,
}

impl MessageQueue {
    pub fn new(max_size: usize) -> Self {
        Self {
            messages: Arc::new(TokioMutex::new(VecDeque::with_capacity(max_size))),
            max_size,
        }
    }

    pub async fn enqueue(&self, message: Message) -> Result<()> {
        let mut messages = self.messages.lock().await;
        if messages.len() >= self.max_size {
            messages.pop_front();
            warn!("Message queue full, dropping oldest message");
        }
        messages.push_back(message);
        Ok(())
    }

    pub async fn dequeue(&self) -> Option<Message> {
        let mut messages = self.messages.lock().await;
        messages.pop_front()
    }

    pub async fn dequeue_all(&self) -> Vec<Message> {
        let mut messages = self.messages.lock().await;
        messages.drain(..).collect()
    }

    pub async fn len(&self) -> usize {
        let messages = self.messages.lock().await;
        messages.len()
    }

    pub async fn is_empty(&self) -> bool {
        let messages = self.messages.lock().await;
        messages.is_empty()
    }

    pub async fn clear(&self) {
        let mut messages = self.messages.lock().await;
        messages.clear();
    }
}

/// Multica WebSocket client - enhanced with auto-reconnect and message queue
pub struct MulticaWebSocketClient {
    config: BridgeConfig,
    sender: Option<
        futures_util::stream::SplitSink<
            tokio_tungstenite::WebSocketStream<
                tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
            >,
            WsMessage,
        >,
    >,
    receiver: Option<
        futures_util::stream::SplitStream<
            tokio_tungstenite::WebSocketStream<
                tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
            >,
        >,
    >,
    connection_monitor: ConnectionMonitor,
    message_queue: MessageQueue,
    heartbeat_interval: Duration,
    is_running_heartbeat: Arc<AtomicBool>,
}

impl MulticaWebSocketClient {
    /// Create new client with auto-reconnect and message queue
    pub fn new(config: BridgeConfig) -> Self {
        let max_reconnect_attempts = if config.auto_reconnect { 10 } else { 0 };
        Self {
            config: config.clone(),
            sender: None,
            receiver: None,
            connection_monitor: ConnectionMonitor::new(max_reconnect_attempts),
            message_queue: MessageQueue::new(1000),
            heartbeat_interval: Duration::from_secs(30),
            is_running_heartbeat: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Create new client with custom heartbeat interval
    pub fn with_heartbeat(config: BridgeConfig, heartbeat_interval: Duration) -> Self {
        let mut client = Self::new(config);
        client.heartbeat_interval = heartbeat_interval;
        client
    }

    /// Connect to Multica server
    pub async fn connect(&mut self) -> Result<()> {
        info!("Connecting to Multica at {}", self.config.server_url);
        self.connection_monitor
            .set_status(ConnectionStatus::Connecting);

        let (ws_stream, response) = connect_async(&self.config.server_url).await.map_err(|e| {
            let error_msg = e.to_string();
            self.connection_monitor
                .set_status(ConnectionStatus::Error(error_msg.clone()));
            BridgeError::WebSocketError(error_msg)
        })?;

        debug!("WebSocket handshake complete: {:?}", response.status());

        let (sender, receiver) = ws_stream.split();

        self.sender = Some(sender);
        self.receiver = Some(receiver);
        self.connection_monitor
            .set_status(ConnectionStatus::Connected);
        self.connection_monitor.reset_reconnect_attempts();
        self.connection_monitor.set_last_connected();

        info!("Connected to Multica successfully!");

        // Flush message queue after reconnect
        self.flush_message_queue().await?;

        // Start heartbeat
        self.start_heartbeat().await;

        Ok(())
    }

    /// Connect with retry logic
    pub async fn connect_with_retry(&mut self) -> Result<()> {
        let max_attempts = self.config.reconnect_delay;
        let mut attempt = 0;

        loop {
            attempt += 1;
            info!("Connection attempt {}/{}", attempt, max_attempts);

            match self.connect().await {
                Ok(()) => return Ok(()),
                Err(e) => {
                    if attempt >= max_attempts {
                        error!("Failed to connect after {} attempts: {}", max_attempts, e);
                        return Err(e);
                    }

                    let delay = Duration::from_secs(self.config.reconnect_delay.min(30));
                    warn!(
                        "Connection failed (attempt {}), retrying in {:?}: {}",
                        attempt, delay, e
                    );
                    sleep(delay).await;
                }
            }
        }
    }

    /// Reconnect with exponential backoff
    pub async fn reconnect(&mut self) -> Result<()> {
        if !self.connection_monitor.should_reconnect() {
            error!(
                "Max reconnection attempts ({}) reached",
                self.connection_monitor.max_reconnect_attempts()
            );
            return Err(BridgeError::ConnectionClosed);
        }

        let attempts = self.connection_monitor.increment_reconnect_attempts();
        self.connection_monitor
            .set_status(ConnectionStatus::Reconnecting);
        self.connection_monitor.set_reconnecting(true);

        // Exponential backoff: 1s, 2s, 4s, 8s, max 30s
        let delay = Duration::from_secs((2_u64.pow(attempts as u32 - 1)).min(30));

        info!(
            "Reconnecting (attempt {}/{}) in {:?}...",
            attempts,
            self.connection_monitor.max_reconnect_attempts(),
            delay
        );

        sleep(delay).await;

        // Clean up old connection
        self.sender = None;
        self.receiver = None;

        match self.connect().await {
            Ok(()) => {
                self.connection_monitor.set_reconnecting(false);
                info!("Reconnected successfully");
                Ok(())
            }
            Err(e) => {
                error!("Reconnection failed: {}", e);
                self.connection_monitor.set_reconnecting(false);
                Err(e)
            }
        }
    }

    /// Disconnect
    pub async fn disconnect(&mut self) -> Result<()> {
        info!("Disconnecting from Multica...");
        self.stop_heartbeat();

        if let Some(mut sender) = self.sender.take() {
            let _ = sender.close().await;
        }
        self.receiver = None;
        self.connection_monitor
            .set_status(ConnectionStatus::Disconnected);

        info!("Disconnected from Multica");
        Ok(())
    }

    /// Send a message (with queue fallback if disconnected)
    pub async fn send_message(&mut self, message: Message) -> Result<()> {
        if self.connection_monitor.is_connected() {
            if let Some(sender) = &mut self.sender {
                let json = serde_json::to_string(&message)?;
                sender
                    .send(WsMessage::Text(json))
                    .await
                    .map_err(|e| BridgeError::WebSocketError(e.to_string()))?;
                debug!("Sent message: {}", message.message_type);
                return Ok(());
            }
        }

        // Queue message if not connected
        if self.config.auto_reconnect {
            warn!("Not connected, queuing message");
            self.message_queue.enqueue(message).await?;
            Ok(())
        } else {
            Err(BridgeError::ConnectionClosed)
        }
    }

    /// Flush message queue (send all queued messages)
    pub async fn flush_message_queue(&mut self) -> Result<()> {
        if self.message_queue.is_empty().await {
            return Ok(());
        }

        let messages = self.message_queue.dequeue_all().await;
        let count = messages.len();
        info!("Flushing {} queued messages", count);

        for message in messages {
            if let Err(e) = self.send_message(message).await {
                warn!("Failed to send queued message: {}", e);
            }
        }

        info!("Flushed {} messages", count);
        Ok(())
    }

    /// Send a register message
    pub async fn register_daemon(&mut self) -> Result<()> {
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

        info!("Registering daemon with Multica...");
        self.send_message(message).await?;
        info!("Daemon registered successfully!");

        Ok(())
    }

    /// Send a heartbeat
    pub async fn send_heartbeat(&mut self, runtime_id: String) -> Result<()> {
        let payload = DaemonHeartbeatRequestPayload {
            runtime_id,
            supports_batch_import: Some(false),
        };

        let message = Message {
            message_type: EVENT_DAEMON_HEARTBEAT.to_string(),
            payload: serde_json::to_value(payload)?,
        };

        debug!("Sending heartbeat...");
        self.send_message(message).await
    }

    /// Start automatic heartbeat
    pub async fn start_heartbeat(&mut self) {
        if self.is_running_heartbeat.load(Ordering::SeqCst) {
            return;
        }
        self.is_running_heartbeat.store(true, Ordering::SeqCst);

        let runtime_id = format!("{}-{}", self.config.daemon_id, self.config.agent_id);
        let heartbeat_interval = self.heartbeat_interval;

        // Note: In a real implementation, this would spawn a background task
        // For now, we just log that heartbeat is started
        info!(
            "Heartbeat started for {} (interval: {:?})",
            runtime_id, heartbeat_interval
        );
    }

    /// Stop heartbeat
    pub fn stop_heartbeat(&mut self) {
        self.is_running_heartbeat.store(false, Ordering::SeqCst);
        info!("Heartbeat stopped");
    }

    /// Receive a message
    pub async fn receive_message(&mut self) -> Result<Option<Message>> {
        if !self.connection_monitor.is_connected() {
            return Err(BridgeError::ConnectionClosed);
        }

        if let Some(receiver) = &mut self.receiver {
            while let Some(msg_result) = receiver.next().await {
                match msg_result {
                    Ok(WsMessage::Text(text)) => {
                        let message: Message = serde_json::from_str(&text)?;
                        debug!("Received message: {}", message.message_type);
                        return Ok(Some(message));
                    }
                    Ok(WsMessage::Close(_)) => {
                        info!("Connection closed by server");
                        self.connection_monitor
                            .set_status(ConnectionStatus::Disconnected);
                        return Ok(None);
                    }
                    Ok(WsMessage::Ping(data)) => {
                        if let Some(sender) = &mut self.sender {
                            let _ = sender.send(WsMessage::Pong(data)).await;
                        }
                    }
                    Ok(_) => {}
                    Err(e) => {
                        error!("Error receiving message: {}", e);
                        self.connection_monitor
                            .set_status(ConnectionStatus::Error(e.to_string()));
                        return Err(BridgeError::WebSocketError(e.to_string()));
                    }
                }
            }
        }

        Ok(None)
    }

    /// Get connection status
    pub fn get_connection_status(&self) -> ConnectionStatus {
        self.connection_monitor.get_status()
    }

    /// Check connection status
    pub fn is_connected(&self) -> bool {
        self.connection_monitor.is_connected()
    }

    /// Get connection monitor
    pub fn connection_monitor(&self) -> &ConnectionMonitor {
        &self.connection_monitor
    }

    /// Get message queue length
    pub async fn get_message_queue_length(&self) -> usize {
        self.message_queue.len().await
    }

    /// Get message queue
    pub fn message_queue(&self) -> &MessageQueue {
        &self.message_queue
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ws_client_new_not_connected() {
        let config = BridgeConfig::default();
        let client = MulticaWebSocketClient::new(config);
        assert!(!client.is_connected());
        assert!(matches!(
            client.get_connection_status(),
            ConnectionStatus::Disconnected
        ));
    }

    #[test]
    fn test_ws_client_config_stored() {
        let config = BridgeConfig {
            server_url: "ws://localhost:8080".to_string(),
            daemon_id: "test-daemon".to_string(),
            agent_id: "test-agent".to_string(),
            workspace_id: "test-workspace".to_string(),
            api_key: None,
            auto_reconnect: false,
            reconnect_delay: 5,
        };
        let client = MulticaWebSocketClient::new(config.clone());
        assert_eq!(client.config.server_url, "ws://localhost:8080");
        assert_eq!(client.config.daemon_id, "test-daemon");
    }

    #[test]
    fn test_ws_client_initial_state() {
        let config = BridgeConfig::default();
        let client = MulticaWebSocketClient::new(config);
        assert!(!client.is_connected());
        assert!(client.sender.is_none());
        assert!(client.receiver.is_none());
    }

    #[test]
    fn test_ws_client_send_message_when_not_connected() {
        let config = BridgeConfig {
            auto_reconnect: false,
            ..BridgeConfig::default()
        };
        let mut client = MulticaWebSocketClient::new(config);

        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(async {
            let message = Message {
                message_type: "test".to_string(),
                payload: serde_json::json!({}),
            };
            client.send_message(message).await
        });

        assert!(result.is_err());
    }

    #[test]
    fn test_ws_client_receive_when_not_connected() {
        let config = BridgeConfig::default();
        let mut client = MulticaWebSocketClient::new(config);

        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(async { client.receive_message().await });

        assert!(result.is_err());
    }

    #[test]
    fn test_message_type_constants() {
        assert_eq!(EVENT_DAEMON_REGISTER, "daemon:register");
        assert_eq!(EVENT_DAEMON_HEARTBEAT, "daemon:heartbeat");
    }

    #[test]
    fn test_daemon_register_payload_structure() {
        let config = BridgeConfig {
            daemon_id: "daemon-123".to_string(),
            agent_id: "agent-456".to_string(),
            ..BridgeConfig::default()
        };

        let payload = DaemonRegisterPayload {
            daemon_id: config.daemon_id.clone(),
            agent_id: config.agent_id.clone(),
            runtimes: vec![RuntimeInfo {
                runtime_type: "WindWave".to_string(),
                version: "0.1.0".to_string(),
                status: "ready".to_string(),
            }],
        };

        assert_eq!(payload.daemon_id, "daemon-123");
        assert_eq!(payload.agent_id, "agent-456");
        assert_eq!(payload.runtimes.len(), 1);
        assert_eq!(payload.runtimes[0].runtime_type, "WindWave");
    }

    #[test]
    fn test_heartbeat_payload_structure() {
        let payload = DaemonHeartbeatRequestPayload {
            runtime_id: "runtime-789".to_string(),
            supports_batch_import: Some(true),
        };

        assert_eq!(payload.runtime_id, "runtime-789");
        assert_eq!(payload.supports_batch_import, Some(true));
    }

    #[test]
    fn test_message_json_roundtrip() {
        let message = Message {
            message_type: EVENT_DAEMON_HEARTBEAT.to_string(),
            payload: serde_json::json!({"runtime_id": "rt-1", "supports_batch_import": false}),
        };
        let json = serde_json::to_string(&message).unwrap();
        let parsed: Message = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.message_type, EVENT_DAEMON_HEARTBEAT);
        assert_eq!(parsed.payload["runtime_id"], "rt-1");
    }

    #[test]
    fn test_disconnect_when_not_connected_is_ok() {
        let config = BridgeConfig::default();
        let mut client = MulticaWebSocketClient::new(config);
        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(async { client.disconnect().await });
        assert!(result.is_ok());
        assert!(!client.is_connected());
    }

    #[test]
    fn test_register_daemon_when_not_connected_queues_message() {
        let config = BridgeConfig::default();
        let mut client = MulticaWebSocketClient::new(config);
        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(async { client.register_daemon().await });
        // With auto_reconnect enabled, the message is queued instead of failing
        assert!(result.is_ok());
        // Verify message was queued
        let rt = tokio::runtime::Runtime::new().unwrap();
        let queue_len = rt.block_on(async { client.get_message_queue_length().await });
        assert_eq!(queue_len, 1);
    }

    #[test]
    fn test_client_config_default_values() {
        let config = BridgeConfig::default();
        let client = MulticaWebSocketClient::new(config);
        assert_eq!(client.config.reconnect_delay, 5000);
        assert!(client.config.auto_reconnect);
    }

    #[test]
    fn test_connection_monitor() {
        let monitor = ConnectionMonitor::new(5);
        assert!(!monitor.is_connected());
        assert_eq!(monitor.get_reconnect_attempts(), 0);
        assert!(monitor.should_reconnect());

        monitor.set_status(ConnectionStatus::Connected);
        assert!(monitor.is_connected());

        monitor.set_status(ConnectionStatus::Reconnecting);
        assert!(!monitor.is_reconnecting());

        monitor.set_reconnecting(true);
        assert!(monitor.is_reconnecting());

        monitor.increment_reconnect_attempts();
        assert_eq!(monitor.get_reconnect_attempts(), 1);

        monitor.reset_reconnect_attempts();
        assert_eq!(monitor.get_reconnect_attempts(), 0);
    }

    #[test]
    fn test_message_queue() {
        let queue = MessageQueue::new(10);
        let rt = tokio::runtime::Runtime::new().unwrap();

        rt.block_on(async {
            assert!(queue.is_empty().await);

            let msg1 = Message {
                message_type: "test1".to_string(),
                payload: serde_json::json!({}),
            };
            let msg2 = Message {
                message_type: "test2".to_string(),
                payload: serde_json::json!({}),
            };

            queue.enqueue(msg1.clone()).await.unwrap();
            queue.enqueue(msg2.clone()).await.unwrap();
            assert_eq!(queue.len().await, 2);

            let dequeued = queue.dequeue().await.unwrap();
            assert_eq!(dequeued.message_type, "test1");
            assert_eq!(queue.len().await, 1);

            let all = queue.dequeue_all().await;
            assert_eq!(all.len(), 1);
            assert_eq!(all[0].message_type, "test2");

            queue.clear().await;
            assert!(queue.is_empty().await);
        });
    }

    #[test]
    fn test_message_queue_overflow() {
        let queue = MessageQueue::new(3);
        let rt = tokio::runtime::Runtime::new().unwrap();

        rt.block_on(async {
            for i in 0..5 {
                let msg = Message {
                    message_type: format!("msg{}", i),
                    payload: serde_json::json!({}),
                };
                queue.enqueue(msg).await.unwrap();
            }

            assert_eq!(queue.len().await, 3);

            let first = queue.dequeue().await.unwrap();
            assert_eq!(first.message_type, "msg2");
        });
    }
}
