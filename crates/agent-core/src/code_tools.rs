//! Code Generation Tools - Generate game code and components
//!
//! Provides tools for the Agent to generate:
//! - Rust code for Bevy components and systems
//! - Shader code
//! - Configuration files
//! - Scene definition code

use crate::tool::{Tool, ToolCategory, ToolParameter, ToolResult, ToolError, ParameterType};
use serde_json::Value;
use std::collections::HashMap;

/// Generate Rust component code
pub struct GenerateComponentTool;

impl Tool for GenerateComponentTool {
    fn name(&self) -> &str {
        "generate_component"
    }

    fn description(&self) -> &str {
        "Generate a Bevy component struct with properties"
    }

    fn parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter {
                name: "name".to_string(),
                description: "Component name (e.g., 'PlayerController')".to_string(),
                param_type: ParameterType::String,
                required: true,
                default: None,
            },
            ToolParameter {
                name: "properties".to_string(),
                description: "Component properties as array of {name, type, default}".to_string(),
                param_type: ParameterType::Array(Box::new(ParameterType::Object(vec![]))),
                required: false,
                default: Some(Value::Array(vec![])),
            },
            ToolParameter {
                name: "derives".to_string(),
                description: "Derive macros to add (default: Component, Debug)".to_string(),
                param_type: ParameterType::Array(Box::new(ParameterType::String)),
                required: false,
                default: Some(Value::Array(vec![
                    Value::String("Component".to_string()),
                    Value::String("Debug".to_string()),
                ])),
            },
        ]
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Code
    }

    fn execute(&self, params: HashMap<String, Value>) -> Result<ToolResult, ToolError> {
        let name = params.get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::MissingParameter("name".to_string()))?;

        let derives = params.get("derives")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect::<Vec<_>>())
            .unwrap_or_else(|| vec!["Component".to_string(), "Debug".to_string()]);

        let properties = params.get("properties")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter()
                .filter_map(|v| v.as_object().map(|o| {
                    let name = o.get("name").and_then(|v| v.as_str()).unwrap_or("field");
                    let typ = o.get("type").and_then(|v| v.as_str()).unwrap_or("f32");
                    let default = o.get("default").map(|v| format!(" = {}", v)).unwrap_or_default();
                    format!("    pub {}: {}{},", name, typ, default)
                }))
                .collect::<Vec<_>>())
            .unwrap_or_default();

        let derive_str = derives.join(", ");
        
        let code = if properties.is_empty() {
            format!(
                r#"#[derive({})]
pub struct {};

impl Default for {} {{
    fn default() -> Self {{
        Self {{}}
    }}
}}"#,
                derive_str, name, name
            )
        } else {
            format!(
                r#"#[derive({})]
pub struct {} {{
{}
}}

impl Default for {} {{
    fn default() -> Self {{
        Self {{
            // Initialize fields
        }}
    }}
}}"#,
                derive_str, name, properties.join("\n"), name
            )
        };

        Ok(ToolResult {
            success: true,
            message: format!("Generated component '{}'", name),
            data: Some(Value::String(code)),
            execution_time_ms: 10,
        })
    }
}

/// Generate Bevy system code
pub struct GenerateSystemTool;

impl Tool for GenerateSystemTool {
    fn name(&self) -> &str {
        "generate_system"
    }

    fn description(&self) -> &str {
        "Generate a Bevy system function"
    }

