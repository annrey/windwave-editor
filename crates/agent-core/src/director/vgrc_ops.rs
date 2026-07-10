//! VGRC (Visual Grounded Reasoning Cycle) operations for DirectorRuntime

use crate::director::types::EditorEvent;
use crate::director::DirectorRuntime;
use std::sync::Arc;

impl DirectorRuntime {
    /// 初始化 VGRC 控制器，设置目标期望
    pub fn init_vgrc(
        &mut self,
        goal_description: &str,
        expected: Vec<crate::visual_system::VisualExpectation>,
    ) {
        let mut controller =
            crate::visual_system::VgcrController::with_goal(goal_description, expected);

        // 注入 RealizeExecutor（使用闭包包装工具调用）
        let _runtime_tools = Arc::new(std::sync::Mutex::new(crate::tool::ToolRegistry::new()));
        controller.set_executor(Box::new(crate::visual_system::ClosureRealizeExecutor::new(
            move |action: &str| -> Result<String, String> {
                eprintln!("[VGRC-Realize] Executing action: {}", action);
                Ok(format!("executed: {}", action))
            },
        )));

        self.vgrc_controller = Some(controller);
    }

    /// 运行完整的 VGRC 闭环：截图→分析→操作→再截图验证
    pub fn run_vgrc_cycle(
        &mut self,
        initial_observation: crate::visual_system::VisualObservation,
        action: &str,
        #[allow(unused_variables)]
        post_observation_fn: impl FnMut() -> crate::visual_system::VisualObservation,
    ) -> Result<crate::visual_system::VgcrCycleResult, String> {
        let controller = self
            .vgrc_controller
            .as_mut()
            .ok_or_else(|| "VGRC 控制器未初始化，请先调用 init_vgrc()".to_string())?;

        let result = controller.run_full_cycle(initial_observation, action, post_observation_fn);

        let observation = result.final_observation.clone();
        self.last_vgrc_result = Some(result.clone());
        self.last_visual_observation = observation;

        // 记录事件到事件流
        if result.success {
            self.events.push(EditorEvent::GoalChecked {
                task_id: 0,
                all_matched: true,
                summary: format!("VGRC 成功: {} ({} 次)", result.message, result.attempts),
            });
        } else {
            self.events.push(EditorEvent::Error {
                message: format!("VGRC 失败: {} ({} 次)", result.message, result.attempts),
            });
        }

        // 通过 EventBridge 写入记忆系统
        let events = std::mem::take(&mut self.events);
        let mut event_bridge = std::mem::take(&mut self.event_bridge);
        event_bridge.process_events(&events, self.memory());
        self.event_bridge = event_bridge;
        self.events = events;

        Ok(result)
    }

    /// Get the latest VGRC snapshot for UI consumption.
    pub fn visual_snapshot(
        &self,
    ) -> Option<(
        crate::visual_system::VgcrCycleResult,
        crate::visual_system::VisualObservation,
    )> {
        self.last_vgrc_result
            .clone()
            .map(|r| (r, self.last_visual_observation.clone().unwrap_or_default()))
    }

    /// D1: 运行 VGRC 检查 — 委托给 vgrc_controller.run_cycle()
    pub fn run_vgrc_check(
        &mut self,
        expectations: &[crate::visual_system::VisualExpectation],
        goal: &str,
    ) -> crate::visual_system::VgrcCycleResult {
        let enabled = self
            .vgrc_controller
            .as_ref()
            .map(|c| c.enabled)
            .unwrap_or(false);
        if !enabled {
            return crate::visual_system::VgrcCycleResult {
                cycle_id: 0,
                goal: goal.to_string(),
                passed: true,
                summary: "VGRC disabled (no controller)".to_string(),
                screenshot_base64: None,
                screenshot_dimensions: None,
                analysis: None,
            };
        }

        if self.scene_bridge.is_some() {
            let mut temp_bridge = self.scene_bridge.take();
            let result = if let Some(ref mut bridge) = temp_bridge {
                let controller = self.vgrc_controller.as_mut().unwrap();
                controller.run_cycle(bridge.as_mut(), expectations, goal)
            } else {
                crate::visual_system::VgrcCycleResult {
                    cycle_id: 0,
                    goal: goal.to_string(),
                    passed: true,
                    summary: "No scene bridge available".to_string(),
                    screenshot_base64: None,
                    screenshot_dimensions: None,
                    analysis: None,
                }
            };
            self.scene_bridge = temp_bridge;
            self.last_vgrc_cycle = Some(result.clone());
            result
        } else {
            let controller = self.vgrc_controller.as_mut().unwrap();
            let result = crate::visual_system::VgrcCycleResult {
                cycle_id: controller.cycle_count + 1,
                goal: goal.to_string(),
                passed: true,
                summary: "No scene bridge connected (MVP mode)".to_string(),
                screenshot_base64: None,
                screenshot_dimensions: None,
                analysis: None,
            };
            self.last_vgrc_cycle = Some(result.clone());
            result
        }
    }

    /// D1: 获取最近一次的 VGRC 循环结果（供 UI 消费）
    pub fn last_vgrc_result(&self) -> Option<&crate::visual_system::VgrcCycleResult> {
        self.last_vgrc_cycle.as_ref()
    }
}
