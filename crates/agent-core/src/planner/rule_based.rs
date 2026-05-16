//! RuleBasedPlanner — keyword-driven edit plan generator.
//!
//! Parses natural-language requests into structured `EditPlan`s via
//! keyword matching for complexity, risk, execution mode, and step building.

use crate::permission::OperationRisk;
use crate::plan::{EditPlan, EditPlanStep, EditPlanStatus, ExecutionMode, TargetModule};
use crate::prompt::{PromptSystem, PromptContext, PromptType};
use super::{ComplexityLevel, PlannerContext, Planner};

pub struct RuleBasedPlanner {
    prompt_system: PromptSystem,
}

impl RuleBasedPlanner {
    pub fn new() -> Self {
        Self {
            prompt_system: PromptSystem::with_defaults(),
        }
    }

    pub fn with_prompt_system(prompt_system: PromptSystem) -> Self {
        Self { prompt_system }
    }

    pub fn build_system_identity(&self, engine_name: &str, project_name: &str) -> String {
        let ctx = PromptContext {
            engine_name: engine_name.into(),
            project_name: project_name.into(),
            ..PromptContext::default()
        };
        self.prompt_system
            .build_prompt(PromptType::SystemIdentity, &ctx)
    }

    /// 根据用户请求文本、任务 ID 和上下文创建编辑计划
    ///
    /// 这是 Planner 的主入口方法，依次完成复杂度估算、风险评估、
    /// 模式选择和步骤构建。
    pub fn create_plan(
        &self,
        request_text: &str,
        task_id: u64,
        context: PlannerContext,
    ) -> EditPlan {
        let complexity = self.estimate_complexity(request_text);
        let risk = self.estimate_risk(request_text);
        let mode = self.choose_execution_mode(&complexity, &risk);
        let steps = self.build_steps(request_text, &context, &mode);

        EditPlan {
            id: format!("plan_{}", task_id),
            task_id,
            title: Self::extract_title(request_text),
            summary: request_text.to_string(),
            mode,
            steps,
            risk_level: risk,
            status: EditPlanStatus::Draft,
        }
    }

    /// 估算请求的复杂度
    ///
    /// 通过关键词检测请求涉及多少个操作领域：
    /// - 场景 (Scene)："创建"/"create"/"添加"/"add"/"放置"/"place"/"删除"/"delete"/"移动"/"move"
    /// - 代码逻辑 (Code)："代码"/"code"/"系统"/"system"/"脚本"/"script"/"逻辑"/"logic"
    /// - 素材 (Asset)："素材"/"asset"/"图片"/"image"/"声音"/"sound"/"纹理"/"texture"
    /// - 视觉 (Vision)："氛围"/"视觉"/"visual"/"颜色"/"color"/"粒子"/"particle"/"光照"/"light"
    ///
    /// 涉及领域数：0-1 -> Simple，2 -> Medium，3+ -> Complex
    fn estimate_complexity(&self, text: &str) -> ComplexityLevel {
        let lower = text.to_lowercase();

        let scene_hit = [
            "创建", "create", "添加", "add", "放置", "place",
            "删除", "delete", "移除", "remove",
            "实体", "entity",
        ]
        .iter()
        .any(|kw| lower.contains(kw));

        let code_hit = [
            "代码", "code", "系统", "system", "脚本", "script",
            "逻辑", "logic", "编程", "program",
        ]
        .iter()
        .any(|kw| lower.contains(kw));

        let asset_hit = [
            "素材", "asset", "图片", "image", "声音", "sound",
            "纹理", "texture", "音乐", "music", "音频", "audio",
            "模型", "model",
        ]
        .iter()
        .any(|kw| lower.contains(kw));

        let visual_hit = [
            "氛围", "视觉", "visual", "颜色", "color", "粒子",
            "particle", "光照", "light", "渲染", "render",
            "特效", "effect", "动画", "animation",
            "红色", "蓝色", "绿色", "黄色", "紫色", "白色", "黑色", "橙色", "粉色", "灰色",
            "red", "blue", "green", "yellow", "purple", "white", "black", "orange", "pink", "gray", "grey",
        ]
        .iter()
        .any(|kw| lower.contains(kw));

        let domain_count =
            [scene_hit, code_hit, asset_hit, visual_hit]
                .iter()
                .filter(|&&h| h)
                .count();

        match domain_count {
            0 | 1 => ComplexityLevel::Simple,
            2 => ComplexityLevel::Medium,
            _ => ComplexityLevel::Complex,
        }
    }

