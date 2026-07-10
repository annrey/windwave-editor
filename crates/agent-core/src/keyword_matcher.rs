//! KeywordMatcher — independent keyword-based text analysis.
//!
//! Extracted from router.rs and fallback.rs to provide a shared,
//! testable module for domain detection, risk assessment, and
//! step-count estimation. Used by SmartRouter and the fallback engine.

use crate::permission::OperationRisk;

const SCENE_KW: &[&str] = &[
    "创建", "create", "生成", "spawn", "添加", "add", "删除", "delete", "移除", "remove", "移动",
    "move", "放置", "place", "实体", "entity", "编辑", "edit", "场景", "scene",
];

const CODE_KW: &[&str] = &[
    "代码",
    "code",
    "系统",
    "system",
    "脚本",
    "script",
    "逻辑",
    "logic",
    "编程",
    "program",
    "函数",
    "function",
    "组件",
    "component",
    "插件",
    "plugin",
];

const ASSET_KW: &[&str] = &[
    "素材", "asset", "图片", "image", "声音", "sound", "纹理", "texture", "模型", "model", "音乐",
    "music", "导入", "import",
];

const BATCH_KW: &[&str] = &["批量", "batch", "全部", "all", "所有", "每个", "every"];

const REVIEW_KW: &[&str] = &["审查", "review", "检查", "check", "规则", "rule"];
const ORCHESTRATE_KW: &[&str] = &["规划", "plan", "编排", "复杂", "orchestrate"];
const MOVE_KW: &[&str] = &["移动", "move", "变换", "transform", "位移", "translate"];
const QUERY_KW: &[&str] = &[
    "查询", "query", "列表", "list", "搜索", "search", "寻找", "find",
];
const IMPORT_KW: &[&str] = &["导入", "import", "资源", "asset"];

const DESTRUCTIVE_KW: &[&str] = &["清空", "clear", "销毁", "destroy", "彻底", "wipe"];
const HIGH_RISK_KW: &[&str] = &["删除", "delete", "移除", "remove"];
const MEDIUM_RISK_KW: &[&str] = &["批量", "batch", "全部", "all", "所有"];

/// Result of keyword-based complexity scoring.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeywordComplexity {
    pub domains_touched: usize,
    pub entity_references: usize,
    pub has_code_gen: bool,
    pub has_asset_ops: bool,
    pub has_batch: bool,
    pub total_score: u8,
}

/// Stateless keyword matcher for text analysis.
pub struct KeywordMatcher;

impl KeywordMatcher {
    /// Score request text complexity (0-10 scale).
    pub fn score_complexity(text: &str) -> KeywordComplexity {
        let lower = text.to_lowercase();

        let scene_hit = SCENE_KW.iter().any(|kw| lower.contains(kw));
        let code_hit = CODE_KW.iter().any(|kw| lower.contains(kw));
        let asset_hit = ASSET_KW.iter().any(|kw| lower.contains(kw));
        let batch_hit = BATCH_KW.iter().any(|kw| lower.contains(kw));

        let domains_touched = [scene_hit, code_hit, asset_hit]
            .iter()
            .filter(|&&h| h)
            .count();

        let entity_references = text
            .split_whitespace()
            .filter(|w| {
                w.chars().next().map(|c| c.is_uppercase()).unwrap_or(false)
                    && w.len() > 1
                    && !w.starts_with("//")
            })
            .count();

        let mut total_score: u8 = 0;
        if scene_hit {
            total_score += 1;
        }
        if code_hit {
            total_score += 3;
        }
        if asset_hit {
            total_score += 2;
        }
        if batch_hit {
            total_score += 2;
        }
        if entity_references >= 2 {
            total_score += 1;
        }
        if entity_references >= 4 {
            total_score += 2;
        }
        if lower.contains("多个") || lower.contains("multi") {
            total_score += 1;
        }
        total_score = total_score.min(10);

        KeywordComplexity {
            domains_touched,
            entity_references,
            has_code_gen: code_hit,
            has_asset_ops: asset_hit,
            has_batch: batch_hit,
            total_score,
        }
    }

    /// Assess operation risk from text keywords.
    pub fn assess_risk(text: &str) -> OperationRisk {
        let lower = text.to_lowercase();

        if DESTRUCTIVE_KW.iter().any(|kw| lower.contains(kw)) {
            OperationRisk::Destructive
        } else if HIGH_RISK_KW.iter().any(|kw| lower.contains(kw)) {
            OperationRisk::HighRisk
        } else if MEDIUM_RISK_KW.iter().any(|kw| lower.contains(kw)) {
            OperationRisk::MediumRisk
        } else {
            OperationRisk::LowRisk
        }
    }

    /// Estimate the number of steps needed based on complexity.
    pub fn estimate_steps(text: &str, complexity: &KeywordComplexity) -> usize {
        let lower = text.to_lowercase();
        let mut steps = complexity.domains_touched.max(1);

        if complexity.has_code_gen {
            steps += 1;
        }
        if complexity.has_batch {
            steps += 2;
        }
        if lower.contains("所有") || lower.contains("all") || lower.contains("全部") {
            steps += 1;
        }
        if complexity.entity_references > 2 {
            steps += 1;
        }

        steps
    }

    /// Determine whether text targets the scene domain.
    pub fn targets_scene_domain(text: &str) -> bool {
        let lower = text.to_lowercase();
        SCENE_KW.iter().any(|kw| lower.contains(kw))
            || lower.contains("entity")
            || lower.contains("实体")
    }

    /// Determine whether text targets the code domain.
    pub fn targets_code_domain(text: &str) -> bool {
        let lower = text.to_lowercase();
        CODE_KW.iter().any(|kw| lower.contains(kw))
    }

