use serde::{Deserialize, Serialize};
use std::time::Instant;

/// LLM连接检测状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HybridLlmStatus {
    /// LLM正常运行
    Available,
    /// LLM连接中（首次检测）
    Connecting,
    /// LLM暂时不可用（网络问题等）
    Unavailable,
    /// LLM被用户禁用
    Disabled,
}

/// 当前使用的规划器模式
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EditorMode {
    /// 使用LLM CoT推理
    Llm,
    /// 使用规则引擎（LLM不可用或被禁用）
    RuleBased,
}

/// 降级事件记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FallbackEvent {
    /// 时间戳
    pub timestamp: u64,
    /// 降级原因
    pub reason: FallbackReason,
    /// 降级前状态
    pub from_status: HybridLlmStatus,
    /// 降级发生时的请求
    pub request_preview: String,
}

/// 降级原因
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FallbackReason {
    /// LLM客户端未初始化
    NoClient,
    /// LLM客户端未就绪
    NotReady,
    /// LLM API调用失败
    ApiError,
    /// LLM响应超时
    Timeout,
    /// LLM响应无效（无法解析）
    InvalidResponse,
    /// 用户主动禁用
    UserDisabled,
}

/// 性能统计
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HybridStats {
    /// LLM成功次数
    pub llm_successes: u64,
    /// LLM失败次数
    pub llm_failures: u64,
    /// 规则引擎使用次数
    pub rule_fallbacks: u64,
    /// 总降级次数
    pub total_fallbacks: u64,
    /// 最后降级时间
    pub last_fallback: Option<u64>,
    /// LLM平均响应时间（毫秒）
    pub avg_llm_response_ms: f64,
    /// 最后LLM检查时间
    pub last_llm_check: Option<u64>,
}

impl HybridStats {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record_llm_success(&mut self, response_time_ms: f64) {
        self.llm_successes += 1;
        // 更新平均响应时间
        let total = self.llm_successes as f64;
        self.avg_llm_response_ms =
            (self.avg_llm_response_ms * (total - 1.0) + response_time_ms) / total;
    }

    pub fn record_llm_failure(&mut self) {
        self.llm_failures += 1;
    }

    pub fn record_fallback(&mut self, _reason: &FallbackReason) {
        self.rule_fallbacks += 1;
        self.total_fallbacks += 1;
        self.last_fallback = Some(crate::types::current_timestamp());
    }

    pub fn llm_success_rate(&self) -> f64 {
        let total = self.llm_successes + self.llm_failures;
        if total == 0 {
            0.0
        } else {
            self.llm_successes as f64 / total as f64
        }
    }
}

/// 控制器内部状态（单一锁，避免死锁）
#[derive(Debug)]
pub(crate) struct ControllerState {
    pub(crate) llm_status: HybridLlmStatus,
    pub(crate) current_mode: EditorMode,
    pub(crate) fallback_history: Vec<FallbackEvent>,
    pub(crate) stats: HybridStats,
    pub(crate) last_check: Option<Instant>,
    pub(crate) pending_confirmation: Option<(crate::plan::EditPlan, u64)>,
}

impl Default for ControllerState {
    fn default() -> Self {
        Self {
            llm_status: HybridLlmStatus::Unavailable,
            current_mode: EditorMode::RuleBased,
            fallback_history: Vec::new(),
            stats: HybridStats::new(),
            last_check: None,
            pending_confirmation: None,
        }
    }
}