    /// 估算操作的风险等级
    ///
    /// 风险判定规则：
    /// - "删除"/"delete"/"清空"/"clear"/"销毁"/"destroy" -> HighRisk
    /// - "批量"/"batch"/"全部"/"all"/"所有" -> MediumRisk
    /// - 其他情况 -> LowRisk
    /// - 同时命中多个高风险关键词 -> Destructive
    fn estimate_risk(&self, text: &str) -> OperationRisk {
        let lower = text.to_lowercase();

        let has_destructive = [
            "清空", "clear", "销毁", "destroy", "彻底", "wipe",
        ]
        .iter()
        .any(|kw| lower.contains(kw));

        let has_high_risk = [
            "删除", "delete", "移除", "remove",
        ]
        .iter()
        .any(|kw| lower.contains(kw));

        let has_medium_risk = [
            "批量", "batch", "全部", "all", "所有", "批量操作",
        ]
        .iter()
        .any(|kw| lower.contains(kw));

        if has_destructive {
            OperationRisk::Destructive
        } else if has_high_risk {
            OperationRisk::HighRisk
        } else if has_medium_risk {
            OperationRisk::MediumRisk
        } else {
            OperationRisk::LowRisk
        }
    }

    /// 根据复杂度和风险选择执行模式
    ///
    /// 决策规则（按优先级）：
    /// 1. Destructive / HighRisk -> Plan（必须人工确认）
    /// 2. Complex -> Team（需要多 Agent 协作）
    /// 3. Simple + (Safe / LowRisk) -> Direct（可以直接执行）
    /// 4. 其他情况 -> Plan（默认走计划流程）
    fn choose_execution_mode(
        &self,
        complexity: &ComplexityLevel,
        risk: &OperationRisk,
    ) -> ExecutionMode {
        match risk {
            OperationRisk::Destructive | OperationRisk::HighRisk => ExecutionMode::Plan,
            OperationRisk::MediumRisk => match complexity {
                ComplexityLevel::Complex => ExecutionMode::Team,
                _ => ExecutionMode::Plan,
            },
            OperationRisk::Safe | OperationRisk::LowRisk => match complexity {
                ComplexityLevel::Simple => ExecutionMode::Direct,
                ComplexityLevel::Complex => ExecutionMode::Team,
                ComplexityLevel::Medium => ExecutionMode::Plan,
            },
        }
    }

