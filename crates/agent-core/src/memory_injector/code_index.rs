use std::collections::HashMap;
use serde::{Deserialize, Serialize};

/// 符号类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum SymbolKind {
    Struct,
    Fn,
    Trait,
    Enum,
    System,
    Component,
    Resource,
    Event,
    Plugin,
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
    pub symbols: Vec<Symbol>,
    pub file_index: HashMap<String, Vec<usize>>,
    pub name_index: HashMap<String, usize>,
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

    pub fn add(&mut self, symbol: Symbol) {
        let idx = self.symbols.len();
        let name = symbol.name.clone();
        let file = symbol.file_path.clone();
        let kind = symbol.kind.clone();
        self.symbols.push(symbol);
        self.file_index.entry(file).or_default().push(idx);
        self.name_index.insert(name, idx);
        self.kind_index.entry(kind).or_default().push(idx);
    }

    pub fn find_by_name(&self, name: &str) -> Option<&Symbol> {
        self.name_index.get(name).and_then(|&idx| self.symbols.get(idx))
    }

    pub fn find_by_file(&self, file_path: &str) -> Vec<&Symbol> {
        self.file_index
            .get(file_path)
            .map(|indices| indices.iter().filter_map(|&i| self.symbols.get(i)).collect())
            .unwrap_or_default()
    }

    pub fn find_by_kind(&self, kind: SymbolKind) -> Vec<&Symbol> {
        self.kind_index
            .get(&kind)
            .map(|indices| indices.iter().filter_map(|&i| self.symbols.get(i)).collect())
            .unwrap_or_default()
    }

    pub fn find_relevant(&self, query: &str) -> Vec<&Symbol> {
        let query_lower = query.to_lowercase();
        self.symbols
            .iter()
            .filter(|s| s.name.to_lowercase().contains(&query_lower))
            .collect()
    }

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

#[cfg(test)]
mod tests {
    use super::*;

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
}
