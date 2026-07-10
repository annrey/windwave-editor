//! SmartRouter — automatic execution-mode selection.
//!
//! Analyzes a user request and chooses between `Direct` (immediate execution),
//! `Plan` (plan → permission → execute), and `Team` (plan → multi-agent dispatch)
//! based on keyword complexity, estimated step count, and risk heuristics.
//!
//! When an LLM client is available, uses semantic analysis for more accurate
//! classification, falling back to keyword matching when LLM is unavailable.
//!
//! Reference: design §3.4

use crate::keyword_matcher::{complexity_label, KeywordComplexity, KeywordMatcher};
use crate::permission::OperationRisk;
use crate::plan::ExecutionMode;

/// LLM system prompt for request classification.
const ROUTER_SYSTEM_PROMPT: &str = r#"You are a request classifier for a game editor AI agent. Your job is to analyze a user's request and output a JSON classification.

Classify the request into one of three execution modes:
- "Direct": Simple single-operation tasks (create entity, change color, move object). No plan needed.
- "Plan": Multi-step tasks, code generation, batch operations, or anything requiring review.
- "Team": Tasks spanning multiple domains (scene+code+assets) or requiring 5+ distinct steps.

Output ONLY valid JSON, no markdown or explanation:
{
  "mode": "Direct|Plan|Team",
  "risk": "LowRisk|MediumRisk|HighRisk|Destructive",
  "complexity_score": <0-10>,
  "estimated_steps": <1-10>,
  "reason": "<one-line explanation>"
}

Risk guidelines:
- LowRisk: Simple create/query/read operations
- MediumRisk: Batch operations, multi-entity changes
- HighRisk: Delete or modify existing entities
- Destructive: Clear/reset/destroy/wipedata

Complexity guidelines:
- 1-2: Simple single operation
- 3-5: Multi-step or cross-domain
- 6-8: Complex with code generation
- 9-10: Large-scale architectural changes
"#;

/// Result of routing a user request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoutingDecision {
    pub mode: ExecutionMode,
    pub complexity: KeywordComplexity,
    pub risk: OperationRisk,
    pub estimated_steps: usize,
    pub reason: String,
}

// ===========================================================================
// SmartRouter
// ===========================================================================

/// Stateless router that maps a user request to its optimal execution mode.
pub struct SmartRouter;

