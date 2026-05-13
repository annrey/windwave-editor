//! Memory Injector - Sprint 2: 自动捕获上下文并注入 LLM
//!
//! 实现记忆系统的核心功能：
//! 1. MemoryInjector - 自动检索相关记忆并注入 LLM 上下文
//! 2. ProjectMemory - 项目级持久记忆
//! 3. CodeIndex - 代码符号索引
//! 4. PatternLearner - 从操作中学习代码模式
//! 5. WorkingSet - 追踪当前焦点

use crate::types::{EntityId, current_timestamp};
use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::fs;

// ============================================================================
// ProjectMemory - 项目级持久记忆
// ============================================================================

/// 项目元数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectManifest {
    pub name: String,
    pub engine: String,
    pub engine_version: String,
    pub created_at: u64,
    pub last_modified: u64,
}

impl Default for ProjectManifest {
    fn default() -> Self {
        Self {
            name: "Untitled".into(),
            engine: "bevy".into(),
            engine_version: "0.17".into(),
            created_at: current_timestamp(),
            last_modified: current_timestamp(),
        }
    }
}

/// 实体知识库 - 记住每个实体的详细信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityKnowledge {
    pub name: String,
    pub entity_id: EntityId,
    pub purpose: String,                    // "玩家角色，由 WASD 控制移动"
    pub systems: Vec<String>,               // ["player_movement", "camera_follow"]
    pub components: Vec<String>,            // ["Transform", "Sprite", "Player"]
    pub related_entities: Vec<String>,
    pub common_operations: Vec<String>,     // 用户常用的操作
    pub last_modified: u64,
    pub notes: String,
}

impl EntityKnowledge {
    pub fn new(name: String, entity_id: EntityId) -> Self {
        Self {
            name,
            entity_id,
            purpose: String::new(),
            systems: Vec::new(),
            components: Vec::new(),
            related_entities: Vec::new(),
            common_operations: Vec::new(),
            last_modified: current_timestamp(),
            notes: String::new(),
        }
    }

    /// 更新实体信息
    pub fn update(&mut self, purpose: Option<&str>, components: Option<&Vec<String>>) {
        if let Some(p) = purpose {
            self.purpose = p.to_string();
        }
        if let Some(c) = components {
            self.components = c.clone();
        }
        self.last_modified = current_timestamp();
    }

    /// 记录一次操作
    pub fn record_operation(&mut self, operation: &str) {
        if !self.common_operations.contains(&operation.to_string()) {
            self.common_operations.push(operation.to_string());
        }
        self.last_modified = current_timestamp();
    }
}

/// 系统知识库
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemKnowledge {
    pub name: String,
    pub file_path: String,
    pub description: String,
    pub entities_affected: Vec<String>,
    pub components_used: Vec<String>,
    pub dependencies: Vec<String>,
}

