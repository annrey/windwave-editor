//! Memory Injector - Sprint 2: 自动捕获上下文并注入 LLM
//!
//! 分解自原 monolithic memory_injector.rs (1821 行)。
//! 7 个子模块，每个关注一个独立子系统。

mod code_index;
mod compressor;
mod event_bridge;
mod injector;
mod pattern_learner;
mod project_memory;
mod working_set;

pub use code_index::*;
pub use compressor::*;
pub use event_bridge::*;
pub use injector::*;
pub use pattern_learner::*;
pub use project_memory::*;
pub use working_set::*;

/// 记忆系统错误
#[derive(Debug, thiserror::Error)]
pub enum MemoryError {
    #[error("Serialization error: {0}")]
    Serialization(String),
    #[error("IO error: {0}")]
    Io(String),
    #[error("LLM error: {0}")]
    Llm(String),
}
