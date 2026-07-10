use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Python initialization failed: {0}")]
    PythonInitError(String),

    #[error("LangChain error: {0}")]
    LangChainError(String),

    #[error("LlamaIndex error: {0}")]
    LlamaIndexError(String),

    #[error("DSPy error: {0}")]
    DSPyError(String),

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Async error: {0}")]
    AsyncError(String),

    #[error("Tool execution failed: {0}")]
    ToolExecutionError(String),
}