    /// 根据请求文本和上下文构建具体的执行步骤
    ///
    /// 解析中英文关键词，生成对应的 EditPlanStep 列表。
    ///
    /// 支持的意图识别：
    /// - 创建实体："创建"/"create"/"生成"/"spawn" + 实体名
    /// - 设置颜色："红色"/"red"/"蓝色"/"blue"/"绿色"/"green" 等
    /// - 位置关系："右侧"/"right"/"左侧"/"left"/"上方"/"above"/"下方"/"below"
    /// - 删除实体："删除"/"delete" + 实体名
    /// - 批量操作："批量"/"batch"/"全部"/"all"
    fn build_steps(
        &self,
        text: &str,
        context: &PlannerContext,
        mode: &ExecutionMode,
    ) -> Vec<EditPlanStep> {
        let lower = text.to_lowercase();
        let mut steps = Vec::new();
        let mut step_index: usize = 0;

        // ---- 检测创建意图 ----
        let create_keywords = ["创建", "create", "生成", "spawn", "新建", "建立"];
        let is_create = create_keywords.iter().any(|kw| lower.contains(kw));

        if is_create {
            let entity_name = Self::extract_entity_name(text);

            // 步骤1：创建实体
            steps.push(EditPlanStep {
                id: format!("step_{}", step_index),
                title: format!("创建实体: {}", entity_name),
                target_module: TargetModule::Scene,
                action_description: format!(
                    "调用 create_entity 工具，创建名为 \"{}\" 的实体",
                    entity_name
                ),
                risk: OperationRisk::LowRisk,
                validation_requirements: Vec::new(),
            });
            step_index += 1;

            // 检测颜色关键词
            if let Some((color_name, rgba)) = Self::extract_color(text) {
                steps.push(EditPlanStep {
                    id: format!("step_{}", step_index),
                    title: format!("设置 {} 颜色为 {}", entity_name, color_name),
                    target_module: TargetModule::Vision,
                    action_description: format!(
                        "调用 update_component 工具，设置 {} 的 Sprite 颜色为 {:?}",
                        entity_name, rgba
                    ),
                    risk: OperationRisk::LowRisk,
                validation_requirements: Vec::new(),
            });
            step_index += 1;
        }

        // 检测位置关系
            if let Some((position_desc, offset)) = Self::extract_position(text, context) {
                steps.push(EditPlanStep {
                    id: format!("step_{}", step_index),
                    title: format!("放置 {} 在 {}", entity_name, position_desc),
                    target_module: TargetModule::Scene,
                    action_description: format!(
                        "调用 update_component 工具，设置 {} 的 Transform.position 为 {:?}",
                        entity_name, offset
                    ),
                    risk: OperationRisk::LowRisk,
                validation_requirements: Vec::new(),
            });
            step_index += 1;
        }
    }

    // ---- 检测删除意图 ----
        let delete_keywords = ["删除", "delete", "移除", "remove", "销毁", "destroy"];
        let is_delete = delete_keywords.iter().any(|kw| lower.contains(kw));

        if is_delete {
            let entity_name = Self::extract_entity_name(text);

            steps.push(EditPlanStep {
                id: format!("step_{}", step_index),
                title: format!("删除实体: {}", entity_name),
                target_module: TargetModule::Scene,
                action_description: format!(
                    "调用 delete_entity 工具，确认删除实体 \"{}\"",
                    entity_name
                ),
                risk: OperationRisk::HighRisk,
            validation_requirements: Vec::new(),
        });
        step_index += 1;
    }

    // ---- 检测批量意图 ----
        let batch_keywords = ["批量", "batch", "全部", "all", "所有"];
        let is_batch = batch_keywords.iter().any(|kw| lower.contains(kw));

        if is_batch && !is_delete {
            steps.push(EditPlanStep {
                id: format!("step_{}", step_index),
                title: "批量操作".to_string(),
                target_module: TargetModule::Scene,
                action_description: format!(
                    "对场景中 {} 个实体执行批量操作: {}",
                    context.scene_entity_names.len(),
                    text
                ),
                risk: OperationRisk::MediumRisk,
            validation_requirements: Vec::new(),
        });
        step_index += 1;
    }

    // ---- 如果无法识别任何意图，生成一个通用步骤 ----
        if steps.is_empty() {
            steps.push(EditPlanStep {
                id: format!("step_{}", step_index),
                title: "解析用户请求".to_string(),
                target_module: TargetModule::Workflow,
                action_description: format!(
                    "分析并执行用户请求: {}。建议先 query_entities 获取场景状态。",
                    text
                ),
                risk: OperationRisk::LowRisk,
            validation_requirements: Vec::new(),
        });
    }

    // ---- 对于 Team 模式，添加协调步骤 ----
        if *mode == ExecutionMode::Team {
            steps.push(EditPlanStep {
                id: format!("step_{}", step_index),
                title: "多Agent协调".to_string(),
                target_module: TargetModule::Workflow,
                action_description:
                    "将任务分派给多个 Agent 并行执行，并收集各 Agent 的执行结果。"
                        .to_string(),
                risk: OperationRisk::LowRisk,
            validation_requirements: Vec::new(),
        });
    }

    steps
}

