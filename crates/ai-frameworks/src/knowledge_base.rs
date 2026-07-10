//! Knowledge Base Module
//!
//! Provides document indexing, storage, and retrieval capabilities
//! using LlamaIndex and LangChain integrations.

use crate::error::{Error, Result};
use crate::types::{Document, QueryResult};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// Knowledge Base Manager for handling document indexing and retrieval
pub struct KnowledgeBaseManager {
    /// Stored documents by ID
    documents: HashMap<String, Document>,
    /// Index configurations
    indexes: HashMap<String, IndexConfig>,
    /// Root directory for document storage
    root_dir: Option<PathBuf>,
}

/// Configuration for a knowledge base index
#[derive(Debug, Clone)]
pub struct IndexConfig {
    pub name: String,
    pub description: String,
    pub created_at: String,
    pub document_count: usize,
}

impl KnowledgeBaseManager {
    /// Create a new knowledge base manager
    pub fn new() -> Self {
        Self {
            documents: HashMap::new(),
            indexes: HashMap::new(),
            root_dir: None,
        }
    }

    /// Set the root directory for document scanning
    pub fn with_root_dir<P: AsRef<Path>>(mut self, dir: P) -> Self {
        self.root_dir = Some(dir.as_ref().to_path_buf());
        self
    }

    /// Add a single document to the knowledge base
    pub fn add_document(&mut self, document: Document) {
        self.documents.insert(document.id.clone(), document);
    }

    /// Add multiple documents to the knowledge base
    pub fn add_documents(&mut self, documents: Vec<Document>) {
        for doc in documents {
            self.add_document(doc);
        }
    }

    /// Scan a directory and add supported files as documents
    pub async fn scan_directory<P: AsRef<Path>>(&mut self, path: P) -> Result<Vec<String>> {
        let mut added_docs = Vec::new();
        let path = path.as_ref();

        if !path.exists() {
            return Err(Error::ConfigError(format!(
                "Directory does not exist: {:?}",
                path
            )));
        }

        for entry in WalkDir::new(path)
            .follow_links(true)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let file_path = entry.path();

            if file_path.is_file() {
                if let Some(doc) = Self::file_to_document(file_path) {
                    let doc_id = doc.id.clone();
                    self.add_document(doc);
                    added_docs.push(doc_id);
                }
            }
        }

        Ok(added_docs)
    }

    /// Convert a file to a Document
    fn file_to_document(path: &Path) -> Option<Document> {
        let extension = path.extension()?.to_str()?.to_lowercase();

        // Skip binary files
        let binary_extensions = vec!["exe", "dll", "so", "dylib", "bin", "obj", "o", "a", "lib"];
        if binary_extensions.contains(&extension.as_str()) {
            return None;
        }

        // Try to read the file content
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => return None, // Skip files that can't be read as text
        };

        // Limit content size
        let content = if content.len() > 100_000 {
            content.chars().take(100_000).collect()
        } else {
            content
        };

        let file_name = path.file_name()?.to_str()?;
        let file_path = path.to_str()?;

        Some(Document {
            id: format!("file:{}", file_path),
            content,
            metadata: serde_json::json!({
                "file_name": file_name,
                "file_path": file_path,
                "extension": extension,
            }),
        })
    }

    /// Retrieve a document by ID
    pub fn get_document(&self, doc_id: &str) -> Option<&Document> {
        self.documents.get(doc_id)
    }

    /// Simple keyword-based search (for demonstration)
    pub fn search(&self, query: &str) -> QueryResult {
        let query_lower = query.to_lowercase();
        let mut results = Vec::new();
        let mut scores = Vec::new();

        for doc in self.documents.values() {
            let score = self.calculate_relevance(&doc.content, &query_lower);
            if score > 0.0 {
                results.push(doc.clone());
                scores.push(score);
            }
        }

        // Sort by relevance score
        let mut combined: Vec<_> = results.into_iter().zip(scores).collect();
        combined.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        let (sorted_docs, sorted_scores): (Vec<_>, Vec<_>) = combined.into_iter().unzip();

        QueryResult {
            documents: sorted_docs,
            scores: sorted_scores,
        }
    }

    /// Calculate simple relevance score based on keyword matching
    fn calculate_relevance(&self, content: &str, query: &str) -> f32 {
        let content_lower = content.to_lowercase();
        let query_words: Vec<_> = query.split_whitespace().collect();

        if query_words.is_empty() {
            return 0.0;
        }

        let mut matches = 0;
        for word in &query_words {
            if content_lower.contains(word) {
                matches += 1;
            }
        }

        matches as f32 / query_words.len() as f32
    }

    /// Get all document IDs
    pub fn get_all_document_ids(&self) -> Vec<String> {
        self.documents.keys().cloned().collect()
    }

    /// Get document count
    pub fn document_count(&self) -> usize {
        self.documents.len()
    }

    /// Create a named index
    pub fn create_index(&mut self, name: String, description: String) -> String {
        let index_id = format!("index:{}", uuid::Uuid::new_v4());
        let config = IndexConfig {
            name,
            description,
            created_at: chrono::Utc::now().to_rfc3339(),
            document_count: self.documents.len(),
        };
        self.indexes.insert(index_id.clone(), config);
        index_id
    }

    /// Get list of available indexes
    pub fn list_indexes(&self) -> Vec<&IndexConfig> {
        self.indexes.values().collect()
    }

    /// Remove a document
    pub fn remove_document(&mut self, doc_id: &str) -> bool {
        self.documents.remove(doc_id).is_some()
    }

    /// Clear all documents
    pub fn clear_documents(&mut self) {
        self.documents.clear();
    }
}

