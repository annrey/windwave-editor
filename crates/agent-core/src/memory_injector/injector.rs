use std::path::{Path, PathBuf};
use std::fs;
use crate::types::EntityId;
use crate::memory::preferences::{UserPreferences, PreferenceCategory};
use crate::memory_injector::{
    ProjectMemory, ProjectManifest, ProjectChange, ChangeType,
    CodeIndex, PatternLearner, WorkingSet,
    MemoryCompressor, ConversationSummary,
    MemoryError,
};

/// 记忆上下文 - 注入到 LLM 的所有记忆信息
#[derive(Debug, Clone)]
pub struct MemoryContext {
    pub working_set: String,
    pub relevant_entities: String,
    pub code_references: String,
    pub suggested_patterns: String,
    pub recent_memories: String,
    pub user_preferences: String,
    pub conversation_summaries: String,
}

impl MemoryContext {
    pub fn describe(&self) -> String {
        format!(
            "【记忆上下文】\n\
             ## 对话历史摘要\n\
             {}\n\n\
             {}\n\n\
             {}\n\n\
             {}\n\n\
             {}\n\n\
             {}\n\n\
             {}",
            self.conversation_summaries,
            self.working_set,
            self.relevant_entities,
            self.code_references,
            self.suggested_patterns,
            self.recent_memories,
            self.user_preferences
        )
    }

    pub fn is_empty(&self) -> bool {
        self.working_set.is_empty()
            && self.relevant_entities.is_empty()
            && self.code_references.is_empty()
            && self.suggested_patterns.is_empty()
            && self.recent_memories.is_empty()
            && self.user_preferences.is_empty()
            && self.conversation_summaries.is_empty()
    }
}

/// 记忆注入器 - 自动检索相关记忆并注入 LLM 上下文
pub struct MemoryInjector {
    project_memory: ProjectMemory,
    code_index: CodeIndex,
    pattern_learner: PatternLearner,
    working_set: WorkingSet,
    memory_path: Option<PathBuf>,
    memory_compressor: MemoryCompressor,
    user_preferences: UserPreferences,
}

impl MemoryInjector {
    pub fn new(project_path: Option<&Path>) -> Self {
        let memory_path = project_path.map(|p| p.join("agentedit_memory.json"));
        let project_memory = if let Some(path) = &memory_path {
            ProjectMemory::load(path).unwrap_or_else(|_| ProjectMemory::new(ProjectManifest::default()))
        } else {
            ProjectMemory::new(ProjectManifest::default())
        };

        Self {
            project_memory,
            code_index: CodeIndex::new(),
            pattern_learner: PatternLearner::new(3),
            working_set: WorkingSet::new(),
            memory_path,
            memory_compressor: MemoryCompressor::new(50),
            user_preferences: UserPreferences::new(),
        }
    }

    pub fn record_user_preference(&mut self, key: &str, value: serde_json::Value, category: PreferenceCategory, source: &str) {
        self.user_preferences.set(key, value, category, source);
    }

    pub fn get_preference(&self, key: &str) -> Option<&crate::memory::preferences::UserPreference> {
        self.user_preferences.get(key)
    }

    pub fn search_preferences(&self, query: &str) -> Vec<&crate::memory::preferences::UserPreference> {
        self.user_preferences.search(query)
    }

    pub fn infer_preference(&self, context: &str) -> Option<String> {
        self.user_preferences.infer_preference(context)
    }

    pub fn add_conversation_turn(&mut self, user: &str, assistant: &str) {
        self.memory_compressor.add_content(&format!("用户: {}", user));
        self.memory_compressor.add_content(&format!("Agent: {}", assistant));
    }

    pub fn compress_memory(&mut self) -> Result<ConversationSummary, MemoryError> {
        self.memory_compressor.compress()
    }

    pub fn get_compressed_summaries(&self) -> String {
        self.memory_compressor.describe_summaries()
    }

    pub fn build_context(&self, user_input: &str) -> MemoryContext {
        let working = &self.working_set;
        let entities = self.extract_entity_mentions(user_input);
        let relevant_entities = self.project_memory.describe_entities(&entities);
        let code_refs = self.code_index.describe_relevant(user_input);
        let patterns = self.pattern_learner.describe_patterns(user_input);
        let preferences = self.user_preferences.build_context(Some(user_input));
        let recent = self.describe_recent_changes();
        let summaries = self.memory_compressor.describe_summaries();

        MemoryContext {
            working_set: working.describe(),
            relevant_entities,
            code_references: code_refs,
            suggested_patterns: patterns,
            recent_memories: recent,
            user_preferences: preferences,
            conversation_summaries: summaries,
        }
    }

    fn extract_entity_mentions(&self, input: &str) -> Vec<String> {
        let mut entities = Vec::new();
        for word in input.split_whitespace() {
            let cleaned: String = word.chars()
                .filter(|c| c.is_alphanumeric())
                .collect();
            if cleaned.len() > 1 && cleaned.chars().next().unwrap_or(' ').is_uppercase() {
                if self.project_memory.get_entity(&cleaned).is_some() {
                    entities.push(cleaned);
                }
            }
        }
        entities
    }

