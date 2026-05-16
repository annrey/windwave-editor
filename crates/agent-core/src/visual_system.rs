//! Visual System - Sprint 3: 让 Agent 真正"看见"编辑器世界
//!
//! 实现三层"看"的能力：
//! 1. 结构化读取 - SceneIndex → 实体/组件/层级的精确数据
//! 2. 视觉截图 - Screenshot → Vision LLM 分析的视觉理解
//! 3. 融合感知 - 结构化数据 + 视觉分析 → Agent 的世界模型

use crate::types::{EntityId, current_timestamp};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

// ============================================================================
// 第 1 层：结构化世界视图
// ============================================================================

/// 组件摘要
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ComponentSummary {
    pub type_name: String,
    pub properties: HashMap<String, serde_json::Value>,
}

/// 实体详情 - 完整的实体信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityDetail {
    pub id: EntityId,
    pub name: String,
    pub components: Vec<ComponentSummary>,
    pub children: Vec<EntityId>,
    pub parent: Option<EntityId>,
}

/// 层级关系
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParentChildRelation {
    pub parent_id: EntityId,
    pub parent_name: String,
    pub child_id: EntityId,
    pub child_name: String,
}

/// 场景度量
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneMetrics {
    pub total_entities: usize,
    pub component_types: Vec<String>,
    pub avg_depth: f32,
    pub max_depth: usize,
}

/// 世界快照 - 完整的场景状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldSnapshot {
    pub entities: Vec<EntityDetail>,
    pub hierarchy: Vec<ParentChildRelation>,
    pub timestamp: u64,
    pub metrics: SceneMetrics,
}

impl WorldSnapshot {
    pub fn new() -> Self {
        Self {
            entities: Vec::new(),
            hierarchy: Vec::new(),
            timestamp: current_timestamp(),
            metrics: SceneMetrics {
                total_entities: 0,
                component_types: Vec::new(),
                avg_depth: 0.0,
                max_depth: 0,
            },
        }
    }

    /// 生成 LLM 可读的世界描述
    pub fn describe_for_llm(&self, detail_level: DetailLevel) -> String {
        match detail_level {
            DetailLevel::Brief => self.brief_summary(),
            DetailLevel::Normal => self.full_description(),
            DetailLevel::Detailed => self.detailed_with_components(),
        }
    }

    /// 简要摘要
    fn brief_summary(&self) -> String {
        format!(
            "场景包含 {} 个实体，{} 种组件类型",
            self.metrics.total_entities,
            self.metrics.component_types.len()
        )
    }

    /// 完整描述
    fn full_description(&self) -> String {
        let mut parts = Vec::new();
        parts.push(format!("## 场景概览 ({} 实体)\n", self.metrics.total_entities));
        
        for entity in &self.entities {
            parts.push(format!(
                "- {} (ID: {}): {} 个组件",
                entity.name,
                entity.id.0,
                entity.components.len()
            ));
        }
        
        parts.join("\n")
    }

    /// 详细组件描述
    fn detailed_with_components(&self) -> String {
        let mut parts = Vec::new();
        parts.push("## 完整场景状态\n".into());
        
        for entity in &self.entities {
            parts.push(format!("### {} (ID: {})\n", entity.name, entity.id.0));
            
            for comp in &entity.components {
                parts.push(format!("- {}: {:?}", comp.type_name, comp.properties));
            }
            
            if !entity.children.is_empty() {
                parts.push(format!("  子实体: {:?}", entity.children));
            }
            
            parts.push("\n".into());
        }
        
        parts.join("\n")
    }

    /// 转为 JSON 格式（供 Agent 推理）
    pub fn to_structured_json(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or(serde_json::json!({}))
    }
}

impl Default for WorldSnapshot {
    fn default() -> Self {
        Self::new()
    }
}

/// 详细程度
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetailLevel {
    Brief,      // 仅实体数量和类型
    Normal,     // 实体列表和组件数量
    Detailed,   // 完整组件属性
}

// ============================================================================
// 第 2 层：视觉截图与 Vision LLM 分析
// ============================================================================

/// 截图 artifact
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenshotArtifact {
    pub path: PathBuf,
    pub width: u32,
    pub height: u32,
    pub timestamp: u64,
    pub format: String,  // "png", "jpg", etc.
    pub base64_data: Option<String>,  // 可选：base64 编码的图像数据
}