impl SmartRouter {
    /// Route using keyword matching (always available, no LLM dependency).
    pub fn route(request_text: &str) -> RoutingDecision {
        let jailbreak_risk = crate::permission::JailbreakDetector::detect(request_text);
        if matches!(jailbreak_risk, crate::permission::JailbreakRisk::High) {
            let categories = crate::permission::JailbreakDetector::matched_categories(request_text);
            return RoutingDecision {
                mode: ExecutionMode::Plan,
                complexity: KeywordComplexity {
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

        let complexity = KeywordMatcher::score_complexity(request_text);
        let risk = KeywordMatcher::assess_risk(request_text);
        let steps = KeywordMatcher::estimate_steps(request_text, &complexity);
        let mode = Self::choose_mode(&complexity, &risk, steps);

        let reason = format!(
            "[keyword] complexity={}(score={}), risk={:?}, estimated_steps={} → mode={:?}",
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

    /// Route using LLM semantic analysis when available, falling back to keyword matching.
    ///
    /// # Arguments
    /// * `request_text` - The user's natural language request
    /// * `llm_client` - Optional LLM client for semantic classification
    pub fn route_with_llm(
        request_text: &str,
        llm_client: Option<&dyn crate::llm::LlmClient>,
    ) -> RoutingDecision {
        // Jailbreak check runs before any routing
        let jailbreak_risk = crate::permission::JailbreakDetector::detect(request_text);
        if matches!(jailbreak_risk, crate::permission::JailbreakRisk::High) {
            let categories = crate::permission::JailbreakDetector::matched_categories(request_text);
            return RoutingDecision {
                mode: ExecutionMode::Plan,
                complexity: KeywordComplexity {
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

        // Try LLM semantic analysis first
        if let Some(client) = llm_client {
            if client.is_ready() {
                match Self::llm_route(request_text, client) {
                    Ok(decision) => return decision,
                    Err(e) => {
                        log::warn!(
                            "SmartRouter: LLM routing failed ({}), falling back to keyword matching",
                            e
                        );
                    }
                }
            }
        }

        // Fallback: keyword matching (always available)
        Self::route(request_text)
    }

    /// Call LLM for semantic request classification.
    fn llm_route(
        request_text: &str,
        client: &dyn crate::llm::LlmClient,
    ) -> Result<RoutingDecision, String> {
        let prompt = format!("Classify this editor request: \"{}\"", request_text);

        let request = crate::llm::LlmRequest {
            model: crate::planner::get_default_model(),
            messages: vec![
                crate::llm::LlmMessage {
                    role: crate::llm::Role::System,
                    content: ROUTER_SYSTEM_PROMPT.to_string(),
                },
                crate::llm::LlmMessage {
                    role: crate::llm::Role::User,
                    content: prompt,
                },
            ],
            max_tokens: Some(256),
            temperature: Some(0.1),
            tools: None,
        };

        let runtime = crate::planner::get_llm_runtime();
        let response = runtime
            .block_on(client.chat(request))
            .map_err(|e| format!("LLM call failed: {}", e))?;

        let content = response.content.trim();
        Self::parse_llm_routing(content, request_text)
    }

    /// Parse LLM JSON response into a RoutingDecision.
    fn parse_llm_routing(json_text: &str, _request_text: &str) -> Result<RoutingDecision, String> {
        // Extract JSON from potential markdown fences
        let json_str = if let Some(start) = json_text.find("```json") {
            let inner = &json_text[start + 7..];
            if let Some(end) = inner.find("```") {
                inner[..end].trim()
            } else {
                inner.trim()
            }
        } else if let Some(start) = json_text.find('{') {
            if let Some(end) = json_text.rfind('}') {
                &json_text[start..=end]
            } else {
                json_text
            }
        } else {
            json_text
        };

        let parsed: serde_json::Value =
            serde_json::from_str(json_str).map_err(|e| format!("JSON parse error: {}", e))?;

        let mode_str = parsed["mode"].as_str().unwrap_or("Plan");
        let mode = match mode_str {
            "Direct" => ExecutionMode::Direct,
            "Team" => ExecutionMode::Team,
            _ => ExecutionMode::Plan,
        };

        let risk = match parsed["risk"].as_str().unwrap_or("MediumRisk") {
            "LowRisk" => OperationRisk::LowRisk,
            "MediumRisk" => OperationRisk::MediumRisk,
            "HighRisk" => OperationRisk::HighRisk,
            "Destructive" => OperationRisk::Destructive,
            _ => OperationRisk::MediumRisk,
        };

        let complexity_score = parsed["complexity_score"].as_u64().unwrap_or(3) as u8;
        let estimated_steps = parsed["estimated_steps"].as_u64().unwrap_or(1) as usize;
        let reason = parsed["reason"]
            .as_str()
            .unwrap_or("LLM classified")
            .to_string();

        // Build KeywordComplexity from LLM scores
        let complexity = KeywordComplexity {
            domains_touched: if complexity_score >= 7 {
                2
            } else if complexity_score >= 4 {
                1
            } else {
                0
            },
            entity_references: 0,
            has_code_gen: complexity_score >= 6,
            has_asset_ops: complexity_score >= 5,
            has_batch: estimated_steps >= 3,
            total_score: complexity_score.min(10),
        };

        // Sanity: high risk always goes to Plan
        let mode = if matches!(risk, OperationRisk::Destructive | OperationRisk::HighRisk) {
            ExecutionMode::Plan
        } else {
            mode
        };

        let reason = format!(
            "[LLM] {} (complexity={}, risk={:?}, steps={} → mode={:?})",
            reason, complexity_score, risk, estimated_steps, mode,
        );

        Ok(RoutingDecision {
            mode,
            complexity,
            risk,
            estimated_steps,
            reason,
        })
    }

    fn choose_mode(
        score: &KeywordComplexity,
        risk: &OperationRisk,
        estimated_steps: usize,
    ) -> ExecutionMode {
        if matches!(risk, OperationRisk::Destructive | OperationRisk::HighRisk) {
            return ExecutionMode::Plan;
        }

        if score.domains_touched >= 2 && score.total_score >= 5 {
            return ExecutionMode::Team;
        }

        if score.has_batch && score.has_code_gen {
            return ExecutionMode::Team;
        }

        if score.has_code_gen && score.total_score >= 7 {
            return ExecutionMode::Team;
        }

        if estimated_steps >= 5 && score.entity_references >= 3 {
            return ExecutionMode::Team;
        }

        if score.has_code_gen || score.has_batch {
            return ExecutionMode::Plan;
        }

        if score.total_score <= 3 {
            return ExecutionMode::Direct;
        }

        if score.total_score <= 4 && !score.has_code_gen && !score.has_asset_ops && !score.has_batch
        {
            return ExecutionMode::Direct;
        }

        ExecutionMode::Plan
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
        assert!(decision.complexity.total_score <= 3);
    }

    #[test]
    fn test_code_gen_routes_to_plan() {
        let decision = SmartRouter::route("生成一个自动移动的脚本");
        assert_eq!(decision.mode, ExecutionMode::Plan);
    }

    #[test]
    fn test_batch_creation_routes_to_plan() {
        let decision = SmartRouter::route("批量创建50个敌人");
        assert_eq!(decision.mode, ExecutionMode::Plan);
    }

    #[test]
    fn test_delete_triggers_high_risk() {
        let decision = SmartRouter::route("删除所有敌人");
        assert!(matches!(
            decision.risk,
            OperationRisk::HighRisk | OperationRisk::Destructive
        ));
        assert_eq!(decision.mode, ExecutionMode::Plan);
    }

    #[test]
    fn test_clear_triggers_destructive() {
        let decision = SmartRouter::route("清空场景");
        assert_eq!(decision.risk, OperationRisk::Destructive);
        assert_eq!(decision.mode, ExecutionMode::Plan);
    }

    #[test]
    fn test_multi_domain_triggers_team() {
        let decision = SmartRouter::route("创建敌人并为它编写AI脚本");
        assert!(decision.complexity.domains_touched >= 2);
    }

    #[test]
    fn test_empty_text_is_direct() {
        let decision = SmartRouter::route("");
        assert_eq!(decision.mode, ExecutionMode::Direct);
        assert_eq!(decision.risk, OperationRisk::LowRisk);
    }

    #[test]
    fn test_route_with_llm_falls_back_to_keyword() {
        let decision = SmartRouter::route_with_llm("创建一个红色敌人", None);
        assert_eq!(decision.mode, ExecutionMode::Direct);
        assert!(decision.reason.starts_with("[keyword]"));
    }

    #[test]
    fn test_parse_llm_json_direct() {
        let json = r#"{"mode":"Direct","risk":"LowRisk","complexity_score":2,"estimated_steps":1,"reason":"Simple entity creation"}"#;
        let decision = SmartRouter::parse_llm_routing(json, "创建一个红色敌人").unwrap();
        assert_eq!(decision.mode, ExecutionMode::Direct);
        assert_eq!(decision.risk, OperationRisk::LowRisk);
        assert_eq!(decision.estimated_steps, 1);
    }

    #[test]
    fn test_parse_llm_json_plan() {
        let json = r#"{"mode":"Plan","risk":"HighRisk","complexity_score":7,"estimated_steps":4,"reason":"Multi-entity deletion"}"#;
        let decision = SmartRouter::parse_llm_routing(json, "删除所有红色敌人").unwrap();
        assert_eq!(decision.mode, ExecutionMode::Plan);
        assert_eq!(decision.risk, OperationRisk::HighRisk);
    }

    #[test]
    fn test_parse_llm_json_team() {
        let json = r#"{"mode":"Team","risk":"MediumRisk","complexity_score":8,"estimated_steps":6,"reason":"Cross-domain scene+code task"}"#;
        let decision =
            SmartRouter::parse_llm_routing(json, "创建敌人并编写AI脚本和导入素材").unwrap();
        assert_eq!(decision.mode, ExecutionMode::Team);
    }

    #[test]
    fn test_parse_llm_json_with_markdown_fence() {
        let json = "```json\n{\"mode\":\"Direct\",\"risk\":\"LowRisk\",\"complexity_score\":1,\"estimated_steps\":1,\"reason\":\"Simple query\"}\n```";
        let decision = SmartRouter::parse_llm_routing(json, "列出所有实体").unwrap();
        assert_eq!(decision.mode, ExecutionMode::Direct);
    }

    #[test]
    fn test_parse_llm_json_high_risk_forced_plan() {
        let json = r#"{"mode":"Direct","risk":"Destructive","complexity_score":5,"estimated_steps":1,"reason":"Clear scene"}"#;
        let decision = SmartRouter::parse_llm_routing(json, "清空场景").unwrap();
        // Even if LLM says Direct, Destructive risk forces Plan
        assert_eq!(decision.mode, ExecutionMode::Plan);
    }
}
