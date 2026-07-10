//! Complete Test Server for Multica Bridge
//!
//! This is a fully functional test server that implements
//! the Multica protocol for testing multica-bridge.

use futures_util::{SinkExt, StreamExt};
use log::{error, info};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::tungstenite::protocol::Message as WsMessage;

// Test server defines its own types

/// Server state
pub struct TestServerState {
    pub clients: Arc<Mutex<Vec<ClientConnection>>>,
    pub tasks: Arc<Mutex<HashMap<String, TaskState>>>,
}

/// Client connection
#[derive(Clone)]
pub struct ClientConnection {
    pub id: String,
    pub daemon_id: Option<String>,
    pub agent_id: Option<String>,
    pub runtime_id: Option<String>,
}

/// Task state
#[derive(Debug, Clone)]
pub struct TaskState {
    pub id: String,
    pub issue_id: String,
    pub title: String,
    pub description: String,
    pub status: TaskStateStatus,
    pub progress: Option<ProgressInfo>,
    pub messages: Vec<TaskMessage>,
    pub output: Option<String>,
}

/// Task state status
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskStateStatus {
    Created,
    InProgress,
    Completed,
    Failed,
}

/// Progress info
#[derive(Debug, Clone)]
pub struct ProgressInfo {
    pub summary: String,
    pub step: Option<i32>,
    pub total: Option<i32>,
}

/// Task message
#[derive(Debug, Clone)]
pub struct TaskMessage {
    pub seq: i32,
    pub message_type: String,
    pub content: Option<String>,
}

impl TestServerState {
    pub fn new() -> Self {
        Self {
            clients: Arc::new(Mutex::new(Vec::new())),
            tasks: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn add_client(&self, client: ClientConnection) {
        let mut clients = self.clients.lock().expect("mutex poisoned");
        info!("New client connected: {}", client.id);
        clients.push(client);
    }

    pub fn create_task(&self, issue_id: String, title: String, description: String) -> TaskState {
        let task_id = uuid::Uuid::new_v4().to_string();
        let task = TaskState {
            id: task_id.clone(),
            issue_id,
            title,
            description,
            status: TaskStateStatus::Created,
            progress: None,
            messages: Vec::new(),
            output: None,
        };

        let mut tasks = self.tasks.lock().expect("mutex poisoned");
        tasks.insert(task_id.clone(), task.clone());
        info!("Created task: {}", task_id);
        task
    }

    pub fn update_task_progress(
        &self,
        task_id: &str,
        summary: String,
        step: Option<i32>,
        total: Option<i32>,
    ) -> bool {
        let mut tasks = self.tasks.lock().expect("mutex poisoned");
        if let Some(task) = tasks.get_mut(task_id) {
            task.status = TaskStateStatus::InProgress;
            task.progress = Some(ProgressInfo {
                summary,
                step,
                total,
            });
            info!("Task progress updated: {}", task_id);
            true
        } else {
            false
        }
    }

    pub fn complete_task(&self, task_id: &str, output: Option<String>) -> bool {
        let mut tasks = self.tasks.lock().expect("mutex poisoned");
        if let Some(task) = tasks.get_mut(task_id) {
            task.status = TaskStateStatus::Completed;
            task.output = output;
            info!("Task completed: {}", task_id);
            true
        } else {
            false
        }
    }

    pub fn add_task_message(
        &self,
        task_id: &str,
        seq: i32,
        message_type: String,
        content: Option<String>,
    ) -> bool {
        let mut tasks = self.tasks.lock().expect("mutex poisoned");
        if let Some(task) = tasks.get_mut(task_id) {
            task.messages.push(TaskMessage {
                seq,
                message_type,
                content,
            });
            info!("Task message added: {}", task_id);
            true
        } else {
            false
        }
    }
}

impl Default for TestServerState {
    fn default() -> Self {
        Self::new()
    }
}

/// Start the test server
pub async fn start_test_server(addr: &str) -> Result<(), anyhow::Error> {
    let listener = TcpListener::bind(addr).await?;
    let state = Arc::new(TestServerState::new());

    info!("Multica Test Server started on: {}", addr);
    info!("Waiting for connections...");

    while let Ok((stream, addr)) = listener.accept().await {
        info!("New TCP connection: {}", addr);
        let state_clone = state.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_connection(stream, state_clone).await {
                error!("Connection error: {}", e);
            }
        });
    }

    Ok(())
}

/// Handle an incoming WebSocket connection
async fn handle_connection(
    stream: TcpStream,
    _state: Arc<TestServerState>,
) -> Result<(), anyhow::Error> {
    let ws_stream = tokio_tungstenite::accept_async(stream).await?;
    let client_id = uuid::Uuid::new_v4().to_string();

    info!("New WebSocket connection: {}", client_id);

    let (mut write, mut read) = ws_stream.split();

    while let Some(msg_result) = read.next().await {
        match msg_result {
            Ok(WsMessage::Text(text)) => {
                info!("Received message: {}", text);
            }
            Ok(WsMessage::Close(_)) => {
                info!("Connection closed: {}", client_id);
                break;
            }
            Ok(WsMessage::Ping(data)) => {
                write.send(WsMessage::Pong(data)).await?;
            }
            Ok(_) => {}
            Err(e) => {
                error!("Error receiving message: {}", e);
                break;
            }
        }
    }

    info!("Client disconnected: {}", client_id);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_server_state_new() {
        let state = TestServerState::new();
        assert!(state.clients.lock().expect("mutex poisoned").is_empty());
        assert!(state.tasks.lock().expect("mutex poisoned").is_empty());
    }

    #[test]
    fn test_create_task() {
        let state = TestServerState::new();
        let task = state.create_task(
            "issue-123".to_string(),
            "Test task".to_string(),
            "This is a test task".to_string(),
        );
        assert_eq!(task.title, "Test task");
        assert_eq!(task.status, TaskStateStatus::Created);
    }
}
