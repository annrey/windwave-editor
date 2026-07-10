//! Visual System - Sprint 3: 让 Agent 真正"看见"编辑器世界
//!
//! 实现三层"看"的能力：
//! 1. 结构化读取 - SceneIndex → 实体/组件/层级的精确数据
//! 2. 视觉截图 - Screenshot → Vision LLM 分析的视觉理解
//! 3. 融合感知 - 结构化数据 + 视觉分析 → Agent 的世界模型

pub mod agent_world;
mod helpers;
pub mod screenshot;
#[cfg(test)]
mod tests;
pub mod vgrc;
pub mod world;

pub use agent_world::*;
pub use helpers::{
    build_scene_summary, color_approx_eq, colors_match, ClosureRealizeExecutor, GoalCheckResult,
    RealizeExecutor, VgcrCycleResult, VgrcCycleResult,
};
pub use screenshot::*;
pub use vgrc::*;
pub use world::*;
