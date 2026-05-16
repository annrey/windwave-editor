//! Semantic Memory (L1) - Knowledge graph with vector similarity
//!
//! Stores conceptual knowledge as a graph of nodes and relations.
//! Uses a lightweight vector similarity approach (no external embeddings API).
//! Instead, uses TF-IDF weighted term vectors for semantic matching.

use crate::memory::{MemoryEntryId, MemoryMetadata, MemoryTier};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Types of semantic relations
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RelationType {
    IsA,           // Player IsA Entity
    HasA,          // Player HasA Transform
    PartOf,        // Wheel PartOf Car
    RelatedTo,     // Player RelatedTo Enemy
    UsedBy,        // MovementSystem UsedBy Player
    DependsOn,     // Physics DependsOn Transform
    CreatedBy,     // Enemy CreatedBy SpawnSystem
    SimilarTo,     // Goblin SimilarTo Orc
}

/// A node in the semantic knowledge graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticNode {
    pub metadata: MemoryMetadata,
    pub name: String,
    pub node_type: String, // "entity", "component", "system", "concept"
    pub description: String,
    pub properties: HashMap<String, serde_json::Value>,
    /// TF-IDF vector (term -> weight)
    #[serde(skip)]
    pub vector: HashMap<String, f32>,
}

impl SemanticNode {
    pub fn new(id: u64, name: impl Into<String>, node_type: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            metadata: MemoryMetadata::new(id, MemoryTier::Semantic),
            name: name.into(),
            node_type: node_type.into(),
            description: description.into(),
            properties: HashMap::new(),
            vector: HashMap::new(),
        }
    }

    pub fn with_property(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.properties.insert(key.into(), value);
        self
    }

    pub fn full_text(&self) -> String {
        let props_text = self.properties.iter()
            .map(|(k, v)| format!("{}: {}", k, v))
            .collect::<Vec<_>>()
            .join(" ");
        format!("{} {} {} {}", self.name, self.node_type, self.description, props_text)
    }
}

/// A relation between two semantic nodes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticRelation {
    pub from_id: MemoryEntryId,
    pub to_id: MemoryEntryId,
    pub relation_type: RelationType,
    pub strength: f32, // 0.0 - 1.0
    pub description: String,
}

/// Semantic Memory - L1 knowledge graph
///
/// Design principles:
/// - Nodes represent concepts (entities, components, systems, patterns)
/// - Relations form a navigable knowledge graph
/// - TF-IDF vectors enable lightweight semantic similarity
/// - Graph traversal for related concept discovery
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticMemory {
    nodes: Vec<SemanticNode>,
    relations: Vec<SemanticRelation>,
    next_id: u64,
    /// Node name -> index for quick lookup
    name_index: HashMap<String, usize>,
    /// Node ID -> index
    id_index: HashMap<u64, usize>,
    /// Document frequency for TF-IDF
    #[serde(skip)]
    df_cache: HashMap<String, usize>,
    #[serde(skip)]
    cache_valid: bool,
}

