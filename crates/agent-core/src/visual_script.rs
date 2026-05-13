//! Visual Script/Blueprint - Sprint 8: 可视化脚本与行为树
//!
//! 节点编辑器、执行流、行为树编辑器

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 节点类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VisualNodeType {
    /// 事件开始
    EventStart,
    /// 条件判断
    Condition,
    /// 动作执行
    Action,
    /// 变量赋值
    VariableSet,
    /// 循环
    Loop,
    /// 函数调用
    FunctionCall,
    /// 等待
    Wait,
    /// 分支
    Branch,
    /// 合并
    Merge,
}

/// 节点端口
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodePort {
    pub id: String,
    pub name: String,
    pub port_type: PortType,
    pub data_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PortType {
    FlowInput, FlowOutput,
    DataInput, DataOutput,
}

/// 可视化节点
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisualNode {
    pub id: String,
    pub node_type: VisualNodeType,
    pub title: String,
    pub inputs: Vec<NodePort>,
    pub outputs: Vec<NodePort>,
    pub properties: HashMap<String, serde_json::Value>,
    pub position: (f32, f32),
}

/// 节点连接
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeConnection {
    pub id: String,
    pub source_node: String,
    pub source_port: String,
    pub target_node: String,
    pub target_port: String,
}

/// 可视化脚本图
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisualScript {
    pub id: String,
    pub name: String,
    pub nodes: Vec<VisualNode>,
    pub connections: Vec<NodeConnection>,
    pub variables: HashMap<String, String>,
}

impl VisualScript {
    pub fn new(id: String, name: String) -> Self {
        Self { id, name, nodes: Vec::new(), connections: Vec::new(), variables: HashMap::new() }
    }

    pub fn add_node(&mut self, node: VisualNode) {
        self.nodes.push(node);
    }

    pub fn connect(&mut self, source_node: &str, source_port: &str, target_node: &str, target_port: &str) {
        self.connections.push(NodeConnection {
            id: format!("conn_{}", self.connections.len()),
            source_node: source_node.into(),
            source_port: source_port.into(),
            target_node: target_node.into(),
            target_port: target_port.into(),
        });
    }

    /// 验证图完整性
    pub fn validate(&self) -> Vec<String> {
        let mut errors = Vec::new();
        for conn in &self.connections {
            if !self.nodes.iter().any(|n| n.id == conn.source_node) {
                errors.push(format!("连接 {} 引用不存在的源节点 {}", conn.id, conn.source_node));
            }
            if !self.nodes.iter().any(|n| n.id == conn.target_node) {
                errors.push(format!("连接 {} 引用不存在的目标节点 {}", conn.id, conn.target_node));
            }
        }
        errors
    }
}

// ============================================================================
// 行为树
// ============================================================================

/// 行为树节点类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BtNodeType {
    /// 顺序节点（全部成功才成功）
    Sequence,
    /// 选择节点（任一成功即成功）
    Selector,
    /// 条件节点
    Condition,
    /// 动作节点
    Action,
    /// 并行节点
    Parallel,
    /// 装饰器（取反/重试/超时）
    Decorator,
}

/// 行为树节点状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BtStatus {
    Success,
    Failure,
    Running,
    Idle,
}

/// 行为树节点
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BehaviorTreeNode {
    pub id: String,
    pub node_type: BtNodeType,
    pub name: String,
    pub children: Vec<String>,
    pub properties: HashMap<String, serde_json::Value>,
    pub status: BtStatus,
}

/// 行为树
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BehaviorTree {
    pub id: String,
    pub name: String,
    pub nodes: HashMap<String, BehaviorTreeNode>,
    pub root_id: Option<String>,
}

impl BehaviorTree {
    pub fn new(id: String, name: String) -> Self {
        Self { id, name, nodes: HashMap::new(), root_id: None }
    }

    pub fn add_node(&mut self, node: BehaviorTreeNode) {
        if self.root_id.is_none() {
            self.root_id = Some(node.id.clone());
        }
        self.nodes.insert(node.id.clone(), node);
    }

    pub fn get_node(&self, id: &str) -> Option<&BehaviorTreeNode> {
        self.nodes.get(id)
    }

