use crate::llm::LlmClient;
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
    pub(crate) state: Arc<Mutex<ConnectionState>>,
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
        state
            .last_result
            .as_ref()
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
                CheckResult::Success {
                    response_time_ms: elapsed,
                }
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