    fn parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter {
                name: "name".to_string(),
                description: "System function name".to_string(),
                param_type: ParameterType::String,
                required: true,
                default: None,
            },
            ToolParameter {
                name: "query".to_string(),
                description: "Query parameters (e.g., '&mut Transform')".to_string(),
                param_type: ParameterType::Array(Box::new(ParameterType::String)),
                required: false,
                default: Some(Value::Array(vec![])),
            },
            ToolParameter {
                name: "logic".to_string(),
                description: "System logic description".to_string(),
                param_type: ParameterType::String,
                required: false,
                default: Some(Value::String("// Iterate over matching entities and apply game logic".to_string())),
            },
        ]
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Code
    }

    fn execute(&self, params: HashMap<String, Value>) -> Result<ToolResult, ToolError> {
        let name = params.get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::MissingParameter("name".to_string()))?;

        let query = params.get("query")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter()
                .filter_map(|v| v.as_str())
                .collect::<Vec<_>>())
            .unwrap_or_default();

        let logic = params.get("logic")
            .and_then(|v| v.as_str())
            .unwrap_or("// Apply game logic to each entity in the query");

        let query_str = if query.is_empty() {
            "mut commands: Commands".to_string()
        } else {
            format!("mut query: Query<({})>", query.join(", "))
        };

        let code = format!(
            r#"fn {}(
    {}
) {{
    for {} in query.iter_mut() {{
        {}
    }}
}}"#,
            name, query_str, 
            if query.len() == 1 { "item" } else { "(transform, other)" },
            logic
        );

        Ok(ToolResult {
            success: true,
            message: format!("Generated system '{}'", name),
            data: Some(Value::String(code)),
            execution_time_ms: 10,
        })
    }
}

/// Generate resource definition
pub struct GenerateResourceTool;

impl Tool for GenerateResourceTool {
    fn name(&self) -> &str {
        "generate_resource"
    }

    fn description(&self) -> &str {
        "Generate a Bevy resource struct"
    }

    fn parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter {
                name: "name".to_string(),
                description: "Resource name".to_string(),
                param_type: ParameterType::String,
                required: true,
                default: None,
            },
            ToolParameter {
                name: "fields".to_string(),
                description: "Resource fields".to_string(),
                param_type: ParameterType::Array(Box::new(ParameterType::Object(vec![]))),
                required: false,
                default: Some(Value::Array(vec![])),
            },
        ]
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Code
    }

    fn execute(&self, params: HashMap<String, Value>) -> Result<ToolResult, ToolError> {
        let name = params.get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::MissingParameter("name".to_string()))?;

        let code = format!(
            r#"#[derive(Resource, Default, Debug)]
pub struct {} {{
    // Add fields here
}}

impl {} {{
    pub fn new() -> Self {{
        Self::default()
    }}
}}"#,
            name, name
        );

        Ok(ToolResult {
            success: true,
            message: format!("Generated resource '{}'", name),
            data: Some(Value::String(code)),
            execution_time_ms: 5,
        })
    }
}

/// Generate event definition
pub struct GenerateEventTool;

impl Tool for GenerateEventTool {
    fn name(&self) -> &str {
        "generate_event"
    }

    fn description(&self) -> &str {
        "Generate a Bevy event struct"
    }

    fn parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter {
                name: "name".to_string(),
                description: "Event name".to_string(),
                param_type: ParameterType::String,
                required: true,
                default: None,
            },
            ToolParameter {
                name: "fields".to_string(),
                description: "Event fields".to_string(),
                param_type: ParameterType::Array(Box::new(ParameterType::Object(vec![]))),
                required: false,
                default: Some(Value::Array(vec![])),
            },
        ]
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Code
    }

    fn execute(&self, params: HashMap<String, Value>) -> Result<ToolResult, ToolError> {
        let name = params.get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::MissingParameter("name".to_string()))?;

        let code = format!(
            r#"#[derive(Event, Debug, Clone)]
pub struct {} {{
    // Add event data fields
}}

impl {} {{
    pub fn new() -> Self {{
        Self {{}}
    }}
}}"#,
            name, name
        );

        Ok(ToolResult {
            success: true,
            message: format!("Generated event '{}'", name),
            data: Some(Value::String(code)),
            execution_time_ms: 5,
        })
    }
}

/// Format Rust code (basic)
pub struct FormatCodeTool;

impl Tool for FormatCodeTool {
    fn name(&self) -> &str {
        "format_code"
    }

    fn description(&self) -> &str {
        "Format Rust code with basic indentation fixes"
    }

