//! SmartRouter — automatic execution-mode selection.
//!
//! Analyzes a user request and chooses between `Direct` (immediate execution),
//! `Plan` (plan → permission → execute), and `Team` (plan → multi-agent dispatch)
//! based on keyword complexity, estimated step count, and risk heuristics.
//!
//! Reference: design §3.4

use crate::plan::ExecutionMode;
use crate::permission::OperationRisk;

/// Result of routing a user request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoutingDecision {
    pub mode: ExecutionMode,
    pub complexity: ComplexityScore,
    pub risk: OperationRisk,
    pub estimated_steps: usize,
    pub reason: String,
}

/// Numeric complexity breakdown for debugging / UI display.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComplexityScore {
    pub domains_touched: usize,
    pub entity_references: usize,
    pub has_code_gen: bool,
    pub has_asset_ops: bool,
    pub has_batch: bool,
    pub total_score: u8,
}

// ===========================================================================
// SmartRouter
// ===========================================================================

/// Stateless router that maps a user request to its optimal execution mode.
pub struct SmartRouter;

impl SmartRouter {
    /// Route a user request text to the best execution mode.
    pub fn route(request_text: &str) -> RoutingDecision {
        // Check for jailbreak / prompt injection attempts
        let jailbreak_risk = crate::permission::JailbreakDetector::detect(request_text);
        if matches!(jailbreak_risk, crate::permission::JailbreakRisk::High) {
            let categories = crate::permission::JailbreakDetector::matched_categories(request_text);
            return RoutingDecision {
                mode: ExecutionMode::Plan, // force review — user must approve
                complexity: ComplexityScore {
                    domains_touched: 0,
                    entity_references: 0,
                    has_code_gen: false,
                    has_asset_ops: false,
                    has_batch: false,
                    total_score: 0,
                },
                risk: OperationRisk::Destructive,
                estimated_steps: 0,
                reason: format!(
                    "Jailbreak detected (categories: {}) — forced Plan mode for review",
                    categories.join(", "),
                ),
            };
        }

        let complexity = Self::score_complexity(request_text);
        let risk = Self::assess_risk(request_text);
        let steps = Self::estimate_steps(request_text, &complexity);
        let mode = Self::choose_mode(&complexity, &risk, steps);

        let reason = format!(
            "complexity={:?}(score={}), risk={:?}, estimated_steps={} → mode={:?}",
            complexity_label(complexity.total_score),
            complexity.total_score,
            risk,
            steps,
            mode,
        );

        RoutingDecision {
            mode,
            complexity,
            risk,
            estimated_steps: steps,
            reason,
        }
    }

    // ------------------------------------------------------------------
    // Complexity scoring (0-10 scale)
    // ------------------------------------------------------------------

    fn score_complexity(text: &str) -> ComplexityScore {
        let lower = text.to_lowercase();

        let scene_kw = [
            "创建", "create", "生成", "spawn", "添加", "add",
            "删除", "delete", "移除", "remove", "移动", "move",
            "放置", "place", "实体", "entity",
        ];
        let code_kw = [
            "代码", "code", "系统", "system", "脚本", "script",
            "逻辑", "logic", "编程", "program", "函数", "function",
            "组件", "component", "插件", "plugin",
        ];
        let asset_kw = [
            "素材", "asset", "图片", "image", "声音", "sound",
            "纹理", "texture", "模型", "model", "音乐", "music",
            "导入", "import",
        ];
        let batch_kw = [
            "批量", "batch", "全部", "all", "所有", "每个", "every",
        ];

        let scene_hit = scene_kw.iter().any(|kw| lower.contains(kw));
        let code_hit = code_kw.iter().any(|kw| lower.contains(kw));
        let asset_hit = asset_kw.iter().any(|kw| lower.contains(kw));
        let batch_hit = batch_kw.iter().any(|kw| lower.contains(kw));

        let domains_touched =
            [scene_hit, code_hit, asset_hit].iter().filter(|&&h| h).count();

        // Count entity-like references (capitalized words after creation verbs, etc.)
        let entity_references = text
            .split_whitespace()
            .filter(|w| {
                w.chars().next().map(|c| c.is_uppercase()).unwrap_or(false)
                    && w.len() > 1
                    && !w.starts_with("//")
            })
            .count();

        let mut total_score: u8 = 0;
        if scene_hit { total_score += 1; }
        if code_hit { total_score += 3; }
        if asset_hit { total_score += 2; }
        if batch_hit { total_score += 2; }
        if entity_references >= 2 { total_score += 1; }
        if entity_references >= 4 { total_score += 2; }
        if lower.contains("多个") || lower.contains("multi") { total_score += 1; }
        total_score = total_score.min(10);

        ComplexityScore {
            domains_touched,
            entity_references,
            has_code_gen: code_hit,
            has_asset_ops: asset_hit,
            has_batch: batch_hit,
            total_score,
        }
    }

    // ------------------------------------------------------------------
    // Risk assessment
    // ------------------------------------------------------------------

    fn assess_risk(text: &str) -> OperationRisk {
        let lower = text.to_lowercase();

        let destructive = ["清空", "clear", "销毁", "destroy", "彻底", "wipe"];
        let high_risk = ["删除", "delete", "移除", "remove"];
        let medium_risk = ["批量", "batch", "全部", "all", "所有"];

        if destructive.iter().any(|kw| lower.contains(kw)) {
            OperationRisk::Destructive
        } else if high_risk.iter().any(|kw| lower.contains(kw)) {
            OperationRisk::HighRisk
        } else if medium_risk.iter().any(|kw| lower.contains(kw)) {
            OperationRisk::MediumRisk
        } else {
            OperationRisk::LowRisk
        }
    }

