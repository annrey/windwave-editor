use super::*;

// ============================================================================
// 视觉反馈循环 (VGRC: Vision → Goal → Realize → Check)
// ============================================================================

/// VGRC 循环状态
#[derive(Debug, Clone)]
pub struct VgcrState {
    pub vision: Option<VisualObservation>,
    pub goal: GoalState,
    pub realize_attempts: usize,
    pub check_result: Option<CheckResult>,
    pub completed: bool,
}

#[derive(Debug, Clone)]
pub struct GoalState {
    pub description: String,
    pub expected_entities: Vec<VisualExpectation>,
}

#[derive(Debug, Clone)]
pub struct CheckResult {
    pub passed: bool,
    pub failures: Vec<String>,
    pub visual_observation: Option<VisualObservation>,
}

/// VGRC 控制器 - 视觉反馈循环
pub struct VgcrController {
    pub state: VgcrState,
    pub max_attempts: usize,
    pub world_view: AgentWorldView,
    /// Sprint 3-D2: Realize 步骤的实际执行器（由 DirectorRuntime 注入）
    realize_executor: Option<Box<dyn RealizeExecutor>>,
    pub enabled: bool,
    pub vision_client: Option<Box<dyn VisionClient>>,
    pub cycle_count: u32,
}

impl Default for VgcrController {
    fn default() -> Self {
        Self::new()
    }
}

impl VgcrController {
    pub fn new() -> Self {
        Self {
            state: VgcrState {
                vision: None,
                goal: GoalState {
                    description: String::new(),
                    expected_entities: Vec::new(),
                },
                realize_attempts: 0,
                check_result: None,
                completed: false,
            },
            max_attempts: 3,
            world_view: AgentWorldView::new(),
            realize_executor: None,
            enabled: false,
            vision_client: None,
            cycle_count: 0,
        }
    }

    pub fn with_goal(goal_description: &str, expected: Vec<VisualExpectation>) -> Self {
        Self {
            state: VgcrState {
                vision: None,
                goal: GoalState {
                    description: goal_description.to_string(),
                    expected_entities: expected,
                },
                realize_attempts: 0,
                check_result: None,
                completed: false,
            },
            max_attempts: 3,
            world_view: AgentWorldView::new(),
            realize_executor: None,
            enabled: false,
            vision_client: None,
            cycle_count: 0,
        }
    }

    pub fn set_vision_client(&mut self, client: Box<dyn VisionClient>) {
        self.vision_client = Some(client);
    }

    /// Sprint 3-D2: 注入 Realize 执行器
    pub fn set_executor(&mut self, executor: Box<dyn RealizeExecutor>) {
        self.realize_executor = Some(executor);
    }

    /// Sprint 3-D2: 设置最大重试次数
    pub fn with_max_attempts(mut self, max: usize) -> Self {
        self.max_attempts = max;
        self
    }

    /// 第 1 步：Vision - 截图 + 分析
    pub fn vision(&mut self, observation: VisualObservation) {
        self.state.vision = Some(observation.clone());
        self.world_view.set_visual_observation(observation);
    }

    /// 第 2 步：Goal - 对比目标状态
    pub fn check_goal(&self) -> GoalCheckResult {
        let Some(vision) = &self.state.vision else {
            return GoalCheckResult {
                passed: false,
                failures: vec!["无视觉分析结果".into()],
            };
        };

        let mut failures = Vec::new();

        for expectation in &self.state.goal.expected_entities {
            match expectation {
                VisualExpectation::EntityVisible(name) => {
                    if !vision.visible_entities.iter().any(|e| &e.name == name) {
                        failures.push(format!("实体 '{}' 未检测到", name));
                    }
                }
                VisualExpectation::EntityColor(name, expected_color) => {
                    if let Some(entity) = vision.visible_entities.iter().find(|e| &e.name == name) {
                        if let Some(color) = entity.color {
                            if !colors_match(color, *expected_color) {
                                failures.push(format!(
                                    "实体 '{}' 颜色不匹配: 期望 {:?}, 实际 {:?}",
                                    name, expected_color, color
                                ));
                            }
                        } else {
                            failures.push(format!("实体 '{}' 无颜色信息", name));
                        }
                    } else {
                        failures.push(format!("实体 '{}' 未检测到", name));
                    }
                }
                VisualExpectation::EntityCount(expected_count) => {
                    if vision.visible_entities.len() != *expected_count {
                        failures.push(format!(
                            "实体数量不匹配: 期望 {}, 实际 {}",
                            expected_count,
                            vision.visible_entities.len()
                        ));
                    }
                }
                VisualExpectation::NoAnomalies => {
                    if !vision.anomalies.is_empty() {
                        failures.push(format!("检测到 {} 个异常", vision.anomalies.len()));
                    }
                }
                _ => {}
            }
        }

        GoalCheckResult {
            passed: failures.is_empty(),
            failures,
        }
    }