impl ScreenshotArtifact {
    pub fn new(path: PathBuf, width: u32, height: u32) -> Self {
        Self {
            path,
            width,
            height,
            timestamp: current_timestamp(),
            format: "png".into(),
            base64_data: None,
        }
    }
}

/// 视觉观察结果 - Vision LLM 分析截图后的输出
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisualObservation {
    /// 检测到的实体
    pub visible_entities: Vec<VisualEntity>,
    /// 检测到的异常
    pub anomalies: Vec<Anomaly>,
    /// 置信度 (0.0-1.0)
    pub confidence: f32,
    /// 原始 LLM 响应文本
    pub raw_response: Option<String>,
}

/// 视觉检测到的实体
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisualEntity {
    pub name: String,
    pub detected_type: String,  // "player", "enemy", "npc", "environment"
    pub position: Option<[f32; 3]>,
    pub color: Option<[f32; 4]>,
    pub bounding_box: Option<[f32; 4]>,  // [x, y, width, height]
    pub confidence: f32,
}

/// 检测到的异常
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Anomaly {
    pub description: String,
    pub severity: AnomalySeverity,
    pub location: Option<[f32; 3]>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AnomalySeverity {
    Low,
    Medium,
    High,
}

/// 视觉期望 - 用于验证操作结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VisualExpectation {
    EntityVisible(String),
    EntityColor(String, [f32; 4]),
    EntityPosition(String, [f32; 3]),
    EntityCount(usize),
    NoAnomalies,
    Custom(String),
}

/// Vision LLM 请求
#[derive(Debug, Clone)]
pub struct VisionRequest {
    pub model: String,
    pub messages: Vec<VisionMessage>,
    pub max_tokens: Option<u32>,
}

/// Vision LLM 消息
#[derive(Debug, Clone)]
pub struct VisionMessage {
    pub role: VisionRole,
    pub content: VisionContent,
}

#[derive(Debug, Clone)]
pub enum VisionRole {
    User,
    Assistant,
    System,
}

#[derive(Debug, Clone)]
pub enum VisionContent {
    Text(String),
    Image { data: Vec<u8>, format: String },
    MultiModal { text: String, image_data: Vec<u8> },
}

/// Vision LLM 响应
#[derive(Debug, Clone)]
pub struct VisionResponse {
    pub content: String,
    pub usage: VisionUsage,
}

#[derive(Debug, Clone)]
pub struct VisionUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

/// Vision LLM 客户端 trait
pub trait VisionClient: Send + Sync {
    fn vision(&self, request: VisionRequest) -> Result<VisionResponse, VisionError>;
}

/// Vision 错误
#[derive(Debug, thiserror::Error)]
pub enum VisionError {
    #[error("LLM error: {0}")]
    LlmError(String),
    #[error("Screenshot error: {0}")]
    ScreenshotError(String),
    #[error("Parse error: {0}")]
    ParseError(String),
    #[error("IO error: {0}")]
    IoError(String),
}

// ============================================================================
// 第 3 层：融合感知 + Agent 世界模型
// ============================================================================

