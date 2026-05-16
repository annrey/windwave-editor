//! Planner module — types, trait, and implementations.
//!
//! # Components
//! - `Planner` trait — unified planning interface
//! - `RuleBasedPlanner` — keyword-driven (always available)
//! - `LlmPlanner` — LLM CoT-driven, with configurable fallback

pub mod rule_based;
pub mod llm_planner;

pub use rule_based::RuleBasedPlanner;
pub use llm_planner::LlmPlanner;

use crate::permission::OperationRisk;
use crate::plan::{EditPlan, EditPlanStep, EditPlanStatus, ExecutionMode, TargetModule};
use std::sync::OnceLock;

/// Lazily-created Tokio runtime for synchronous LLM calls.
static LLM_RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();

pub(crate) fn llm_runtime() -> &'static tokio::runtime::Runtime {
    LLM_RUNTIME.get_or_init(|| {
        tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime for LLM calls")
    })
}

#[doc(hidden)]
pub fn get_llm_runtime() -> &'static tokio::runtime::Runtime {
    llm_runtime()
}

pub(crate) fn get_default_model() -> String {
    if let Ok(model) = std::env::var("LLM_MODEL") {
        if !model.is_empty() {
            return model;
        }
    }
    let config = crate::config::get_config_ref(|c| c.llm.model.clone());
    if !config.is_empty() {
        return config;
    }
    "gpt-4o-mini".to_string()
}

/// 编辑任务的复杂度级别
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ComplexityLevel {
    /// 简单的单一操作（如：创建一个实体）
    Simple,
    /// 涉及两个领域的操作（如：创建实体并设置颜色）
    Medium,
    /// 涉及三个或更多领域的复杂操作
    Complex,
}

/// 传递给 Planner 的上下文信息
#[derive(Debug, Clone)]
pub struct PlannerContext {
    /// 当前任务 ID
    pub task_id: u64,
    /// 可用的工具名称列表
    pub available_tools: Vec<String>,
    /// 场景中已有实体的名称列表
    pub scene_entity_names: Vec<String>,
    /// Memory system context (Working + Episodic + Semantic + Procedural)
    pub memory_context: Option<crate::memory::MemoryContext>,
}

// ============================================================================
// Planner trait — 统一规划接口
// ============================================================================

/// Trait for plan generators. Implementations include:
/// - `RuleBasedPlanner` — keyword-driven (always available)
/// - `LlmPlanner` — LLM CoT-driven (requires AI backend)
pub trait Planner: Send + Sync {
    /// Create a structured `EditPlan` from a user request.
    fn create_plan(
        &self,
        request_text: &str,
        task_id: u64,
        context: PlannerContext,
    ) -> EditPlan;

    /// Estimate the complexity of a request (domain count).
    fn estimate_complexity(&self, text: &str) -> ComplexityLevel;
}
