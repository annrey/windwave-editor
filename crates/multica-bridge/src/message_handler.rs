//! Multica Message Handler
//!
//! 处理来自 Multica 服务器的各类消息
//! 包括任务创建、任务更新、任务删除、心跳响应等

use crate::error::Result;
use crate::task_bridge::UnifiedTask;
use crate::task_sync_module::TaskSynchronizer;
use crate::types::Message;
use log::{debug, info};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// Multica 消息类型
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MulticaMessageType {
    /// 任务创建
    TaskCreated,
    /// 任务更新
    TaskUpdated,
    /// 任务删除
    TaskDeleted,
    /// 心跳响应
    HeartbeatResponse,
    /// 守护进程注册响应
    DaemonRegisterResponse,
    /// 场景变更通知
    SceneChanged,
    /// 自定义消息
    Custom(String),
}

impl MulticaMessageType {
    /// 从字符串解析
    pub fn from_str(s: &str) -> Self {
        match s {
            "task:created" => Self::TaskCreated,
            "task:updated" => Self::TaskUpdated,
            "task:deleted" => Self::TaskDeleted,
            "heartbeat:response" => Self::HeartbeatResponse,
            "daemon:register:response" => Self::DaemonRegisterResponse,
            "scene:changed" => Self::SceneChanged,
            other => Self::Custom(other.to_string()),
        }
    }

    /// 转换为字符串
    pub fn as_str(&self) -> &str {
        match self {
            Self::TaskCreated => "task:created",
            Self::TaskUpdated => "task:updated",
            Self::TaskDeleted => "task:deleted",
            Self::HeartbeatResponse => "heartbeat:response",
            Self::DaemonRegisterResponse => "daemon:register:response",
            Self::SceneChanged => "scene:changed",
            Self::Custom(s) => s,
        }
    }
}

/// 消息处理结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageHandleResult {
    /// 是否成功处理
    pub success: bool,
    /// 处理的消息类型
    pub message_type: String,
    /// 错误信息（如果失败）
    pub error: Option<String>,
    /// 处理时间戳
    pub processed_at: String,
    /// 额外数据
    pub metadata: HashMap<String, String>,
}

/// 消息处理统计信息
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MessageHandlerStats {
    /// 总处理消息数
    pub total_processed: usize,
    /// 成功处理的消息数
    pub successful: usize,
    /// 失败处理的消息数
    pub failed: usize,
    /// 忽略的消息数
    pub ignored: usize,
    /// 各类型消息计数
    pub messages_by_type: HashMap<String, usize>,
    /// 最后处理时间
    pub last_processed_at: Option<u64>,
    /// 平均处理时间（毫秒）
    pub avg_processing_time_ms: f64,
    /// 总处理时间（毫秒）
    pub total_processing_time_ms: u128,
}

impl MessageHandlerStats {
    /// 更新平均处理时间
    fn update_avg_processing_time(&mut self, new_time_ms: u128) {
        self.total_processing_time_ms += new_time_ms;
        self.avg_processing_time_ms =
            self.total_processing_time_ms as f64 / self.total_processed as f64;
    }
}

/// Multica 消息处理器 trait
pub trait MessageHandler: Send + Sync {
    /// 处理消息
    fn handle_message(&self, message: &Message) -> Result<MessageHandleResult>;

    /// 获取处理器名称
    fn handler_name(&self) -> &str;
}

/// 默认消息处理器
pub struct DefaultMessageHandler {
    /// 消息处理统计
    stats: Arc<Mutex<MessageHandlerStats>>,
    /// 任务同步器
    task_synchronizer: Option<Arc<TaskSynchronizer>>,
    /// 自定义处理器注册表
    custom_handlers: Arc<Mutex<HashMap<String, Box<dyn MessageHandler>>>>,
}

impl MessageHandler for DefaultMessageHandler {
    fn handle_message(&self, message: &Message) -> Result<MessageHandleResult> {
        self.handle_message_internal(message)
    }

    fn handler_name(&self) -> &str {
        "default"
    }
}

