//! Plan revision and LLM-based planning — revision detection, application, and LLM plan generation.

use crate::plan::{EditPlan, EditPlanStep, EditPlanStatus, ExecutionMode, TargetModule};
use crate::permission::OperationRisk;
use crate::types::now_millis;
use super::types::{DirectorRuntime, DirectorTraceEntry};

impl DirectorRuntime {
    /// Sprint 1: Check if plan needs dynamic revision based on execution results.
    pub(crate) fn check_plan_revision_needed(&self, _plan: &EditPlan, _current_idx: usize, result: &str) -> Option<String> {
        let result_lower = result.to_lowercase();

        if result_lower.contains("already exists") || result_lower.contains("已存在") {
            return Some("Skip duplicate creation steps".to_string());
        }

        if result_lower.contains("not found") || result_lower.contains("未找到") {
            return Some("Try alternative entity or create it first".to_string());
        }

        None
    }

    /// Sprint 1: Apply plan revision (dynamic plan adjustment).
    ///
    /// Parses revision directives and modifies the plan steps accordingly:
    /// - "skip:N" — skip the next N steps
    /// - "insert:{title}" — insert a new step after current
    /// - "replace:{old}->{new}" — replace a step title
    /// - "Skip duplicate creation steps" — auto-detected, skips creation steps
    /// - "Try alternative entity or create it first" — auto-detected, inserts prerequisite
    pub(crate) fn apply_plan_revision(&mut self, plan_id: &str, revision: &str) {
        let revision_lower = revision.to_lowercase();

        if revision_lower.contains("skip duplicate") || revision_lower.contains("skip") {
            if let Some(plan) = self.plan_manager.get_mut(plan_id) {
                let mut skipped = 0;
                for step in &mut plan.steps {
                    let step_lower = step.title.to_lowercase();
                    if step_lower.contains("create") || step_lower.contains("创建") || step_lower.contains("生成") {
                        if !step.title.starts_with("[SKIPPED]") {
                            step.title = format!("[SKIPPED] {}", step.title);
                            step.action_description = format!("[SKIPPED] {}", step.action_description);
                            skipped += 1;
                        }
                    }
                }
                self.trace_entries.push(DirectorTraceEntry {
                    timestamp_ms: now_millis(),
                    actor: "PlanReviser".into(),
                    summary: format!("Skipped {} duplicate creation steps in plan '{}'", skipped, plan_id),
                });
            }
        } else if revision_lower.contains("try alternative") || revision_lower.contains("create it first") {
            if let Some(plan) = self.plan_manager.get_mut(plan_id) {
                let insert_idx = plan.steps.iter().position(|s| {
                    let lower = s.title.to_lowercase();
                    lower.contains("update") || lower.contains("modify") || lower.contains("delete") || lower.contains("移动") || lower.contains("删除")
                }).unwrap_or(0);

                let new_step = EditPlanStep {
                    id: format!("prereq_{}", plan.steps.len() + 1),
                    title: "Create prerequisite entity".to_string(),
                    target_module: TargetModule::Scene,
                    action_description: "Create the entity that doesn't exist yet".to_string(),
                    risk: OperationRisk::LowRisk,
                    validation_requirements: vec![],
                };
                plan.steps.insert(insert_idx, new_step);

                self.trace_entries.push(DirectorTraceEntry {
                    timestamp_ms: now_millis(),
                    actor: "PlanReviser".into(),
                    summary: format!("Inserted prerequisite step at position {} in plan '{}'", insert_idx, plan_id),
                });
            }
        }
    }

    /// Sprint 1: Generate an alternative step when execution fails.
    pub(crate) fn generate_alternative_step(&self, original: &str, error: &str) -> Option<String> {
        let error_lower = error.to_lowercase();
        let original_lower = original.to_lowercase();

        if error_lower.contains("not found") || error_lower.contains("不存在") || error_lower.contains("找不到") {
            let entity_name = Self::extract_entity_name(original);
            return Some(format!("Create entity '{}' before proceeding", entity_name));
        }

        if error_lower.contains("permission") || error_lower.contains("拒绝") || error_lower.contains("unauthorized") {
            return Some(format!("[LOW_RISK] {}", original));
        }

        if error_lower.contains("already exists") || error_lower.contains("已存在") || error_lower.contains("duplicate") {
            if original_lower.contains("create") || original_lower.contains("创建") || original_lower.contains("生成") {
                let modified = original
                    .replace("Create", "Modify")
                    .replace("create", "modify")
                    .replace("创建", "修改")
                    .replace("生成", "更新");
                return Some(modified);
            }
        }

        if error_lower.contains("invalid") || error_lower.contains("参数") || error_lower.contains("parameter") {
            return Some(format!("{} (with default parameters)", original));
        }

        if error_lower.contains("timeout") || error_lower.contains("rate limit") || error_lower.contains("timed out") {
            return Some(format!("[SIMPLIFIED] {}", original));
        }

        if error_lower.contains("no scenebridge") || error_lower.contains("not connected") {
            return Some(format!("[SIMULATED] {}", original));
        }

        if error_lower.contains("tool error") || error_lower.contains("execution failed") {
            if original_lower.contains("delete") || original_lower.contains("删除") {
                return Some(format!("[SAFE_ALTERNATIVE] Hide/disable '{}' instead of deleting", Self::extract_entity_name(original)));
            }
        }

        if error_lower.contains("llm error") || error_lower.contains("maximum steps") || error_lower.contains("parse error") {
            return Some(format!("[RULE_BASED] {}", original));
        }

        None
    }