/// 场景变化摘要
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneChangeSummary {
    pub timestamp: u64,
    pub entities_created: Vec<String>,
    pub entities_deleted: Vec<String>,
    pub entities_modified: Vec<String>,
    pub components_changed: Vec<ComponentChange>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentChange {
    pub entity_name: String,
    pub component_type: String,
    pub property: String,
    pub old_value: Option<serde_json::Value>,
    pub new_value: Option<serde_json::Value>,
}

/// Agent 世界视图 - Agent 的"眼睛"
pub struct AgentWorldView {
    /// 结构化数据
    pub snapshot: WorldSnapshot,
    /// 视觉分析结果
    pub visual_observation: Option<VisualObservation>,
    /// 上次操作后的变化
    pub last_change: Option<SceneChangeSummary>,
    /// 世界状态摘要（供 LLM 使用）
    pub summary: String,
    /// 截图历史
    pub screenshot_history: Vec<ScreenshotArtifact>,
    /// 最大截图历史数
    max_screenshot_history: usize,
}

impl AgentWorldView {
    pub fn new() -> Self {
        Self {
            snapshot: WorldSnapshot::new(),
            visual_observation: None,
            last_change: None,
            summary: String::new(),
            screenshot_history: Vec::new(),
            max_screenshot_history: 10,
        }
    }

    /// 更新世界视图（结构化数据）
    pub fn update_snapshot(&mut self, snapshot: WorldSnapshot) {
        // 检测变化
        if let Some(change) = self.detect_change(&snapshot) {
            self.last_change = Some(change);
        }
        
        self.snapshot = snapshot;
        self.summary = self.generate_summary();
    }

    /// 设置视觉分析结果
    pub fn set_visual_observation(&mut self, observation: VisualObservation) {
        self.visual_observation = Some(observation);
        self.summary = self.generate_summary();
    }

    /// 添加截图
    pub fn add_screenshot(&mut self, screenshot: ScreenshotArtifact) {
        self.screenshot_history.push(screenshot);
        // 限制历史数量
        if self.screenshot_history.len() > self.max_screenshot_history {
            self.screenshot_history.remove(0);
        }
    }

    /// 检测变化
    fn detect_change(&self, new: &WorldSnapshot) -> Option<SceneChangeSummary> {
        let old = &self.snapshot;
        
        let mut changes = SceneChangeSummary {
            timestamp: current_timestamp(),
            entities_created: Vec::new(),
            entities_deleted: Vec::new(),
            entities_modified: Vec::new(),
            components_changed: Vec::new(),
        };

        // 检测创建/删除
        let old_names: std::collections::HashSet<_> = old.entities.iter().map(|e| &e.name).collect();
        let new_names: std::collections::HashSet<_> = new.entities.iter().map(|e| &e.name).collect();
        
        for name in new_names.difference(&old_names) {
            changes.entities_created.push((*name).clone());
        }
        
        for name in old_names.difference(&new_names) {
            changes.entities_deleted.push((*name).clone());
        }

        // 检测修改（简化版）
        changes.entities_modified = new.entities.iter()
            .filter(|e| {
                old.entities.iter().any(|old_e| {
                    old_e.name == e.name && old_e.components.len() != e.components.len()
                })
            })
            .map(|e| e.name.clone())
            .collect();

        if changes.entities_created.is_empty() 
            && changes.entities_deleted.is_empty() 
            && changes.entities_modified.is_empty() {
            return None;
        }

        Some(changes)
    }

    /// 生成摘要
    fn generate_summary(&self) -> String {
        let mut parts = Vec::new();
        
        // 结构化数据摘要
        parts.push(self.snapshot.describe_for_llm(DetailLevel::Brief));
        
        // 视觉分析摘要
        if let Some(visual) = &self.visual_observation {
            parts.push(format!(
                "\n【视觉分析】检测到 {} 个实体，{} 个异常 (置信度: {:.0}%)",
                visual.visible_entities.len(),
                visual.anomalies.len(),
                visual.confidence * 100.0
            ));
        }
        
        // 最近变化
        if let Some(change) = &self.last_change {
            parts.push(format!(
                "\n【最近变化】创建 {} 个，删除 {} 个，修改 {} 个",
                change.entities_created.len(),
                change.entities_deleted.len(),
                change.entities_modified.len()
            ));
        }
        
        parts.join("\n")
    }

    /// 生成 LLM 可读的完整世界描述
    pub fn describe(&self) -> String {
        format!(
            "【当前场景】({})\n{}\n\n【视觉分析】\n{}\n\n【最近变化】\n{}",
            chrono::Utc::now().format("%Y-%m-%d %H:%M:%S"),
            self.snapshot.describe_for_llm(DetailLevel::Normal),
            self.visual_description(),
            self.change_description()
        )
    }

    /// 视觉描述
    fn visual_description(&self) -> String {
        match &self.visual_observation {
            Some(obs) => {
                let mut parts = Vec::new();
                for entity in &obs.visible_entities {
                    parts.push(format!(
                        "- {}: {} (置信度: {:.0}%)",
                        entity.name,
                        entity.detected_type,
                        entity.confidence * 100.0
                    ));
                }
                for anomaly in &obs.anomalies {
                    parts.push(format!("- 异常: {} ({:?})", anomaly.description, anomaly.severity));
                }
                if parts.is_empty() {
                    "(无视觉分析)".into()
                } else {
                    parts.join("\n")
                }
            }
            None => "(未进行视觉分析)".into(),
        }
    }

    /// 变化描述
    fn change_description(&self) -> String {
        match &self.last_change {
            Some(change) => {
                let mut parts = Vec::new();
                for name in &change.entities_created {
                    parts.push(format!("- 创建: {}", name));
                }
                for name in &change.entities_deleted {
                    parts.push(format!("- 删除: {}", name));
                }
                for name in &change.entities_modified {
                    parts.push(format!("- 修改: {}", name));
                }
                if parts.is_empty() {
                    "(无变化)".into()
                } else {
                    parts.join("\n")
                }
            }
            None => "(无变化记录)".into(),
        }
    }

    /// 比较操作前后的状态差异
    pub fn diff_since(&self, after: &AgentWorldView) -> SceneDiff {
        SceneDiff::compute(self, after)
    }
}

impl Default for AgentWorldView {
    fn default() -> Self {
        Self::new()
    }
}

/// 场景差异
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneDiff {
    pub entities_added: Vec<EntityDetail>,
    pub entities_removed: Vec<EntityDetail>,
    pub entities_modified: Vec<EntityModification>,
    pub component_changes: Vec<ComponentChange>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityModification {
    pub entity: EntityDetail,
    pub changed_components: Vec<String>,
}

impl SceneDiff {
    pub fn compute(before: &AgentWorldView, after: &AgentWorldView) -> Self {
        let mut diff = Self {
            entities_added: Vec::new(),
            entities_removed: Vec::new(),
            entities_modified: Vec::new(),
            component_changes: Vec::new(),
        };

        let before_map: HashMap<_, _> = before.snapshot.entities.iter()
            .map(|e| (e.name.clone(), e.clone()))
            .collect();
        let after_map: HashMap<_, _> = after.snapshot.entities.iter()
            .map(|e| (e.name.clone(), e.clone()))
            .collect();

        // 添加的实体
        for (name, entity) in &after_map {
            if !before_map.contains_key(name) {
                diff.entities_added.push(entity.clone());
            }
        }

        // 删除的实体
        for (name, entity) in &before_map {
            if !after_map.contains_key(name) {
                diff.entities_removed.push(entity.clone());
            }
        }

        // 修改的实体
        for (name, after_entity) in &after_map {
            if let Some(before_entity) = before_map.get(name) {
                if before_entity.components != after_entity.components {
                    let changed_components: Vec<String> = after_entity.components.iter()
                        .filter(|c| {
                            !before_entity.components.iter().any(|bc| {
                                bc.type_name == c.type_name && bc.properties == c.properties
                            })
                        })
                        .map(|c| c.type_name.clone())
                        .collect();
                    
                    if !changed_components.is_empty() {
                        diff.entities_modified.push(EntityModification {
                            entity: after_entity.clone(),
                            changed_components,
                        });
                    }
                }
            }
        }

        diff
    }
}

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
}

impl VgcrController {
    pub fn new(goal_description: &str, expected: Vec<VisualExpectation>) -> Self {
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
        }
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
                                failures.push(format!("实体 '{}' 颜色不匹配: 期望 {:?}, 实际 {:?}", name, expected_color, color));
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
                        failures.push(format!("实体数量不匹配: 期望 {}, 实际 {}", expected_count, vision.visible_entities.len()));
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
                eprintln!("[VGRC] Realize #{}: {} → OK", self.state.realize_attempts, action);
            } else {
                eprintln!("[VGRC] Realize #{}: {} → FAIL: {}", self.state.realize_attempts, action, result.as_ref().err().unwrap());
            }
            result
        } else {
            eprintln!("[VGRC] Realize #{}: {} (no executor, dry-run)", self.state.realize_attempts, action);
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
                    message: format!("达到最大重试次数 ({})，失败项: {}", 
                        self.max_attempts, check.failures.join("; ")),
                    attempts: self.state.realize_attempts,
                    final_observation: Some(check.visual_observation.unwrap()),
                };
            }

            eprintln!("[VGRC] 重试 #{} / {} (失败: {})", 
                self.state.realize_attempts, self.max_attempts, 
                check.failures.join(", "));
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
    pub fn run_cycle(&mut self, action: &str) -> VgcrCycleResult {
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
        !self.state.completed 
            && self.state.realize_attempts < self.max_attempts
    }
}

