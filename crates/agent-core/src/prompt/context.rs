//! L0-L3 分层上下文类型
//!
//! - L0: System-level (agent identity, capabilities)
//! - L1: Session-level (conversation history, project context)
//! - L2: Task-level (current task, selected entities)
//! - L3: Entity-level (specific entity details, component state)

use std::collections::HashMap;

use crate::prompt::examples::FewShotExample;

#[derive(Debug, Clone, Default)]
pub struct L0SystemContext {
    pub agent_name: String,
    pub engine_name: String,
    pub language: String,
    pub capabilities: Vec<String>,
    pub principles: Vec<String>,
}

impl L0SystemContext {
    pub fn default_bevy() -> Self {
        Self {
            agent_name: "AgentEdit".into(),
            engine_name: "bevy".into(),
            language: "rust".into(),
            capabilities: vec![
                "Scene manipulation".into(),
                "Code generation".into(),
                "Asset management".into(),
                "Debugging assistance".into(),
            ],
            principles: vec![
                "Always confirm destructive operations before executing".into(),
                "Provide clear explanations of actions".into(),
                "Follow Bevy ECS best practices".into(),
                "Consider performance implications".into(),
                "Maintain consistency with existing codebase".into(),
            ],
        }
    }

    pub fn describe(&self) -> String {
        format!(
            "## Agent Identity\n\
             You are {}, operating within the {} engine ({})\n\
             \n\
             ## Capabilities\n\
             {}\n\
             \n\
             ## Principles\n\
             {}",
            self.agent_name,
            self.engine_name,
            self.language,
            self.capabilities
                .iter()
                .map(|c| format!("- {}", c))
                .collect::<Vec<_>>()
                .join("\n"),
            self.principles
                .iter()
                .map(|p| format!("- {}", p))
                .collect::<Vec<_>>()
                .join("\n"),
        )
    }
}

#[derive(Debug, Clone, Default)]
pub struct L1SessionContext {
    pub project_name: String,
    pub engine_version: String,
    pub conversation_summary: String,
    pub conventions: Vec<String>,
    pub recent_actions: Vec<String>,
}

impl L1SessionContext {
    pub fn describe(&self) -> String {
        format!(
            "## Project Context\n\
             Project: {} (v{})\n\
             \n\
             ## Recent Actions\n\
             {}\n\
             \n\
             ## Conventions\n\
             {}",
            self.project_name,
            self.engine_version,
            if self.recent_actions.is_empty() {
                "(none)".into()
            } else {
                self.recent_actions
                    .iter()
                    .map(|a| format!("- {}", a))
                    .collect::<Vec<_>>()
                    .join("\n")
            },
            if self.conventions.is_empty() {
                "(none)".into()
            } else {
                self.conventions
                    .iter()
                    .map(|c| format!("- {}", c))
                    .collect::<Vec<_>>()
                    .join("\n")
            },
        )
    }
}

#[derive(Debug, Clone, Default)]
pub struct L2TaskContext {
    pub current_task: String,
    pub selected_entities: Vec<String>,
    pub goals: Vec<String>,
    pub constraints: Vec<String>,
}

impl L2TaskContext {
    pub fn describe(&self) -> String {
        format!(
            "## Current Task\n\
             {}\n\
             \n\
             ## Selected Entities\n\
             {}\n\
             \n\
             ## Goals\n\
             {}\n\
             \n\
             ## Constraints\n\
             {}",
            self.current_task,
            if self.selected_entities.is_empty() {
                "(none)".into()
            } else {
                self.selected_entities
                    .iter()
                    .map(|e| format!("- {}", e))
                    .collect::<Vec<_>>()
                    .join("\n")
            },
            if self.goals.is_empty() {
                "(none)".into()
            } else {
                self.goals
                    .iter()
                    .map(|g| format!("- {}", g))
                    .collect::<Vec<_>>()
                    .join("\n")
            },
            if self.constraints.is_empty() {
                "(none)".into()
            } else {
                self.constraints
                    .iter()
                    .map(|c| format!("- {}", c))
                    .collect::<Vec<_>>()
                    .join("\n")
            },
        )
    }
}