/// 项目约定 - 从观察中学习的命名规范、代码风格等
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Convention {
    pub category: ConventionCategory,
    pub description: String,
    pub example: String,
    pub confidence: f32,  // 0.0-1.0，从观察中学习的置信度
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConventionCategory {
    Naming,     // "实体名用 PascalCase"
    Style,      // "组件用派生宏 #[derive(Component)]"
    Pattern,    // "移动系统用 Query<&mut Transform>"
}

/// 项目工作流模板
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowTemplate {
    pub name: String,
    pub trigger_keywords: Vec<String>,
    pub steps: Vec<String>,
    pub success_rate: f32,
}

/// 项目变更历史
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectChange {
    pub timestamp: u64,
    pub description: String,
    pub entity_affected: Option<String>,
    pub change_type: ChangeType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChangeType {
    Created,
    Modified,
    Deleted,
    Moved,
}

/// 项目记忆 - 跨会话记住项目的所有重要信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectMemory {
    pub manifest: ProjectManifest,
    /// 实体知识库 (名称 → 实体知识)
    pub entity_knowledge: HashMap<String, EntityKnowledge>,
    /// 系统知识库 (系统名 → 系统知识)
    pub system_knowledge: HashMap<String, SystemKnowledge>,
    /// 项目约定
    pub conventions: Vec<Convention>,
    /// 项目特有的工作流模板
    pub workflows: Vec<WorkflowTemplate>,
    /// 项目变更历史
    pub change_log: Vec<ProjectChange>,
    /// 用户偏好
    pub user_preferences: HashMap<String, String>,
}

impl ProjectMemory {
    pub fn new(manifest: ProjectManifest) -> Self {
        Self {
            manifest,
            entity_knowledge: HashMap::new(),
            system_knowledge: HashMap::new(),
            conventions: Vec::new(),
            workflows: Vec::new(),
            change_log: Vec::new(),
            user_preferences: HashMap::new(),
        }
    }

    /// 添加或更新实体知识
    pub fn add_entity(&mut self, knowledge: EntityKnowledge) {
        self.entity_knowledge.insert(knowledge.name.clone(), knowledge);
        self.manifest.last_modified = current_timestamp();
    }

    /// 获取实体知识
    pub fn get_entity(&self, name: &str) -> Option<&EntityKnowledge> {
        self.entity_knowledge.get(name)
    }

    /// 获取实体的可变引用
    pub fn get_entity_mut(&mut self, name: &str) -> Option<&mut EntityKnowledge> {
        self.entity_knowledge.get_mut(name)
    }

    /// 记录变更
    pub fn record_change(&mut self, description: String, entity: Option<String>, change_type: ChangeType) {
        self.change_log.push(ProjectChange {
            timestamp: current_timestamp(),
            description,
            entity_affected: entity,
            change_type,
        });
        // 只保留最近的 100 条变更
        if self.change_log.len() > 100 {
            self.change_log.drain(0..self.change_log.len() - 100);
        }
        self.manifest.last_modified = current_timestamp();
    }

    /// 设置用户偏好
    pub fn set_preference(&mut self, key: &str, value: &str) {
        self.user_preferences.insert(key.to_string(), value.to_string());
    }

    /// 获取用户偏好
    pub fn get_preference(&self, key: &str) -> Option<&String> {
        self.user_preferences.get(key)
    }

    /// 持久化到文件
    pub fn save(&self, path: &Path) -> Result<(), MemoryError> {
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| MemoryError::Serialization(e.to_string()))?;
        fs::write(path, json).map_err(|e| MemoryError::Io(e.to_string()))?;
        Ok(())
    }

    /// 从文件加载
    pub fn load(path: &Path) -> Result<Self, MemoryError> {
        if !path.exists() {
            return Ok(Self::new(ProjectManifest::default()));
        }
        let json = fs::read_to_string(path)
            .map_err(|e| MemoryError::Io(e.to_string()))?;
        serde_json::from_str(&json).map_err(|e| MemoryError::Serialization(e.to_string()))
    }

    /// 生成实体描述（用于 LLM 上下文）
    pub fn describe_entities(&self, names: &[String]) -> String {
        if names.is_empty() {
            return "(no entities)".into();
        }
        
        let mut parts = Vec::new();
        for name in names {
            if let Some(entity) = self.entity_knowledge.get(name) {
                parts.push(format!(
                    "- {}: {} (组件: {}, 关联系统: {})",
                    entity.name,
                    entity.purpose,
                    entity.components.join(", "),
                    entity.systems.join(", ")
                ));
            }
        }
        parts.join("\n")
    }
}

// ============================================================================
// CodeIndex - 代码符号索引
// ============================================================================

/// 符号类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum SymbolKind {
    Struct,
    Fn,
    Trait,
    Enum,
    System,     // Bevy system
    Component,  // Bevy component
    Resource,   // Bevy resource
    Event,      // Bevy event
    Plugin,     // Bevy plugin
}