    // ------------------------------------------------------------------
    // Step count estimation
    // ------------------------------------------------------------------

    fn estimate_steps(text: &str, score: &ComplexityScore) -> usize {
        let lower = text.to_lowercase();
        let mut count = 0usize;

        // Each domain adds 1 base step
        count += score.domains_touched;

        // Each entity reference may be a separate step
        count += score.entity_references.min(3);

        // Color keywords usually mean an extra "set color" step
        let colors = [
            "红色", "红色", "蓝色", "绿色", "黄色", "紫色", "白色", "黑色", "橙色",
            "red", "blue", "green", "yellow", "purple", "white", "black", "orange",
        ];
        if colors.iter().any(|c| lower.contains(c)) {
            count += 1;
        }

        // Position keywords ("右边", "左边", etc.) add a placement step
        let positions = [
            "右侧", "右边", "right", "左侧", "左边", "left",
            "上方", "上面", "above", "下方", "下面", "below",
        ];
        if positions.iter().any(|p| lower.contains(p)) {
            count += 1;
        }

        count.max(1)
    }

    // ------------------------------------------------------------------
    // Mode selection
    // ------------------------------------------------------------------

    fn choose_mode(
        score: &ComplexityScore,
        risk: &OperationRisk,
        estimated_steps: usize,
    ) -> ExecutionMode {
        // Destructive/HighRisk always needs a plan
        if matches!(risk, OperationRisk::Destructive | OperationRisk::HighRisk) {
            return ExecutionMode::Plan;
        }

        // Team mode: multi-domain complex operations
        // Scene+Code, Scene+Asset, Code+Asset, or all three
        if score.domains_touched >= 2 && score.total_score >= 5 {
            return ExecutionMode::Team;
        }

        // Team mode: batch operations with code gen
        if score.has_batch && score.has_code_gen {
            return ExecutionMode::Team;
        }

        // Team mode: complex code gen requiring blueprint + implementation
        if score.has_code_gen && score.total_score >= 7 {
            return ExecutionMode::Team;
        }

        // Team mode: many steps across multiple entities
        if estimated_steps >= 5 && score.entity_references >= 3 {
            return ExecutionMode::Team;
        }

        // Plan mode: code generation or batch operations
        if score.has_code_gen || score.has_batch {
            return ExecutionMode::Plan;
        }

        // Direct mode: low complexity, no code, no batch, low/medium risk
        if score.total_score <= 3 {
            return ExecutionMode::Direct;
        }

        // Multi-domain but no code/assets/batch → still Direct
        if score.total_score <= 4
            && !score.has_code_gen
            && !score.has_asset_ops
            && !score.has_batch
        {
            return ExecutionMode::Direct;
        }

        // Everything else goes through Plan
        ExecutionMode::Plan
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn complexity_label(score: u8) -> &'static str {
    match score {
        0..=2 => "Simple",
        3..=5 => "Medium",
        _ => "Complex",
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_create_routes_to_direct() {
        let decision = SmartRouter::route("创建一个实体");
        assert_eq!(decision.mode, ExecutionMode::Direct);
        assert_eq!(decision.estimated_steps, 1);
    }

    #[test]
    fn test_create_with_color_still_direct() {
        let decision = SmartRouter::route("创建一个红色敌人");
        assert_eq!(decision.mode, ExecutionMode::Direct);
        assert!(decision.estimated_steps >= 1);
    }

    #[test]
    fn test_create_with_position_direct() {
        let decision = SmartRouter::route("在右边创建一个红色敌人");
        assert_eq!(decision.mode, ExecutionMode::Direct);
    }

    #[test]
    fn test_code_gen_routes_to_plan() {
        let decision = SmartRouter::route("生成一个跳跃系统代码");
        assert_eq!(decision.mode, ExecutionMode::Plan);
        assert!(decision.complexity.has_code_gen);
    }

    #[test]
    fn test_delete_is_high_risk_plan() {
        let decision = SmartRouter::route("删除全部实体");
        assert_eq!(decision.mode, ExecutionMode::Plan);
        assert!(matches!(decision.risk, OperationRisk::HighRisk | OperationRisk::Destructive | OperationRisk::MediumRisk));
    }

    #[test]
    fn test_multi_entity_direct() {
        let decision = SmartRouter::route("创建 Player 和 Enemy");
        assert_eq!(decision.mode, ExecutionMode::Direct);
    }

    #[test]
    fn test_batch_plan() {
        let decision = SmartRouter::route("批量创建敌人");
        assert_eq!(decision.mode, ExecutionMode::Plan);
    }

    #[test]
    fn test_destructive_plan() {
        let decision = SmartRouter::route("清空场景");
        assert_eq!(decision.mode, ExecutionMode::Plan);
        assert_eq!(decision.risk, OperationRisk::Destructive);
    }

    #[test]
    fn test_move_entity_direct() {
        let decision = SmartRouter::route("移动 Player 到右边");
        assert_eq!(decision.mode, ExecutionMode::Direct);
    }

    #[test]
    fn test_code_plugin_plan() {
        let decision = SmartRouter::route("创建一个插件");
        assert_eq!(decision.mode, ExecutionMode::Plan);
    }

    #[test]
    fn test_asset_import_still_routable() {
        let decision = SmartRouter::route("导入一个纹理");
        // Asset ops without code gen are medium; check routing works
        assert!(decision.complexity.has_asset_ops);
    }

    #[test]
    fn test_routing_decision_reason_not_empty() {
        let decision = SmartRouter::route("查询场景中所有实体");
        assert!(!decision.reason.is_empty());
    }
}