    pub fn get_children(&self, parent_id: &str) -> Vec<&BehaviorTreeNode> {
        self.nodes.get(parent_id)
            .map(|parent| {
                parent.children.iter()
                    .filter_map(|cid| self.nodes.get(cid))
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn validate(&self) -> Vec<String> {
        let mut errors = Vec::new();
        if self.root_id.is_none() {
            errors.push("行为树没有根节点".into());
        }
        for (id, node) in &self.nodes {
            for child_id in &node.children {
                if !self.nodes.contains_key(child_id) {
                    errors.push(format!("节点 {} 引用不存在的子节点 {}", id, child_id));
                }
            }
        }
        errors
    }
}

// ============================================================================
// 数据表编辑器
// ============================================================================

/// 数据表
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataTable {
    pub id: String,
    pub name: String,
    pub columns: Vec<DataColumn>,
    pub rows: Vec<DataRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataColumn {
    pub name: String,
    pub data_type: DataType,
    pub default_value: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DataType {
    String, Integer, Float, Boolean, EntityRef, AssetRef,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataRow {
    pub id: String,
    pub cells: HashMap<String, serde_json::Value>,
}

impl DataTable {
    pub fn new(id: String, name: String) -> Self {
        Self { id, name, columns: Vec::new(), rows: Vec::new() }
    }

    pub fn add_column(&mut self, column: DataColumn) {
        self.columns.push(column);
    }

    pub fn add_row(&mut self, row: DataRow) {
        self.rows.push(row);
    }

    pub fn get_cell(&self, row_id: &str, column_name: &str) -> Option<&serde_json::Value> {
        self.rows.iter()
            .find(|r| r.id == row_id)
            .and_then(|r| r.cells.get(column_name))
    }

    pub fn row_count(&self) -> usize { self.rows.len() }
    pub fn column_count(&self) -> usize { self.columns.len() }
}

// ============================================================================
// 测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_visual_script_validation() {
        let mut script = VisualScript::new("s1".into(), "Test".into());
        script.add_node(VisualNode {
            id: "n1".into(), node_type: VisualNodeType::EventStart,
            title: "Start".into(), inputs: vec![], outputs: vec![],
            properties: HashMap::new(), position: (0.0, 0.0),
        });
        script.connect("n1", "out", "n2", "in");
        let errors = script.validate();
        assert!(!errors.is_empty());  // n2 doesn't exist
    }

    #[test]
    fn test_behavior_tree_validation() {
        let mut bt = BehaviorTree::new("bt1".into(), "Patrol".into());
        bt.add_node(BehaviorTreeNode {
            id: "root".into(), node_type: BtNodeType::Sequence,
            name: "Root".into(), children: vec!["child1".into()],
            properties: HashMap::new(), status: BtStatus::Idle,
        });
        let errors = bt.validate();
        assert!(!errors.is_empty());  // child1 doesn't exist
    }

    #[test]
    fn test_behavior_tree_children() {
        let mut bt = BehaviorTree::new("bt2".into(), "Test".into());
        bt.add_node(BehaviorTreeNode {
            id: "root".into(), node_type: BtNodeType::Selector,
            name: "Root".into(), children: vec!["a1".into()],
            properties: HashMap::new(), status: BtStatus::Idle,
        });
        bt.add_node(BehaviorTreeNode {
            id: "a1".into(), node_type: BtNodeType::Action,
            name: "Move".into(), children: vec![],
            properties: HashMap::new(), status: BtStatus::Idle,
        });
        assert_eq!(bt.get_children("root").len(), 1);
        assert!(bt.validate().is_empty());
    }

    #[test]
    fn test_data_table() {
        let mut table = DataTable::new("d1".into(), "Enemies".into());
        table.add_column(DataColumn {
            name: "name".into(), data_type: DataType::String, default_value: None,
        });
        table.add_column(DataColumn {
            name: "hp".into(), data_type: DataType::Integer, default_value: None,
        });
        let mut cells = HashMap::new();
        cells.insert("name".into(), serde_json::json!("Goblin"));
        cells.insert("hp".into(), serde_json::json!(50));
        table.add_row(DataRow { id: "r1".into(), cells });
        assert_eq!(table.row_count(), 1);
        assert_eq!(table.column_count(), 2);
        assert_eq!(table.get_cell("r1", "name").and_then(|v| v.as_str()), Some("Goblin"));
    }

    #[test]
    fn test_visual_script_connect() {
        let mut script = VisualScript::new("s2".into(), "Test".into());
        script.add_node(VisualNode {
            id: "start".into(), node_type: VisualNodeType::EventStart,
            title: "Start".into(), inputs: vec![], outputs: vec![
                NodePort { id: "out1".into(), name: "out".into(), port_type: PortType::FlowOutput, data_type: "flow".into() },
            ],
            properties: HashMap::new(), position: (0.0, 0.0),
        });
        script.add_node(VisualNode {
            id: "action".into(), node_type: VisualNodeType::Action,
            title: "Do Something".into(), inputs: vec![
                NodePort { id: "in1".into(), name: "in".into(), port_type: PortType::FlowInput, data_type: "flow".into() },
            ], outputs: vec![],
            properties: HashMap::new(), position: (200.0, 0.0),
        });
        script.connect("start", "out1", "action", "in1");
        assert_eq!(script.connections.len(), 1);
    }
}