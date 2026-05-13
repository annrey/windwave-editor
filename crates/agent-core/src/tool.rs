//! Tool System - Registry and execution of Agent tools
//!
//! All tools implement the Tool trait and are registered in ToolRegistry.
//! Supports categorization, parameter validation, and parallel execution.

use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Tool trait - all tools must implement this
pub trait Tool: Send + Sync {
    /// Unique tool name (e.g., "query_entity", "create_component")
    fn name(&self) -> &str;
    
    /// Human-readable description
    fn description(&self) -> &str;
    
    /// Parameters the tool accepts
    fn parameters(&self) -> Vec<ToolParameter>;
    
    /// Tool category for organization
    fn category(&self) -> ToolCategory;
    
    /// Execute the tool with given parameters
    /// 
    /// Returns ToolResult with success status and output data
    fn execute(&self, params: HashMap<String, Value>) -> Result<ToolResult, ToolError>;
}

/// Tool parameter definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolParameter {
    pub name: String,
    pub description: String,
    pub param_type: ParameterType,
    pub required: bool,
    pub default: Option<Value>,
}

/// Parameter types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ParameterType {
    String,
    Number,
    Integer,
    Boolean,
    EntityId,
    Vec2,
    Vec3,
    Color,
    Enum(Vec<String>),
    Object(Vec<ToolParameter>), // Nested object
    Array(Box<ParameterType>), // Array of type
}

/// Tool categories for organization
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ToolCategory {
    Scene,      // Scene manipulation (entities, components)
    Code,       // Code generation and modification
    Asset,      // Asset management
    Engine,     // Engine control (build, play, export)
    External,   // External APIs (search, AI generation)
    Utility,    // Utility tools (math, string ops)
}

impl ToolCategory {
    pub fn name(&self) -> &'static str {
        match self {
            ToolCategory::Scene => "Scene",
            ToolCategory::Code => "Code",
            ToolCategory::Asset => "Asset",
            ToolCategory::Engine => "Engine",
            ToolCategory::External => "External",
            ToolCategory::Utility => "Utility",
        }
    }
    
    pub fn description(&self) -> &'static str {
        match self {
            ToolCategory::Scene => "Scene manipulation tools for entities and components",
            ToolCategory::Code => "Code generation and modification tools",
            ToolCategory::Asset => "Asset import and management tools",
            ToolCategory::Engine => "Engine control and build tools",
            ToolCategory::External => "External API integrations",
            ToolCategory::Utility => "General utility tools",
        }
    }
}

/// Result of a tool execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub success: bool,
    pub message: String,
    pub data: Option<Value>,
    pub execution_time_ms: u64,
}

impl ToolResult {
    /// Create a success result
    pub fn success(data: impl Serialize) -> Self {
        Self {
            success: true,
            message: "Success".to_string(),
            data: serde_json::to_value(data).ok(),
            execution_time_ms: 0,
        }
    }
    
    /// Create an error result
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            success: false,
            message: message.into(),
            data: None,
            execution_time_ms: 0,
        }
    }
    
    /// Get a summary of the result
    pub fn summary(&self) -> String {
        if self.success {
            format!("OK: {}", self.message)
        } else {
            format!("ERR: {}", self.message)
        }
    }
}

/// Tool error types
#[derive(Debug, thiserror::Error)]
pub enum ToolError {
    #[error("Tool not found: {0}")]
    NotFound(String),
    
    #[error("Missing required parameter: {0}")]
    MissingParameter(String),
    
    #[error("Invalid parameter value: {0}")]
    InvalidParameter(String),
    
    #[error("Tool execution failed: {0}")]
    ExecutionFailed(String),
    
    #[error("Tool timeout")]
    Timeout,
}

/// Tool call request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub tool_name: String,
    pub parameters: HashMap<String, Value>,
    pub call_id: String,
}

/// Tool Registry - manages available tools
/// 
/// Provides registration, lookup, and execution of tools.
/// Supports categorization and filtering.
pub struct ToolRegistry {
    tools: HashMap<String, Box<dyn Tool>>,
    categories: HashMap<ToolCategory, Vec<String>>,
}