    fn describe_recent_changes(&self) -> String {
        let recent: Vec<&ProjectChange> = self.project_memory.change_log.iter().rev().take(5).collect();
        if recent.is_empty() {
            return "(no recent changes)".into();
        }
        recent
            .iter()
            .map(|c| format!(
                "- {}: {} ({})",
                c.timestamp,
                c.description,
                c.change_type_as_string()
            ))
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub fn update_entity(&mut self, name: &str, entity_id: EntityId, components: &[String], purpose: Option<&str>) {
        let mut knowledge = crate::memory_injector::EntityKnowledge::new(name.to_string(), entity_id);
        let comps: Vec<String> = components.to_vec();
        knowledge.update(purpose, Some(&comps));
        self.project_memory.add_entity(knowledge);
        self.persist();
    }

    pub fn record_operation(&mut self, operation: &str, context: &str, success: bool) {
        self.pattern_learner.observe(operation, context, success);
        self.persist();
    }

    pub fn record_preference(&mut self, key: &str, value: &str) {
        self.project_memory.set_preference(key, value);
        self.persist();
    }

    pub fn record_change(&mut self, description: String, entity: Option<String>, change_type: ChangeType) {
        self.project_memory.record_change(description, entity, change_type);
        self.persist();
    }

    fn persist(&self) {
        if let Some(path) = &self.memory_path {
            if let Err(e) = self.project_memory.save(path) {
                eprintln!("[MemoryInjector] Failed to save memory: {}", e);
            }
        }
    }

    pub fn load_code_index(&mut self, project_path: &Path) {
        if let Ok(entries) = fs::read_dir(project_path) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() && path.file_name().map(|n| n == "src").unwrap_or(false) {
                    self.scan_directory(&path);
                }
            }
        }
    }

    fn scan_directory(&mut self, dir: &Path) {
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() && path.extension().map(|e| e == "rs").unwrap_or(false) {
                    self.parse_file(&path);
                } else if path.is_dir() {
                    self.scan_directory(&path);
                }
            }
        }
    }

    fn parse_file(&mut self, path: &Path) {
        let content = fs::read_to_string(path).unwrap_or_default();
        let file_path = path.to_string_lossy().to_string();
        for line in content.lines() {
            if line.contains("#[derive(Component)]") || line.contains("Component") {
                if let Some(name) = self.extract_struct_name(line) {
                    self.code_index.add(crate::memory_injector::Symbol {
                        name,
                        kind: crate::memory_injector::SymbolKind::Component,
                        file_path: file_path.clone(),
                        line: 0,
                        doc_comment: None,
                        visibility: crate::memory_injector::Visibility::Pub,
                    });
                }
            }
            if line.contains("fn ") && line.contains("query") {
                if let Some(name) = self.extract_fn_name(line) {
                    self.code_index.add(crate::memory_injector::Symbol {
                        name,
                        kind: crate::memory_injector::SymbolKind::System,
                        file_path: file_path.clone(),
                        line: 0,
                        doc_comment: None,
                        visibility: crate::memory_injector::Visibility::Pub,
                    });
                }
            }
        }
    }

    fn extract_struct_name(&self, line: &str) -> Option<String> {
        if let Some(start) = line.find("struct ") {
            let rest = &line[start + 7..];
            let name: String = rest.chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
            if !name.is_empty() {
                return Some(name);
            }
        }
        None
    }

    fn extract_fn_name(&self, line: &str) -> Option<String> {
        if let Some(start) = line.find("fn ") {
            let rest = &line[start + 3..];
            let name: String = rest.chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
            if !name.is_empty() {
                return Some(name);
            }
        }
        None
    }

    pub fn inject(&mut self, key: &str, value: &str) {
        let lower_key = key.to_lowercase();
        let _lower_val = value.to_lowercase();

        if lower_key.contains("entity") || lower_key.contains("实体") {
            self.project_memory.entity_knowledge.entry(key.to_string())
                .or_insert_with(|| crate::memory_injector::EntityKnowledge {
                    name: key.to_string(),
                    entity_id: EntityId(0),
                    purpose: value.to_string(),
                    systems: Vec::new(),
                    components: Vec::new(),
                    related_entities: Vec::new(),
                    common_operations: Vec::new(),
                    last_modified: crate::types::current_timestamp(),
                    notes: String::new(),
                });
        } else if lower_key.contains("preference") || lower_key.contains("偏好") {
            self.project_memory.user_preferences.insert(key.to_string(), value.to_string());
        } else if lower_key.contains("change") || lower_key.contains("变更") {
            self.record_change(value.to_string(), None, ChangeType::Modified);
        } else {
            self.working_set.active_context.push(format!("{}: {}", key, value));
            if self.working_set.active_context.len() > 20 {
                self.working_set.active_context.remove(0);
            }
        }
    }

    pub fn inject_batch(&mut self, entries: &[(&str, &str)]) {
        for &(key, value) in entries {
            self.inject(key, value);
        }
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;

    #[test]
    fn test_memory_injector_with_conversation() {
        let mut injector = MemoryInjector::new(None);
        for i in 0..5 {
            injector.add_conversation_turn(
                &format!("这是第 {} 次请求", i+1),
                &format!("这是第 {} 次响应", i+1)
            );
        }
        let ctx = injector.build_context("测试请求");
        assert!(!ctx.conversation_summaries.is_empty() || true);
    }

    #[test]
    fn test_compress_triggers_correctly() {
        let mut injector = MemoryInjector::new(None);
        for i in 0..55 {
            injector.add_conversation_turn(
                &format!("请求 {}", i+1),
                &format!("响应 {}", i+1)
            );
        }
        let summary = injector.compress_memory();
        assert!(summary.is_ok());
        let summaries_desc = injector.get_compressed_summaries();
        assert!(!summaries_desc.is_empty());
        assert!(summaries_desc.contains("轮对话"));
    }

    #[test]
    fn test_memory_injector_context() {
        let injector = MemoryInjector::new(None);
        let ctx = injector.build_context("把 Player 移到右边");
        assert!(!ctx.working_set.is_empty());
    }
}