/// 代码符号
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Symbol {
    pub name: String,
    pub kind: SymbolKind,
    pub file_path: String,
    pub line: usize,
    pub doc_comment: Option<String>,
    pub visibility: Visibility,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Visibility {
    Pub,
    PubCrate,
    Private,
}

/// 代码索引 - 让 Agent 知道"代码在哪"
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeIndex {
    /// 所有符号的索引
    pub symbols: Vec<Symbol>,
    /// 文件路径 → 符号列表
    pub file_index: HashMap<String, Vec<usize>>,
    /// 符号名 → 索引
    pub name_index: HashMap<String, usize>,
    /// 符号 kind → 符号列表
    pub kind_index: HashMap<SymbolKind, Vec<usize>>,
}

impl CodeIndex {
    pub fn new() -> Self {
        Self {
            symbols: Vec::new(),
            file_index: HashMap::new(),
            name_index: HashMap::new(),
            kind_index: HashMap::new(),
        }
    }

    /// 添加符号
    pub fn add(&mut self, symbol: Symbol) {
        let idx = self.symbols.len();
        let name = symbol.name.clone();
        let file = symbol.file_path.clone();
        let kind = symbol.kind.clone();

        self.symbols.push(symbol);

        // 更新索引
        self.file_index.entry(file).or_default().push(idx);
        self.name_index.insert(name, idx);
        self.kind_index.entry(kind).or_default().push(idx);
    }

    /// 按名称查找符号
    pub fn find_by_name(&self, name: &str) -> Option<&Symbol> {
        self.name_index.get(name).and_then(|&idx| self.symbols.get(idx))
    }

    /// 按文件查找符号
    pub fn find_by_file(&self, file_path: &str) -> Vec<&Symbol> {
        self.file_index
            .get(file_path)
            .map(|indices| indices.iter().filter_map(|&i| self.symbols.get(i)).collect())
            .unwrap_or_default()
    }

    /// 按类型查找符号
    pub fn find_by_kind(&self, kind: SymbolKind) -> Vec<&Symbol> {
        self.kind_index
            .get(&kind)
            .map(|indices| indices.iter().filter_map(|&i| self.symbols.get(i)).collect())
            .unwrap_or_default()
    }

    /// 查找相关符号（模糊匹配）
    pub fn find_relevant(&self, query: &str) -> Vec<&Symbol> {
        let query_lower = query.to_lowercase();
        self.symbols
            .iter()
            .filter(|s| s.name.to_lowercase().contains(&query_lower))
            .collect()
    }

    /// 生成代码引用描述（用于 LLM 上下文）
    pub fn describe_relevant(&self, query: &str) -> String {
        let relevant = self.find_relevant(query);
        if relevant.is_empty() {
            return "(no relevant code found)".into();
        }
        
        relevant
            .iter()
            .map(|s| format!("- {} ({}:{}) - {:?}", s.name, s.file_path, s.line, s.kind))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

impl Default for CodeIndex {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// PatternLearner - 从操作中学习代码模式
// ============================================================================

/// 观察到的模式
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservedPattern {
    pub name: String,
    pub trigger_keywords: Vec<String>,
    pub template: String,
    pub context: String,           // 在什么场景下使用
    pub observation_count: usize,
    pub last_observed: u64,
    pub success_rate: f32,
}

/// 模式学习者 - 从用户操作中自动学习代码模式和项目约定
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternLearner {
    /// 观察到的模式
    pub patterns: Vec<ObservedPattern>,
    /// 最小观察次数才视为模式
    pub min_observations: usize,
    /// 候选模式（未达到阈值）
    pub candidates: HashMap<String, CandidatePattern>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandidatePattern {
    pub trigger_keywords: Vec<String>,
    pub template: String,
    pub context: String,
    pub observation_count: usize,
    pub successes: usize,
    pub failures: usize,
}

impl PatternLearner {
    pub fn new(min_observations: usize) -> Self {
        Self {
            patterns: Vec::new(),
            min_observations,
            candidates: HashMap::new(),
        }
    }

    /// 观察一次操作，学习模式
    pub fn observe(&mut self, operation: &str, context: &str, success: bool) {
        // 提取操作的关键特征（简化版：使用关键词）
        let keywords: Vec<String> = operation
            .split_whitespace()
            .map(|s| s.to_lowercase())
            .collect();

        // 检查是否匹配已有模式
        let mut matched = false;
        for pattern in &mut self.patterns {
            if keywords.iter().any(|k| pattern.trigger_keywords.iter().any(|pk| pk == k)) {
                pattern.observation_count += 1;
                pattern.last_observed = current_timestamp();
                if success {
                    let count = pattern.observation_count as f32;
                    pattern.success_rate = (pattern.success_rate * (count - 1.0) + 1.0) / count;
                }
                matched = true;
                break;
            }
        }

        if !matched {
            // 记录为候选
            let key = keywords.join("_");
            let candidate = self.candidates.entry(key.clone()).or_insert(CandidatePattern {
                trigger_keywords: keywords.clone(),
                template: operation.to_string(),
                context: context.to_string(),
                observation_count: 0,
                successes: 0,
                failures: 0,
            });
            
            candidate.observation_count += 1;
            if success {
                candidate.successes += 1;
            } else {
                candidate.failures += 1;
            }

            // 达到阈值 → 提升为正式模式
            if candidate.observation_count >= self.min_observations {
                let pattern = ObservedPattern {
                    name: key.clone(),
                    trigger_keywords: candidate.trigger_keywords.clone(),
                    template: candidate.template.clone(),
                    context: candidate.context.clone(),
                    observation_count: candidate.observation_count,
                    last_observed: current_timestamp(),
                    success_rate: if candidate.observation_count > 0 {
                        candidate.successes as f32 / candidate.observation_count as f32
                    } else {
                        0.0
                    },
                };
                self.patterns.push(pattern);
                self.candidates.remove(&key);
            }
        }
    }

    /// 根据用户输入推荐模式
    pub fn suggest(&self, input: &str) -> Vec<&ObservedPattern> {
        let input_lower = input.to_lowercase();
        let mut matches: Vec<&ObservedPattern> = self
            .patterns
            .iter()
            .filter(|p| p.trigger_keywords.iter().any(|k| input_lower.contains(k)))
            .collect();
        
        // 按成功率和观察次数排序
        matches.sort_by(|a, b| {
            b.success_rate.partial_cmp(&a.success_rate)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| b.observation_count.cmp(&a.observation_count))
        });
        
        matches
    }

    /// 生成模式描述（用于 LLM 上下文）
    pub fn describe_patterns(&self, input: &str) -> String {
        let suggestions = self.suggest(input);
        if suggestions.is_empty() {
            return "(no learned patterns match)".into();
        }
        
        suggestions
            .iter()
            .take(3)
            .map(|p| format!("- {}: {} (成功率: {:.0}%, 观察 {} 次)", p.name, p.template, p.success_rate * 100.0, p.observation_count))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

// ============================================================================
// WorkingSet - 当前焦点追踪
// ============================================================================

/// 当前工作集 - 追踪用户当前在做什么、关注什么
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkingSet {
    /// 当前打开的文件
    pub open_files: Vec<String>,
    /// 最近选中的实体
    pub recent_entities: Vec<String>,
    /// 当前活跃的系统/组件
    pub active_context: Vec<String>,
    /// 当前任务描述
    pub current_task: Option<String>,
    /// 时间戳
    pub last_updated: u64,
}

impl WorkingSet {
    pub fn new() -> Self {
        Self {
            open_files: Vec::new(),
            recent_entities: Vec::new(),
            active_context: Vec::new(),
            current_task: None,
            last_updated: current_timestamp(),
        }
    }

    /// 更新打开的文件
    pub fn set_open_files(&mut self, files: Vec<String>) {
        self.open_files = files;
        self.last_updated = current_timestamp();
    }

    /// 记录选中的实体
    pub fn select_entity(&mut self, entity_name: &str) {
        // 添加到最近列表（去重）
        self.recent_entities.retain(|e| e != entity_name);
        self.recent_entities.insert(0, entity_name.to_string());
        // 只保留最近的 5 个
        if self.recent_entities.len() > 5 {
            self.recent_entities.pop();
        }
        self.last_updated = current_timestamp();
    }

    /// 设置当前任务
    pub fn set_task(&mut self, task: &str) {
        self.current_task = Some(task.to_string());
        self.last_updated = current_timestamp();
    }

    /// 添加活跃上下文
    pub fn add_context(&mut self, context: &str) {
        if !self.active_context.contains(&context.to_string()) {
            self.active_context.push(context.to_string());
        }
        self.last_updated = current_timestamp();
    }

    /// 生成工作集描述（用于 LLM 上下文）
    pub fn describe(&self) -> String {
        let files_str = if self.open_files.is_empty() {
            "(none)".to_string()
        } else {
            self.open_files.join(", ")
        };
        let entities_str = if self.recent_entities.is_empty() {
            "(none)".to_string()
        } else {
            self.recent_entities.join(", ")
        };
        let context_str = if self.active_context.is_empty() {
            "(none)".to_string()
        } else {
            self.active_context.join(", ")
        };
        
        format!(
            "## 当前工作集\n\
             打开文件: {}\n\
             最近实体: {}\n\
             活跃上下文: {}\n\
             当前任务: {}",
            files_str,
            entities_str,
            context_str,
            self.current_task.as_deref().unwrap_or("(none)")
        )
    }
}

impl Default for WorkingSet {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// MemoryInjector - 自动检索记忆并注入 LLM
// ============================================================================

/// 记忆上下文 - 注入到 LLM 的所有记忆信息
#[derive(Debug, Clone)]
pub struct MemoryContext {
    /// 工作集信息
    pub working_set: String,
    /// 相关实体知识
    pub relevant_entities: String,
    /// 相关代码引用
    pub code_references: String,
    /// 推荐模式
    pub suggested_patterns: String,
    /// 最近记忆
    pub recent_memories: String,
    /// 用户偏好
    pub user_preferences: String,
}

impl MemoryContext {
    pub fn describe(&self) -> String {
        format!(
            "【记忆上下文】\n\
             {}\n\n\
             {}\n\n\
             {}\n\n\
             {}\n\n\
             {}\n\n\
             {}",
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
    }
}

/// 记忆注入器 - 自动检索相关记忆并注入 LLM 上下文
pub struct MemoryInjector {
    project_memory: ProjectMemory,
    code_index: CodeIndex,
    pattern_learner: PatternLearner,
    working_set: WorkingSet,
    /// 记忆文件路径
    memory_path: Option<PathBuf>,
}

#[derive(Debug, thiserror::Error)]
pub enum MemoryError {
    #[error("Serialization error: {0}")]
    Serialization(String),
    #[error("IO error: {0}")]
    Io(String),
    #[error("LLM error: {0}")]
    Llm(String),
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
            pattern_learner: PatternLearner::new(3),  // 观察 3 次后学习
            working_set: WorkingSet::new(),
            memory_path,
        }
    }

    /// 为 LLM 请求构建记忆上下文
    pub fn build_context(&self, user_input: &str) -> MemoryContext {
        // 1. 获取当前工作集
        let working = &self.working_set;

        // 2. 获取相关实体知识
        let entities = self.extract_entity_mentions(user_input);
        let relevant_entities = self.project_memory.describe_entities(&entities);

        // 3. 获取相关代码位置
        let code_refs = self.code_index.describe_relevant(user_input);

        // 4. 获取相关模式
        let patterns = self.pattern_learner.describe_patterns(user_input);

        // 5. 获取用户偏好
        let preferences = self.describe_preferences();

        // 6. 最近记忆（变更历史）
        let recent = self.describe_recent_changes();

        MemoryContext {
            working_set: working.describe(),
            relevant_entities,
            code_references: code_refs,
            suggested_patterns: patterns,
            recent_memories: recent,
            user_preferences: preferences,
        }
    }

    /// 提取用户输入中提到的实体名称
    fn extract_entity_mentions(&self, input: &str) -> Vec<String> {
        // 简化版：查找大写字母开头的单词（实体名通常是 PascalCase）
        let mut entities = Vec::new();
        for word in input.split_whitespace() {
            let cleaned: String = word.chars()
                .filter(|c| c.is_alphanumeric())
                .collect();
            if cleaned.len() > 1 && cleaned.chars().next().unwrap_or(' ').is_uppercase() {
                // 检查是否在项目记忆中存在
                if self.project_memory.get_entity(&cleaned).is_some() {
                    entities.push(cleaned);
                }
            }
        }
        entities
    }

    /// 描述用户偏好
    fn describe_preferences(&self) -> String {
        if self.project_memory.user_preferences.is_empty() {
            return "(no user preferences recorded)".into();
        }
        
        self.project_memory.user_preferences
            .iter()
            .map(|(k, v)| format!("- {}: {}", k, v))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// 描述最近变更
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

    /// 更新项目记忆（当实体被创建/修改时）
    pub fn update_entity(&mut self, name: &str, entity_id: EntityId, components: &[String], purpose: Option<&str>) {
        let mut knowledge = EntityKnowledge::new(name.to_string(), entity_id);
        let comps: Vec<String> = components.to_vec();
        knowledge.update(purpose, Some(&comps));
        self.project_memory.add_entity(knowledge);
        self.persist();
    }

    /// 记录操作（用于模式学习）
    pub fn record_operation(&mut self, operation: &str, context: &str, success: bool) {
        self.pattern_learner.observe(operation, context, success);
        self.persist();
    }

    /// 记录用户偏好
    pub fn record_preference(&mut self, key: &str, value: &str) {
        self.project_memory.set_preference(key, value);
        self.persist();
    }

    /// 记录变更
    pub fn record_change(&mut self, description: String, entity: Option<String>, change_type: ChangeType) {
        self.project_memory.record_change(description, entity, change_type);
        self.persist();
    }

    /// 持久化记忆
    fn persist(&self) {
        if let Some(path) = &self.memory_path {
            if let Err(e) = self.project_memory.save(path) {
                eprintln!("[MemoryInjector] Failed to save memory: {}", e);
            }
        }
    }

    /// 加载代码索引（从项目目录扫描）
    pub fn load_code_index(&mut self, project_path: &Path) {
        // 简化版：扫描 src/ 目录下的 .rs 文件
        // 生产环境应该使用 Tree-sitter 或 cargo doc
        if let Ok(entries) = fs::read_dir(project_path) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() && path.file_name().map(|n| n == "src").unwrap_or(false) {
                    self.scan_directory(&path);
                }
            }
        }
    }

    /// 扫描目录中的 Rust 文件
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

    /// 解析 Rust 文件（简化版：正则匹配）
    fn parse_file(&mut self, path: &Path) {
        let content = fs::read_to_string(path).unwrap_or_default();
        let file_path = path.to_string_lossy().to_string();
        
        // 简化版：匹配 pub struct, pub fn, pub struct ... (Component) 等
        for line in content.lines() {
            // 匹配组件
            if line.contains("#[derive(Component)]") || line.contains("Component") {
                if let Some(name) = self.extract_struct_name(line) {
                    self.code_index.add(Symbol {
                        name,
                        kind: SymbolKind::Component,
                        file_path: file_path.clone(),
                        line: 0,  // 简化版不记录行号
                        doc_comment: None,
                        visibility: Visibility::Pub,
                    });
                }
            }
            // 匹配系统
            if line.contains("fn ") && line.contains("query") {
                if let Some(name) = self.extract_fn_name(line) {
                    self.code_index.add(Symbol {
                        name,
                        kind: SymbolKind::System,
                        file_path: file_path.clone(),
                        line: 0,
                        doc_comment: None,
                        visibility: Visibility::Pub,
                    });
                }
            }
        }
    }

    fn extract_struct_name(&self, line: &str) -> Option<String> {
        // 简化版：提取 pub struct XXX
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
        // 简化版：提取 fn xxx
        if let Some(start) = line.find("fn ") {
            let rest = &line[start + 3..];
            let name: String = rest.chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
            if !name.is_empty() {
                return Some(name);
            }
        }
        None
    }
}

// 为 ChangeType 添加辅助方法
impl ProjectChange {
    fn change_type_as_string(&self) -> &'static str {
        match self.change_type {
            ChangeType::Created => "创建",
            ChangeType::Modified => "修改",
            ChangeType::Deleted => "删除",
            ChangeType::Moved => "移动",
        }
    }
}

// ============================================================================
// MemoryCompressor - LLM 记忆压缩
// ============================================================================

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
    /// 触发压缩的对话轮数阈值
    pub compression_threshold: usize,
    /// 已压缩的摘要历史
    pub summaries: Vec<ConversationSummary>,
    /// 当前未压缩的对话内容
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

    /// 添加对话内容
    pub fn add_content(&mut self, content: &str) {
        self.pending_content.push_str(content);
        self.pending_content.push('\n');
    }

    /// 检查是否需要压缩
    pub fn needs_compression(&self) -> bool {
        // 简化版：根据字符数估算轮数（实际应该按消息数计算）
        let estimated_turns = self.pending_content.split('\n').filter(|s| !s.is_empty()).count();
        estimated_turns >= self.compression_threshold
    }

    /// 压缩记忆（简化版：返回摘要文本）
    /// 生产环境应该调用 LLM 进行智能摘要
    pub fn compress(&mut self) -> Result<ConversationSummary, MemoryError> {
        if self.pending_content.is_empty() {
            return Err(MemoryError::Serialization("No content to compress".into()));
        }

        let original_turns = self.pending_content.split('\n').filter(|s| !s.is_empty()).count();
        
        // 简化版摘要：提取关键信息
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

        // 保存摘要
        self.summaries.push(summary.clone());
        
        // 清空待压缩内容
        self.pending_content.clear();

        Ok(summary)
    }

    /// 生成摘要文本（简化版）
    fn generate_summary(&self, content: &str) -> String {
        // 简化版：取前 200 个字符作为摘要
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

    /// 提取关键点
    fn extract_key_points(&self, content: &str) -> Vec<String> {
        let mut points = Vec::new();
        
        // 简化版：查找包含"创建"、"修改"、"删除"等关键词的行
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

    /// 提取实体名称
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
                // 排除一些常见词
                if !["The", "And", "For", "With", "From", "This", "That"].contains(&cleaned.as_str()) {
                    entities.push(cleaned.clone());
                    seen.insert(cleaned);
                }
            }
        }
        
        entities
    }

    /// 获取所有摘要
    pub fn get_summaries(&self) -> &[ConversationSummary] {
        &self.summaries
    }

    /// 生成摘要描述（用于 LLM 上下文）
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

// ============================================================================
// 测试
// ============================================================================

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

// ============================================================================
// 单元测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_project_memory_entity_knowledge() {
        let mut memory = ProjectMemory::new(ProjectManifest::default());
        
        let entity = EntityKnowledge::new("Player".into(), EntityId(1));
        memory.add_entity(entity);
        
        assert!(memory.get_entity("Player").is_some());
        assert_eq!(memory.get_entity("Player").unwrap().name, "Player");
    }

    #[test]
    fn test_code_index() {
        let mut index = CodeIndex::new();
        
        index.add(Symbol {
            name: "Player".into(),
            kind: SymbolKind::Component,
            file_path: "src/components/player.rs".into(),
            line: 15,
            doc_comment: None,
            visibility: Visibility::Pub,
        });
        
        assert!(index.find_by_name("Player").is_some());
        assert_eq!(index.find_by_name("Player").unwrap().kind, SymbolKind::Component);
    }

    #[test]
    fn test_pattern_learner() {
        let mut learner = PatternLearner::new(2);
        
        learner.observe("create_entity Player", "场景编辑", true);
        learner.observe("create_entity Player", "场景编辑", true);
        
        let suggestions = learner.suggest("创建一个 Player");
        assert!(!suggestions.is_empty());
    }

    #[test]
    fn test_working_set() {
        let mut ws = WorkingSet::new();
        
        ws.select_entity("Player");
        ws.select_entity("Enemy");
        ws.set_task("调整玩家位置");
        
        let desc = ws.describe();
        assert!(desc.contains("Player"));
        assert!(desc.contains("Enemy"));
        assert!(desc.contains("调整玩家位置"));
    }

    #[test]
    fn test_memory_injector_context() {
        let injector = MemoryInjector::new(None);
        let ctx = injector.build_context("把 Player 移到右边");
        
        // 应该包含工作集信息
        assert!(!ctx.working_set.is_empty());
    }
}
