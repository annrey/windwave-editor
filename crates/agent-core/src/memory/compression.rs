//! Memory compression operations for MemorySystem

use super::system::MemorySystem;
use crate::memory::{Episode, EpisodeType, MemoryMetadata, MemoryTier};

impl MemorySystem {
    /// Compress episodic memory using LLM summarization
    pub async fn compress_episodic_memory(
        &mut self,
        max_episodes: usize,
        llm_client: Option<&dyn crate::llm::LlmClient>,
    ) -> usize {
        if self.episodic.len() <= max_episodes {
            return 0;
        }

        let episodes_to_compress = self.episodic.len() - max_episodes;
        let old_episodes: Vec<Episode> = self.episodic.drain_old_episodes(episodes_to_compress);

        if old_episodes.is_empty() {
            return 0;
        }

        if let Some(client) = llm_client {
            let summary = Self::generate_llm_summary(&old_episodes, client).await;

            let compressed = Episode {
                metadata: MemoryMetadata::new(
                    self.episodic.next_id_counter(),
                    MemoryTier::Episodic,
                ),
                episode_type: EpisodeType::Summary,
                summary: format!("[Compressed {} episodes] {}", old_episodes.len(), summary),
                details: serde_json::json!({
                    "compressed_count": old_episodes.len(),
                    "original_ids": old_episodes.iter().map(|e| e.metadata.id.0).collect::<Vec<_>>(),
                }),
                entity_ids: Vec::new(),
                success: None,
                duration_ms: None,
            };
            self.episodic.record_compressed(compressed);
        } else {
            let summary = Self::generate_rule_based_summary(&old_episodes);

            let compressed = Episode {
                metadata: MemoryMetadata::new(
                    self.episodic.next_id_counter(),
                    MemoryTier::Episodic,
                ),
                episode_type: EpisodeType::Summary,
                summary: format!(
                    "[Auto-compressed {} episodes] {}",
                    old_episodes.len(),
                    summary
                ),
                details: serde_json::json!({
                    "compressed_count": old_episodes.len(),
                    "method": "rule_based",
                }),
                entity_ids: Vec::new(),
                success: None,
                duration_ms: None,
            };
            self.episodic.record_compressed(compressed);
        }

        episodes_to_compress
    }

    /// Generate summary using LLM
    async fn generate_llm_summary(
        episodes: &[Episode],
        client: &dyn crate::llm::LlmClient,
    ) -> String {
        let episodes_text: String = episodes
            .iter()
            .enumerate()
            .map(|(i, ep)| format!("{}. [{}] {}", i + 1, format_episode_short(ep), ep.summary))
            .collect::<Vec<_>>()
            .join("\n");

        let prompt = format!(
            "Summarize the following conversation history into key points. \
             Preserve important decisions, user preferences, and errors encountered.\n\n\
             ---\n{}\n---\n\n\
             Summary:",
            episodes_text
        );

        match client
            .chat(crate::llm::LlmRequest {
                model: crate::planner::get_default_model(),
                messages: vec![crate::llm::LlmMessage {
                    role: crate::llm::Role::User,
                    content: prompt,
                }],
                max_tokens: Some(500),
                temperature: Some(0.3),
                tools: None,
            })
            .await
        {
            Ok(response) => response.content,
            Err(_) => Self::generate_rule_based_summary(episodes),
        }
    }

    /// Generate summary using rules (no LLM needed)
    fn generate_rule_based_summary(episodes: &[Episode]) -> String {
        let mut key_points = Vec::new();
        let mut user_requests = Vec::new();
        let mut errors_encountered = Vec::new();

        for ep in episodes {
            match ep.episode_type {
                EpisodeType::UserRequest => {
                    user_requests.push(truncate_str(&ep.summary, 100));
                }
                EpisodeType::ErrorOccurred => {
                    errors_encountered.push(truncate_str(&ep.summary, 80));
                }
                _ => {
                    if key_points.len() < 5 {
                        key_points.push(format!("\u{2022} {}", truncate_str(&ep.summary, 80)));
                    }
                }
            }
        }

        let mut parts = Vec::new();

        if !user_requests.is_empty() {
            parts.push(format!("User requests: {}", user_requests.join("; ")));
        }
        if !errors_encountered.is_empty() {
            parts.push(format!("Issues: {}", errors_encountered.join("; ")));
        }
        if !key_points.is_empty() {
            parts.push(format!("Key events: {}", key_points.join(" ")));
        }

        if parts.is_empty() {
            "[No significant events]".to_string()
        } else {
            parts.join(" | ")
        }
    }