impl ToolRegistry {
    /// Create empty registry
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
            categories: HashMap::new(),
        }
    }
    
    /// Register a tool
    pub fn register<T: Tool + 'static>(&mut self, tool: T) {
        let name = tool.name().to_string();
        let category = tool.category();
        
        self.tools.insert(name.clone(), Box::new(tool));
        
        self.categories
            .entry(category)
            .or_default()
            .push(name);
    }
    
    /// Get a tool by name
    pub fn get(&self, name: &str) -> Option<&dyn Tool> {
        self.tools.get(name).map(|t| t.as_ref())
    }
    
    /// Check if tool exists
    pub fn has(&self, name: &str) -> bool {
        self.tools.contains_key(name)
    }
    
    /// Execute a single tool call
    pub fn execute(&self, call: &ToolCall) -> Result<ToolResult, ToolError> {
        let tool = self.get(&call.tool_name)
            .ok_or_else(|| ToolError::NotFound(call.tool_name.clone()))?;
        
        // Validate required parameters
        for param in tool.parameters() {
            if param.required && !call.parameters.contains_key(&param.name) {
                return Err(ToolError::MissingParameter(param.name.clone()));
            }
        }
        
        // Execute
        let start = std::time::Instant::now();
        let mut result = tool.execute(call.parameters.clone())
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;
        
        result.execution_time_ms = start.elapsed().as_millis() as u64;
        Ok(result)
    }
    
    /// Execute multiple tool calls (serial)
    pub fn execute_all(&self, calls: Vec<ToolCall>) -> Vec<Result<ToolResult, ToolError>> {
        calls.into_iter()
            .map(|call| self.execute(&call))
            .collect()
    }
    
    /// List all available tool names
    pub fn list_tools(&self) -> Vec<String> {
        self.tools.keys().cloned().collect()
    }
    
    /// List tools in a category
    pub fn list_by_category(&self, category: ToolCategory) -> Vec<String> {
        self.categories.get(&category)
            .cloned()
            .unwrap_or_default()
    }
    
    /// Get all categories
    pub fn categories(&self) -> Vec<ToolCategory> {
        self.categories.keys().cloned().collect()
    }
    
    /// Generate tool descriptions for LLM context
    pub fn describe_all(&self) -> String {
        self.tools.values()
            .map(|t| format_tool_description(t.as_ref()))
            .collect::<Vec<_>>()
            .join("\n\n")
    }
    
    /// Generate descriptions for relevant tools only
    pub fn describe_relevant(&self, context: &str) -> String {
        // Simple keyword matching - can be improved with embeddings
        let lowercase_context = context.to_lowercase();
        let keywords: Vec<_> = lowercase_context.split_whitespace().collect();
        
        let relevant: Vec<_> = self.tools.values()
            .filter(|t| {
                let desc = t.description().to_lowercase();
                keywords.iter().any(|kw| desc.contains(*kw))
            })
            .map(|t| format_tool_description(t.as_ref()))
            .collect();
        
        if relevant.is_empty() {
            self.describe_all()
        } else {
            relevant.join("\n\n")
        }
    }
    
    /// Generate structured MCP-style tool descriptions for LLM tool selection
    pub fn all_mcp_descriptions(&self) -> Vec<serde_json::Value> {
        self.tools.values()
            .map(|t| tool_to_mcp_description(t.as_ref()))
            .collect()
    }

    /// Remove a tool
    pub fn unregister(&mut self, name: &str) -> Option<Box<dyn Tool>> {
        let tool = self.tools.remove(name)?;
        
        // Remove from category
        if let Some(cat_tools) = self.categories.get_mut(&tool.category()) {
            cat_tools.retain(|n| n != name);
        }
        
        Some(tool)
    }
    
    /// Clear all tools
    pub fn clear(&mut self) {
        self.tools.clear();
        self.categories.clear();
    }
    
    /// Get count of registered tools
    pub fn len(&self) -> usize {
        self.tools.len()
    }
    
    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Format a tool description for LLM consumption
