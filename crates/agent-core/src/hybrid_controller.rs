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

use crate::llm::LlmClient;
use crate::plan::{EditPlan, EditPlanStatus, ExecutionMode, TargetModule};
use crate::planner::{Planner, PlannerContext, RuleBasedPlanner};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

// ============================================================================
// LlmConnectionChecker - LLM 连接状态检测器
// ============================================================================

/// LLM 连接检测器
///
/// 主动检测 LLM API 的实际可用性，而非仅检查配置。
///
/// # 功能
/// 1. **Ping 检测** - 发送简单请求验证 API 响应
/// 2. **超时监控** - 监控 LLM 请求响应时间
/// 3. **失败计数** - 累积失败次数用于降级决策
///
/// # 使用方式
/// ```ignore
/// let checker = LlmConnectionChecker::new(Arc::new(client));
/// loop {
///     if !checker.is_available() {
///         // 降级到规则引擎
///     }
///     tokio::time::sleep(Duration::from_secs(30)).await;
/// }
/// ```
#[derive(Clone)]
pub struct LlmConnectionChecker {
    /// LLM 客户端
    client: Arc<dyn LlmClient>,
    /// 检测配置
    config: ConnectionCheckConfig,
    /// 共享状态
    state: Arc<Mutex<ConnectionState>>,
}

/// 连接检测配置
#[derive(Debug, Clone)]
pub struct ConnectionCheckConfig {
    /// 检测间隔（秒）
    pub check_interval_secs: u64,
    /// 连续失败降级阈值
    pub failure_threshold: u32,
    /// 单次检测超时（秒）
    pub timeout_secs: u64,
    /// Ping 请求内容
    pub ping_message: String,
}

impl Default for ConnectionCheckConfig {
    fn default() -> Self {
        Self {
            check_interval_secs: 30,
            failure_threshold: 3,
            timeout_secs: 10,
            ping_message: "ping".to_string(),
        }
    }
}

/// 连接检测状态
#[derive(Debug, Clone)]
pub struct ConnectionState {
    /// 连续失败计数
    pub consecutive_failures: u32,
    /// 最后检测时间
    pub last_check: Option<Instant>,
    /// 最后检测结果
    pub last_result: Option<CheckResult>,
    /// 最后成功时间
    pub last_success: Option<Instant>,
}

/// 检测结果
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckResult {
    /// 检测成功
    Success { response_time_ms: u64 },
    /// API Key 未配置
    ApiKeyMissing,
    /// 网络错误
    NetworkError { message: String },
    /// API 错误（账户额度等）
    ApiError { message: String },
    /// 超时
    Timeout,
    /// 未知错误
    UnknownError { message: String },
}

impl LlmConnectionChecker {
    /// 创建新的检测器
    pub fn new(client: Arc<dyn LlmClient>) -> Self {
        Self::with_config(client, ConnectionCheckConfig::default())
    }

    /// 使用自定义配置创建检测器
    pub fn with_config(client: Arc<dyn LlmClient>, config: ConnectionCheckConfig) -> Self {
        Self {
            client,
            config,
            state: Arc::new(Mutex::new(ConnectionState {
                consecutive_failures: 0,
                last_check: None,
                last_result: None,
                last_success: None,
            })),
        }
    }

    /// 检查是否可用
    ///
    /// 返回 true 如果：
    /// 1. API Key 已配置
    /// 2. 连续失败次数 < 阈值
    /// 3. 最后检测成功或从未失败
    pub fn is_available(&self) -> bool {
        let state = self.state.lock().unwrap();

        // 如果 API Key 未配置，直接返回 false
        if !self.client.is_ready() {
            return false;
        }

        // 如果连续失败次数 >= 阈值，返回 false
        if state.consecutive_failures >= self.config.failure_threshold {
            return false;
        }

        // 如果从未失败过，认为可用
        if state.consecutive_failures == 0 {
            return true;
        }

        // 如果上次检测成功，返回 true
        state.last_result.as_ref()
            .map(|r| matches!(r, CheckResult::Success { .. }))
            .unwrap_or(false)
    }