#[derive(Debug, Clone)]
pub struct L3EntityContext {
    pub entity_name: String,
    pub entity_id: u64,
    pub components: Vec<EntityComponent>,
    pub parent: Option<String>,
    pub children: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct EntityComponent {
    pub name: String,
    pub properties: HashMap<String, String>,
}

impl L3EntityContext {
    pub fn describe(&self) -> String {
        let children_desc = format!(
            "Parent: {}\nChildren: {}",
            self.parent.as_deref().unwrap_or("(none)"),
            if self.children.is_empty() {
                "(none)".into()
            } else {
                self.children
                    .iter()
                    .map(|c| format!("- {}", c))
                    .collect::<Vec<_>>()
                    .join(", ")
            }
        );
        format!(
            "## Entity: {} (ID: {})\n\
             {}\n\
             \n\
             ## Components\n\
             {}",
            self.entity_name,
            self.entity_id,
            children_desc,
            if self.components.is_empty() {
                "(none)".into()
            } else {
                self.components
                    .iter()
                    .map(|c| {
                        let props = c
                            .properties
                            .iter()
                            .map(|(k, v)| format!("{}={}", k, v))
                            .collect::<Vec<_>>()
                            .join(", ");
                        format!("- {}: {}", c.name, props)
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            },
        )
    }
}

/// Combined context for LLM prompt (all 4 layers).
#[derive(Debug, Clone, Default)]
pub struct LayeredContext {
    pub l0_system: L0SystemContext,
    pub l1_session: L1SessionContext,
    pub l2_task: L2TaskContext,
    pub l3_entities: Vec<L3EntityContext>,
    pub few_shot_examples: Vec<FewShotExample>,
    pub memory_context: Option<crate::memory::MemoryContext>,
}

impl LayeredContext {
    pub fn add_few_shot(&mut self, example: FewShotExample) {
        self.few_shot_examples.push(example);
    }

    pub fn select_few_shot_examples(
        &self,
        user_request: &str,
        top_n: usize,
    ) -> Vec<&FewShotExample> {
        let request_lower = user_request.to_lowercase();
        let request_words: Vec<&str> = request_lower.split_whitespace().collect();

        let mut scored: Vec<(f32, &FewShotExample)> = self
            .few_shot_examples
            .iter()
            .map(|example| {
                let mut score = 0.0f32;

                let ex_request_lower = example.user_request.to_lowercase();
                for word in &request_words {
                    if ex_request_lower.contains(word) {
                        score += 2.0;
                    }
                }

                let action_lower = example.action.to_lowercase();

                if (request_lower.contains("create")
                    || request_lower.contains("创建")
                    || request_lower.contains("生成"))
                    && action_lower.contains("create")
                {
                    score += 5.0;
                }
                if (request_lower.contains("update")
                    || request_lower.contains("修改")
                    || request_lower.contains("改")
                    || request_lower.contains("换"))
                    && action_lower.contains("update")
                {
                    score += 5.0;
                }
                if (request_lower.contains("delete")
                    || request_lower.contains("删除")
                    || request_lower.contains("移除"))
                    && action_lower.contains("delete")
                {
                    score += 5.0;
                }
                if (request_lower.contains("query")
                    || request_lower.contains("list")
                    || request_lower.contains("查询")
                    || request_lower.contains("列表"))
                    && action_lower.contains("query")
                {
                    score += 5.0;
                }
                if (request_lower.contains("generate")
                    || request_lower.contains("写")
                    || request_lower.contains("生成代码")
                    || request_lower.contains("组件")
                    || request_lower.contains("系统"))
                    && action_lower.contains("generate")
                {
                    score += 5.0;
                }
                if (request_lower.contains("prefab")
                    || request_lower.contains("预制体")
                    || request_lower.contains("模板"))
                    && action_lower.contains("prefab")
                {
                    score += 5.0;
                }
                if (request_lower.contains("agent")
                    || request_lower.contains("ai")
                    || request_lower.contains("智能")
                    || request_lower.contains("挂上")
                    || request_lower.contains("附加"))
                    && (action_lower.contains("agent") || action_lower.contains("attach"))
                {
                    score += 5.0;
                }
                if (request_lower.contains("review")
                    || request_lower.contains("审查")
                    || request_lower.contains("检查")
                    || request_lower.contains("有没有问题"))
                    && action_lower.contains("review")
                {
                    score += 5.0;
                }
                if (request_lower.contains("read")
                    || request_lower.contains("读取")
                    || request_lower.contains("看看")
                    || request_lower.contains("查看")
                    || request_lower.contains("打开"))
                    && action_lower.contains("read_file")
                {
                    score += 5.0;
                }
                if (request_lower.contains("write")
                    || request_lower.contains("写入")
                    || request_lower.contains("保存")
                    || request_lower.contains("创建文件")
                    || request_lower.contains("新建文件"))
                    && action_lower.contains("write_file")
                {
                    score += 5.0;
                }
                if (request_lower.contains("search")
                    || request_lower.contains("搜索")
                    || request_lower.contains("找")
                    || request_lower.contains("查找")
                    || request_lower.contains("grep"))
                    && action_lower.contains("grep")
                {
                    score += 5.0;
                }
                if (request_lower.contains("edit")
                    || request_lower.contains("编辑")
                    || request_lower.contains("改文件")
                    || request_lower.contains("替换")
                    || request_lower.contains("修改配置"))
                    && action_lower.contains("edit_file")
                {
                    score += 5.0;
                }

                if (request_lower.contains("enemy") || request_lower.contains("敌人"))
                    && (ex_request_lower.contains("enemy") || ex_request_lower.contains("敌人"))
                {
                    score += 2.0;
                }
                if (request_lower.contains("player") || request_lower.contains("玩家"))
                    && (ex_request_lower.contains("player") || ex_request_lower.contains("玩家"))
                {
                    score += 2.0;
                }
                if (request_lower.contains("boss") || request_lower.contains("首领"))
                    && (ex_request_lower.contains("boss") || ex_request_lower.contains("首领"))
                {
                    score += 2.0;
                }

                if (request_lower.contains("red") || request_lower.contains("红色"))
                    && (ex_request_lower.contains("red") || ex_request_lower.contains("红色"))
                {
                    score += 1.5;
                }
                if (request_lower.contains("blue") || request_lower.contains("蓝色"))
                    && (ex_request_lower.contains("blue") || ex_request_lower.contains("蓝色"))
                {
                    score += 1.5;
                }
                if (request_lower.contains("yellow")
                    || request_lower.contains("黄色")
                    || request_lower.contains("金色"))
                    && (ex_request_lower.contains("yellow")
                        || ex_request_lower.contains("黄色")
                        || ex_request_lower.contains("金色"))
                {
                    score += 1.5;
                }

                (score, example)
            })
            .collect();

        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        scored.into_iter().take(top_n).map(|(_, ex)| ex).collect()
    }

    pub fn build_prompt_with_selected_examples(&self, user_request: &str, top_n: usize) -> String {
        let mut parts = Vec::new();

        parts.push(self.l0_system.describe());
        parts.push(self.l1_session.describe());
        parts.push(self.l2_task.describe());

        if !self.l3_entities.is_empty() {
            parts.push("## Entity Details\n".into());
            for entity in &self.l3_entities {
                parts.push(entity.describe());
                parts.push("\n".into());
            }
        }

        if let Some(ref mem_ctx) = self.memory_context {
            let mem_text = mem_ctx.to_prompt_section();
            if !mem_text.is_empty() {
                parts.push(mem_text);
            }
        }

        let selected = self.select_few_shot_examples(user_request, top_n);
        if !selected.is_empty() {
            parts.push("## Few-Shot Examples (Relevant to Your Request)\n".into());
            parts.push("Here are examples similar to your request:\n".into());
            for example in selected {
                parts.push(example.describe());
            }
        }

        parts.join("\n\n")
    }

    pub fn with_memory(mut self, memory: crate::memory::MemoryContext) -> Self {
        self.memory_context = Some(memory);
        self
    }

    pub fn describe(&self) -> String {
        let mut parts = Vec::new();

        parts.push(self.l0_system.describe());
        parts.push(self.l1_session.describe());
        parts.push(self.l2_task.describe());

        if !self.l3_entities.is_empty() {
            parts.push("## Entity Details\n".into());
            for entity in &self.l3_entities {
                parts.push(entity.describe());
                parts.push("\n".into());
            }
        }

        if let Some(ref mem_ctx) = self.memory_context {
            let mem_text = mem_ctx.to_prompt_section();
            if !mem_text.is_empty() {
                parts.push(mem_text);
            }
        }

        if !self.few_shot_examples.is_empty() {
            parts.push("## Few-Shot Examples (Tool Usage Patterns)\n".into());
            parts.push("Here are examples of how to use tools effectively:\n".into());
            for example in &self.few_shot_examples {
                parts.push(example.describe());
            }
        }

        parts.join("\n\n")
    }

    pub fn is_empty(&self) -> bool {
        self.l0_system.agent_name.is_empty()
            && self.l1_session.project_name.is_empty()
            && self.l2_task.current_task.is_empty()
            && self.l3_entities.is_empty()
            && self.few_shot_examples.is_empty()
            && self.memory_context.is_none()
    }

    pub fn truncate_to_budget(&mut self, budget: &crate::prompt::budget::TokenBudget) {
        let allocation = budget.allocate();
        let _l0_budget = allocation.system_prompt;
        let l1_budget = allocation.conversation;
        let l2_budget = allocation.scene_context;
        let l3_budget = allocation.tool_description;

        if !self.few_shot_examples.is_empty() {
            let few_shot_text = self
                .few_shot_examples
                .iter()
                .map(|e| e.describe())
                .collect::<Vec<_>>()
                .join("\n");
            let few_shot_tokens = crate::prompt::budget::estimate_tokens(&few_shot_text);
            let few_shot_budget = l3_budget / 2;

            if few_shot_tokens > few_shot_budget {
                let mut kept = Vec::new();
                let mut current_tokens = 0;
                for example in &self.few_shot_examples {
                    let ex_tokens = crate::prompt::budget::estimate_tokens(&example.describe());
                    if current_tokens + ex_tokens <= few_shot_budget {
                        kept.push(example.clone());
                        current_tokens += ex_tokens;
                    } else {
                        break;
                    }
                }
                self.few_shot_examples = kept;
            }
        }

        if !self.l3_entities.is_empty() {
            let l3_text = self
                .l3_entities
                .iter()
                .map(|e| e.describe())
                .collect::<Vec<_>>()
                .join("\n");
            let l3_tokens = crate::prompt::budget::estimate_tokens(&l3_text);

            if l3_tokens > l3_budget {
                let mut kept = Vec::new();
                let mut current_tokens = 0;
                for entity in &self.l3_entities {
                    let entity_tokens = crate::prompt::budget::estimate_tokens(&entity.describe());
                    if current_tokens + entity_tokens <= l3_budget {
                        kept.push(entity.clone());
                        current_tokens += entity_tokens;
                    } else {
                        break;
                    }
                }
                self.l3_entities = kept;
            }
        }

        let l1_text = self.l1_session.describe();
        let l1_tokens = crate::prompt::budget::estimate_tokens(&l1_text);
        if l1_tokens > l1_budget {
            let max_actions = (l1_budget / 20).max(1);
            if self.l1_session.recent_actions.len() > max_actions {
                let start = self.l1_session.recent_actions.len() - max_actions;
                self.l1_session.recent_actions = self.l1_session.recent_actions[start..].to_vec();
            }

            let summary_budget = l1_budget / 4;
            let summary_tokens =
                crate::prompt::budget::estimate_tokens(&self.l1_session.conversation_summary);
            if summary_tokens > summary_budget {
                let chars_per_token = 4;
                let max_chars = summary_budget * chars_per_token;
                if self.l1_session.conversation_summary.len() > max_chars {
                    self.l1_session.conversation_summary = format!(
                        "{}...[truncated]",
                        &self.l1_session.conversation_summary[..max_chars.saturating_sub(15)]
                    );
                }
            }
        }

        let total_after = crate::prompt::budget::estimate_tokens(&self.describe());
        if total_after > budget.total {
            let l2_text = self.l2_task.describe();
            let l2_tokens = crate::prompt::budget::estimate_tokens(&l2_text);
            if l2_tokens > l2_budget {
                let chars_per_token = 4;
                let max_chars = l2_budget * chars_per_token;
                if self.l2_task.current_task.len() > max_chars {
                    self.l2_task.current_task = format!(
                        "{}...[truncated]",
                        &self.l2_task.current_task[..max_chars.saturating_sub(15)]
                    );
                }

                let max_goals = (l2_budget / 30).max(1);
                if self.l2_task.goals.len() > max_goals {
                    let start = self.l2_task.goals.len() - max_goals;
                    self.l2_task.goals = self.l2_task.goals[start..].to_vec();
                }
                if self.l2_task.constraints.len() > max_goals {
                    let start = self.l2_task.constraints.len() - max_goals;
                    self.l2_task.constraints = self.l2_task.constraints[start..].to_vec();
                }
            }
        }
    }

    pub fn with_token_budget(&self, budget: &crate::prompt::budget::TokenBudget) -> Self {
        let mut result = self.clone();
        result.truncate_to_budget(budget);
        result
    }
}
