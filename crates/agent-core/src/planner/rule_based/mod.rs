//! RuleBasedPlanner — keyword-driven edit plan generator.
//!
//! Parses natural-language requests into structured `EditPlan`s via
//! keyword matching for complexity, risk, execution mode, and step building.

mod keywords;

use crate::keyword_matcher::KeywordMatcher;
use crate::permission::OperationRisk;
use crate::plan::{EditPlan, EditPlanStatus, EditPlanStep, ExecutionMode, TargetModule};
use crate::planner::{ComplexityLevel, Planner, PlannerContext};
use crate::prompt::{PromptContext, PromptSystem, PromptType};
use keywords::*;

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

        let scene_hit = SCENE_CN_KEYWORDS
            .iter()
            .chain(SCENE_EN_KEYWORDS)
            .any(|kw| lower.contains(kw));

        let code_hit = CODE_CN_KEYWORDS
            .iter()
            .chain(CODE_EN_KEYWORDS)
            .any(|kw| lower.contains(kw));

        let asset_hit = ASSET_CN_KEYWORDS
            .iter()
            .chain(ASSET_EN_KEYWORDS)
            .any(|kw| lower.contains(kw));

        let visual_hit = VISUAL_CN_KEYWORDS
            .iter()
            .chain(VISUAL_EN_KEYWORDS)
            .chain(ENGLISH_COLOR_WORDS)
            .chain(CHINESE_COLOR_PREFIXES)
            .any(|kw| lower.contains(kw));

        let domain_count = [scene_hit, code_hit, asset_hit, visual_hit]
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
        KeywordMatcher::assess_risk(text)
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
        let is_create = CREATE_KEYWORDS.iter().any(|kw| lower.contains(kw));

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
                    target_module: TargetModule::Scene,
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
        let is_delete = DELETE_KEYWORDS.iter().any(|kw| lower.contains(kw));

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
        let is_batch = BATCH_KEYWORDS.iter().any(|kw| lower.contains(kw));

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
                action_description: "将任务分派给多个 Agent 并行执行，并收集各 Agent 的执行结果。"
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

        for pattern in ENGLISH_PATTERNS {
            if let Some(pos) = lower.find(pattern) {
                let after = &lower[pos + pattern.len()..];
                // 取第一个空格前的词作为实体名
                let mut name = after
                    .split_whitespace()
                    .next()
                    .unwrap_or("entity")
                    .to_string();
                // 如果提取到的是颜色词，则取下一个词作为实体名
                if ENGLISH_COLOR_WORDS.contains(&name.as_str()) {
                    // 跳过颜色词，取下一个词
                    let rest = &after[after.find(' ').unwrap_or(0)..].trim();
                    name = rest
                        .split_whitespace()
                        .next()
                        .unwrap_or("entity")
                        .to_string();
                }
                return name;
            }
        }

        // 中文：尝试在动词后找到实体名
        // "创建X" -> X, "删除Y" -> Y
        for verb in CHINESE_VERBS {
            if let Some(pos) = text.find(verb) {
                let after = &text[pos + verb.len()..];
                // 中文字符通常连续，取第一个非中文分隔符前的部分
                let name: String = after
                    .chars()
                    .take_while(|c| !c.is_whitespace() && !c.is_ascii_punctuation())
                    .collect();
                if !name.is_empty() {
                    let mut cleaned = name;
                    for prefix in ["一个新的", "一个", "一只", "新的", "新"] {
                        if let Some(stripped) = cleaned.strip_prefix(prefix) {
                            cleaned = stripped.to_string();
                            break;
                        }
                    }
                    // 去除颜色前缀（红色、蓝色等）
                    for prefix in CHINESE_COLOR_PREFIXES {
                        if let Some(stripped) = cleaned.strip_prefix(prefix) {
                            cleaned = stripped.to_string();
                            break;
                        }
                    }
                    for (marker, _, _) in POSITION_MARKERS {
                        if let Some((before_marker, _)) = cleaned.split_once(marker) {
                            cleaned = before_marker.to_string();
                        }
                    }
                    for suffix in ["放置在", "放在", "放到", "在"] {
                        if let Some((before_suffix, _)) = cleaned.split_once(suffix) {
                            cleaned = before_suffix.to_string();
                        }
                    }
                    if !cleaned.is_empty() {
                        return cleaned;
                    }
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

        for (kw_list, name, rgba) in COLOR_KEYWORDS {
            if kw_list.iter().any(|kw| lower.contains(kw)) {
                return Some((name.to_string(), *rgba));
            }
        }

        None
    }

    /// 从请求文本中提取位置信息
    ///
    /// 支持相对于已有实体的位置描述，如：
    /// "放在玩家右侧" -> ("玩家右侧", [player.x + offset, player.y, 0])
    fn extract_position(text: &str, context: &PlannerContext) -> Option<(String, [f32; 3])> {
        let lower = text.to_lowercase();

        for (chinese_dir, english_dir, offset) in POSITION_MARKERS {
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
                } else {
                    return Some((chinese_dir.to_string(), *offset));
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
    fn create_plan(&self, request_text: &str, task_id: u64, context: PlannerContext) -> EditPlan {
        self.create_plan(request_text, task_id, context)
    }

    fn estimate_complexity(&self, text: &str) -> ComplexityLevel {
        self.estimate_complexity(text)
    }
}

#[cfg(test)]
mod tests;