    /// 执行连接检测
    ///
    /// 发送 ping 请求到 LLM API 并返回检测结果。
    /// 会更新内部状态（失败计数等）。
    pub async fn check_connection(&self) -> CheckResult {
        let now = Instant::now();

        // 更新最后检测时间
        {
            let mut state = self.state.lock().unwrap();
            state.last_check = Some(now);
        }

        // 执行检测
        let result = self.perform_check().await;

        // 更新状态
        {
            let mut state = self.state.lock().unwrap();
            state.last_result = Some(result.clone());

            match &result {
                CheckResult::Success { .. } => {
                    state.consecutive_failures = 0;
                    state.last_success = Some(now);
                }
                _ => {
                    state.consecutive_failures += 1;
                }
            }
        }

        result
    }

    /// 执行实际检测
    async fn perform_check(&self) -> CheckResult {
        // 检查 API Key
        if !self.client.is_ready() {
            return CheckResult::ApiKeyMissing;
        }

        // 发送 ping 请求
        use crate::llm::{LlmMessage, LlmRequest, Role};

        let request = LlmRequest {
            model: "".to_string(), // 使用默认模型
            messages: vec![LlmMessage {
                role: Role::User,
                content: self.config.ping_message.clone(),
            }],
            tools: None,
            max_tokens: Some(1),
            temperature: Some(0.0),
        };

        let start = Instant::now();
        let timeout = Duration::from_secs(self.config.timeout_secs);

        // 使用 tokio::time::timeout 执行带超时的请求
        let result = tokio::time::timeout(timeout, self.client.chat(request)).await;

        match result {
            Ok(Ok(_)) => {
                let elapsed = start.elapsed().as_millis() as u64;
                CheckResult::Success { response_time_ms: elapsed }
            }
            Ok(Err(e)) => {
                let msg = e.to_string();
                if msg.contains("401") || msg.contains("403") {
                    CheckResult::ApiError { message: msg }
                } else {
                    CheckResult::NetworkError { message: msg }
                }
            }
            Err(_) => CheckResult::Timeout,
        }
    }

    /// 获取当前连续失败计数
    pub fn consecutive_failures(&self) -> u32 {
        self.state.lock().unwrap().consecutive_failures
    }

    /// 获取最后检测结果
    pub fn last_result(&self) -> Option<CheckResult> {
        self.state.lock().unwrap().last_result.clone()
    }

    /// 重置失败计数
    pub fn reset_failures(&self) {
        let mut state = self.state.lock().unwrap();
        state.consecutive_failures = 0;
    }

    /// 获取检查间隔
    pub fn check_interval(&self) -> Duration {
        Duration::from_secs(self.config.check_interval_secs)
    }

    /// 获取失败阈值
    pub fn failure_threshold(&self) -> u32 {
        self.config.failure_threshold
    }
}

#[cfg(test)]
mod connection_checker_tests {
    use super::*;
    use crate::llm::LlmProvider;

    // Mock LLM Client for testing
    struct MockLlmClient {
        ready: bool,
    }

    impl MockLlmClient {
        fn new(ready: bool) -> Self {
            Self { ready }
        }
    }

    #[async_trait::async_trait]
    impl LlmClient for MockLlmClient {
        async fn chat(&self, _request: crate::llm::LlmRequest) -> Result<crate::llm::LlmResponse, crate::llm::LlmError> {
            Ok(crate::llm::LlmResponse {
                content: "pong".to_string(),
                tool_calls: vec![],
                usage: crate::llm::TokenUsage::default(),
            })
        }

        fn is_ready(&self) -> bool {
            self.ready
        }

        fn provider(&self) -> crate::llm::LlmProvider {
            LlmProvider::OpenAI
        }
    }

    #[test]
    fn test_checker_not_ready_when_api_key_missing() {
        let client = Arc::new(MockLlmClient::new(false));
        let checker = LlmConnectionChecker::new(client);

        assert!(!checker.is_available());
    }