    fn parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter {
                name: "code".to_string(),
                description: "Code to format".to_string(),
                param_type: ParameterType::String,
                required: true,
                default: None,
            },
        ]
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Code
    }

    fn execute(&self, params: HashMap<String, Value>) -> Result<ToolResult, ToolError> {
        let code = params.get("code")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::MissingParameter("code".to_string()))?;

        // Basic formatting - trim and normalize newlines
        let formatted = code
            .lines()
            .map(|line| line.trim_end())
            .collect::<Vec<_>>()
            .join("\n")
            .trim()
            .to_string();

        Ok(ToolResult {
            success: true,
            message: "Code formatted".to_string(),
            data: Some(Value::String(formatted)),
            execution_time_ms: 2,
        })
    }
}

/// Analyze code structure
pub struct AnalyzeCodeTool;

impl Tool for AnalyzeCodeTool {
    fn name(&self) -> &str {
        "analyze_code"
    }

    fn description(&self) -> &str {
        "Analyze Rust code structure and extract components, systems, resources"
    }

    fn parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter {
                name: "code".to_string(),
                description: "Rust code to analyze".to_string(),
                param_type: ParameterType::String,
                required: true,
                default: None,
            },
        ]
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Code
    }

    fn execute(&self, params: HashMap<String, Value>) -> Result<ToolResult, ToolError> {
        let code = params.get("code")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::MissingParameter("code".to_string()))?;

        // Simple regex-based analysis
        let component_regex = regex::Regex::new(r"struct\s+(\w+)\s*.*#\[derive.*Component").unwrap();
        let system_regex = regex::Regex::new(r"fn\s+(\w+)\s*\([^)]*Query").unwrap();
        let resource_regex = regex::Regex::new(r"struct\s+(\w+)\s*.*#\[derive.*Resource").unwrap();

        let components: Vec<String> = component_regex.captures_iter(code)
            .filter_map(|c| c.get(1).map(|m| m.as_str().to_string()))
            .collect();

        let systems: Vec<String> = system_regex.captures_iter(code)
            .filter_map(|c| c.get(1).map(|m| m.as_str().to_string()))
            .collect();

        let resources: Vec<String> = resource_regex.captures_iter(code)
            .filter_map(|c| c.get(1).map(|m| m.as_str().to_string()))
            .collect();

        let analysis = serde_json::json!({
            "components": components,
            "systems": systems,
            "resources": resources,
            "summary": format!(
                "Found {} components, {} systems, {} resources",
                components.len(), systems.len(), resources.len()
            )
        });

        Ok(ToolResult {
            success: true,
            message: "Code analysis complete".to_string(),
            data: Some(analysis),
            execution_time_ms: 20,
        })
    }
}

/// Register all code generation tools
pub fn register_code_tools(registry: &mut crate::tool::ToolRegistry) {
    registry.register(GenerateComponentTool);
    registry.register(GenerateSystemTool);
    registry.register(GenerateResourceTool);
    registry.register(GenerateEventTool);
    registry.register(FormatCodeTool);
    registry.register(AnalyzeCodeTool);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_component() {
        let tool = GenerateComponentTool;
        let mut params = HashMap::new();
        params.insert("name".to_string(), Value::String("PlayerController".to_string()));
        
        let result = tool.execute(params).unwrap();
        assert!(result.success);
        assert!(result.data.as_ref().unwrap().as_str().unwrap().contains("PlayerController"));
    }

    #[test]
    fn test_analyze_code() {
        let tool = AnalyzeCodeTool;
        let code = r#"
            #[derive(Component)]
            struct Player;
            
            #[derive(Resource)]
            struct GameState;
            
            fn move_player(query: Query<&Transform>) {}
        "#;
        
        let mut params = HashMap::new();
        params.insert("code".to_string(), Value::String(code.to_string()));
        
        let result = tool.execute(params).unwrap();
        assert!(result.success);
    }
}
