//! Memory Injector - Sprint 2: 自动捕获上下文并注入 LLM
//!
//! 分解自原 monolithic memory_injector.rs (1821 行)。
//! 7 个子模块，每个关注一个独立子系统。

mod project_memory;
mod code_index;
mod pattern_learner;
mod working_set;
mod compressor;
mod injector;
mod event_bridge;

pub use project_memory::*;
pub use code_index::*;
pub use pattern_learner::*;
pub use working_set::*;
pub use compressor::*;
pub use injector::*;
pub use event_bridge::*;

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