    /// 从请求文本中提取实体名称
    ///
    /// 尝试匹配常见的中英文实体名模式，例如：
    /// "创建红色敌人" -> "敌人"
    /// "create a red enemy" -> "enemy"
    fn extract_entity_name(text: &str) -> String {
        let lower = text.to_lowercase();

        // 英文：尝试提取 "create/spawn/delete a/an/the X" 中的 X
        let english_patterns = ["create a ", "create an ", "create the ", "create ",
            "spawn a ", "spawn an ", "spawn the ", "spawn ",
            "delete a ", "delete an ", "delete the ", "delete ",
            "add a ", "add an ", "add the ", "add "];

        for pattern in &english_patterns {
            if let Some(pos) = lower.find(pattern) {
                let after = &lower[pos + pattern.len()..];
                // 取第一个空格前的词作为实体名
                let mut name = after.split_whitespace().next().unwrap_or("entity").to_string();
                // 如果提取到的是颜色词，则取下一个词作为实体名
                let english_colors = ["red", "blue", "green", "yellow", "purple",
                    "white", "black", "orange", "pink", "gray", "grey"];
                if english_colors.contains(&name.as_str()) {
                    // 跳过颜色词，取下一个词
                    let rest = &after[after.find(' ').unwrap_or(0)..].trim();
                    name = rest.split_whitespace().next().unwrap_or("entity").to_string();
                }
                return name;
            }
        }

        // 中文：尝试在动词后找到实体名
        // "创建X" -> X, "删除Y" -> Y
        let chinese_verbs = ["创建", "删除", "移除", "销毁", "生成"];
        for verb in &chinese_verbs {
            if let Some(pos) = text.find(verb) {
                let after = &text[pos + verb.len()..];
                // 中文字符通常连续，取第一个非中文分隔符前的部分
                let name: String = after
                    .chars()
                    .take_while(|c| !c.is_whitespace() && !c.is_ascii_punctuation())
                    .collect();
                if !name.is_empty() {
                    // 去除颜色前缀（红色、蓝色等）
                    let color_prefixes = ["红色", "蓝色", "绿色", "黄色", "紫色", "白色", "黑色", "橙色"];
                    for prefix in &color_prefixes {
                        if let Some(stripped) = name.strip_prefix(prefix) {
                            return stripped.to_string();
                        }
                    }
                    return name;
                }
            }
        }

        // 兜底：尝试从场景已有实体中匹配
        "Entity".to_string()
    }

    /// 从请求文本中提取颜色信息
    ///
    /// 支持中英文颜色名称，返回 (颜色名称, [r, g, b, a])
    fn extract_color(text: &str) -> Option<(String, [f32; 4])> {
        let lower = text.to_lowercase();

        let color_map: &[(&[&str], &str, [f32; 4])] = &[
            (&["红色", "红", "red"], "红色", [1.0, 0.0, 0.0, 1.0]),
            (&["蓝色", "蓝", "blue"], "蓝色", [0.0, 0.0, 1.0, 1.0]),
            (&["绿色", "绿", "green"], "绿色", [0.0, 1.0, 0.0, 1.0]),
            (&["黄色", "黄", "yellow"], "黄色", [1.0, 1.0, 0.0, 1.0]),
            (&["紫色", "紫", "purple"], "紫色", [0.5, 0.0, 0.5, 1.0]),
            (&["白色", "白", "white"], "白色", [1.0, 1.0, 1.0, 1.0]),
            (&["黑色", "黑", "black"], "黑色", [0.0, 0.0, 0.0, 1.0]),
            (&["橙色", "橙", "orange"], "橙色", [1.0, 0.65, 0.0, 1.0]),
            (&["粉色", "粉", "pink"], "粉色", [1.0, 0.75, 0.8, 1.0]),
            (&["灰色", "灰", "gray", "grey"], "灰色", [0.5, 0.5, 0.5, 1.0]),
        ];

        for (keywords, name, rgba) in color_map {
            if keywords.iter().any(|kw| lower.contains(kw)) {
                return Some((name.to_string(), *rgba));
            }
        }

        None
    }