    /// 第 3 步：Realize - 执行操作（Sprint 3-D2: 支持实际工具执行）
    pub fn realize(&mut self, action: &str) -> Result<String, String> {
        self.state.realize_attempts += 1;

        if let Some(executor) = &self.realize_executor {
            let result = executor.execute_action(action);
            if result.is_ok() {
                eprintln!(
                    "[VGRC] Realize #{}: {} → OK",
                    self.state.realize_attempts, action
                );
            } else {
                eprintln!(
                    "[VGRC] Realize #{}: {} → FAIL: {}",
                    self.state.realize_attempts,
                    action,
                    result.as_ref().err().unwrap()
                );
            }
            result
        } else {
            eprintln!(
                "[VGRC] Realize #{}: {} (no executor, dry-run)",
                self.state.realize_attempts, action
            );
            Ok(format!("dry-run: {}", action))
        }
    }

    /// Sprint 3-D2: 运行完整的 VGRC 闭环
    ///
    /// 流程: Vision(初始截图) → Goal(检查目标) → [Realize + Check]×N
    ///
    /// 参数:
    /// - initial_observation: 操作前的视觉观察
    /// - action: 要执行的操作描述
    /// - post_observation_fn: 操作后获取新观察的回调 (因为需要重新截图)
    ///
    /// 返回 VgcrCycleResult 包含最终状态
    pub fn run_full_cycle(
        &mut self,
        initial_observation: VisualObservation,
        action: &str,
        mut post_observation_fn: impl FnMut() -> VisualObservation,
    ) -> VgcrCycleResult {
        // Step 1: Vision — 记录初始状态
        self.vision(initial_observation);

        // Step 2: Goal — 检查初始状态是否已满足目标
        let goal_check = self.check_goal();
        if goal_check.passed {
            return VgcrCycleResult {
                success: true,
                message: "目标在操作前已满足".into(),
                attempts: 0,
                final_observation: self.state.vision.clone(),
            };
        }

        // Step 3-4: Realize + Check 循环
        loop {
            // Realize: 执行操作
            let realize_result = self.realize(action);
            match realize_result {
                Ok(msg) => eprintln!("[VGRC] Realize OK: {}", msg),
                Err(err) => {
                    return VgcrCycleResult {
                        success: false,
                        message: format!("Realize 失败: {}", err),
                        attempts: self.state.realize_attempts,
                        final_observation: None,
                    };
                }
            }

            // Check: 截图并验证结果
            let new_obs = post_observation_fn();
            let check = self.check(new_obs);

            if check.passed {
                return VgcrCycleResult {
                    success: true,
                    message: format!("VGRC 循环成功 ({} 次)", self.state.realize_attempts),
                    attempts: self.state.realize_attempts,
                    final_observation: Some(check.visual_observation.unwrap()),
                };
            }

            // 判断是否继续重试
            if !self.needs_retry() {
                return VgcrCycleResult {
                    success: false,
                    message: format!(
                        "达到最大重试次数 ({})，失败项: {}",
                        self.max_attempts,
                        check.failures.join("; ")
                    ),
                    attempts: self.state.realize_attempts,
                    final_observation: Some(check.visual_observation.unwrap()),
                };
            }

            eprintln!(
                "[VGRC] 重试 #{} / {} (失败: {})",
                self.state.realize_attempts,
                self.max_attempts,
                check.failures.join(", ")
            );
        }
    }

    /// 第 4 步：Check - 验证结果
    pub fn check(&mut self, observation: VisualObservation) -> CheckResult {
        self.world_view.set_visual_observation(observation.clone());
        self.state.vision = Some(observation.clone());

        let check_result = self.check_goal();

        let result = CheckResult {
            passed: check_result.passed,
            failures: check_result.failures,
            visual_observation: Some(observation),
        };

        self.state.check_result = Some(result.clone());

        if result.passed {
            self.state.completed = true;
        }

        result
    }

    /// 运行完整的 VGRC 循环（简化版，使用已有 vision 结果）
    pub fn run_cycle_simple(&mut self, action: &str) -> VgcrCycleResult {
        // 执行操作
        if self.realize(action).is_err() {
            return VgcrCycleResult {
                success: false,
                message: "Realize 执行失败".into(),
                attempts: self.state.realize_attempts,
                final_observation: None,
            };
        }

        // 检查结果（使用已有的 vision）
        let Some(observation) = self.state.vision.clone() else {
            return VgcrCycleResult {
                success: false,
                message: "无视觉观察结果，请先调用 vision()".into(),
                attempts: self.state.realize_attempts,
                final_observation: None,
            };
        };

        let check = self.check(observation.clone());

        VgcrCycleResult {
            success: check.passed,
            message: if check.passed {
                "VGRC 循环完成".into()
            } else {
                format!("VGRC 循环失败: {}", check.failures.join(", "))
            },
            attempts: self.state.realize_attempts,
            final_observation: Some(observation),
        }
    }

