//! CodeGraph - Sprint 5: 代码影响半径分析
//!
//! 分析代码变更的影响范围，帮助 Agent 理解修改的连锁反应。
//! 基于符号依赖图计算影响半径。

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// 代码符号节点
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeNode {
    pub id: String,
    pub name: String,
    pub kind: NodeKind,
    pub file_path: String,
    pub line: usize,
}

/// 节点类型
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NodeKind {
    Struct,
    Fn,
    Trait,
    Enum,
    Impl,
    Mod,
    System,
    Component,
    Resource,
    Event,
    Plugin,
}

/// 依赖边
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyEdge {
    pub source_id: String,
    pub target_id: String,
    pub dep_kind: DepKind,
}

/// 依赖类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DepKind {
    /// 直接使用 (use/import)
    DirectUse,
    /// 继承/实现 (impl Trait for Struct)
    Inheritance,
    /// 函数调用
    FunctionCall,
    /// 类型引用
    TypeReference,
    /// 宏调用
    MacroUse,
}

/// 代码图谱 - 符号依赖图
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeGraph {
    nodes: HashMap<String, CodeNode>,
    edges: Vec<DependencyEdge>,
    /// 文件名 → 节点ID
    file_index: HashMap<String, Vec<String>>,
}

impl CodeGraph {
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            edges: Vec::new(),
            file_index: HashMap::new(),
        }
    }

    /// 添加节点
    pub fn add_node(&mut self, node: CodeNode) {
        let id = node.id.clone();
        let file = node.file_path.clone();
        self.nodes.insert(id.clone(), node);
        self.file_index.entry(file).or_default().push(id);
    }

    /// 添加依赖边
    pub fn add_edge(&mut self, edge: DependencyEdge) {
        self.edges.push(edge);
    }

    /// 获取节点的直接依赖（出边）
    pub fn direct_deps(&self, node_id: &str) -> Vec<&CodeNode> {
        let targets: HashSet<&str> = self.edges
            .iter()
            .filter(|e| e.source_id == node_id)
            .map(|e| e.target_id.as_str())
            .collect();
        self.nodes.values().filter(|n| targets.contains(n.id.as_str())).collect()
    }

    /// 获取节点的直接依赖者（入边）
    pub fn direct_dependents(&self, node_id: &str) -> Vec<&CodeNode> {
        let sources: HashSet<&str> = self.edges
            .iter()
            .filter(|e| e.target_id == node_id)
            .map(|e| e.source_id.as_str())
            .collect();
        self.nodes.values().filter(|n| sources.contains(n.id.as_str())).collect()
    }

    /// 计算影响半径 - 从指定节点出发，所有被影响的节点（反向遍历依赖）
    pub fn impact_radius(&self, node_ids: &[String], max_depth: usize) -> Vec<&CodeNode> {
        let mut visited = HashSet::new();
        let mut result = Vec::new();
        
        for node_id in node_ids {
            self.traverse_backward(node_id, 0, max_depth, &mut visited, &mut result);
        }
        
        result
    }

    /// 反向遍历（寻找依赖者）
    fn traverse_backward<'a>(
        &'a self,
        node_id: &str,
        depth: usize,
        max_depth: usize,
        visited: &mut HashSet<String>,
        result: &mut Vec<&'a CodeNode>,
    ) {
        if depth > max_depth || visited.contains(node_id) {
            return;
        }
        visited.insert(node_id.to_string());

        if let Some(node) = self.nodes.get(node_id) {
            result.push(node);
        }

        let sources: Vec<String> = self.edges
            .iter()
            .filter(|e| e.target_id == node_id)
            .map(|e| e.source_id.clone())
            .collect();

        for source in sources {
            self.traverse_backward(&source, depth + 1, max_depth, visited, result);
        }
    }

    /// 获取文件中的所有节点
    pub fn nodes_in_file(&self, file_path: &str) -> Vec<&CodeNode> {
        self.file_index
            .get(file_path)
            .map(|ids| ids.iter().filter_map(|id| self.nodes.get(id)).collect())
            .unwrap_or_default()
    }

    /// 生成影响分析报告
    pub fn impact_report(&self, changed_files: &[String]) -> String {
        let mut report = Vec::new();
        report.push("## 代码影响分析\n".into());

        for file in changed_files {
            let nodes = self.nodes_in_file(file);
            if nodes.is_empty() {
                report.push(format!("- {}: 无已知符号", file));
                continue;
            }

            let node_ids: Vec<String> = nodes.iter().map(|n| n.id.clone()).collect();
            let impacted = self.impact_radius(&node_ids, 3);

            report.push(format!(
                "- {}: {} 个符号变更，影响 {} 个其他符号",
                file,
                nodes.len(),
                impacted.len() - nodes.len()
            ));

            // 列出受影响的外部文件
            let impacted_files: HashSet<&str> = impacted
                .iter()
                .filter(|n| !changed_files.contains(&n.file_path))
                .map(|n| n.file_path.as_str())
                .collect();

            for f in impacted_files {
                report.push(format!("  → 影响: {}", f));
            }
        }

        report.join("\n")
    }

    /// 节点数量
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// 边数量
    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }
}

