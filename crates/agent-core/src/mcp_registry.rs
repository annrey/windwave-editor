//! McpToolRegistry — MCP-compatible tool registry with size limits.
//!
//! Provides a thin safety layer over the existing ToolRegistry and
//! SkillRegistry that enforces result size caps and model compatibility
//! checks. Inspired by UI-TARS-desktop's MCP practice notes.

use serde::{Deserialize, Serialize};

/// Compatibility tag for model-specific tool filtering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModelCompatibility {
    All,
    OpenAI,
    Claude,
}

/// A registered MCP tool descriptor with safety metadata.
#[derive(Debug, Clone, Serialize)]
pub struct McpToolDescriptor {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
    pub max_result_tokens: usize,
    pub model_compatibility: Vec<ModelCompatibility>,
}

impl McpToolDescriptor {
    pub fn is_compatible_with(&self, model: ModelCompatibility) -> bool {
        self.model_compatibility.contains(&ModelCompatibility::All)
            || self.model_compatibility.contains(&model)
    }
}

/// Registry of MCP tools with result-size enforcement.
pub struct McpToolRegistry {
    tools: std::collections::HashMap<String, McpToolDescriptor>,
    max_total_output_tokens: usize,
}

impl McpToolRegistry {
    pub fn new(max_total_output_tokens: usize) -> Self {
        Self {
            tools: std::collections::HashMap::new(),
            max_total_output_tokens,
        }
    }

    /// Register a tool with its MCP descriptor.
    pub fn register(&mut self, descriptor: McpToolDescriptor) {
        self.tools.insert(descriptor.name.clone(), descriptor);
    }

    /// List all MCP-compatible tool descriptions.
    pub fn list_descriptions(&self) -> Vec<&McpToolDescriptor> {
        self.tools.values().collect()
    }

    /// List tool descriptions filtered by model compatibility.
    pub fn list_for_model(&self, model: ModelCompatibility) -> Vec<&McpToolDescriptor> {
        self.tools.values().filter(|d| d.is_compatible_with(model)).collect()
    }

    /// Get a single tool by name.
    pub fn get(&self, name: &str) -> Option<&McpToolDescriptor> {
        self.tools.get(name)
    }

    /// Check if adding a tool with this max_result_tokens would exceed budget.
    pub fn would_exceed_budget(&self, additional_tokens: usize) -> bool {
        let current_max: usize = self.tools.values().map(|d| d.max_result_tokens).sum();
        current_max + additional_tokens > self.max_total_output_tokens
    }

    /// Total token budget available.
    pub fn budget(&self) -> usize { self.max_total_output_tokens }

    /// Current token usage across all registered tools.
    pub fn current_usage(&self) -> usize {
        self.tools.values().map(|d| d.max_result_tokens).sum()
    }

    pub fn tool_count(&self) -> usize { self.tools.len() }

    /// Generate an MCP-formatted tool list for LLM consumption.
    pub fn to_mcp_list(&self) -> serde_json::Value {
        let tools: Vec<serde_json::Value> = self.tools.values().map(|d| {
            serde_json::json!({
                "name": d.name,
                "description": d.description,
                "inputSchema": d.input_schema,
            })
        }).collect();
        serde_json::json!(tools)
    }

    /// Generate MCP list filtered by model compatibility.
    pub fn to_mcp_list_for_model(&self, model: ModelCompatibility) -> serde_json::Value {
        let tools: Vec<serde_json::Value> = self.list_for_model(model).iter().map(|d| {
            serde_json::json!({
                "name": d.name,
                "description": d.description,
                "inputSchema": d.input_schema,
            })
        }).collect();
        serde_json::json!(tools)
    }
}

impl Default for McpToolRegistry {
    fn default() -> Self { Self::new(8192) }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_and_list() {
        let mut reg = McpToolRegistry::new(4096);
        reg.register(McpToolDescriptor {
            name: "create_entity".into(),
            description: "Create a new entity".into(),
            input_schema: serde_json::json!({}),
            max_result_tokens: 200,
            model_compatibility: vec![ModelCompatibility::All],
        });
        assert_eq!(reg.tool_count(), 1);
        assert_eq!(reg.list_for_model(ModelCompatibility::OpenAI).len(), 1);
    }

    #[test]
    fn test_model_filter() {
        let mut reg = McpToolRegistry::new(4096);
        reg.register(McpToolDescriptor {
            name: "claude_only".into(),
            description: "".into(),
            input_schema: serde_json::json!({}),
            max_result_tokens: 100,
            model_compatibility: vec![ModelCompatibility::Claude],
        });
        assert_eq!(reg.list_for_model(ModelCompatibility::OpenAI).len(), 0);
        assert_eq!(reg.list_for_model(ModelCompatibility::Claude).len(), 1);
    }

    #[test]
    fn test_budget_exceeded() {
        let mut reg = McpToolRegistry::new(500);
        reg.register(McpToolDescriptor {
            name: "a".into(), description: "".into(),
            input_schema: serde_json::json!({}),
            max_result_tokens: 300,
            model_compatibility: vec![ModelCompatibility::All],
        });
        assert!(!reg.would_exceed_budget(100)); // 300 + 100 = 400 < 500
        assert!(reg.would_exceed_budget(300)); // 300 + 300 = 600 > 500
    }
}
