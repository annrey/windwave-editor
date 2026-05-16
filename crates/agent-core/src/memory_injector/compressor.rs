use crate::types::current_timestamp;
use crate::memory_injector::MemoryError;
use serde::{Deserialize, Serialize};

/// 对话摘要
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationSummary {
    pub timestamp: u64,
    pub original_turns: usize,
    pub summary_text: String,
    pub key_points: Vec<String>,
    pub entities_mentioned: Vec<String>,
}

/// 记忆压缩器 - 使用 LLM 对长对话进行摘要压缩
pub struct MemoryCompressor {
    pub compression_threshold: usize,
    pub summaries: Vec<ConversationSummary>,
    pub pending_content: String,
}

impl MemoryCompressor {
    pub fn new(compression_threshold: usize) -> Self {
        Self {
            compression_threshold,
            summaries: Vec::new(),
            pending_content: String::new(),
        }
    }

    pub fn add_content(&mut self, content: &str) {
        self.pending_content.push_str(content);
        self.pending_content.push('\n');
    }

    pub fn needs_compression(&self) -> bool {
        let estimated_turns = self.pending_content.split('\n').filter(|s| !s.is_empty()).count();
        estimated_turns >= self.compression_threshold
    }

    pub fn compress(&mut self) -> Result<ConversationSummary, MemoryError> {
        if self.pending_content.is_empty() {
            return Err(MemoryError::Serialization("No content to compress".into()));
        }

        let original_turns = self.pending_content.split('\n').filter(|s| !s.is_empty()).count();
        let summary_text = self.generate_summary(&self.pending_content);
        let key_points = self.extract_key_points(&self.pending_content);
        let entities = self.extract_entities(&self.pending_content);

        let summary = ConversationSummary {
            timestamp: current_timestamp(),
            original_turns,
            summary_text,
            key_points,
            entities_mentioned: entities,
        };

        self.summaries.push(summary.clone());
        self.pending_content.clear();
        Ok(summary)
    }

    fn generate_summary(&self, content: &str) -> String {
        let lines: Vec<&str> = content.lines().collect();
        if lines.is_empty() {
            return "(empty)".into();
        }
        let mut summary = String::new();
        for line in lines.iter().take(5) {
            if summary.len() + line.len() + 2 > 200 {
                break;
            }
            summary.push_str(line);
            summary.push(' ');
        }
        if summary.len() > 200 {
            summary.truncate(197);
            summary.push_str("...");
        }
        summary
    }

    fn extract_key_points(&self, content: &str) -> Vec<String> {
        let mut points = Vec::new();
        for line in content.lines() {
            let lower = line.to_lowercase();
            if lower.contains("创建") || lower.contains("create")
                || lower.contains("修改") || lower.contains("update")
                || lower.contains("删除") || lower.contains("delete") {
                points.push(line.trim().to_string());
            }
        }
        points.into_iter().take(5).collect()
    }

    fn extract_entities(&self, content: &str) -> Vec<String> {
        let mut entities = Vec::new();
        let mut seen = std::collections::HashSet::new();

        for word in content.split_whitespace() {
            let cleaned: String = word.chars()
                .filter(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            if cleaned.len() > 2
                && cleaned.chars().next().unwrap_or(' ').is_uppercase()
                && !seen.contains(&cleaned) {
                if !["The", "And", "For", "With", "From", "This", "That"].contains(&cleaned.as_str()) {
                    entities.push(cleaned.clone());
                    seen.insert(cleaned);
                }
            }
        }
        entities
    }

    pub fn get_summaries(&self) -> &[ConversationSummary] {
        &self.summaries
    }

    pub fn describe_summaries(&self) -> String {
        if self.summaries.is_empty() {
            return "(no conversation summaries)".into();
        }
        let mut parts = Vec::new();
        for summary in &self.summaries {
            parts.push(format!(
                "- {} ({} 轮对话, {} 个关键点)",
                summary.timestamp,
                summary.original_turns,
                summary.key_points.len()
            ));
        }
        parts.join("\n")
    }
}

#[cfg(test)]
mod compression_tests {
    use super::*;

    #[test]
    fn test_memory_compressor() {
        let mut compressor = MemoryCompressor::new(3);
        compressor.add_content("用户: 创建一个红色敌人");
        compressor.add_content("Agent: 已创建 Enemy_01，颜色红色");
        compressor.add_content("用户: 把 Player 移到右边");
        assert!(compressor.needs_compression());
        let summary = compressor.compress().unwrap();
        assert!(summary.summary_text.contains("创建"));
        assert!(summary.entities_mentioned.contains(&"Enemy_01".to_string())
            || summary.entities_mentioned.contains(&"Player".to_string()));
    }

    #[test]
    fn test_compressor_empty() {
        let mut compressor = MemoryCompressor::new(3);
        let result = compressor.compress();
        assert!(result.is_err());
    }
}