fn format_tool_description(tool: &dyn Tool) -> String {
    let params = tool.parameters()
        .iter()
        .map(|p| {
            let required = if p.required { "required" } else { "optional" };
            format!("  - {} ({:?}, {}): {}", 
                p.name, p.param_type, required, p.description)
        })
        .collect::<Vec<_>>()
        .join("\n");
    
    format!(
        "Tool: {}\nCategory: {:?}\nDescription: {}\nParameters:\n{}",
        tool.name(),
        tool.category(),
        tool.description(),
        if params.is_empty() { "  (none)".to_string() } else { params }
    )
}

/// Generate MCP-style JSON tool description from a Tool instance
fn tool_to_mcp_description(tool: &dyn Tool) -> serde_json::Value {
    let properties: serde_json::Map<String, serde_json::Value> = tool.parameters()
        .iter()
        .map(|p| {
            (p.name.clone(), serde_json::json!({
                "type": parameter_type_to_string(&p.param_type),
                "description": p.description,
            }))
        })
        .collect();

    serde_json::json!({
        "name": tool.name(),
        "description": tool.description(),
        "inputSchema": {
            "type": "object",
            "properties": properties,
            "required": tool.parameters().iter()
                .filter(|p| p.required)
                .map(|p| p.name.clone())
                .collect::<Vec<_>>(),
        }
    })
}

fn parameter_type_to_string(pt: &ParameterType) -> &'static str {
    match pt {
        ParameterType::String => "string",
        ParameterType::Number => "number",
        ParameterType::Integer => "integer",
        ParameterType::Boolean => "boolean",
        ParameterType::EntityId => "string",
        ParameterType::Vec2 => "array",
        ParameterType::Vec3 => "array",
        ParameterType::Color => "array",
        ParameterType::Enum(_) => "string",
        ParameterType::Object(_) => "object",
        ParameterType::Array(_) => "array",
    }
}

/// Builder for creating ToolParameters fluently
pub struct ParameterBuilder {
    params: Vec<ToolParameter>,
}

impl Default for ParameterBuilder {
    fn default() -> Self {
        Self { params: Vec::new() }
    }
}

impl ParameterBuilder {
    pub fn new() -> Self {
        Self::default()
    }
    
    pub fn string(mut self, name: impl Into<String>, description: impl Into<String>, required: bool) -> Self {
        self.params.push(ToolParameter {
            name: name.into(),
            description: description.into(),
            param_type: ParameterType::String,
            required,
            default: None,
        });
        self
    }
    
    pub fn number(mut self, name: impl Into<String>, description: impl Into<String>, required: bool) -> Self {
        self.params.push(ToolParameter {
            name: name.into(),
            description: description.into(),
            param_type: ParameterType::Number,
            required,
            default: None,
        });
        self
    }
    
    pub fn entity(mut self, name: impl Into<String>, description: impl Into<String>, required: bool) -> Self {
        self.params.push(ToolParameter {
            name: name.into(),
            description: description.into(),
            param_type: ParameterType::EntityId,
            required,
            default: None,
        });
        self
    }
    
    pub fn build(self) -> Vec<ToolParameter> {
        self.params
    }
}

// Example tools for testing

/// Echo tool - returns input (for testing)
pub struct EchoTool;

impl Tool for EchoTool {
    fn name(&self) -> &str {
        "echo"
    }
    
    fn description(&self) -> &str {
        "Echoes back the input message for testing"
    }
    
    fn parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter {
                name: "message".to_string(),
                description: "Message to echo".to_string(),
                param_type: ParameterType::String,
                required: true,
                default: None,
            }
        ]
    }
    
    fn category(&self) -> ToolCategory {
        ToolCategory::Utility
    }
    
    fn execute(&self, params: HashMap<String, Value>) -> Result<ToolResult, ToolError> {
        let message = params.get("message")
            .and_then(|v: &Value| v.as_str())
            .ok_or_else(|| ToolError::MissingParameter("message".to_string()))?;
        
        Ok(ToolResult {
            success: true,
            message: format!("Echo: {}", message),
            data: None,
            execution_time_ms: 0,
        })
    }
}

