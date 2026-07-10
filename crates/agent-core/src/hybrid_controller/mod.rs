//! HybridEditorController - LLM与规则引擎混合决策控制器
//!
//! 当LLM不可用时，自动降级到RuleBasedPlanner，确保系统始终可用。
//!
//! # 核心功能
//! 1. **LLM状态检测** - 主动检测LLM连接状态
//! 2. **自动降级** - LLM不可用时平滑切换到规则引擎
//! 3. **状态通知** - 通知UI系统当前模式变化
//! 4. **性能追踪** - 记录降级次数和原因
//!
//! # 使用方式
//! ```ignore
//! let controller = HybridEditorController::new(None); // 无LLM客户端时
//! let plan = controller.create_plan("创建一个敌人", task_id, context);
//! // 如果LLM不可用，自动使用RuleBasedPlanner
//! ```

pub mod connection_checker;
pub mod controller;
pub mod feature_limiter;
pub mod types;

// Re-exports for backward compatibility
pub use connection_checker::{
    CheckResult, ConnectionCheckConfig, ConnectionState, LlmConnectionChecker,
};
pub use controller::{HybridEditorController, VisualGoal, VisualVerifyResult};
pub use feature_limiter::{BypassEntry, Feature, FeatureLimiter, LimitReason};
pub use types::{EditorMode, FallbackEvent, FallbackReason, HybridLlmStatus, HybridStats};

#[cfg(test)]
mod tests;