    /// 从请求文本中提取位置信息
    ///
    /// 支持相对于已有实体的位置描述，如：
    /// "放在玩家右侧" -> ("玩家右侧", [player.x + offset, player.y, 0])
    fn extract_position(
        text: &str,
        context: &PlannerContext,
    ) -> Option<(String, [f32; 3])> {
        let lower = text.to_lowercase();

        let default_offset: f32 = 100.0;

        // 检测相对于哪个实体的位置
        // 尝试匹配 "X的右侧" 或 "right of X"
        let position_markers = [
            ("右侧", "right", [default_offset, 0.0, 0.0]),
            ("左边", "left", [-default_offset, 0.0, 0.0]),
            ("左侧", "left", [-default_offset, 0.0, 0.0]),
            ("上方", "above", [0.0, default_offset, 0.0]),
            ("下方", "below", [0.0, -default_offset, 0.0]),
            ("前面", "front", [0.0, 0.0, default_offset]),
            ("后面", "behind", [0.0, 0.0, -default_offset]),
        ];

        for (chinese_dir, english_dir, offset) in &position_markers {
            if lower.contains(chinese_dir) || lower.contains(english_dir) {
                // 尝试找到引用的实体名（在方向词之前）
                let text_before = if let Some(pos) = lower.find(chinese_dir) {
                    &text[..pos]
                } else if let Some(pos) = lower.find(english_dir) {
                    &text[..pos]
                } else {
                    continue;
                };

                let reference_entity = context
                    .scene_entity_names
                    .iter()
                    .find(|name| text_before.to_lowercase().contains(&name.to_lowercase()))
                    .cloned();

                if let Some(ref_entity) = reference_entity {
                    let desc = format!("{}的{}", ref_entity, chinese_dir);
                    return Some((desc, *offset));
                } else if !context.scene_entity_names.is_empty() {
                    // 没有明确引用，使用第一个场景实体作为参考
                    let ref_entity = &context.scene_entity_names[0];
                    let desc = format!("{}的{}", ref_entity, chinese_dir);
                    return Some((desc, *offset));
                }
            }
        }

        None
    }

    /// 从请求文本中提取计划标题
    ///
    /// 取请求的前若干个字符作为标题，限制最大长度 60 字符。
    fn extract_title(text: &str) -> String {
        let max_len = 60;
        let title = text.lines().next().unwrap_or(text).trim();

        if title.chars().count() <= max_len {
            title.to_string()
        } else {
            let truncated: String = title.chars().take(max_len - 3).collect();
            format!("{}...", truncated)
        }
    }
}

impl Default for RuleBasedPlanner {
    fn default() -> Self {
        Self::new()
    }
}

impl Planner for RuleBasedPlanner {
    fn create_plan(
        &self,
        request_text: &str,
        task_id: u64,
        context: PlannerContext,
    ) -> EditPlan {
        let complexity = self.estimate_complexity(request_text);
        let risk = self.estimate_risk(request_text);
        let mode = self.choose_execution_mode(&complexity, &risk);
        let steps = self.build_steps(request_text, &context, &mode);

        EditPlan {
            id: format!("plan_{}", task_id),
            task_id,
            title: Self::extract_title(request_text),
            summary: request_text.to_string(),
            mode,
            steps,
            risk_level: risk,
            status: EditPlanStatus::Draft,
        }
    }

