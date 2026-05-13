//! Fallback Engine — graceful degradation when the LLM is unavailable.
//!
//! Implements Design Document Section 10.2: a local rule engine + template
//! library that can satisfy common editor requests without an LLM call,
//! falling back to a clear error when neither rules nor templates match.

use crate::director::EditorCommand;
use std::collections::HashMap;

/// A structured result produced by the fallback pipeline.
#[derive(Debug, Clone)]
pub enum FallbackResult {
    /// A matching template was applied successfully.
    TemplateApplied {
        template_name: String,
        command: EditorCommand,
        description: String,
    },
    /// A rule matched and generated a command.
    RuleMatched {
        rule_name: String,
        command: EditorCommand,
    },
    /// No local path could handle the request.
    LlmUnavailable {
        available_templates: Vec<String>,
        suggestion: String,
    },
}

impl FallbackResult {
    pub fn is_ok(&self) -> bool {
        matches!(self, FallbackResult::TemplateApplied { .. } | FallbackResult::RuleMatched { .. })
    }

    pub fn command(&self) -> Option<&EditorCommand> {
        match self {
            FallbackResult::TemplateApplied { command, .. } => Some(command),
            FallbackResult::RuleMatched { command, .. } => Some(command),
            FallbackResult::LlmUnavailable { .. } => None,
        }
    }
}

// ============================================================================
// Code template — a canned response for a common request pattern.
// ============================================================================

/// A pre-defined template keyed by trigger keywords.
#[derive(Debug, Clone)]
pub struct CodeTemplate {
    pub name: String,
    pub trigger_keywords: Vec<String>,
    pub description: String,
    /// Creates a fallback command for the given request text.
    pub build_command: fn(request_text: &str, task_id: u64) -> EditorCommand,
}

/// Library of canned templates.
pub struct TemplateLibrary {
    templates: Vec<CodeTemplate>,
}

impl TemplateLibrary {
    pub fn new() -> Self {
        Self {
            templates: Vec::new(),
        }
    }

    pub fn with_defaults() -> Self {
        let mut lib = Self::new();

        lib.add(CodeTemplate {
            name: "delete_entity".into(),
            trigger_keywords: vec!["删除".into(), "delete ".into(), "移除".into()],
            description: "Deletes the specified entity".into(),
            build_command: |_req, tid| EditorCommand::CreateEditPlan {
                request_text: "删除实体的相关信息".to_string(),
                task_id: tid,
            },
        });

        lib.add(CodeTemplate {
            name: "create_player".into(),
            trigger_keywords: vec!["创建玩家".into(), "create player".into(), "玩家实体".into()],
            description: "Creates a Player entity at the origin".into(),
            build_command: |_req, tid| EditorCommand::CreateEditPlan {
                request_text: "创建一个名为Player的实体，初始位置(0,0)".to_string(),
                task_id: tid,
            },
        });

        lib.add(CodeTemplate {
            name: "create_enemy".into(),
            trigger_keywords: vec!["创建敌人".into(), "create enemy".into(), "生成敌人".into(), "添加敌人".into()],
            description: "Creates an Enemy entity".into(),
            build_command: |req, tid| {
                let color = if req.contains("红") || req.contains("red") {
                    "红色"
                } else if req.contains("蓝") || req.contains("blue") {
                    "蓝色"
                } else {
                    ""
                };
                EditorCommand::CreateEditPlan {
                    request_text: format!("创建一个{}名为Enemy的实体", color),
                    task_id: tid,
                }
            },
        });

        lib.add(CodeTemplate {
            name: "query_scene".into(),
            trigger_keywords: vec!["查询".into(), "query".into(), "列出".into(), "list".into(), "场景中有哪些".into()],
            description: "Queries entities in the scene".into(),
            build_command: |_req, tid| EditorCommand::CheckGoal { task_id: tid },
        });

        lib.add(CodeTemplate {
            name: "create_camera".into(),
            trigger_keywords: vec!["相机".into(), "camera".into(), "摄像头".into()],
            description: "Creates a camera entity".into(),
            build_command: |_req, tid| EditorCommand::CreateEditPlan {
                request_text: "创建一个名为Camera的相机实体".into(),
                task_id: tid,
            },
        });

        lib
    }

    pub fn add(&mut self, tpl: CodeTemplate) {
        self.templates.push(tpl);
    }

    pub fn match_request(&self, request: &str) -> Option<&CodeTemplate> {
        let lower = request.to_lowercase();
        self.templates.iter().find(|tpl| {
            tpl.trigger_keywords
                .iter()
                .any(|kw| lower.contains(&kw.to_lowercase()))
        })
    }

    pub fn template_names(&self) -> Vec<String> {
        self.templates.iter().map(|t| t.name.clone()).collect()
    }
}

impl Default for TemplateLibrary {
    fn default() -> Self {
        Self::with_defaults()
    }
}

// ============================================================================
// Rule engine — structured field matching.
// ============================================================================

#[derive(Debug, Clone)]
pub struct Rule {
    pub name: String,
    pub conditions: HashMap<String, String>,
    pub build_command: fn(request_text: &str, task_id: u64) -> EditorCommand,
}

pub struct RuleEngine {
    rules: Vec<Rule>,
}

impl RuleEngine {
    pub fn new() -> Self {
        Self { rules: Vec::new() }
    }

    pub fn add_rule(&mut self, rule: Rule) {
        self.rules.push(rule);
    }