#[derive(Debug, Clone)]
pub struct GoalCheckResult {
    pub passed: bool,
    pub failures: Vec<String>,
}
/// VGRC 循环结果
#[derive(Debug, Clone)]
pub struct VgcrCycleResult {
    pub success: bool,
    pub message: String,
    pub attempts: usize,
    pub final_observation: Option<VisualObservation>,
}

/// Realize 步骤的回调接口 — 由 DirectorRuntime 注入实际工具执行能力
pub trait RealizeExecutor: Send + Sync {
    fn execute_action(&self, action: &str) -> Result<String, String>;
}

/// 简单的基于闭包的 Realize 执行器（用于测试）
pub struct ClosureRealizeExecutor<F: Fn(&str) -> Result<String, String> + Send + Sync> {
    executor: F,
}

impl<F: Fn(&str) -> Result<String, String> + Send + Sync> ClosureRealizeExecutor<F> {
    pub fn new(f: F) -> Self {
        Self { executor: f }
    }
}

impl<F: Fn(&str) -> Result<String, String> + Send + Sync> RealizeExecutor for ClosureRealizeExecutor<F> {
    fn execute_action(&self, action: &str) -> Result<String, String> {
        (self.executor)(action)
    }
}

/// 颜色匹配（允许小误差）
fn colors_match(a: [f32; 4], b: [f32; 4]) -> bool {
    a.iter().zip(b.iter()).all(|(x, y)| (x - y).abs() < 0.1)
}