impl Default for KnowledgeBaseManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_knowledge_base_creation() {
        let kb = KnowledgeBaseManager::new();
        assert_eq!(kb.document_count(), 0);
    }

    #[test]
    fn test_add_document() {
        let mut kb = KnowledgeBaseManager::new();

        let doc = Document {
            id: "test1".to_string(),
            content: "This is a test document about Bevy game engine.".to_string(),
            metadata: serde_json::json!({}),
        };

        kb.add_document(doc);
        assert_eq!(kb.document_count(), 1);
    }

    #[test]
    fn test_search() {
        let mut kb = KnowledgeBaseManager::new();

        let doc1 = Document {
            id: "test1".to_string(),
            content: "Bevy is a great game engine.".to_string(),
            metadata: serde_json::json!({}),
        };

        let doc2 = Document {
            id: "test2".to_string(),
            content: "Python is a programming language.".to_string(),
            metadata: serde_json::json!({}),
        };

        kb.add_document(doc1);
        kb.add_document(doc2);

        let result = kb.search("Bevy game");
        assert!(!result.documents.is_empty());
        assert!(result.scores[0] > 0.0);
    }

    #[test]
    fn test_add_multiple_documents() {
        let mut kb = KnowledgeBaseManager::new();

        let docs = vec![
            Document {
                id: "doc1".to_string(),
                content: "Document 1 content".to_string(),
                metadata: serde_json::json!({"type": "text"}),
            },
            Document {
                id: "doc2".to_string(),
                content: "Document 2 content".to_string(),
                metadata: serde_json::json!({"type": "text"}),
            },
        ];

        kb.add_documents(docs);
        assert_eq!(kb.document_count(), 2);
    }

    #[test]
    fn test_get_document() {
        let mut kb = KnowledgeBaseManager::new();

        let doc = Document {
            id: "test-doc".to_string(),
            content: "This is a document with id test".to_string(),
            metadata: serde_json::json!({}),
        };

        kb.add_document(doc);

        let retrieved = kb.get_document("test-doc");
        assert!(retrieved.is_some());
        assert_eq!(
            retrieved.unwrap().content,
            "This is a document with id test"
        );
    }

    #[test]
    fn test_remove_document() {
        let mut kb = KnowledgeBaseManager::new();

        let doc = Document {
            id: "to-remove".to_string(),
            content: "Will be removed".to_string(),
            metadata: serde_json::json!({}),
        };

        kb.add_document(doc);
        assert_eq!(kb.document_count(), 1);

        let result = kb.remove_document("to-remove");
        assert!(result);
        assert_eq!(kb.document_count(), 0);
    }

    #[test]
    fn test_clear_documents() {
        let mut kb = KnowledgeBaseManager::new();

        for i in 1..=5 {
            let doc = Document {
                id: format!("doc{}", i),
                content: format!("Content {}", i),
                metadata: serde_json::json!({}),
            };
            kb.add_document(doc);
        }

        assert_eq!(kb.document_count(), 5);

        kb.clear_documents();
        assert_eq!(kb.document_count(), 0);
    }

    #[test]
    fn test_create_index() {
        let mut kb = KnowledgeBaseManager::new();

        let index_id = kb.create_index("test-index".to_string(), "A test index".to_string());

        assert!(!index_id.is_empty());
        assert!(index_id.starts_with("index:"));
    }

    #[test]
    fn test_get_all_document_ids() {
        let mut kb = KnowledgeBaseManager::new();

        let doc1 = Document {
            id: "doc1".to_string(),
            content: "Content 1".to_string(),
            metadata: serde_json::json!({}),
        };

        let doc2 = Document {
            id: "doc2".to_string(),
            content: "Content 2".to_string(),
            metadata: serde_json::json!({}),
        };

        kb.add_document(doc1);
        kb.add_document(doc2);

        let ids = kb.get_all_document_ids();
        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&"doc1".to_string()));
        assert!(ids.contains(&"doc2".to_string()));
    }
}
