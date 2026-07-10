//! Dynamic Planner — Plan-and-Solve 动态修订引擎
//!
//! Sprint 1-A2: 在执行过程中根据中间结果智能调整计划。
//!
//! ## 核心能力
//!
//! 1. **观察驱动修订**: 根据工具执行结果自动调整后续步骤
//! 2. **模式识别**: 检测常见执行模式（实体已存在、权限不足等）
//! 3. **智能跳过**: 自动跳过冗余步骤（如重复创建）
//! 4. **前置条件插入**: 缺少依赖时自动插入前置步骤
//! 5. **降级策略**: 高风险操作失败时自动降级为低风险替代方案
//!
//! ## 架构
//!
//! ```text
//! ReAct Loop (Observe)
//!     ↓
//! DynamicPlanner::analyze_observation()
//!     ├─ Pattern Match → RevisionType
//!     └─ Confidence Score
//!         ↓
//! DynamicPlanner::apply_revision()
//!     ├─ Skip redundant steps
//!     ├─ Insert prerequisites
//!     ├─ Adapt failed steps
//!     └─ Log revision history
//! ```

mod helpers;
mod observation_pattern;
mod planner;
mod revision_entry;
mod revision_type;

#[cfg(test)]
mod tests;

pub use observation_pattern::ObservationPattern;
pub use planner::DynamicPlanner;
pub use revision_entry::RevisionEntry;
pub use revision_type::RevisionType;