    /// Classify text into a routing capability based on keyword matching.
    /// Returns `None` when no capability keywords are found (caller falls back to default).
    pub fn classify_capability(text: &str) -> Option<crate::registry::CapabilityKind> {
        let lower = text.to_lowercase();
        if CODE_KW.iter().any(|kw| lower.contains(kw)) {
            Some(crate::registry::CapabilityKind::CodeWrite)
        } else if REVIEW_KW.iter().any(|kw| lower.contains(kw)) {
            Some(crate::registry::CapabilityKind::RuleCheck)
        } else if SCENE_KW.iter().any(|kw| lower.contains(kw)) {
            Some(crate::registry::CapabilityKind::SceneWrite)
        } else if ORCHESTRATE_KW.iter().any(|kw| lower.contains(kw)) {
            Some(crate::registry::CapabilityKind::Orchestrate)
        } else {
            None
        }
    }

    /// Map step title text to a skill name for `lookup_skill_for_step`.
    pub fn resolve_skill_name(text: &str) -> Option<&'static str> {
        let lower = text.to_lowercase();
        if Self::is_create_operation(text) {
            return Some("create_entity");
        }
        if Self::is_delete_operation(text) {
            return Some("delete_entity");
        }
        if Self::has_move_keywords(text) {
            return Some("modify_entity_transform");
        }
        if Self::has_color_keywords(text) {
            return Some("modify_entity_color");
        }
        if QUERY_KW.iter().any(|kw| lower.contains(kw)) {
            return Some("query_scene");
        }
        if IMPORT_KW.iter().any(|kw| lower.contains(kw)) {
            return Some("import_asset");
        }
        None
    }

    /// Check whether text contains generic CRUD operation keywords
    /// (创建/create/修改/update/删除/delete).
    pub fn has_operation_keywords(text: &str) -> bool {
        let lower = text.to_lowercase();
        lower.contains("创建")
            || lower.contains("create")
            || lower.contains("修改")
            || lower.contains("update")
            || lower.contains("删除")
            || lower.contains("delete")
    }

    // ------------------------------------------------------------------
    //  Operation-level keyword classifiers  (T11 migration targets)
    // ------------------------------------------------------------------

    /// Checks if text requests creating/adding a new entity or object.
    pub fn is_create_operation(text: &str) -> bool {
        let lower = text.to_lowercase();
        lower.contains("创建")
            || lower.contains("create")
            || lower.contains("生成")
            || lower.contains("spawn")
            || lower.contains("添加")
            || lower.contains("add")
    }

    /// Checks if text requests deleting/removing an entity or object.
    pub fn is_delete_operation(text: &str) -> bool {
        let lower = text.to_lowercase();
        lower.contains("删除")
            || lower.contains("delete")
            || lower.contains("移除")
            || lower.contains("remove")
    }

    /// Checks if text indicates code generation (vs analysis).
    pub fn is_code_generation_request(text: &str) -> bool {
        let lower = text.to_lowercase();
        lower.contains("生成")
            || lower.contains("generate")
            || lower.contains("创建")
            || lower.contains("create")
    }

    /// Checks if text contains move/reposition keywords.
    pub fn has_move_keywords(text: &str) -> bool {
        let lower = text.to_lowercase();
        MOVE_KW.iter().any(|kw| lower.contains(kw)) || lower.contains("位置")
    }

    /// Checks if text contains modify/update keywords.
    pub fn has_modify_keywords(text: &str) -> bool {
        let lower = text.to_lowercase();
        lower.contains("修改") || lower.contains("更新") || lower.contains("改变")
    }

    /// Checks if text contains color/appearance keywords.
    pub fn has_color_keywords(text: &str) -> bool {
        let lower = text.to_lowercase();
        lower.contains("改色")
            || lower.contains("颜色")
            || lower.contains("color")
            || lower.contains("红色")
            || lower.contains("蓝色")
            || lower.contains("绿色")
    }

    /// Checks if text contains query/list/search keywords.
    pub fn has_query_keywords(text: &str) -> bool {
        let lower = text.to_lowercase();
        QUERY_KW.iter().any(|kw| lower.contains(kw))
    }

    /// Checks if text targets the asset/resource domain.
    pub fn targets_asset_domain(text: &str) -> bool {
        let lower = text.to_lowercase();
        ASSET_KW.iter().any(|kw| lower.contains(kw))
    }

    /// Checks if text targets the review/check domain.
    pub fn targets_review_domain(text: &str) -> bool {
        let lower = text.to_lowercase();
        REVIEW_KW.iter().any(|kw| lower.contains(kw))
    }

    // ------------------------------------------------------------------
    //  Team / HR keyword classifiers
    // ------------------------------------------------------------------

    const TEAM_KW: &[&str] = &[
        "hr",
        "hire",
        "fire",
        "agent",
        "team",
        "团队",
        "人事",
        "squad",
        "crew",
        "create squad",
        "add member",
        "assign task",
        "submit task",
        "小组",
        "战队",
    ];

    const HR_KW: &[&str] = &["hr", "hire", "fire", "list", "roster", "人事"];

    /// Checks if text is a team/HR management request.
    pub fn is_team_request(text: &str) -> bool {
        let lower = text.to_lowercase();
        Self::TEAM_KW.iter().any(|kw| lower.contains(kw))
    }

    /// Checks if text is specifically an HR (add/remove/list roster) request.
    pub fn is_hr_request(text: &str) -> bool {
        let lower = text.to_lowercase();
        Self::HR_KW.iter().any(|kw| lower.contains(kw))
    }
}

/// Convert a total_score value to a human-readable label.
#[inline]
pub fn complexity_label(score: u8) -> &'static str {
    match score {
        0..=2 => "simple",
        3..=5 => "moderate",
        6..=8 => "complex",
        _ => "very-complex",
    }
}