    #[test]
    fn test_checker_ready_when_api_key_present() {
        let client = Arc::new(MockLlmClient::new(true));
        let checker = LlmConnectionChecker::new(client);

        assert!(checker.is_available());
        assert_eq!(checker.consecutive_failures(), 0);
    }

    #[test]
    fn test_failure_threshold() {
        let client = Arc::new(MockLlmClient::new(true));
        let checker = LlmConnectionChecker::with_config(
            client,
            ConnectionCheckConfig {
                failure_threshold: 3,
                ..Default::default()
            },
        );

        assert_eq!(checker.failure_threshold(), 3);
    }

    #[test]
    fn test_reset_failures() {
        let client = Arc::new(MockLlmClient::new(true));
        let checker = LlmConnectionChecker::new(client);

        // 模拟失败
        {
            let mut state = checker.state.lock().unwrap();
            state.consecutive_failures = 5;
        }

        // 重置
        checker.reset_failures();
        assert_eq!(checker.consecutive_failures(), 0);
    }
}

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
        self.avg_llm_response_ms = (self.avg_llm_response_ms * (total - 1.0) + response_time_ms) / total;
    }

    pub fn record_llm_failure(&mut self) {
        self.llm_failures += 1;
    }

    pub fn record_fallback(&mut self, reason: &FallbackReason) {
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
struct ControllerState {
    llm_status: HybridLlmStatus,
    current_mode: EditorMode,
    fallback_history: Vec<FallbackEvent>,
    stats: HybridStats,
    last_check: Option<Instant>,
}

impl Default for ControllerState {
    fn default() -> Self {
        Self {
            llm_status: HybridLlmStatus::Unavailable,
            current_mode: EditorMode::RuleBased,
            fallback_history: Vec::new(),
            stats: HybridStats::new(),
            last_check: None,
        }
    }
}

/// HybridEditorController - LLM与规则引擎的统一入口
pub struct HybridEditorController {
    /// LLM客户端（可选）
    llm_client: Option<Arc<dyn LlmClient>>,
    /// 规则引擎（始终可用）
    rule_planner: RuleBasedPlanner,
    /// LLM连接检测器
    connection_checker: Option<LlmConnectionChecker>,
    /// 统一状态锁（避免多锁死锁）
    state: Mutex<ControllerState>,
    /// LLM检查间隔
    check_interval: Duration,
    /// 是否启用自动降级
    auto_fallback_enabled: bool,
    /// 降级历史最大长度
    max_history_size: usize,
    /// 连续失败计数（触发降级）
    failure_threshold: u32,
}

impl HybridEditorController {
    /// 创建新的控制器（无LLM客户端）
    pub fn new() -> Self {
        Self::with_llm_client(None)
    }

    /// 创建带有LLM客户端的控制器
    pub fn with_llm_client(llm_client: Option<Arc<dyn LlmClient>>) -> Self {
        let initial_status = match &llm_client {
            Some(client) => {
                let checker = LlmConnectionChecker::new(client.clone());
                HybridLlmStatus::Connecting
            }
            None => HybridLlmStatus::Unavailable,
        };

        let connection_checker = llm_client.as_ref().map(|c| {
            LlmConnectionChecker::new(c.clone())
        });

        Self {
            llm_client,
            rule_planner: RuleBasedPlanner::new(),
            connection_checker,
            state: Mutex::new(ControllerState {
                llm_status: initial_status,
                ..Default::default()
            }),
            check_interval: Duration::from_secs(30),
            auto_fallback_enabled: true,
            max_history_size: 100,
            failure_threshold: 3,
        }
    }

    /// 创建带有连接检测器的控制器
    pub fn with_connection_checker(
        llm_client: Option<Arc<dyn LlmClient>>,
        checker: Option<LlmConnectionChecker>,
    ) -> Self {
        let initial_status = match &llm_client {
            Some(_) => HybridLlmStatus::Connecting,
            None => HybridLlmStatus::Unavailable,
        };

        Self {
            llm_client,
            rule_planner: RuleBasedPlanner::new(),
            connection_checker: checker,
            state: Mutex::new(ControllerState {
                llm_status: initial_status,
                ..Default::default()
            }),
            check_interval: Duration::from_secs(30),
            auto_fallback_enabled: true,
            max_history_size: 100,
            failure_threshold: 3,
        }
    }

    /// 更新LLM客户端
    pub fn set_llm_client(&mut self, client: Option<Arc<dyn LlmClient>>) {
        self.llm_client = client;
        if let Ok(mut state) = self.state.lock() {
            state.llm_status = if self.llm_client.is_some() {
                HybridLlmStatus::Connecting
            } else {
                HybridLlmStatus::Unavailable
            };
        }
    }

    /// 启用/禁用自动降级
    pub fn set_auto_fallback(&mut self, enabled: bool) {
        self.auto_fallback_enabled = enabled;
    }

    /// 禁用LLM（强制使用规则引擎）
    pub fn disable_llm(&mut self) {
        if let Ok(mut state) = self.state.lock() {
            state.llm_status = HybridLlmStatus::Disabled;
            state.current_mode = EditorMode::RuleBased;
        }
    }

    /// 启用LLM
    pub fn enable_llm(&mut self) {
        if self.llm_client.is_some() {
            if let Ok(mut state) = self.state.lock() {
                state.llm_status = HybridLlmStatus::Connecting;
            }
        }
    }

    /// 获取当前LLM状态
    pub fn llm_status(&self) -> HybridLlmStatus {
        self.state.lock().map(|s| s.llm_status).unwrap_or(HybridLlmStatus::Unavailable)
    }

    /// 获取当前编辑模式
    pub fn current_mode(&self) -> EditorMode {
        self.state.lock().map(|s| s.current_mode).unwrap_or(EditorMode::RuleBased)
    }

    /// 获取性能统计
    pub fn stats(&self) -> HybridStats {
        self.state.lock().map(|s| s.stats.clone()).unwrap_or_default()
    }

    /// 获取降级历史
    pub fn fallback_history(&self) -> Vec<FallbackEvent> {
        self.state.lock().map(|s| s.fallback_history.clone()).unwrap_or_default()
    }

    /// 创建编辑计划（主入口）
    ///
    /// 智能选择LLM或规则引擎：
    /// 1. 如果LLM可用且启用，使用LLM
    /// 2. 如果LLM不可用或失败，自动降级到规则引擎
    /// 3. 记录所有降级事件用于分析
    pub fn create_plan(
        &self,
        request_text: &str,
        task_id: u64,
        context: PlannerContext,
    ) -> EditPlan {
        // 检查是否需要刷新LLM状态
        self.check_llm_status();

        let llm_status = self.llm_status();
        let current_mode = self.current_mode();

        // 如果LLM不可用或禁用，直接使用规则引擎
        if !matches!(llm_status, HybridLlmStatus::Available | HybridLlmStatus::Connecting) {
            // 记录fallback事件
            self.record_fallback(FallbackReason::NoClient, request_text);
            return self.create_rule_plan(request_text, task_id, context);
        }

        // 尝试使用LLM
        if let Some(ref client) = self.llm_client {
            if client.is_ready() {
                let start_time = Instant::now();
                match self.try_llm_plan(client, request_text, task_id, context.clone()) {
                    Ok(plan) => {
                        let elapsed = start_time.elapsed().as_millis() as f64;
                        if let Ok(mut state) = self.state.lock() {
                            state.stats.record_llm_success(elapsed);
                            if current_mode != EditorMode::Llm {
                                state.current_mode = EditorMode::Llm;
                            }
                        }

                        return plan;
                    }
                    Err(reason) => {
                        if let Ok(mut state) = self.state.lock() {
                            state.stats.record_llm_failure();
                        }
                        self.record_fallback(reason, request_text);
                    }
                }
            }
        }

        // LLM不可用或失败，降级到规则引擎
        self.create_rule_plan(request_text, task_id, context)
    }

    /// 尝试使用LLM创建计划
    fn try_llm_plan(
        &self,
        client: &Arc<dyn LlmClient>,
        request_text: &str,
        task_id: u64,
        context: PlannerContext,
    ) -> Result<EditPlan, FallbackReason> {
        use crate::llm::{LlmMessage, LlmRequest, Role};
        use crate::prompt::{PromptContext, PromptSystem, PromptType};

        let prompt_system = PromptSystem::with_defaults();
        let mut prompt_ctx = PromptContext {
            engine_name: "Bevy".into(),
            project_name: "AgentEdit".into(),
            selected_entities: context.scene_entity_names.join(", "),
            ..PromptContext::default()
        };

        if let Some(ref mem_ctx) = context.memory_context {
            let layered = crate::prompt::LayeredContext::default().with_memory(mem_ctx.clone());
            prompt_ctx.layered_context = Some(layered);
        }

        let sys = prompt_system.build_prompt(PromptType::TaskPlanning, &prompt_ctx);

        let cot_instruction = format!(
            "User request: \"{}\"\n\
             Available tools: {}\n\
             Existing entities: {}\n\n\
             Think step by step and output valid JSON:\n\
             {{\"title\": \"...\", \"summary\": \"...\", \"risk_level\": \"...\", \"mode\": \"...\", \"steps\": []}}",
            request_text,
            context.available_tools.join(", "),
            context.scene_entity_names.join(", ")
        );

        let messages = vec![
            LlmMessage { role: Role::System, content: sys },
            LlmMessage { role: Role::User, content: cot_instruction },
        ];

        let model = if let Ok(model) = std::env::var("LLM_MODEL") {
            if !model.is_empty() { model } else { crate::llm::models::OPENAI_FAST.to_string() }
        } else {
            crate::llm::models::OPENAI_FAST.to_string()
        };

        let request = LlmRequest {
            model,
            messages,
            max_tokens: Some(1000),
            temperature: Some(0.3),
            tools: None,
        };

        // 同步调用异步LLM
        let runtime = crate::planner::get_llm_runtime();
        let result = runtime.block_on(client.chat(request));

        match result {
            Ok(response) => {
                if let Some(plan) = self.parse_llm_response(&response.content, task_id, request_text) {
                    if !plan.steps.is_empty() {
                        if let Ok(mut state) = self.state.lock() {
                            state.llm_status = HybridLlmStatus::Available;
                        }
                        return Ok(plan);
                    }
                    return Err(FallbackReason::InvalidResponse);
                }
                Err(FallbackReason::InvalidResponse)
            }
            Err(_) => Err(FallbackReason::ApiError),
        }
    }

    /// 解析LLM响应
    fn parse_llm_response(&self, raw: &str, task_id: u64, request_text: &str) -> Option<EditPlan> {
        use crate::permission::OperationRisk;

        let json_str = if let Some(start) = raw.find("```json") {
            let after_start = &raw[start + 7..];
            after_start.find("```").map(|end| after_start[..end].trim()).unwrap_or(after_start.trim())
        } else if let Some(start) = raw.find('{') {
            let end = raw[start..].find('}').map(|i| start + i + 1)?;
            &raw[start..end]
        } else {
            return None;
        };

        let parsed: serde_json::Value = serde_json::from_str(json_str).ok()?;

        let risk_level = match parsed.get("risk_level").and_then(|v| v.as_str()) {
            Some("Safe") => OperationRisk::Safe,
            Some("LowRisk") => OperationRisk::LowRisk,
            Some("MediumRisk") => OperationRisk::MediumRisk,
            Some("HighRisk") => OperationRisk::HighRisk,
            Some("Destructive") => OperationRisk::Destructive,
            _ => OperationRisk::LowRisk,
        };

        let mode = match parsed.get("mode").and_then(|v| v.as_str()) {
            Some("Direct") => ExecutionMode::Direct,
            Some("Team") => ExecutionMode::Team,
            _ => ExecutionMode::Plan,
        };

        let steps: Vec<crate::plan::EditPlanStep> = parsed
            .get("steps")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .enumerate()
                    .map(|(i, step)| {
                        let target = match step.get("target_module").and_then(|v| v.as_str()) {
                            Some("Code") => TargetModule::Code,
                            Some("Asset") => TargetModule::Asset,
                            _ => TargetModule::Scene,
                        };
                        crate::plan::EditPlanStep {
                            id: step.get("step_id").and_then(|v| v.as_str())
                                .unwrap_or(&format!("step_{}", i + 1))
                                .to_string(),
                            title: step.get("title").and_then(|v| v.as_str())
                                .unwrap_or("unknown")
                                .to_string(),
                            target_module: target,
                            action_description: step.get("action").and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string(),
                            risk: risk_level.clone(),
                            validation_requirements: Vec::new(),
                        }
                    })
                    .collect()
            })
            .unwrap_or_default();

        let title = parsed
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or("Unnamed Plan")
            .to_string();

        Some(EditPlan {
            id: format!("hybrid_plan_{}", task_id),
            task_id,
            title,
            summary: request_text.to_string(),
            mode,
            steps,
            risk_level,
            status: EditPlanStatus::Draft,
        })
    }

    /// 使用规则引擎创建计划
    fn create_rule_plan(
        &self,
        request_text: &str,
        task_id: u64,
        context: PlannerContext,
    ) -> EditPlan {
        if let Ok(mut state) = self.state.lock() {
            state.current_mode = EditorMode::RuleBased;
        }
        self.rule_planner.create_plan(request_text, task_id, context)
    }

    /// 检查LLM状态
    ///
    /// 使用 LlmConnectionChecker 检测 LLM 可用性，
    /// 根据连续失败计数决定是否降级。
    fn check_llm_status(&self) {
        let now = Instant::now();

        {
            let mut state = match self.state.lock() {
                Ok(s) => s,
                Err(_) => return,
            };

            // 避免频繁检查
            if let Some(last_instant) = state.last_check {
                if now.duration_since(last_instant) < self.check_interval {
                    return;
                }
            }

            state.last_check = Some(now);
        }

        // 使用 LlmConnectionChecker 检测
        if let Some(ref checker) = self.connection_checker {
            let is_available = checker.is_available();
            let consecutive_failures = checker.consecutive_failures();

            if let Ok(mut state) = self.state.lock() {
                // 根据检测结果更新状态
                match (state.llm_status, is_available, consecutive_failures) {
                    // 连接中 → 可用
                    (HybridLlmStatus::Connecting, true, _) => {
                        if state.llm_status != HybridLlmStatus::Available {
                            state.llm_status = HybridLlmStatus::Available;
                            state.current_mode = EditorMode::Llm;
                        }
                    }
                    // 可用 → 不可用（连续失败超过阈值）
                    (HybridLlmStatus::Available, false, failures) if failures >= self.failure_threshold => {
                        state.llm_status = HybridLlmStatus::Unavailable;
                        state.current_mode = EditorMode::RuleBased;
                    }
                    // 可用 → 不可用（检测到失败但未超过阈值）
                    (HybridLlmStatus::Available, false, failures) if failures < self.failure_threshold => {
                        // 仍然可用，但记录失败
                    }
                    // 不可用 → 可用（检测到恢复）
                    (HybridLlmStatus::Unavailable, true, _) => {
                        state.llm_status = HybridLlmStatus::Available;
                        state.current_mode = EditorMode::Llm;
                    }
                    _ => {}
                }

                state.stats.last_llm_check = Some(crate::types::current_timestamp());
            }
        } else if let Some(ref client) = self.llm_client {
            // 回退到原有的简单检测
            let is_ready = client.is_ready();
            if let Ok(mut state) = self.state.lock() {
                match (state.llm_status, is_ready) {
                    (HybridLlmStatus::Connecting, true) => state.llm_status = HybridLlmStatus::Available,
                    (HybridLlmStatus::Available, false) => state.llm_status = HybridLlmStatus::Unavailable,
                    _ => {}
                }
                state.stats.last_llm_check = Some(crate::types::current_timestamp());
            }
        }
    }

    /// 记录降级事件
    fn record_fallback(&self, reason: FallbackReason, request: &str) {
        let from_status = self.llm_status();

        let event = FallbackEvent {
            timestamp: crate::types::current_timestamp(),
            reason: reason.clone(),
            from_status,
            request_preview: request.chars().take(50).collect(),
        };

        if let Ok(mut state) = self.state.lock() {
            state.fallback_history.push(event);

            while state.fallback_history.len() > self.max_history_size {
                state.fallback_history.remove(0);
            }

            state.stats.record_fallback(&reason);
        }
    }

    /// 获取状态描述
    pub fn status_description(&self) -> String {
        let status = self.llm_status();
        let mode = self.current_mode();
        let stats = self.stats();

        match (status, mode) {
            (HybridLlmStatus::Available, EditorMode::Llm) => {
                format!(
                    "🟢 LLM模式 | 成功率: {:.1}% | 平均响应: {:.0}ms",
                    stats.llm_success_rate() * 100.0,
                    stats.avg_llm_response_ms
                )
            }
            (HybridLlmStatus::Connecting, EditorMode::Llm) => {
                "🟡 LLM连接中...".to_string()
            }
            (HybridLlmStatus::Unavailable, EditorMode::RuleBased) => {
                let last_reason = self.fallback_history()
                    .last()
                    .map(|e| e.reason.clone())
                    .unwrap_or(FallbackReason::NoClient);
                format!(
                    "🔴 规则引擎模式 | 降级: {}次 | 原因: {:?}",
                    stats.total_fallbacks,
                    last_reason
                )
            }
            (HybridLlmStatus::Disabled, EditorMode::RuleBased) => {
                "⚫ 规则引擎模式 (LLM已禁用)".to_string()
            }
            _ => {
                "⚪ 未知状态".to_string()
            }
        }
    }
}