    /// Check if compression is needed based on configured thresholds
    pub fn compress_needed(&mut self, threshold: usize) -> bool {
        let conversation_count = self.conversation_turn_count();
        conversation_count > threshold || self.episodic.len() > threshold * 2
    }

    /// Get the number of conversation turns in working memory
    pub fn conversation_turn_count(&mut self) -> usize {
        self.working.conversation_indices().len()
    }

    /// Compress working memory by converting old conversation turns into a summary context hint
    pub fn compress_working_memory(&mut self, keep_recent: usize) -> usize {
        let indices = self.working.conversation_indices();

        if indices.len() <= keep_recent {
            return 0;
        }

        let to_compress: Vec<String> = indices
            .iter()
            .take(indices.len().saturating_sub(keep_recent))
            .filter_map(|&idx| self.working.get_entry_by_index(idx))
            .filter_map(|e| e.source_message.as_ref())
            .map(|m| format!("[{:?}] {}", m.message_type, m.content))
            .collect();

        if to_compress.is_empty() {
            return 0;
        }

        let compressed_count = to_compress.len();
        let summary = Self::summarize_conversation(&to_compress);

        let ids_to_remove: Vec<u64> = indices
            .iter()
            .take(indices.len().saturating_sub(keep_recent))
            .filter_map(|&idx| self.working.get_entry_by_index(idx))
            .map(|e| e.metadata.id.0)
            .collect();

        let _ = self.working.remove_by_ids(&ids_to_remove);
        self.working
            .add_hint(&format!("[Memory Summary] {}", summary));

        compressed_count
    }

    /// Generate a rule-based summary of conversation turns
    fn summarize_conversation(turns: &[String]) -> String {
        if turns.len() <= 2 {
            return turns.iter().fold(String::new(), |acc, t| {
                if acc.is_empty() {
                    t.chars().take(120).collect()
                } else {
                    format!("{} | {}", acc, t.chars().take(120).collect::<String>())
                }
            });
        }

        let keywords = Self::extract_summary_keywords(turns);
        let turn_count = turns.len();

        format!(
            "Conversation covering {} turns. Topics discussed: {}. Key actions: {}. Summary: {}",
            turn_count,
            keywords,
            Self::extract_action_summary(turns),
            Self::extract_gist(turns),
        )
    }

    fn extract_summary_keywords(turns: &[String]) -> String {
        let word_priorities = [
            "entity",
            "create",
            "change",
            "move",
            "color",
            "position",
            "component",
            "scene",
            "transform",
            "sprite",
            "delete",
            "modify",
            "add",
            "animation",
            "physics",
        ];

        let mut found: Vec<&str> = Vec::new();
        for &keyword in &word_priorities {
            let count = turns
                .iter()
                .filter(|t| t.to_lowercase().contains(&keyword.to_lowercase()))
                .count();
            if count > 0 {
                found.push(keyword);
            }
        }
        if found.is_empty() {
            "general discussion".to_string()
        } else {
            found.join(", ")
        }
    }

    fn extract_action_summary(turns: &[String]) -> String {
        let action_words = [
            "create", "delete", "change", "move", "set", "add", "remove", "update",
        ];
        let mut actions: Vec<&str> = Vec::new();
        for &action in &action_words {
            let count = turns
                .iter()
                .filter(|t| t.to_lowercase().contains(&action.to_lowercase()))
                .count();
            if count > 0 {
                actions.push(action);
            }
        }
        if actions.is_empty() {
            "none detected".to_string()
        } else {
            actions.join(", ")
        }
    }

    fn extract_gist(turns: &[String]) -> String {
        let first = turns
            .first()
            .map(|t| t.chars().take(80).collect::<String>())
            .unwrap_or_default();
        let last = turns
            .last()
            .map(|t| t.chars().take(80).collect::<String>())
            .unwrap_or_default();
        format!("Started with '{}...', ended with '{}...'", first, last)
    }
}

/// Helper: format episode type as short tag
fn format_episode_short(ep: &Episode) -> &'static str {
    match ep.episode_type {
        EpisodeType::UserRequest => "USER",
        EpisodeType::ToolCalled => "TOOL",
        EpisodeType::Observation => "OBS",
        EpisodeType::ErrorOccurred => "ERR",
        EpisodeType::Summary => "SUM",
        EpisodeType::UserPreference => "PREF",
        _ => "???",
    }
}

/// Helper: truncate string to max length with ellipsis
fn truncate_str(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
}