impl Default for CodeGraph {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// 测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_graph() -> CodeGraph {
        let mut graph = CodeGraph::new();

        graph.add_node(CodeNode {
            id: "Player".into(),
            name: "Player".into(),
            kind: NodeKind::Component,
            file_path: "src/player.rs".into(),
            line: 10,
        });
        graph.add_node(CodeNode {
            id: "player_movement".into(),
            name: "player_movement".into(),
            kind: NodeKind::System,
            file_path: "src/player.rs".into(),
            line: 42,
        });
        graph.add_node(CodeNode {
            id: "GameState".into(),
            name: "GameState".into(),
            kind: NodeKind::Resource,
            file_path: "src/game.rs".into(),
            line: 5,
        });
        graph.add_node(CodeNode {
            id: "enemy_ai".into(),
            name: "enemy_ai".into(),
            kind: NodeKind::System,
            file_path: "src/enemy.rs".into(),
            line: 30,
        });

        graph.add_edge(DependencyEdge {
            source_id: "player_movement".into(),
            target_id: "Player".into(),
            dep_kind: DepKind::TypeReference,
        });
        graph.add_edge(DependencyEdge {
            source_id: "player_movement".into(),
            target_id: "GameState".into(),
            dep_kind: DepKind::TypeReference,
        });
        graph.add_edge(DependencyEdge {
            source_id: "enemy_ai".into(),
            target_id: "GameState".into(),
            dep_kind: DepKind::TypeReference,
        });

        graph
    }

    #[test]
    fn test_graph_creation() {
        let graph = make_test_graph();
        assert_eq!(graph.node_count(), 4);
        assert_eq!(graph.edge_count(), 3);
    }

    #[test]
    fn test_direct_deps() {
        let graph = make_test_graph();
        let deps = graph.direct_deps("player_movement");
        assert_eq!(deps.len(), 2);
    }

    #[test]
    fn test_direct_dependents() {
        let graph = make_test_graph();
        let deps = graph.direct_dependents("GameState");
        assert_eq!(deps.len(), 2);
    }

    #[test]
    fn test_impact_radius() {
        let graph = make_test_graph();
        let impacted = graph.impact_radius(&["Player".into()], 2);
        // Player → player_movement → GameState
        assert!(impacted.iter().any(|n| n.name == "Player"));
        assert!(impacted.iter().any(|n| n.name == "player_movement"));
    }

    #[test]
    fn test_nodes_in_file() {
        let graph = make_test_graph();
        let nodes = graph.nodes_in_file("src/player.rs");
        assert_eq!(nodes.len(), 2);
    }

    #[test]
    fn test_impact_report() {
        let graph = make_test_graph();
        let report = graph.impact_report(&["src/player.rs".into()]);
        assert!(report.contains("src/player.rs"));
        assert!(report.contains("影响"));
    }
}