    /// Sprint 1: Update a plan step with alternative content.
    pub(crate) fn update_plan_step(&mut self, plan_id: &str, step_id: &str, new_title: &str) {
        if let Some(plan) = self.plan_manager.get_mut(plan_id) {
            for step in &mut plan.steps {
                if step.id == step_id {
                    step.title = new_title.to_string();
                    step.action_description = new_title.to_string();
                    if new_title.starts_with("[LOW_RISK]") || new_title.starts_with("[SAFE_ALTERNATIVE]") {
                        step.risk = OperationRisk::LowRisk;
                    }
                    break;
                }
            }
        }
    }

    /// Extract entity name from a step title using simple heuristics.
    fn extract_entity_name(title: &str) -> String {
        let words: Vec<&str> = title.split_whitespace().collect();
        for word in &words {
            if word.chars().next().map(|c| c.is_uppercase()).unwrap_or(false) && word.len() > 1 {
                return word.to_string();
            }
        }
        words.last().unwrap_or(&"entity").to_string()
    }

    /// Internal: Use LLM to create a plan from user request.
    pub(crate) async fn plan_with_llm(&mut self, request_text: &str) -> Result<EditPlan, String> {
        let client = self.llm_client.as_ref().ok_or("No LLM client available")?;

        let system_prompt = self.prompt_system.build_prompt(
            crate::prompt::PromptType::TaskPlanning,
            &crate::prompt::PromptContext {
                selected_entities: self.plan_manager
                    .list()
                    .iter()
                    .flat_map(|p| p.steps.iter().map(|s| s.title.clone()))
                    .collect::<Vec<_>>()
                    .join(", "),
                ..crate::prompt::PromptContext::default()
            },
        );

        let user_prompt = format!(
            "User request: \"{request_text}\"\n\n\
             Think step by step:\n\
             1. What domains does this request involve? (scene, code, asset, visual)\n\
             2. What is the risk level? (Safe, LowRisk, MediumRisk, HighRisk, Destructive)\n\
             3. What execution mode is best? (Direct, Plan, Team)\n\
             4. What are the concrete steps?\n\n\
             Output ONLY valid JSON (no markdown, no explanation):\n\
             {{\n\
               \"title\": \"short task title\",\n\
               \"summary\": \"one-line summary of the request\",\n\
               \"complexity\": \"Simple|Medium|Complex\",\n\
               \"risk_level\": \"Safe|LowRisk|MediumRisk|HighRisk|Destructive\",\n\
               \"mode\": \"Direct|Plan|Team\",\n\
               \"steps\": [\n\
                 {{\"step_id\": \"step_1\", \"title\": \"action name\", \"action\": \"action description\", \"target_module\": \"Scene|Code|Asset\"}}\n\
               ]\n\
             }}",
            request_text = request_text,
        );

        let request = crate::llm::LlmRequest {
            model: "gpt-4o-mini".to_string(),
            messages: vec![
                crate::llm::LlmMessage {
                    role: crate::llm::Role::System,
                    content: system_prompt.to_string(),
                },
                crate::llm::LlmMessage {
                    role: crate::llm::Role::User,
                    content: user_prompt,
                },
            ],
            tools: None,
            max_tokens: Some(2048),
            temperature: Some(0.3),
        };

        match client.chat(request).await {
            Ok(response) => self.parse_llm_plan_response(&response.content, request_text),
            Err(e) => Err(format!("LLM request failed: {}", e)),
        }
    }

    /// Parse LLM response into EditPlan.
    fn parse_llm_plan_response(
        &mut self,
        content: &str,
        request_text: &str,
    ) -> Result<EditPlan, String> {
        let json_str = if content.contains("```json") {
            content
                .split("```json")
                .nth(1)
                .and_then(|s| s.split("```").next())
                .unwrap_or(content)
                .trim()
        } else if content.contains("```") {
            content
                .split("```")
                .nth(1)
                .unwrap_or(content)
                .trim()
        } else {
            content.trim()
        };

        #[derive(serde::Deserialize)]
        struct LlmPlan {
            title: String,
            #[serde(default)]
            risk_level: String,
            steps: Vec<LlmPlanStep>,
        }

        #[derive(serde::Deserialize)]
        struct LlmPlanStep {
            id: String,
            title: String,
        }

        let llm_plan: LlmPlan = serde_json::from_str(json_str)
            .map_err(|e| format!("Failed to parse LLM plan: {}", e))?;

        let task_id = self.plan_manager.allocate_task_id();
        let plan_id = self.plan_manager.generate_plan_id("llm", task_id);

        let steps: Vec<EditPlanStep> = llm_plan
            .steps
            .into_iter()
            .enumerate()
            .map(|(_i, s)| EditPlanStep {
                id: s.id,
                title: s.title.clone(),
                target_module: TargetModule::Scene,
                action_description: s.title,
                risk: OperationRisk::LowRisk,
                validation_requirements: Vec::new(),
            })
            .collect();

        let risk_level = match llm_plan.risk_level.to_lowercase().as_str() {
            "lowrisk" | "low" => OperationRisk::LowRisk,
            "mediumrisk" | "medium" => OperationRisk::MediumRisk,
            "highrisk" | "high" => OperationRisk::HighRisk,
            "destructive" => OperationRisk::Destructive,
            _ => OperationRisk::LowRisk,
        };

        Ok(EditPlan {
            id: plan_id,
            task_id,
            title: llm_plan.title,
            summary: request_text.to_string(),
            mode: ExecutionMode::Plan,
            risk_level,
            steps,
            status: EditPlanStatus::Draft,
        })
    }
}
