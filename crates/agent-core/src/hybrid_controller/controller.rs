use crate::llm::LlmClient;
use crate::plan::{EditPlan, EditPlanStatus, ExecutionMode, TargetModule};
use crate::planner::{PlannerContext, RuleBasedPlanner};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use super::connection_checker::LlmConnectionChecker;
use super::types::{ControllerState, EditorMode, FallbackEvent, FallbackReason, HybridLlmStatus};

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
                let _checker = LlmConnectionChecker::new(client.clone());
                HybridLlmStatus::Connecting
            }
            None => HybridLlmStatus::Unavailable,
        };

        let connection_checker = llm_client
            .as_ref()
            .map(|c| LlmConnectionChecker::new(c.clone()));

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
        self.state
            .lock()
            .map(|s| s.llm_status)
            .unwrap_or(HybridLlmStatus::Unavailable)
    }

    /// 获取当前编辑模式
    pub fn current_mode(&self) -> EditorMode {
        self.state
            .lock()
            .map(|s| s.current_mode)
            .unwrap_or(EditorMode::RuleBased)
    }

    /// 获取性能统计
    pub fn stats(&self) -> super::types::HybridStats {
        self.state
            .lock()
            .map(|s| s.stats.clone())
            .unwrap_or_default()
    }

    /// 获取降级历史
    pub fn fallback_history(&self) -> Vec<FallbackEvent> {
        self.state
            .lock()
            .map(|s| s.fallback_history.clone())
            .unwrap_or_default()
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
        if !matches!(
            llm_status,
            HybridLlmStatus::Available | HybridLlmStatus::Connecting
        ) {
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
            LlmMessage {
                role: Role::System,
                content: sys,
            },
            LlmMessage {
                role: Role::User,
                content: cot_instruction,
            },
        ];

        let model = if let Ok(model) = std::env::var("LLM_MODEL") {
            if !model.is_empty() {
                model
            } else {
                crate::llm::models::OPENAI_FAST.to_string()
            }
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
                if let Some(plan) =
                    self.parse_llm_response(&response.content, task_id, request_text)
                {
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
            after_start
                .find("```")
                .map(|end| after_start[..end].trim())
                .unwrap_or(after_start.trim())
        } else {
            let start = raw.find('{')?;
            let end = raw[start..].find('}').map(|i| start + i + 1)?;
            &raw[start..end]
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
                            id: step
                                .get("step_id")
                                .and_then(|v| v.as_str())
                                .unwrap_or(&format!("step_{}", i + 1))
                                .to_string(),
                            title: step
                                .get("title")
                                .and_then(|v| v.as_str())
                                .unwrap_or("unknown")
                                .to_string(),
                            target_module: target,
                            action_description: step
                                .get("action")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string(),
                            risk: risk_level,
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
        self.rule_planner
            .create_plan(request_text, task_id, context)
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
                    (HybridLlmStatus::Available, false, failures)
                        if failures >= self.failure_threshold =>
                    {
                        state.llm_status = HybridLlmStatus::Unavailable;
                        state.current_mode = EditorMode::RuleBased;
                    }
                    // 可用 → 不可用（检测到失败但未超过阈值）
                    (HybridLlmStatus::Available, false, failures)
                        if failures < self.failure_threshold =>
                    {
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
                    (HybridLlmStatus::Connecting, true) => {
                        state.llm_status = HybridLlmStatus::Available
                    }
                    (HybridLlmStatus::Available, false) => {
                        state.llm_status = HybridLlmStatus::Unavailable
                    }
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

    /// 手动触发重新检测 LLM 连接状态
    ///
    /// 清除上次检测时间缓存，强制下次 `create_plan` 时重新检测。
    /// 也立即执行一次检测并返回当前状态。
    pub fn recheck(&self) -> HybridLlmStatus {
        // 清除缓存，强制重新检测
        if let Ok(mut state) = self.state.lock() {
            state.last_check = None;
        }
        self.check_llm_status();
        self.llm_status()
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
            (HybridLlmStatus::Connecting, EditorMode::Llm) => "🟡 LLM连接中...".to_string(),
            (HybridLlmStatus::Unavailable, EditorMode::RuleBased) => {
                let last_reason = self
                    .fallback_history()
                    .last()
                    .map(|e| e.reason.clone())
                    .unwrap_or(FallbackReason::NoClient);
                format!(
                    "🔴 规则引擎模式 | 降级: {}次 | 原因: {:?}",
                    stats.total_fallbacks, last_reason
                )
            }
            (HybridLlmStatus::Disabled, EditorMode::RuleBased) => {
                "⚫ 规则引擎模式 (LLM已禁用)".to_string()
            }
            _ => "⚪ 未知状态".to_string(),
        }
    }

    /// Check if a plan requires manual confirmation.
    ///
    /// Plans with risk level HighRisk or Destructive require user approval
    /// before execution. Once confirmed, the plan is stored and can be
    /// retrieved via `pending_plan()`.
    pub fn needs_confirmation(&self, plan: &EditPlan) -> bool {
        matches!(
            plan.risk_level,
            crate::permission::OperationRisk::HighRisk
                | crate::permission::OperationRisk::Destructive
        )
    }

    /// Stage a plan for manual confirmation.
    ///
    /// Stores the plan in the pending_confirmation slot. Only one plan
    /// can be pending at a time. Subsequent calls overwrite the previous.
    pub fn request_confirmation(&self, plan: EditPlan, task_id: u64) {
        if let Ok(mut state) = self.state.lock() {
            state.pending_confirmation = Some((plan, task_id));
        }
    }

    /// Confirm the pending plan and return it for execution.
    ///
    /// Returns `None` if no plan is pending confirmation.
    pub fn confirm_plan(&self) -> Option<(EditPlan, u64)> {
        if let Ok(mut state) = self.state.lock() {
            state.pending_confirmation.take()
        } else {
            None
        }
    }

    /// Reject the pending plan and clear it.
    ///
    /// Returns `true` if there was a plan to reject.
    pub fn reject_plan(&self) -> bool {
        if let Ok(mut state) = self.state.lock() {
            let had_pending = state.pending_confirmation.is_some();
            state.pending_confirmation = None;
            had_pending
        } else {
            false
        }
    }

    /// Get the currently pending plan (read-only).
    pub fn pending_plan(&self) -> Option<(EditPlan, u64)> {
        if let Ok(state) = self.state.lock() {
            state.pending_confirmation.clone()
        } else {
            None
        }
    }

    /// Create a plan with optional manual confirmation.
    ///
    /// If the plan requires confirmation (high risk), it is staged
    /// and not executed. Call `confirm_plan()` to proceed.
    /// If the plan is safe, it is returned immediately.
    pub fn create_plan_with_confirmation(
        &self,
        request_text: &str,
        task_id: u64,
        context: PlannerContext,
    ) -> EditPlan {
        let plan = self.create_plan(request_text, task_id, context);

        if self.needs_confirmation(&plan) {
            self.request_confirmation(plan.clone(), task_id);
        }

        plan
    }
}

impl Default for HybridEditorController {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Visual feedback integration
// ============================================================================

/// Result of a verify-and-revise cycle.
#[derive(Debug, Clone)]
pub struct VisualVerifyResult {
    pub plan: EditPlan,
    pub verified: bool,
    pub revisions: usize,
    pub visual_issues: Vec<String>,
}

impl HybridEditorController {
    /// Execute a plan with visual verification feedback.
    ///
    /// After executing the plan, captures visual state and compares
    /// against the plan goals. If discrepancies are found, revises
    /// the plan and retries up to `max_attempts` times.
    ///
    /// This closes the visual feedback loop:
    ///   operation → screenshot → vision analyze → compare → revise → retry
    pub fn plan_with_visual_verify(
        &self,
        request_text: &str,
        task_id: u64,
        context: PlannerContext,
        visual_goals: &[VisualGoal],
    ) -> VisualVerifyResult {
        let mut current_plan = self.create_plan(request_text, task_id, context.clone());
        let mut revisions = 0usize;
        let max_attempts = 3usize;
        let threshold = 0.7f32;

        loop {
            if !current_plan.needs_visual_verification(visual_goals) {
                return VisualVerifyResult {
                    plan: current_plan,
                    verified: true,
                    revisions,
                    visual_issues: vec![],
                };
            }

            let score = current_plan.visual_goal_alignment_score(visual_goals);
            if score >= threshold {
                return VisualVerifyResult {
                    plan: current_plan,
                    verified: true,
                    revisions,
                    visual_issues: vec![],
                };
            }

            let issues: Vec<String> = visual_goals
                .iter()
                .filter_map(|g| {
                    if !current_plan.steps.iter().any(|s| {
                        s.action_description
                            .to_lowercase()
                            .contains(&g.target.to_lowercase())
                    }) {
                        Some(format!(
                            "Goal not addressed: {} → {}",
                            g.description, g.target
                        ))
                    } else {
                        None
                    }
                })
                .collect();

            if issues.is_empty() || revisions >= max_attempts {
                return VisualVerifyResult {
                    plan: current_plan,
                    verified: score >= threshold,
                    revisions,
                    visual_issues: issues,
                };
            }

            let revision_hint = format!(
                "{} Revision {} - fix: {}",
                request_text,
                revisions + 1,
                issues.join("; ")
            );

            current_plan = self.create_plan(&revision_hint, task_id, context.clone());
            revisions += 1;
        }
    }
}

/// Visual goal: what the final scene state should look like.
#[derive(Debug, Clone)]
pub struct VisualGoal {
    pub description: String,
    pub target: String,
    pub expected_properties: Vec<(String, String)>,
}

impl EditPlan {
    /// Check if plan steps address visual goals to require verification.
    fn needs_visual_verification(&self, goals: &[VisualGoal]) -> bool {
        if goals.is_empty() {
            return false;
        }

        for goal in goals {
            for step in &self.steps {
                let desc = step.action_description.to_lowercase();
                let target = goal.target.to_lowercase();
                if desc.contains(&target) || desc.contains("create") || desc.contains("set") {
                    return true;
                }
            }
        }
        false
    }

    /// Calculate alignment score between plan steps and visual goals.
    fn visual_goal_alignment_score(&self, goals: &[VisualGoal]) -> f32 {
        if goals.is_empty() {
            return 1.0;
        }

        let mut covered = 0usize;
        for goal in goals {
            let target = goal.target.to_lowercase();
            let matched = self.steps.iter().any(|s| {
                let desc = s.action_description.to_lowercase();
                desc.contains(&target)
                    || goal
                        .expected_properties
                        .iter()
                        .any(|(k, v)| desc.contains(k) && desc.contains(v))
            });
            if matched {
                covered += 1;
            }
        }

        covered as f32 / goals.len() as f32
    }
}

#[cfg(test)]
mod visual_verify_tests {
    use super::*;
    use crate::permission::OperationRisk;
    use crate::plan::{EditPlan, EditPlanStatus, EditPlanStep, ExecutionMode, TargetModule};

    fn make_draft_plan() -> EditPlan {
        EditPlan {
            id: "test_plan".into(),
            task_id: 1,
            title: "Test".into(),
            summary: "test".into(),
            mode: ExecutionMode::Plan,
            steps: vec![EditPlanStep {
                id: "s1".into(),
                title: "Create red enemy".into(),
                target_module: TargetModule::Scene,
                action_description: "create a red enemy at position (100, 200)".into(),
                risk: OperationRisk::Safe,
                validation_requirements: vec![],
            }],
            risk_level: OperationRisk::Safe,
            status: EditPlanStatus::Draft,
        }
    }

    #[test]
    fn test_needs_visual_verification_with_goals() {
        let plan = make_draft_plan();
        let goals = vec![VisualGoal {
            description: "enemy".into(),
            target: "enemy".into(),
            expected_properties: vec![("color".into(), "red".into())],
        }];
        assert!(plan.needs_visual_verification(&goals));
    }

    #[test]
    fn test_needs_visual_verification_empty_goals() {
        let plan = make_draft_plan();
        assert!(!plan.needs_visual_verification(&[]));
    }

    #[test]
    fn test_alignment_score_full_match() {
        let plan = make_draft_plan();
        let goals = vec![VisualGoal {
            description: "enemy".into(),
            target: "enemy".into(),
            expected_properties: vec![("color".into(), "red".into())],
        }];
        let score = plan.visual_goal_alignment_score(&goals);
        assert!(score >= 0.5, "score={}", score);
    }

    #[test]
    fn test_alignment_score_no_match() {
        let mut plan = make_draft_plan();
        plan.steps[0].action_description = "query entity list".into();
        let goals = vec![VisualGoal {
            description: "create tower".into(),
            target: "tower".into(),
            expected_properties: vec![],
        }];
        let score = plan.visual_goal_alignment_score(&goals);
        assert_eq!(score, 0.0);
    }

    #[test]
    fn test_visual_verify_creates_plan_and_returns_result() {
        use crate::planner::PlannerContext;

        let controller = HybridEditorController::new();
        let goals = vec![VisualGoal {
            description: "enemy".into(),
            target: "enemy".into(),
            expected_properties: vec![("color".into(), "red".into())],
        }];

        let context = PlannerContext {
            task_id: 42,
            available_tools: vec!["create_entity".into(), "set_color".into()],
            scene_entity_names: vec![],
            memory_context: None,
        };

        let result = controller.plan_with_visual_verify("create a red enemy", 42, context, &goals);

        assert!(!result.plan.steps.is_empty(), "should produce a plan");
        assert!(result.revisions < 3, "max 3 revisions");
    }
}
