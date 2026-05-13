//! Index system — navigable maps that agents use to understand the project,
//! scene, and available skills without touching raw engine internals.
//!
//! Design reference: Section 6 + Section 12.1 of
//! gpt-agent-team-task-event-skill-architecture.md
//!
//! Three index types:
//! - ProjectIndex  → file/crate structure for CodeAgent
//! - SemanticIndex → topic/keyword mapping for MemoryAgent
//! - SkillIndex    → skill discovery for WorkflowAgent

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::registry::CapabilityKind;

// ---------------------------------------------------------------------------
// ProjectIndex
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProjectIndex {
    pub crates: Vec<CrateEntry>,
    pub assets: Vec<AssetEntry>,
    pub docs: Vec<DocEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrateEntry {
    pub name: String,
    pub path: String,
    pub source_files: Vec<String>,
    pub modules: Vec<String>,
    pub dependencies: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetEntry {
    pub path: String,
    pub asset_type: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocEntry {
    pub path: String,
    pub title: String,
    pub tags: Vec<String>,
}

impl ProjectIndex {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_crate(&mut self, entry: CrateEntry) {
        self.crates.push(entry);
    }

    pub fn add_asset(&mut self, entry: AssetEntry) {
        self.assets.push(entry);
    }

    pub fn add_doc(&mut self, entry: DocEntry) {
        self.docs.push(entry);
    }

    pub fn find_crate(&self, name: &str) -> Option<&CrateEntry> {
        self.crates.iter().find(|c| c.name == name)
    }

    pub fn find_file(&self, filename: &str) -> Vec<&CrateEntry> {
        self.crates
            .iter()
            .filter(|c| c.source_files.iter().any(|f| f.contains(filename)))
            .collect()
    }

    pub fn crate_names(&self) -> Vec<&str> {
        self.crates.iter().map(|c| c.name.as_str()).collect()
    }

    pub fn asset_count(&self) -> usize {
        self.assets.len()
    }

    pub fn doc_count(&self) -> usize {
        self.docs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.crates.is_empty() && self.assets.is_empty() && self.docs.is_empty()
    }
}

// ---------------------------------------------------------------------------
// SemanticIndex
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SemanticIndex {
    pub categories: Vec<SemanticCategory>,
    pub keyword_index: HashMap<String, Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticCategory {
    pub name: String,
    pub keywords: Vec<String>,
    pub related_files: Vec<String>,
    pub description: String,
}

impl SemanticIndex {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_category(&mut self, category: SemanticCategory) {
        for kw in &category.keywords {
            self.keyword_index
                .entry(kw.clone())
                .or_default()
                .push(category.name.clone());
        }
        self.categories.push(category);
    }

    pub fn search(&self, query: &str) -> Vec<&SemanticCategory> {
        let lower = query.to_lowercase();
        self.keyword_index
            .get(&lower)
            .map(|names| {
                names
                    .iter()
                    .filter_map(|name| self.categories.iter().find(|c| &c.name == name))
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn search_fuzzy(&self, query: &str) -> Vec<&SemanticCategory> {
        let lower = query.to_lowercase();
        self.categories
            .iter()
            .filter(|cat| {
                cat.name.to_lowercase().contains(&lower)
                    || cat.keywords.iter().any(|kw| kw.to_lowercase().contains(&lower))
                    || cat.description.to_lowercase().contains(&lower)
            })
            .collect()
    }

    pub fn list_categories(&self) -> Vec<&str> {
        self.categories.iter().map(|c| c.name.as_str()).collect()
    }

    pub fn len(&self) -> usize {
        self.categories.len()
    }

    pub fn is_empty(&self) -> bool {
        self.categories.is_empty()
    }
}

// ---------------------------------------------------------------------------
// SkillIndex
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SkillIndex {
    pub entries: Vec<SkillIndexEntry>,
    pub capability_index: HashMap<CapabilityKind, Vec<String>>,
    pub name_index: HashMap<String, usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillIndexEntry {
    pub name: String,
    pub description: String,
    pub required_capabilities: Vec<CapabilityKind>,
    pub node_count: usize,
}

impl SkillIndex {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_entry(&mut self, entry: SkillIndexEntry) {
        let idx = self.entries.len();
        self.name_index.insert(entry.name.clone(), idx);

        for cap in &entry.required_capabilities {
            self.capability_index
                .entry(cap.clone())
                .or_default()
                .push(entry.name.clone());
        }

        self.entries.push(entry);
    }

    pub fn find_by_name(&self, name: &str) -> Option<&SkillIndexEntry> {
        self.name_index
            .get(name)
            .and_then(|&idx| self.entries.get(idx))
    }

    pub fn find_by_capability(
        &self,
        capability: &CapabilityKind,
    ) -> Vec<&SkillIndexEntry> {
        self.capability_index
            .get(capability)
            .map(|names| {
                names
                    .iter()
                    .filter_map(|name| self.find_by_name(name))
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn search(&self, query: &str) -> Vec<&SkillIndexEntry> {
        let lower = query.to_lowercase();
        self.entries
            .iter()
            .filter(|entry| {
                entry.name.to_lowercase().contains(&lower)
                    || entry.description.to_lowercase().contains(&lower)
            })
            .collect()
    }

    pub fn list_names(&self) -> Vec<&str> {
        self.entries.iter().map(|e| e.name.as_str()).collect()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_project_index_add_and_find() {
        let mut index = ProjectIndex::new();
        index.add_crate(CrateEntry {
            name: "agent-core".to_string(),
            path: "crates/agent-core".to_string(),
            source_files: vec!["src/lib.rs".to_string(), "src/agent.rs".to_string()],
            modules: vec!["agent".to_string()],
            dependencies: vec!["serde".to_string()],
        });

        assert!(!index.is_empty());
        let found = index.find_crate("agent-core");
        assert!(found.is_some());
        assert_eq!(found.unwrap().source_files.len(), 2);
    }

    #[test]
    fn test_project_index_find_file() {
        let mut index = ProjectIndex::new();
        index.add_crate(CrateEntry {
            name: "agent-core".to_string(),
            path: "crates/agent-core".to_string(),
            source_files: vec!["src/agent.rs".to_string()],
            modules: vec![],
            dependencies: vec![],
        });

        let found = index.find_file("agent.rs");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].name, "agent-core");
    }

    #[test]
    fn test_semantic_index_search() {
        let mut index = SemanticIndex::new();
        index.add_category(SemanticCategory {
            name: "gameplay".to_string(),
            keywords: vec!["movement".to_string(), "combat".to_string()],
            related_files: vec!["src/player.rs".to_string()],
            description: "Gameplay systems".to_string(),
        });

        let results = index.search("movement");
        assert!(!results.is_empty());
        assert_eq!(results[0].name, "gameplay");
    }

    #[test]
    fn test_semantic_index_fuzzy_search() {
        let mut index = SemanticIndex::new();
        index.add_category(SemanticCategory {
            name: "visual".to_string(),
            keywords: vec!["camera".to_string(), "lighting".to_string()],
            related_files: vec![],
            description: "Visual and rendering".to_string(),
        });

        let results = index.search_fuzzy("render");
        assert!(!results.is_empty());
        assert_eq!(results[0].name, "visual");
    }

    #[test]
    fn test_skill_index_add_and_find() {
        let mut index = SkillIndex::new();
        index.add_entry(SkillIndexEntry {
            name: "create_enemy_ai".to_string(),
            description: "Create an enemy with AI".to_string(),
            required_capabilities: vec![CapabilityKind::SceneWrite],
            node_count: 3,
        });

        let found = index.find_by_capability(&CapabilityKind::SceneWrite);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].name, "create_enemy_ai");
    }

    #[test]
    fn test_skill_index_search() {
        let mut index = SkillIndex::new();
        index.add_entry(SkillIndexEntry {
            name: "import_asset".to_string(),
            description: "Import a texture or model".to_string(),
            required_capabilities: vec![CapabilityKind::AssetManage],
            node_count: 2,
        });

        let results = index.search("import");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "import_asset");
    }

    #[test]
    fn test_skill_index_empty() {
        let index = SkillIndex::new();
        assert!(index.is_empty());
        assert_eq!(index.len(), 0);
    }
}