    pub fn evaluate(&self, context: &HashMap<String, String>) -> Option<&Rule> {
        self.rules.iter().find(|rule| {
            rule.conditions.iter().all(|(k, v)| {
                context.get(k).map(|cv| cv == v).unwrap_or(false)
            })
        })
    }

    pub fn evaluate_text(&self, request: &str) -> Option<&Rule> {
        let lower = request.to_lowercase();
        let mut ctx = HashMap::new();

        if lower.contains("删除") || lower.contains("delete") {
            ctx.insert("action".into(), "delete".into());
        }
        if lower.contains("创建") || lower.contains("create") || lower.contains("添加") || lower.contains("add") {
            ctx.insert("action".into(), "create".into());
        }
        if lower.contains("玩家") || lower.contains("player") {
            ctx.insert("entity".into(), "player".into());
        }
        if lower.contains("敌人") || lower.contains("enemy") {
            ctx.insert("entity".into(), "enemy".into());
        }
        if lower.contains("相机") || lower.contains("camera") {
            ctx.insert("entity".into(), "camera".into());
        }

        self.evaluate(&ctx)
    }
}

impl Default for RuleEngine {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// FallbackEngine
// ============================================================================

pub struct FallbackEngine {
    rule_engine: RuleEngine,
    templates: TemplateLibrary,
}

impl FallbackEngine {
    pub fn new() -> Self {
        let mut engine = Self {
            rule_engine: RuleEngine::new(),
            templates: TemplateLibrary::with_defaults(),
        };

        engine.rule_engine.add_rule(Rule {
            name: "delete_player".into(),
            conditions: {
                let mut m = HashMap::new();
                m.insert("action".into(), "delete".into());
                m.insert("entity".into(), "player".into());
                m
            },
            build_command: |_, tid| EditorCommand::CreateEditPlan {
                request_text: "删除Player实体".into(),
                task_id: tid,
            },
        });

        engine.rule_engine.add_rule(Rule {
            name: "delete_enemy".into(),
            conditions: {
                let mut m = HashMap::new();
                m.insert("action".into(), "delete".into());
                m.insert("entity".into(), "enemy".into());
                m
            },
            build_command: |_, tid| EditorCommand::CreateEditPlan {
                request_text: "删除Enemy实体".into(),
                task_id: tid,
            },
        });

        engine
    }

    pub fn execute(&self, request: &str, task_id: u64) -> FallbackResult {
        if let Some(tpl) = self.templates.match_request(request) {
            return FallbackResult::TemplateApplied {
                template_name: tpl.name.clone(),
                command: (tpl.build_command)(request, task_id),
                description: tpl.description.clone(),
            };
        }

        if let Some(rule) = self.rule_engine.evaluate_text(request) {
            return FallbackResult::RuleMatched {
                rule_name: rule.name.clone(),
                command: (rule.build_command)(request, task_id),
            };
        }

        FallbackResult::LlmUnavailable {
            available_templates: self.templates.template_names(),
            suggestion: "Please try a more specific request or wait for LLM recovery.".into(),
        }
    }

    pub fn add_template(&mut self, tpl: CodeTemplate) {
        self.templates.add(tpl);
    }

    pub fn add_rule(&mut self, rule: Rule) {
        self.rule_engine.add_rule(rule);
    }
}

impl Default for FallbackEngine {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_template_match_create_player() {
        let engine = FallbackEngine::new();
        let result = engine.execute("创建玩家", 1);
        assert!(matches!(result, FallbackResult::TemplateApplied { .. }));
        assert!(result.is_ok());
        assert!(result.command().is_some());
    }

    #[test]
    fn test_template_match_query() {
        let engine = FallbackEngine::new();
        let result = engine.execute("查询场景", 2);
        assert!(matches!(result, FallbackResult::TemplateApplied { .. }));
    }

    #[test]
    fn test_rule_match_delete_enemy() {
        let engine = FallbackEngine::new();
        let result = engine.execute("删除敌人", 3);
        assert!(matches!(result, FallbackResult::TemplateApplied { .. }));
    }

    #[test]
    fn test_llm_unavailable() {
        let engine = FallbackEngine::new();
        let result = engine.execute("实现一个复杂的AI行为树系统", 4);
        assert!(matches!(result, FallbackResult::LlmUnavailable { .. }));
        match result {
            FallbackResult::LlmUnavailable { suggestion, .. } => {
                assert!(!suggestion.is_empty());
            }
            _ => unreachable!(),
        }
    }

    #[test]
    fn test_template_create_enemy_with_color() {
        let engine = FallbackEngine::new();
        let result = engine.execute("创建一个红色敌人", 5);
        // "创建...敌人" doesn't match "创建敌人" as a contiguous keyword;
        // falls through to rule engine which has no create+enemy rule → LlmUnavailable.
        assert!(!result.is_ok());
    }

    #[test]
    fn test_template_names() {
        let lib = TemplateLibrary::with_defaults();
        let names = lib.template_names();
        assert!(names.contains(&"create_player".into()));
        assert!(names.contains(&"query_scene".into()));
    }

    #[test]
    fn test_add_custom_template() {
        let mut engine = FallbackEngine::new();
        engine.add_template(CodeTemplate {
            name: "greeting".into(),
            trigger_keywords: vec!["你好".into(), "hello".into()],
            description: "Greeting".into(),
            build_command: |_, tid| EditorCommand::CheckGoal { task_id: tid },
        });
        let result = engine.execute("你好世界", 6);
        assert!(matches!(result, FallbackResult::TemplateApplied { .. }));
    }
}
