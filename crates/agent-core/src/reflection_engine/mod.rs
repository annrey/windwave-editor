//! Reflection Engine — Agent自我修正与智能重试系统
//!
//! Sprint 1-A3: 当执行失败时，Agent能够自动反思、修正策略并重试。
//!
//! ## 核心能力
//!
//! 1. **错误分类**: 区分可恢复错误（网络超时）和致命错误（权限不足）
//! 2. **智能重试**: 带指数退避的重试循环，避免无限循环
//! 3. **策略降级**: LLM推理 → 规则引擎 → 用户确认的多层降级
//! 4. **反思日志**: 记录每次失败的原因和修正措施，用于未来优化
//! 5. **替代方案生成**: 根据错误类型生成针对性的修复建议
//!
//! ## 架构
//!
//! ```text
//! Tool Execution Failed
//!     ↓
//! ReflectionEngine::analyze_error()
//!     ├─ ErrorClassification (Transient/Fatal/Permission/Invalid)
//!     └─ Confidence Score (0.0-1.0)
//!         ↓
//! ReflectionEngine::generate_reflection()
//!     ├─ What went wrong? (Root cause analysis)
//!     ├─ Why did it fail? (Context analysis)
//!     └─ How to fix it? (Alternative strategy)
//!         ↓
//! ReflectionEngine::execute_with_retry()
//!     ├─ Retry with backoff (if transient)
//!     ├─ Try alternative approach (if available)
//!     ├─ Degrade to fallback (if LLM fails)
//!     └─ Request user approval (if permission needed)
//! ```
//!
//! ## Submodules
//!
//! - `classification` — Error classification types
//! - `reflection_entry` — Reflection history entries
//! - `retry_config` — Retry behavior configuration
//! - `engine` — Core reflection engine
//! - `stats` — Performance statistics
//! - `helpers` — Internal helper functions
//! - `tests` — Unit tests

mod classification;
mod engine;
pub(crate) mod helpers;
mod reflection_entry;
mod retry_config;
mod stats;
#[cfg(test)]
mod tests;

pub use classification::ErrorClassification;
pub use engine::ReflectionEngine;
pub use reflection_entry::ReflectionEntry;
pub use retry_config::RetryConfig;
pub use stats::ReflectionStats;