/// Math tool - performs calculations
pub struct MathTool;

impl Tool for MathTool {
    fn name(&self) -> &str {
        "math"
    }
    
    fn description(&self) -> &str {
        "Performs mathematical calculations"
    }
    
    fn parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter {
                name: "operation".to_string(),
                description: "Operation to perform".to_string(),
                param_type: ParameterType::Enum(vec![
                    "add".to_string(),
                    "subtract".to_string(),
                    "multiply".to_string(),
                    "divide".to_string(),
                ]),
                required: true,
                default: None,
            },
            ToolParameter {
                name: "a".to_string(),
                description: "First operand".to_string(),
                param_type: ParameterType::Number,
                required: true,
                default: None,
            },
            ToolParameter {
                name: "b".to_string(),
                description: "Second operand".to_string(),
                param_type: ParameterType::Number,
                required: true,
                default: None,
            },
        ]
    }
    
    fn category(&self) -> ToolCategory {
        ToolCategory::Utility
    }
    
    fn execute(&self, params: HashMap<String, Value>) -> Result<ToolResult, ToolError> {
        let op = params.get("operation")
            .and_then(|v: &Value| v.as_str())
            .ok_or_else(|| ToolError::MissingParameter("operation".to_string()))?;
        
        let a = params.get("a")
            .and_then(|v: &Value| v.as_f64())
            .ok_or_else(|| ToolError::MissingParameter("a".to_string()))?;
        
        let b = params.get("b")
            .and_then(|v: &Value| v.as_f64())
            .ok_or_else(|| ToolError::MissingParameter("b".to_string()))?;
        
        let result = match op {
            "add" => a + b,
            "subtract" => a - b,
            "multiply" => a * b,
            "divide" => {
                if b == 0.0 {
                    return Err(ToolError::ExecutionFailed("Division by zero".to_string()));
                }
                a / b
            }
            _ => return Err(ToolError::InvalidParameter("operation".to_string())),
        };
        
        Ok(ToolResult::success(result))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_tool_registry() {
        let mut registry = ToolRegistry::new();
        
        registry.register(EchoTool);
        registry.register(MathTool);
        
        assert_eq!(registry.len(), 2);
        assert!(registry.has("echo"));
        assert!(registry.has("math"));
        
        let tools = registry.list_tools();
        assert!(tools.contains(&"echo".to_string()));
        assert!(tools.contains(&"math".to_string()));
    }
    
    #[test]
    fn test_execute_echo() {
        let mut registry = ToolRegistry::new();
        registry.register(EchoTool);
        
        let call = ToolCall {
            tool_name: "echo".to_string(),
            parameters: {
                let mut m = HashMap::new();
                m.insert("message".to_string(), Value::String("Hello".to_string()));
                m
            },
            call_id: "1".to_string(),
        };
        
        let result = registry.execute(&call).unwrap();
        assert!(result.success);
        assert!(result.message.contains("Echo"));
    }
    
    #[test]
    fn test_execute_math() {
        let mut registry = ToolRegistry::new();
        registry.register(MathTool);
        
        let call = ToolCall {
            tool_name: "math".to_string(),
            parameters: {
                let mut m = HashMap::new();
                m.insert("operation".to_string(), Value::String("add".to_string()));
                m.insert("a".to_string(), serde_json::json!(5.0));
                m.insert("b".to_string(), serde_json::json!(3.0));
                m
            },
            call_id: "1".to_string(),
        };
        
        let result = registry.execute(&call).unwrap();
        assert!(result.success);
    }
    
    #[test]
    fn test_missing_parameter() {
        let mut registry = ToolRegistry::new();
        registry.register(EchoTool);
        
        let call = ToolCall {
            tool_name: "echo".to_string(),
            parameters: HashMap::new(), // Missing required "message"
            call_id: "1".to_string(),
        };
        
        let result = registry.execute(&call);
        assert!(result.is_err());
    }
}