impl DefaultMessageHandler {
    /// 创建新的默认消息处理器
    pub fn new() -> Self {
        Self {
            stats: Arc::new(Mutex::new(MessageHandlerStats::default())),
            task_synchronizer: None,
            custom_handlers: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// 创建带任务同步器的消息处理器
    pub fn with_task_synchronizer(task_synchronizer: Arc<TaskSynchronizer>) -> Self {
        Self {
            stats: Arc::new(Mutex::new(MessageHandlerStats::default())),
            task_synchronizer: Some(task_synchronizer),
            custom_handlers: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// 注册自定义消息处理器
    pub fn register_handler(&self, message_type: String, handler: Box<dyn MessageHandler>) {
        self.custom_handlers
            .lock()
            .expect("mutex poisoned")
            .insert(message_type, handler);
    }

    /// 处理接收到的消息
    pub fn process_message(&self, message: &Message) -> MessageHandleResult {
        let start_time = Instant::now();
        let message_type = message.message_type.clone();

        let result = self.handle_message_internal(message);

        let processing_time = start_time.elapsed();
        self.update_stats(&message_type, result.is_ok(), processing_time.as_millis());

        match result {
            Ok(r) => r,
            Err(e) => MessageHandleResult {
                success: false,
                message_type,
                error: Some(e.to_string()),
                processed_at: chrono::Utc::now().to_rfc3339(),
                metadata: HashMap::new(),
            },
        }
    }

    /// 内部消息处理逻辑
    fn handle_message_internal(&self, message: &Message) -> Result<MessageHandleResult> {
        let message_type = MulticaMessageType::from_str(&message.message_type);

        // Check if there's a custom handler for this message type
        {
            let handlers = self.custom_handlers.lock().expect("mutex poisoned");
            if let Some(handler) = handlers.get(&message.message_type) {
                return handler.handle_message(message);
            }
        }

        // Handle built-in message types
        match message_type {
            MulticaMessageType::TaskCreated => self.handle_task_created(message),
            MulticaMessageType::TaskUpdated => self.handle_task_updated(message),
            MulticaMessageType::TaskDeleted => self.handle_task_deleted(message),
            MulticaMessageType::HeartbeatResponse => self.handle_heartbeat_response(message),
            MulticaMessageType::DaemonRegisterResponse => {
                self.handle_daemon_register_response(message)
            }
            MulticaMessageType::SceneChanged => self.handle_scene_changed(message),
            MulticaMessageType::Custom(_) => {
                debug!("Received custom message type: {}", message.message_type);
                Ok(MessageHandleResult {
                    success: true,
                    message_type: message.message_type.clone(),
                    error: None,
                    processed_at: chrono::Utc::now().to_rfc3339(),
                    metadata: HashMap::new(),
                })
            }
        }
    }

    /// 处理任务创建消息
    fn handle_task_created(&self, message: &Message) -> Result<MessageHandleResult> {
        info!("Handling task created message");
        debug!("Task payload: {:?}", message.payload);

        if let Some(ref synchronizer) = self.task_synchronizer {
            // Parse task from payload and sync
            if let Ok(task) = serde_json::from_value::<UnifiedTask>(message.payload.clone()) {
                synchronizer.add_local_task(task);
            }
        }

        Ok(MessageHandleResult {
            success: true,
            message_type: message.message_type.clone(),
            error: None,
            processed_at: chrono::Utc::now().to_rfc3339(),
            metadata: {
                let mut map = HashMap::new();
                map.insert("action".to_string(), "task_created".to_string());
                map
            },
        })
    }

    /// 处理任务更新消息
    fn handle_task_updated(&self, message: &Message) -> Result<MessageHandleResult> {
        info!("Handling task updated message");
        debug!("Task update payload: {:?}", message.payload);

        Ok(MessageHandleResult {
            success: true,
            message_type: message.message_type.clone(),
            error: None,
            processed_at: chrono::Utc::now().to_rfc3339(),
            metadata: {
                let mut map = HashMap::new();
                map.insert("action".to_string(), "task_updated".to_string());
                map
            },
        })
    }

    /// 处理任务删除消息
    fn handle_task_deleted(&self, message: &Message) -> Result<MessageHandleResult> {
        info!("Handling task deleted message");
        debug!("Task delete payload: {:?}", message.payload);

        Ok(MessageHandleResult {
            success: true,
            message_type: message.message_type.clone(),
            error: None,
            processed_at: chrono::Utc::now().to_rfc3339(),
            metadata: {
                let mut map = HashMap::new();
                map.insert("action".to_string(), "task_deleted".to_string());
                map
            },
        })
    }

    /// 处理心跳响应
    fn handle_heartbeat_response(&self, message: &Message) -> Result<MessageHandleResult> {
        debug!("Received heartbeat response");

        Ok(MessageHandleResult {
            success: true,
            message_type: message.message_type.clone(),
            error: None,
            processed_at: chrono::Utc::now().to_rfc3339(),
            metadata: {
                let mut map = HashMap::new();
                map.insert("action".to_string(), "heartbeat_received".to_string());
                map
            },
        })
    }

    /// 处理守护进程注册响应
    fn handle_daemon_register_response(&self, message: &Message) -> Result<MessageHandleResult> {
        info!("Received daemon register response");
        debug!("Response payload: {:?}", message.payload);

        Ok(MessageHandleResult {
            success: true,
            message_type: message.message_type.clone(),
            error: None,
            processed_at: chrono::Utc::now().to_rfc3339(),
            metadata: {
                let mut map = HashMap::new();
                map.insert("action".to_string(), "daemon_registered".to_string());
                map
            },
        })
    }

    /// 处理场景变更消息
    fn handle_scene_changed(&self, message: &Message) -> Result<MessageHandleResult> {
        info!("Handling scene changed message");
        debug!("Scene change payload: {:?}", message.payload);

        Ok(MessageHandleResult {
            success: true,
            message_type: message.message_type.clone(),
            error: None,
            processed_at: chrono::Utc::now().to_rfc3339(),
            metadata: {
                let mut map = HashMap::new();
                map.insert("action".to_string(), "scene_changed".to_string());
                map
            },
        })
    }

    /// 更新统计信息
    fn update_stats(&self, message_type: &str, success: bool, processing_time_ms: u128) {
        let mut stats = self.stats.lock().expect("mutex poisoned");
        stats.total_processed += 1;
        if success {
            stats.successful += 1;
        } else {
            stats.failed += 1;
        }
        *stats
            .messages_by_type
            .entry(message_type.to_string())
            .or_insert(0) += 1;
        stats.last_processed_at = Some(chrono::Utc::now().timestamp() as u64);
        stats.update_avg_processing_time(processing_time_ms);
    }

    /// 获取统计信息
    pub fn get_stats(&self) -> MessageHandlerStats {
        self.stats.lock().expect("mutex poisoned").clone()
    }
}

/// 消息处理管道
///
/// 支持多个处理器串联处理消息
pub struct MessagePipeline {
    handlers: Vec<Box<dyn MessageHandler>>,
    stats: Arc<Mutex<MessageHandlerStats>>,
}

impl MessagePipeline {
    /// 创建新的消息处理管道
    pub fn new() -> Self {
        Self {
            handlers: Vec::new(),
            stats: Arc::new(Mutex::new(MessageHandlerStats::default())),
        }
    }

    /// 添加处理器到管道
    pub fn add_handler(&mut self, handler: Box<dyn MessageHandler>) {
        self.handlers.push(handler);
    }

    /// 处理消息（依次通过所有处理器）
    pub fn process_message(&self, message: &Message) -> Vec<MessageHandleResult> {
        let mut results = Vec::new();

        for handler in &self.handlers {
            let start_time = Instant::now();
            let result = handler.handle_message(message);
            let processing_time = start_time.elapsed();

            // Update stats
            let mut stats = self.stats.lock().expect("mutex poisoned");
            stats.total_processed += 1;
            if result.is_ok() {
                stats.successful += 1;
            } else {
                stats.failed += 1;
            }
            stats.update_avg_processing_time(processing_time.as_millis());

            results.push(match result {
                Ok(r) => r,
                Err(e) => MessageHandleResult {
                    success: false,
                    message_type: message.message_type.clone(),
                    error: Some(e.to_string()),
                    processed_at: chrono::Utc::now().to_rfc3339(),
                    metadata: HashMap::new(),
                },
            });
        }

        results
    }

    /// 获取统计信息
    pub fn get_stats(&self) -> MessageHandlerStats {
        self.stats.lock().expect("mutex poisoned").clone()
    }
}

/// 创建共享的默认消息处理器
pub fn create_shared_message_handler() -> Arc<DefaultMessageHandler> {
    Arc::new(DefaultMessageHandler::new())
}

/// 创建带任务同步器的共享消息处理器
pub fn create_shared_message_handler_with_sync(
    task_synchronizer: Arc<TaskSynchronizer>,
) -> Arc<DefaultMessageHandler> {
    Arc::new(DefaultMessageHandler::with_task_synchronizer(
        task_synchronizer,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_message_type_from_str() {
        assert_eq!(
            MulticaMessageType::from_str("task:created"),
            MulticaMessageType::TaskCreated
        );
        assert_eq!(
            MulticaMessageType::from_str("task:updated"),
            MulticaMessageType::TaskUpdated
        );
        assert_eq!(
            MulticaMessageType::from_str("heartbeat:response"),
            MulticaMessageType::HeartbeatResponse
        );
        assert_eq!(
            MulticaMessageType::from_str("unknown:type"),
            MulticaMessageType::Custom("unknown:type".to_string())
        );
    }

    #[test]
    fn test_message_type_as_str() {
        assert_eq!(MulticaMessageType::TaskCreated.as_str(), "task:created");
        assert_eq!(MulticaMessageType::TaskUpdated.as_str(), "task:updated");
        assert_eq!(
            MulticaMessageType::Custom("custom".to_string()).as_str(),
            "custom"
        );
    }

    #[test]
    fn test_default_message_handler_creation() {
        let handler = DefaultMessageHandler::new();
        let stats = handler.get_stats();
        assert_eq!(stats.total_processed, 0);
        assert_eq!(stats.successful, 0);
        assert_eq!(stats.failed, 0);
    }

    #[test]
    fn test_handle_task_created_message() {
        let handler = DefaultMessageHandler::new();
        let message = Message {
            message_type: "task:created".to_string(),
            payload: json!({
                "id": {"bridge_id": 1, "multica_id": 100},
                "title": "Test Task",
                "description": "Test Description",
                "status": "running",
                "scene_id": null,
                "entity_ids": [],
                "resource_ids": [],
                "scene_snapshot": null,
                "multica_task": null,
                "created_at": "2024-01-01T00:00:00Z",
                "updated_at": "2024-01-01T00:00:00Z"
            }),
        };

        let result = handler.process_message(&message);
        assert!(result.success);
        assert_eq!(result.message_type, "task:created");
        assert_eq!(result.metadata.get("action").unwrap(), "task_created");

        let stats = handler.get_stats();
        assert_eq!(stats.total_processed, 1);
        assert_eq!(stats.successful, 1);
    }

    #[test]
    fn test_handle_heartbeat_response() {
        let handler = DefaultMessageHandler::new();
        let message = Message {
            message_type: "heartbeat:response".to_string(),
            payload: json!({}),
        };

        let result = handler.process_message(&message);
        assert!(result.success);
        assert_eq!(result.message_type, "heartbeat:response");

        let stats = handler.get_stats();
        assert_eq!(stats.total_processed, 1);
    }

    #[test]
    fn test_handle_custom_message() {
        let handler = DefaultMessageHandler::new();
        let message = Message {
            message_type: "custom:type".to_string(),
            payload: json!({}),
        };

        let result = handler.process_message(&message);
        assert!(result.success);
        assert_eq!(result.message_type, "custom:type");
    }

    #[test]
    fn test_message_pipeline() {
        let mut pipeline = MessagePipeline::new();
        pipeline.add_handler(Box::new(DefaultMessageHandler::new()));

        let message = Message {
            message_type: "task:created".to_string(),
            payload: json!({}),
        };

        let results = pipeline.process_message(&message);
        assert_eq!(results.len(), 1);
        assert!(results[0].success);

        let stats = pipeline.get_stats();
        assert_eq!(stats.total_processed, 1);
    }

    #[test]
    fn test_handler_stats_tracking() {
        let handler = DefaultMessageHandler::new();

        // Process multiple messages
        for i in 0..5 {
            let message = Message {
                message_type: "task:created".to_string(),
                payload: json!({"index": i}),
            };
            handler.process_message(&message);
        }

        let stats = handler.get_stats();
        assert_eq!(stats.total_processed, 5);
        assert_eq!(stats.successful, 5);
        assert_eq!(stats.failed, 0);
        assert!(stats.avg_processing_time_ms >= 0.0);
    }

    #[test]
    fn test_messages_by_type_tracking() {
        let handler = DefaultMessageHandler::new();

        let task_created_msg = Message {
            message_type: "task:created".to_string(),
            payload: json!({}),
        };
        let heartbeat_msg = Message {
            message_type: "heartbeat:response".to_string(),
            payload: json!({}),
        };

        handler.process_message(&task_created_msg);
        handler.process_message(&heartbeat_msg);
        handler.process_message(&task_created_msg);

        let stats = handler.get_stats();
        assert_eq!(stats.messages_by_type.get("task:created"), Some(&2));
        assert_eq!(stats.messages_by_type.get("heartbeat:response"), Some(&1));
    }

    #[test]
    fn test_shared_message_handler_creation() {
        let handler = create_shared_message_handler();
        assert!(Arc::strong_count(&handler) == 1);

        let synchronizer = Arc::new(crate::task_sync_module::TaskSynchronizer::new(
            crate::task_sync_module::TaskSyncConfig::default(),
        ));
        let handler_with_sync = create_shared_message_handler_with_sync(synchronizer);
        assert!(Arc::strong_count(&handler_with_sync) == 1);
    }
}
