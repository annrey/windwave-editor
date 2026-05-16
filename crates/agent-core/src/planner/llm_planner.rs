//! LlmPlanner — LLM Chain-of-Thought plan generator with configurable fallback.
//!
//! Sends structured CoT prompts to an LLM, parses JSON responses into
//! `EditPlan`s, and falls back to a `Box<dyn Planner>` when the LLM is
//! unavailable or returns invalid output.

use crate::permission::OperationRisk;
use crate::plan::{EditPlan, EditPlanStep, EditPlanStatus, ExecutionMode, TargetModule};
use crate::prompt::{PromptSystem, PromptContext, PromptType};
use super::{ComplexityLevel, PlannerContext, Planner, RuleBasedPlanner, get_default_model, llm_runtime};

/// LLM CoT 驱动的智能规划器
///
/// Uses LLM Chain-of-Thought reasoning to create structured edit plans.
/// Provides a configurable fallback planner for when the LLM is unavailable.
pub struct LlmPlanner {
    llm_client: Option<Box<dyn crate::llm::LlmClient>>,
    prompt_system: PromptSystem,
    /// Pluggable fallback planner (defaults to RuleBasedPlanner).
    fallback: Box<dyn Planner>,
}

impl LlmPlanner {
    /// Create a new LlmPlanner with a RuleBasedPlanner fallback.
    pub fn new(llm_client: Option<Box<dyn crate::llm::LlmClient>>) -> Self {
        Self {
            llm_client,
            prompt_system: PromptSystem::with_defaults(),
            fallback: Box::new(RuleBasedPlanner::new()),
        }
    }

    /// Replace the fallback planner with a custom implementation.
    pub fn with_fallback(mut self, fallback: Box<dyn Planner>) -> Self {
        self.fallback = fallback;
        self
    }

    pub fn with_prompt_system(mut self, ps: PromptSystem) -> Self {
        self.prompt_system = ps;
        self
    }

    /// Build a Chain-of-Thought prompt for the LLM.
    fn build_cot_prompt(&self, request: &str, context: &PlannerContext) -> Vec<crate::llm::LlmMessage> {
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

        let sys = self.prompt_system.build_prompt(PromptType::TaskPlanning, &prompt_ctx);

        let cot_instruction = format!(
            "User request: \"{request}\"\n\
             Available tools: {tools}\n\
             Existing entities: {entities}\n\n\
             Think step by step:\n\
             1. What domains does this request involve? (scene/code/asset/visual)\n\
             2. What is the risk level? (Safe/LowRisk/MediumRisk/HighRisk/Destructive)\n\
             3. What execution mode is best? (Direct/Plan/Team)\n\
             4. What are the concrete steps?\n\n\
             Output ONLY valid JSON with this structure:\n\
             {{\n  \"title\": \"short task title\",\n  \"summary\": \"one-line summary\",\n  \"complexity\": \"Simple|Medium|Complex\",\n  \"risk_level\": \"Safe|LowRisk|MediumRisk|HighRisk|Destructive\",\n  \"mode\": \"Direct|Plan|Team\",\n  \"steps\": [\n    {{\"step_id\": \"step_1\", \"title\": \"...\", \"action\": \"...\", \"target_module\": \"Scene|Code|Asset\"}}\n  ]\n}}",
            request = request,
            tools = context.available_tools.join(", "),
            entities = context.scene_entity_names.join(", "),
        );

        vec![
            crate::llm::LlmMessage {
                role: crate::llm::Role::System,
                content: sys,
            },
            crate::llm::LlmMessage {
                role: crate::llm::Role::User,
                content: cot_instruction,
            },
        ]
    }

    /// Parse LLM JSON response into EditPlan. Returns None if parsing fails.
    fn parse_cot_response(&self, raw: &str, task_id: u64, request_text: &str) -> Option<EditPlan> {
        let json_str = if let Some(start) = raw.find("```json") {
            let after_start = &raw[start + 7..];
            after_start.find("```").map(|end| after_start[..end].trim()).unwrap_or(after_start.trim())
        } else if let Some(start) = raw.find('{') {
            &raw[start..]
        } else {
            return None;
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

        let steps: Vec<EditPlanStep> = parsed
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
                        EditPlanStep {
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
                            risk: risk_level.clone(),
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
            id: format!("llm_plan_{}", task_id),
            task_id,
            title,
            summary: request_text.to_string(),
            mode,
            steps,
            risk_level,
            status: EditPlanStatus::Draft,
        })
    }
}

impl Planner for LlmPlanner {
    fn create_plan(
        &self,
        request_text: &str,
        task_id: u64,
        context: PlannerContext,
    ) -> EditPlan {
        let llm_ready = self.llm_client.as_ref().map(|c| c.is_ready()).unwrap_or(false);

        if llm_ready {
            let messages = self.build_cot_prompt(request_text, &context);
            let model = get_default_model();
            let request = crate::llm::build_chat_request(model, messages);

            let result = llm_runtime().block_on(async {
                self.llm_client.as_ref().unwrap().chat(request).await
            });

            match result {
                Ok(response) => {
                    if let Some(plan) = self.parse_cot_response(&response.content, task_id, request_text) {
                        if !plan.steps.is_empty() {
                            return plan;
                        }
                    }
                }
                Err(_) => {}
            }
        }

        self.fallback.create_plan(request_text, task_id, context)
    }

    fn estimate_complexity(&self, text: &str) -> ComplexityLevel {
        self.fallback.estimate_complexity(text)
    }
}