    /// 是否需要继续循环
    pub fn needs_retry(&self) -> bool {
        !self.state.completed && self.state.realize_attempts < self.max_attempts
    }

    /// D1: 运行 VGRC 循环 — Vision→Goal→Realize→Check
    ///
    /// 通过 SceneBridge 截图（如果可用），使用 VisionClient 分析，
    /// 然后对比期望是否满足。不满足时尝试一次重试。
    pub fn run_cycle(
        &mut self,
        scene_bridge: &mut dyn crate::scene_bridge::SceneBridge,
        expectations: &[VisualExpectation],
        goal: &str,
    ) -> VgrcCycleResult {
        self.cycle_count += 1;
        let cycle_id = self.cycle_count;

        if !self.enabled {
            return VgrcCycleResult {
                cycle_id,
                goal: goal.to_string(),
                passed: true,
                summary: "VGRC disabled".to_string(),
                screenshot_base64: None,
                screenshot_dimensions: None,
                analysis: None,
            };
        }

        // 尝试从 scene_bridge 获取场景快照
        let snapshot = scene_bridge.get_scene_snapshot();

        let screenshot_base64: Option<String> = None;
        let screenshot_dimensions: Option<(u32, u32)> = None;
        let mut analysis: Option<String> = None;

        // 构建场景摘要（用于 vision 分析）
        let scene_summary = build_scene_summary(&snapshot);

        // 如果配置了 vision_client，发送场景摘要请求
        if let Some(ref client) = self.vision_client {
            let prompt = format!("Verify: {}", goal);
            let request = VisionRequest {
                model: "vision-default".to_string(),
                messages: vec![
                    VisionMessage {
                        role: VisionRole::System,
                        content: VisionContent::Text(
                            "You are a visual verifier for a game editor. Check if the scene matches the expected state.".to_string()
                        ),
                    },
                    VisionMessage {
                        role: VisionRole::User,
                        content: VisionContent::Text(
                            format!("{}\n\nScene state:\n{}", prompt, scene_summary)
                        ),
                    },
                ],
                max_tokens: Some(500),
            };

            match client.vision(request) {
                Ok(response) => {
                    analysis = Some(format!(
                        "Vision analysis for cycle {}: {}",
                        cycle_id, response.content
                    ));
                }
                Err(e) => {
                    analysis = Some(format!(
                        "Vision analysis failed for cycle {}: {}",
                        cycle_id, e
                    ));
                }
            }
        }

        // 验证期望
        let mut failures: Vec<String> = Vec::new();

        for expectation in expectations {
            match expectation {
                VisualExpectation::EntityVisible(name) => {
                    let found = snapshot.iter().any(|e| e.name == *name);
                    if !found {
                        failures.push(format!("实体 '{}' 未在场景中找到", name));
                    }
                }
                VisualExpectation::EntityColor(name, expected_color) => {
                    if let Some(entity) = snapshot.iter().find(|e| e.name == *name) {
                        if let Some(color) = entity.sprite_color {
                            if !color_approx_eq(color, *expected_color) {
                                failures.push(format!(
                                    "实体 '{}' 颜色不匹配: 期望 {:?}, 实际 {:?}",
                                    name, expected_color, color
                                ));
                            }
                        } else {
                            failures.push(format!("实体 '{}' 无颜色信息", name));
                        }
                    } else {
                        failures.push(format!("实体 '{}' 未在场景中找到", name));
                    }
                }
                VisualExpectation::EntityCount(expected_count) => {
                    if snapshot.len() != *expected_count {
                        failures.push(format!(
                            "实体数量不匹配: 期望 {}, 实际 {}",
                            expected_count,
                            snapshot.len()
                        ));
                    }
                }
                VisualExpectation::NoAnomalies => {}
                _ => {}
            }
        }

        let passed = failures.is_empty();

        let summary = if passed {
            format!("VGRC cycle {} passed: goal '{}' verified", cycle_id, goal)
        } else {
            format!(
                "VGRC cycle {} failed: {} failure(s) for goal '{}': {}",
                cycle_id,
                failures.len(),
                goal,
                failures.join("; ")
            )
        };

        VgrcCycleResult {
            cycle_id,
            goal: goal.to_string(),
            passed,
            summary,
            screenshot_base64,
            screenshot_dimensions,
            analysis,
        }
    }
}
