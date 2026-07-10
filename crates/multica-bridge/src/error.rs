use thiserror::Error;

pub type Result<T> = std::result::Result<T, BridgeError>;

#[derive(Error, Debug)]
pub enum BridgeError {
    #[error("WebSocket error: {0}")]
    WebSocketError(String),

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("URL parse error: {0}")]
    UrlParseError(String),

    #[error("Network error: {0}")]
    NetworkError(String),

    #[error("Task not found: {0}")]
    TaskNotFound(String),

    #[error("Agent not found: {0}")]
    AgentNotFound(String),

    #[error("Invalid state: {0}")]
    InvalidState(String),

    #[error("Connection closed")]
    ConnectionClosed,

    #[error("Other error: {0}")]
    Other(String),
}
