//! Token budget allocator for LLM context windows.
//!
//! Divides the total token budget across prompt sections to prevent
//! context overflow. Ratios are configurable.

#[derive(Debug, Clone)]
pub struct TokenBudget {
    pub total: usize,
    pub system_prompt_ratio: f32,
    pub scene_context_ratio: f32,
    pub conversation_ratio: f32,
    pub tool_description_ratio: f32,
}

impl Default for TokenBudget {
    fn default() -> Self {
        Self {
            total: 8192,
            system_prompt_ratio: 0.30,
            scene_context_ratio: 0.40,
            conversation_ratio: 0.20,
            tool_description_ratio: 0.10,
        }
    }
}

impl TokenBudget {
    pub fn new(total: usize) -> Self {
        Self {
            total,
            ..Default::default()
        }
    }

    pub fn allocate(&self) -> TokenAllocation {
        TokenAllocation {
            system_prompt: (self.total as f32 * self.system_prompt_ratio) as usize,
            scene_context: (self.total as f32 * self.scene_context_ratio) as usize,
            conversation: (self.total as f32 * self.conversation_ratio) as usize,
            tool_description: (self.total as f32 * self.tool_description_ratio) as usize,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TokenAllocation {
    pub system_prompt: usize,
    pub scene_context: usize,
    pub conversation: usize,
    pub tool_description: usize,
}

/// Rough token estimator: 1 token ≈ 4 characters for CJK, 1 token ≈ 4 chars for English.
/// Coarse estimate for budget enforcement — not exact.
pub fn estimate_tokens(text: &str) -> usize {
    let cjk_count = text
        .chars()
        .filter(|c| (&'\u{4E00}'..=&'\u{9FFF}').contains(&c))
        .count();
    let ascii_count = text.len() - cjk_count;
    (cjk_count * 3 / 2) + (ascii_count / 4)
}