impl Default for HybridEditorController {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// 单元测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn test_context() -> PlannerContext {
        PlannerContext {
            task_id: 1,
            available_tools: vec!["create_entity".into(), "update_component".into()],
            scene_entity_names: vec!["Player".into()],
            memory_context: None,
        }
    }

    #[test]
    fn test_new_controller() {
        let controller = HybridEditorController::new();
        assert_eq!(controller.llm_status(), HybridLlmStatus::Unavailable);
        assert_eq!(controller.current_mode(), EditorMode::RuleBased);
    }

    #[test]
    fn test_status_description() {
        let controller = HybridEditorController::new();
        let desc = controller.status_description();
        assert!(desc.contains("规则引擎模式"));
    }

    #[test]
    fn test_fallback_event_recorded() {
        let controller = HybridEditorController::new();
        let _plan = controller.create_plan("创建一个敌人", 1, test_context());
        
        let history = controller.fallback_history();
        assert!(!history.is_empty());
        assert_eq!(history[0].reason, FallbackReason::NoClient);
    }

    #[test]
    fn test_stats_tracking() {
        let controller = HybridEditorController::new();
        let _plan = controller.create_plan("创建一个敌人", 1, test_context());
        
        let stats = controller.stats();
        assert_eq!(stats.rule_fallbacks, 1);
        assert_eq!(stats.total_fallbacks, 1);
    }

    #[test]
    fn test_plan_creation_fallback() {
        let controller = HybridEditorController::new();
        let plan = controller.create_plan("创建一个红色敌人", 1, test_context());
        
        assert_eq!(plan.task_id, 1);
        assert!(!plan.title.is_empty());
        assert!(!plan.steps.is_empty());
    }

    #[test]
    fn test_llm_disable() {
        let mut controller = HybridEditorController::new();
        controller.disable_llm();
        
        assert_eq!(controller.llm_status(), HybridLlmStatus::Disabled);
        assert_eq!(controller.current_mode(), EditorMode::RuleBased);
    }

    #[test]
    fn test_stats_success_rate() {
        let controller = HybridEditorController::new();
        let stats = controller.stats();
        
        // 无LLM时，成功率为0
        assert_eq!(stats.llm_success_rate(), 0.0);
    }
}