    fn estimate_complexity(&self, text: &str) -> ComplexityLevel {
        let lower = text.to_lowercase();

        let scene_hit = [
            "创建", "create", "添加", "add", "放置", "place",
            "删除", "delete", "移除", "remove",
            "实体", "entity",
        ]
        .iter()
        .any(|kw| lower.contains(kw));

        let code_hit = [
            "代码", "code", "系统", "system", "脚本", "script",
            "逻辑", "logic", "编程", "program",
        ]
        .iter()
        .any(|kw| lower.contains(kw));

        let asset_hit = [
            "素材", "asset", "图片", "image", "声音", "sound",
            "纹理", "texture", "音乐", "music", "音频", "audio",
            "模型", "model",
        ]
        .iter()
        .any(|kw| lower.contains(kw));

        let visual_hit = [
            "氛围", "视觉", "visual", "颜色", "color", "粒子",
            "particle", "光照", "light", "渲染", "render",
            "特效", "effect", "动画", "animation",
            "红色", "蓝色", "绿色", "黄色", "紫色", "白色", "黑色", "橙色", "粉色", "灰色",
            "red", "blue", "green", "yellow", "purple", "white", "black", "orange", "pink", "gray", "grey",
        ]
        .iter()
        .any(|kw| lower.contains(kw));

        let domain_count =
            [scene_hit, code_hit, asset_hit, visual_hit]
                .iter()
                .filter(|&&h| h)
                .count();

        match domain_count {
            0 | 1 => ComplexityLevel::Simple,
            2 => ComplexityLevel::Medium,
            _ => ComplexityLevel::Complex,
        }
    }
}

// ============================================================================

// ============================================================================
// Tests
// ============================================================================


#[cfg(test)]
mod tests {
    use super::*;

    /// 辅助函数：创建测试用 PlannerContext
    fn test_context() -> PlannerContext {
        PlannerContext {
            task_id: 1,
            available_tools: vec![
                "create_entity".into(),
                "update_component".into(),
                "delete_entity".into(),
                "query_entities".into(),
            ],
            scene_entity_names: vec!["Player".into(), "Camera".into(), "Ground".into()],
            memory_context: None,
        }
    }

    #[test]
    fn test_new_planner() {
        let planner = RuleBasedPlanner::new();
        let _ = planner; // 验证构造不 panic
    }

    #[test]
    fn test_default_planner() {
        let planner = RuleBasedPlanner::default();
        let _ = planner;
    }

    #[test]
    fn test_create_plan_simple_create() {
        let planner = RuleBasedPlanner::new();
        let plan = planner.create_plan("创建一个敌人", 1, test_context());

        assert_eq!(plan.task_id, 1);
        assert_eq!(plan.title, "创建一个敌人");
        assert!(!plan.steps.is_empty());
        assert_eq!(plan.status, EditPlanStatus::Draft);
    }

    #[test]
    fn test_estimate_complexity_simple() {
        let planner = RuleBasedPlanner::new();
        let complexity = planner.estimate_complexity("创建一个敌人");
        assert_eq!(complexity, ComplexityLevel::Simple);
    }

    #[test]
    fn test_estimate_complexity_medium_color() {
        let planner = RuleBasedPlanner::new();
        // 场景 + 视觉 = 2 个领域
        let complexity = planner.estimate_complexity("创建一个红色敌人");
        assert_eq!(complexity, ComplexityLevel::Medium);
    }

    #[test]
    fn test_estimate_complexity_complex() {
        let planner = RuleBasedPlanner::new();
        // 场景 + 视觉 + 代码 = 3 个领域
        let complexity = planner.estimate_complexity("创建一个红色敌人并为其添加AI代码脚本");
        assert_eq!(complexity, ComplexityLevel::Complex);
    }

    #[test]
    fn test_estimate_complexity_code_only() {
        let planner = RuleBasedPlanner::new();
        let complexity = planner.estimate_complexity("生成一个移动脚本");
        assert_eq!(complexity, ComplexityLevel::Simple);
    }