impl SemanticMemory {
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            relations: Vec::new(),
            next_id: 1,
            name_index: HashMap::new(),
            id_index: HashMap::new(),
            df_cache: HashMap::new(),
            cache_valid: false,
        }
    }

    /// Add a node to the knowledge graph
    pub fn add_node(&mut self, node: SemanticNode) -> MemoryEntryId {
        let id = node.metadata.id;
        let idx = self.nodes.len();
        self.name_index.insert(node.name.clone(), idx);
        self.id_index.insert(id.0, idx);
        self.nodes.push(node);
        self.cache_valid = false;
        id
    }

    /// Create and add a new node
    pub fn create_node(
        &mut self,
        name: impl Into<String>,
        node_type: impl Into<String>,
        description: impl Into<String>,
    ) -> MemoryEntryId {
        let id = self.next_id();
        let node = SemanticNode::new(id.0, name, node_type, description);
        self.add_node(node);
        id
    }

    /// Add a relation between nodes
    pub fn add_relation(
        &mut self,
        from_id: MemoryEntryId,
        to_id: MemoryEntryId,
        relation_type: RelationType,
        strength: f32,
        description: impl Into<String>,
    ) {
        self.relations.push(SemanticRelation {
            from_id,
            to_id,
            relation_type,
            strength,
            description: description.into(),
        });
    }

    /// Find node by name
    pub fn find_by_name(&self, name: &str) -> Option<&SemanticNode> {
        self.name_index.get(name)
            .and_then(|&idx| self.nodes.get(idx))
    }

    /// Find node by ID
    pub fn find_by_id(&self, id: MemoryEntryId) -> Option<&SemanticNode> {
        self.id_index.get(&id.0)
            .and_then(|&idx| self.nodes.get(idx))
    }

    /// Get mutable node by ID
    pub fn get_node_mut(&mut self, id: MemoryEntryId) -> Option<&mut SemanticNode> {
        let idx = *self.id_index.get(&id.0)?;
        self.nodes.get_mut(idx)
    }

    /// Rebuild internal indices after bulk modifications
    fn rebuild_indices(&mut self) {
        self.name_index.clear();
        self.id_index.clear();
        for (idx, node) in self.nodes.iter().enumerate() {
            self.name_index.insert(node.name.clone(), idx);
            self.id_index.insert(node.metadata.id.0, idx);
        }
    }

    /// Search nodes by semantic similarity (TF-IDF cosine similarity)
    pub fn search(&mut self, query: &str, top_k: usize) -> Vec<(MemoryEntryId, f32)> {
        if self.nodes.is_empty() || query.trim().is_empty() {
            return Vec::new();
        }

        self.ensure_vectors();

        let query_vector = self.compute_query_vector(query);
        if query_vector.is_empty() {
            return Vec::new();
        }

        let mut scored: Vec<(usize, f32)> = self.nodes.iter()
            .enumerate()
            .map(|(idx, node)| {
                let similarity = cosine_similarity(&query_vector, &node.vector);
                (idx, similarity)
            })
            .filter(|(_, sim)| *sim > 0.0)
            .collect();

        scored.sort_by(|a, b| {
            b.1.partial_cmp(&a.1)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Mark accessed
        for (idx, _) in scored.iter().take(top_k) {
            if let Some(node) = self.nodes.get_mut(*idx) {
                node.metadata.touch();
            }
        }

        scored.into_iter()
            .take(top_k)
            .map(|(idx, score)| (self.nodes[idx].metadata.id, score))
            .collect()
    }

    /// Get related nodes via graph traversal
    pub fn get_related(&self, node_id: MemoryEntryId, relation_type: Option<&RelationType>, max_depth: usize) -> Vec<&SemanticNode> {
        if max_depth == 0 {
            return Vec::new();
        }

        let mut visited = std::collections::HashSet::new();
        let mut result = Vec::new();
        let mut queue = vec![(node_id, 0usize)];

        while let Some((current_id, depth)) = queue.pop() {
            if depth >= max_depth || visited.contains(&current_id.0) {
                continue;
            }
            visited.insert(current_id.0);

            for relation in &self.relations {
                if relation.from_id == current_id {
                    if let Some(rt) = relation_type {
                        if &relation.relation_type != rt {
                            continue;
                        }
                    }
                    if let Some(node) = self.find_by_id(relation.to_id) {
                        if !visited.contains(&node.metadata.id.0) {
                            result.push(node);
                            queue.push((relation.to_id, depth + 1));
                        }
                    }
                }
            }
        }

        result
    }

    /// Get nodes by type
    pub fn nodes_by_type(&self, node_type: &str) -> Vec<&SemanticNode> {
        self.nodes.iter()
            .filter(|n| n.node_type == node_type)
            .collect()
    }

    /// Build a summary for LLM context
    pub fn build_summary(&mut self, query: &str, max_entries: usize) -> String {
        let results = self.search(query, max_entries);
        if results.is_empty() {
            return String::new();
        }

        let mut parts = vec!["## Related Knowledge".to_string()];
        for (node_id, score) in results {
            if let Some(node) = self.find_by_id(node_id) {
                parts.push(format!(
                    "- {} ({}): {} [relevance: {:.2}]",
                    node.name,
                    node.node_type,
                    node.description,
                    score
                ));
            }
        }

        parts.join("\n")
    }

    /// Get all nodes
    pub fn all_nodes(&self) -> &[SemanticNode] {
        &self.nodes
    }

    /// Get all relations
    pub fn all_relations(&self) -> &[SemanticRelation] {
        &self.relations
    }

    /// Node count
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Relation count
    pub fn relation_count(&self) -> usize {
        self.relations.len()
    }

    /// Prune least important nodes (for cleanup)
    pub fn prune_least_important(&mut self, count: usize) {
        if count == 0 || self.nodes.is_empty() {
            return;
        }

        // Score nodes by importance and recency
        let mut scored: Vec<(usize, f32)> = self.nodes.iter()
            .enumerate()
            .map(|(i, node)| {
                let age_factor = 1.0 / (1.0 + (crate::types::current_timestamp() - node.metadata.created_at) as f32 / 86400.0); // Decay over days
                let importance = node.metadata.importance;
                let access_factor = 1.0 + (node.metadata.access_count as f32 * 0.05);
                (i, age_factor * importance * access_factor)
            })
            .collect();

        // Sort by score ascending (lowest first)
        scored.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

        // Collect IDs of nodes to remove
        let to_remove: std::collections::HashSet<u64> = scored.into_iter()
            .take(count)
            .map(|(i, _)| self.nodes[i].metadata.id.0)
            .collect();

        // Remove nodes and their relations
        self.nodes.retain(|n| !to_remove.contains(&n.metadata.id.0));
        self.relations.retain(|r| !to_remove.contains(&r.from_id.0) && !to_remove.contains(&r.to_id.0));

        // Rebuild indices
        self.rebuild_indices();
    }

    /// Seed with common Bevy/game dev knowledge
    pub fn seed_with_defaults(&mut self) {
        let entity = self.create_node("Entity", "concept", "A Bevy ECS entity - a unique ID that can have components");
        let component = self.create_node("Component", "concept", "Data attached to entities in Bevy ECS");
        let system = self.create_node("System", "concept", "A function that queries and updates entities each frame");
        let transform = self.create_node("Transform", "component", "Position, rotation, and scale of an entity");
        let sprite = self.create_node("Sprite", "component", "2D visual representation of an entity");
        let player = self.create_node("Player", "entity_type", "The player-controlled character");
        let enemy = self.create_node("Enemy", "entity_type", "An antagonist that opposes the player");

        self.add_relation(entity, component, RelationType::HasA, 1.0, "Entities have components");
        self.add_relation(component, entity, RelationType::PartOf, 1.0, "Components are part of entities");
        self.add_relation(system, entity, RelationType::UsedBy, 0.8, "Systems operate on entities");
        self.add_relation(transform, entity, RelationType::PartOf, 1.0, "Transform is a component");
        self.add_relation(sprite, entity, RelationType::PartOf, 1.0, "Sprite is a component");
        self.add_relation(player, entity, RelationType::IsA, 1.0, "Player is an entity");
        self.add_relation(enemy, entity, RelationType::IsA, 1.0, "Enemy is an entity");
        self.add_relation(player, enemy, RelationType::RelatedTo, 0.6, "Player and Enemy are related");
        self.add_relation(enemy, player, RelationType::RelatedTo, 0.6, "Enemy opposes Player");
    }

    // ------------------------------------------------------------------
    // Vector computation
    // ------------------------------------------------------------------

    fn ensure_vectors(&mut self) {
        if self.cache_valid {
            return;
        }

        // Compute document frequencies
        self.df_cache.clear();
        let mut total_dl = 0usize;

        for node in &self.nodes {
            let text = node.full_text();
            let tokens = tokenize(&text);
            total_dl += tokens.len();

            let mut seen = std::collections::HashSet::new();
            for token in tokens {
                if seen.insert(token.clone()) {
                    *self.df_cache.entry(token).or_insert(0) += 1;
                }
            }
        }

        let n = self.nodes.len() as f32;
        let avg_dl = if self.nodes.is_empty() { 1.0 } else { total_dl as f32 / n };

        // Compute TF-IDF vectors for each node
        for node in &mut self.nodes {
            let text = node.full_text();
            let tokens = tokenize(&text);
            let doc_len = tokens.len() as f32;

            let mut tf: HashMap<String, f32> = HashMap::new();
            for token in tokens {
                *tf.entry(token).or_insert(0.0) += 1.0;
            }

            let mut vector = HashMap::new();
            for (token, count) in tf {
                let df = *self.df_cache.get(&token).unwrap_or(&1) as f32;
                let idf = (n / df).ln();
                let tf_norm = count / (count + 1.0) * (1.0 - 0.5 + 0.5 * doc_len / avg_dl.max(1.0));
                vector.insert(token, tf_norm * idf.max(0.0));
            }

            node.vector = vector;
        }

        self.cache_valid = true;
    }

    fn compute_query_vector(&self, query: &str) -> HashMap<String, f32> {
        let tokens = tokenize(query);
        let n = self.nodes.len() as f32;

        let mut tf: HashMap<String, f32> = HashMap::new();
        for token in tokens {
            *tf.entry(token).or_insert(0.0) += 1.0;
        }

        let mut vector = HashMap::new();
        for (token, count) in tf {
            let df = *self.df_cache.get(&token).unwrap_or(&1) as f32;
            let idf = (n / df).ln();
            vector.insert(token, count * idf.max(0.0));
        }

        vector
    }

    fn next_id(&mut self) -> MemoryEntryId {
        let id = MemoryEntryId(self.next_id);
        self.next_id += 1;
        id
    }

    // =================================================================
    // Persistence Operations
    // =================================================================

    /// Export all nodes for serialization (vectors are excluded by serde skip)
    pub fn export_nodes(&self) -> Vec<SemanticNode> {
        self.nodes.clone()
    }

    /// Import a node from serialized data
    pub fn import_node(&mut self, node: SemanticNode) {
        if self.next_id <= node.metadata.id.0 {
            self.next_id = node.metadata.id.0 + 1;
        }
        let idx = self.nodes.len();
        self.name_index.insert(node.name.clone(), idx);
        self.id_index.insert(node.metadata.id.0, idx);
        self.nodes.push(node);
        self.cache_valid = false;
    }
}

impl Default for SemanticMemory {
    fn default() -> Self {
        let mut mem = Self::new();
        mem.seed_with_defaults();
        mem
    }
}

// ------------------------------------------------------------------
// Helpers
// ------------------------------------------------------------------

fn tokenize(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|s| s.len() > 1)
        .map(|s| s.to_string())
        .collect()
}

fn cosine_similarity(a: &HashMap<String, f32>, b: &HashMap<String, f32>) -> f32 {
    let mut dot = 0.0;
    let mut norm_a = 0.0;
    let mut norm_b = 0.0;

    for (term, weight_a) in a {
        norm_a += weight_a * weight_a;
        if let Some(weight_b) = b.get(term) {
            dot += weight_a * weight_b;
        }
    }

    for (_, weight_b) in b {
        norm_b += weight_b * weight_b;
    }

    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }

    dot / (norm_a.sqrt() * norm_b.sqrt())
}