// ============================================================================
// 测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_world_snapshot_describe() {
        let mut snapshot = WorldSnapshot::new();
        snapshot.entities.push(EntityDetail {
            id: EntityId(1),
            name: "Player".into(),
            components: vec![ComponentSummary {
                type_name: "Transform".into(),
                properties: HashMap::new(),
            }],
            children: Vec::new(),
            parent: None,
        });
        snapshot.metrics.total_entities = 1;
        snapshot.metrics.component_types = vec!["Transform".into()];

        let desc = snapshot.describe_for_llm(DetailLevel::Normal);
        assert!(desc.contains("Player"));
        assert!(desc.contains("1"));
    }

    #[test]
    fn test_agent_world_view() {
        let mut view = AgentWorldView::new();
        
        let snapshot = WorldSnapshot::new();
        view.update_snapshot(snapshot);
        
        assert!(!view.summary.is_empty());
    }

    #[test]
    fn test_vgcr_controller() {
        let mut controller = VgcrController::new(
            "创建一个红色敌人",
            vec![
                VisualExpectation::EntityVisible("Enemy".into()),
                VisualExpectation::EntityColor("Enemy".into(), [1.0, 0.0, 0.0, 1.0]),
            ]
        );

        // 模拟视觉观察
        let observation = VisualObservation {
            visible_entities: vec![VisualEntity {
                name: "Enemy".into(),
                detected_type: "enemy".into(),
                position: Some([100.0, 0.0, 0.0]),
                color: Some([1.0, 0.0, 0.0, 1.0]),
                bounding_box: None,
                confidence: 0.9,
            }],
            anomalies: Vec::new(),
            confidence: 0.9,
            raw_response: None,
        };

        controller.vision(observation);
        
        let goal_check = controller.check_goal();
        assert!(goal_check.passed);
    }

    #[test]
    fn test_colors_match() {
        assert!(colors_match([1.0, 0.0, 0.0, 1.0], [1.0, 0.0, 0.0, 1.0]));
        assert!(colors_match([1.0, 0.0, 0.0, 1.0], [0.95, 0.0, 0.0, 1.0]));  // 允许误差
        assert!(!colors_match([1.0, 0.0, 0.0, 1.0], [0.0, 0.0, 1.0, 1.0]));
    }

    #[test]
    fn test_scene_diff() {
        let mut before = AgentWorldView::new();
        before.update_snapshot(WorldSnapshot::new());

        let mut after = AgentWorldView::new();
        let mut snapshot = WorldSnapshot::new();
        snapshot.entities.push(EntityDetail {
            id: EntityId(1),
            name: "NewEntity".into(),
            components: Vec::new(),
            children: Vec::new(),
            parent: None,
        });
        snapshot.metrics.total_entities = 1;
        after.update_snapshot(snapshot);

        let diff = before.diff_since(&after);
        assert_eq!(diff.entities_added.len(), 1);
        assert_eq!(diff.entities_added[0].name, "NewEntity");
    }

    // ====================================================================
    // Sprint 3-D2: VGRC 增强功能测试
    // ====================================================================

    #[test]
    fn test_realize_with_executor() {
        let mut controller = VgcrController::new(
            "test goal",
            vec![VisualExpectation::EntityVisible("Player".into())],
        );

        let call_count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let cc = call_count.clone();
        controller.set_executor(Box::new(ClosureRealizeExecutor::new(move |action| {
            cc.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(format!("done: {}", action))
        })));

        let result = controller.realize("create Player");
        assert!(result.is_ok());
        assert_eq!(call_count.load(std::sync::atomic::Ordering::SeqCst), 1);
        assert_eq!(controller.state.realize_attempts, 1);
    }

    #[test]
    fn test_realize_without_executor_dry_run() {
        let mut controller = VgcrController::new(
            "test goal",
            vec![VisualExpectation::EntityVisible("X".into())],
        );

        let result = controller.realize("do something");
        assert!(result.is_ok());
        assert!(result.unwrap().contains("dry-run"));
    }

    #[test]
    fn test_realize_executor_failure() {
        let mut controller = VgcrController::new(
            "test",
            vec![VisualExpectation::NoAnomalies],
        );
        controller.set_executor(Box::new(ClosureRealizeExecutor::new(|_| Err("tool error".into()))));

        let result = controller.realize("fail action");
        assert!(result.is_err());
        assert_eq!(result.err().unwrap(), "tool error");
    }

    #[test]
    fn test_run_full_cycle_success_first_try() {
        let mut controller = VgcrController::new(
            "创建红色玩家",
            vec![
                VisualExpectation::EntityVisible("Player".into()),
                VisualExpectation::EntityColor("Player".into(), [1.0, 0.0, 0.0, 1.0]),
            ],
        ).with_max_attempts(3);

        let initial_obs = make_test_observation("Player", [0.5, 0.5, 0.5, 1.0]);
        
        let mut attempt = 0;
        let result = controller.run_full_cycle(initial_obs, "move player to red", || {
            attempt += 1;
            make_test_observation("Player", [1.0, 0.0, 0.0, 1.0])
        });

        assert!(result.success);
        assert!(result.message.contains("VGRC"));
        assert!(result.final_observation.is_some());
        assert_eq!(result.attempts, 1);
    }

    #[test]
    fn test_run_full_cycle_goal_already_met() {
        let mut controller = VgcrController::new(
            "目标已满足",
            vec![VisualExpectation::EntityVisible("Existing".into())],
        ).with_max_attempts(2);

        let obs = make_test_observation("Existing", [1.0, 1.0, 1.0, 1.0]);
        let result = controller.run_full_cycle(obs, "no-op", || unreachable!("should not be called"));

        assert!(result.success);
        assert_eq!(result.attempts, 0);
        assert!(result.message.contains("操作前"));
    }

    #[test]
    fn test_run_full_cycle_max_attempts_exceeded() {
        let mut controller = VgcrController::new(
            "需要重试",
            vec![
                VisualExpectation::EntityVisible("Target".into()),
                VisualExpectation::EntityColor("Target".into(), [0.0, 1.0, 0.0, 1.0]),
            ],
        ).with_max_attempts(2);

        let initial_obs = make_test_observation("Target", [1.0, 0.0, 0.0, 1.0]);
        
        let mut call_count = 0;
        let result = controller.run_full_cycle(initial_obs, "change color to green", || {
            call_count += 1;
            make_test_observation("Target", [1.0, 0.0, 0.0, 1.0])
        });

        assert!(!result.success);
        assert!(result.message.contains("最大重试"));
        assert_eq!(result.attempts, 2);
    }

    #[test]
    fn test_vgrc_with_max_attempts_builder() {
        let controller = VgcrController::new("test", vec![])
            .with_max_attempts(5);
        assert_eq!(controller.max_attempts, 5);
    }

    #[test]
    fn test_set_executor_replaces() {
        let mut controller = VgcrController::new("test", vec![]);
        
        controller.set_executor(Box::new(ClosureRealizeExecutor::new(|_| Ok("first".into()))));
        assert_eq!(controller.realize("a").unwrap(), "first");

        controller.set_executor(Box::new(ClosureRealizeExecutor::new(|_| Ok("second".into()))));
        assert_eq!(controller.realize("b").unwrap(), "second");
    }
}

/// 测试辅助函数：创建一个包含指定实体的 VisualObservation
fn make_test_observation(name: &str, color: [f32; 4]) -> VisualObservation {
    VisualObservation {
        visible_entities: vec![VisualEntity {
            name: name.into(),
            detected_type: "entity".into(),
            position: Some([0.0, 0.0, 0.0]),
            color: Some(color),
            bounding_box: None,
            confidence: 0.95,
        }],
        anomalies: Vec::new(),
        confidence: 0.95,
        raw_response: None,
    }
}