    #[test]
    fn test_estimate_risk_low() {
        let planner = RuleBasedPlanner::new();
        let risk = planner.estimate_risk("创建一个敌人");
        assert_eq!(risk, OperationRisk::LowRisk);
    }

    #[test]
    fn test_estimate_risk_high() {
        let planner = RuleBasedPlanner::new();
        let risk = planner.estimate_risk("删除这个敌人");
        assert_eq!(risk, OperationRisk::HighRisk);
    }

    #[test]
    fn test_estimate_risk_destructive() {
        let planner = RuleBasedPlanner::new();
        let risk = planner.estimate_risk("清空所有实体");
        assert_eq!(risk, OperationRisk::Destructive);
    }

    #[test]
    fn test_estimate_risk_medium_batch() {
        let planner = RuleBasedPlanner::new();
        let risk = planner.estimate_risk("批量修改所有实体颜色");
        assert_eq!(risk, OperationRisk::MediumRisk);
    }

    #[test]
    fn test_choose_execution_mode_direct_simple() {
        let planner = RuleBasedPlanner::new();
        let mode = planner.choose_execution_mode(
            &ComplexityLevel::Simple,
            &OperationRisk::LowRisk,
        );
        assert_eq!(mode, ExecutionMode::Direct);
    }

    #[test]
    fn test_choose_execution_mode_plan_high_risk() {
        let planner = RuleBasedPlanner::new();
        let mode = planner.choose_execution_mode(
            &ComplexityLevel::Simple,
            &OperationRisk::HighRisk,
        );
        assert_eq!(mode, ExecutionMode::Plan);
    }

    #[test]
    fn test_choose_execution_mode_team_complex() {
        let planner = RuleBasedPlanner::new();
        let mode = planner.choose_execution_mode(
            &ComplexityLevel::Complex,
            &OperationRisk::LowRisk,
        );
        assert_eq!(mode, ExecutionMode::Team);
    }

    #[test]
    fn test_choose_execution_mode_destructive() {
        let planner = RuleBasedPlanner::new();
        let mode = planner.choose_execution_mode(
            &ComplexityLevel::Simple,
            &OperationRisk::Destructive,
        );
        assert_eq!(mode, ExecutionMode::Plan);
    }

    #[test]
    fn test_extract_title_short() {
        let title = RuleBasedPlanner::extract_title("创建一个红色敌人");
        assert_eq!(title, "创建一个红色敌人");
    }

    #[test]
    fn test_extract_title_long() {
        let long_text = "这是一个非常长的请求文本，用于测试标题提取功能是否能够正确地截断过长的标题内容，这里需要写很多文字才能超过六十个字符的限制，继续补充更多文字内容以达到测试目的";
        let title = RuleBasedPlanner::extract_title(long_text);
        assert!(title.chars().count() <= 60);
        assert!(title.ends_with("..."));
    }

    #[test]
    fn test_extract_entity_name_chinese() {
        let name = RuleBasedPlanner::extract_entity_name("创建一个红色敌人");
        // 应该提取 "敌人"
        assert!(name.contains("敌人") || name.contains("entity"));
    }

    #[test]
    fn test_extract_entity_name_english() {
        let name = RuleBasedPlanner::extract_entity_name("create a red enemy");
        assert!(name.contains("enemy") || name.contains("entity"));
    }

    #[test]
    fn test_extract_color_red() {
        let (name, rgba) = RuleBasedPlanner::extract_color("创建一个红色敌人").unwrap();
        assert_eq!(name, "红色");
        assert_eq!(rgba, [1.0, 0.0, 0.0, 1.0]);
    }

    #[test]
    fn test_extract_color_blue() {
        let (name, rgba) = RuleBasedPlanner::extract_color("蓝色方块").unwrap();
        assert_eq!(name, "蓝色");
        assert_eq!(rgba, [0.0, 0.0, 1.0, 1.0]);
    }

    #[test]
    fn test_extract_color_english() {
        let (name, rgba) = RuleBasedPlanner::extract_color("create a green enemy").unwrap();
        assert_eq!(name, "绿色");
        assert_eq!(rgba, [0.0, 1.0, 0.0, 1.0]);
    }

    #[test]
    fn test_extract_color_none() {
        let result = RuleBasedPlanner::extract_color("创建一个敌人");
        assert!(result.is_none());
    }

    #[test]
    fn test_extract_position_with_reference() {
        let ctx = test_context();
        let result = RuleBasedPlanner::extract_position("放在玩家右侧", &ctx);
        assert!(result.is_some());
        let (desc, _) = result.unwrap();
        assert!(desc.contains("Player") || desc.contains("玩家"));
    }

    #[test]
    fn test_extract_position_none() {
        let ctx = test_context();
        let result = RuleBasedPlanner::extract_position("创建一个敌人", &ctx);
        assert!(result.is_none());
    }

    #[test]
    fn test_build_steps_create_with_color() {
        let planner = RuleBasedPlanner::new();
        let ctx = test_context();
        let steps = planner.build_steps(
            "创建一个红色敌人",
            &ctx,
            &ExecutionMode::Direct,
        );
        assert!(steps.len() >= 2);
        // 第一个步骤应该是创建实体
        assert!(steps[0].action_description.contains("create_entity"));
        // 第二个步骤应该是设置颜色
        assert!(steps[1].action_description.contains("Sprite"));
    }

    #[test]
    fn test_build_steps_delete() {
        let planner = RuleBasedPlanner::new();
        let ctx = test_context();
        let steps = planner.build_steps(
            "删除敌人",
            &ctx,
            &ExecutionMode::Plan,
        );
        assert!(!steps.is_empty());
        assert!(steps[0].action_description.contains("delete_entity"));
    }

    #[test]
    fn test_build_steps_generic() {
        let planner = RuleBasedPlanner::new();
        let ctx = test_context();
        let steps = planner.build_steps(
            "分析当前场景的性能",
            &ctx,
            &ExecutionMode::Plan,
        );
        // 无法识别具体意图时应该生成一个通用步骤
        assert!(!steps.is_empty());
    }

    #[test]
    fn test_build_steps_team_mode() {
        let planner = RuleBasedPlanner::new();
        let ctx = test_context();
        let steps = planner.build_steps(
            "创建红色敌人并添加AI脚本",
            &ctx,
            &ExecutionMode::Team,
        );
        // Team 模式应该附加协调步骤
        assert!(steps.iter().any(|s| s.title.contains("协调")));
    }

    #[test]
    fn test_plan_id_format() {
        let planner = RuleBasedPlanner::new();
        let plan = planner.create_plan("测试", 42, test_context());
        assert_eq!(plan.id, "plan_42");
    }

    #[test]
    fn test_build_steps_batch_operation() {
        let planner = RuleBasedPlanner::new();
        let ctx = test_context();
        let steps = planner.build_steps(
            "批量修改颜色",
            &ctx,
            &ExecutionMode::Plan,
        );
        assert!(!steps.is_empty());
        assert!(steps.iter().any(|s| s.risk == OperationRisk::MediumRisk));
    }

    #[test]
    fn test_build_system_identity_prompt() {
        let planner = RuleBasedPlanner::new();
        let prompt = planner.build_system_identity("bevy", "MyGame");
        assert!(prompt.contains("bevy"));
        assert!(prompt.contains("MyGame"));
        assert!(prompt.len() > 100);
    }

    #[test]
    fn test_with_prompt_system_custom() {
        let mut ps = PromptSystem::with_defaults();
        ps.register_user(
            "custom",
            crate::prompt::PromptTemplate {
                name: "test".into(),
                template: "Hello {agent_name}".into(),
            },
        );
        let planner = RuleBasedPlanner::with_prompt_system(ps);
        let prompt = planner.build_system_identity("bevy", "Test");
        assert!(prompt.contains("bevy"));
    }
}

