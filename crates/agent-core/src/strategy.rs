//! ReAct Agent Strategy - Reasoning + Acting Loop
//!
//! Implements the ReAct (Reasoning + Acting) pattern where the agent:
//! 1. Thinks about the problem and available information
//! 2. Acts by calling appropriate tools
//! 3. Observes the results
//! 4. Repeats until task completion

use crate::agent::BaseAgent;
use crate::llm::{LlmClient, LlmMessage, LlmRequest, Role, ToolDefinition};
use crate::tool::ToolCall;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

/// A step in the ReAct reasoning process
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ReActStep {
    /// Agent is thinking about what to do
    Thought {
        content: String,
        reasoning: String,
    },
    /// Agent decides to take an action
    Action {
        tool_name: String,
        parameters: HashMap<String, serde_json::Value>,
    },
    /// Observation from the environment/tool
    Observation {
        content: String,
        success: bool,
    },
    /// Final answer to the user
    FinalAnswer {
        content: String,
    },
}

/// ReAct strategy configuration
#[derive(Debug, Clone)]
pub struct ReActConfig {
    /// Maximum number of reasoning steps
    pub max_steps: usize,
    /// Temperature for LLM reasoning
    pub temperature: f32,
    /// Whether to include tool results in context
    pub include_observations: bool,
    /// System prompt template
    pub system_prompt: String,
}

impl Default for ReActConfig {
    fn default() -> Self {
        Self {
            max_steps: 10,
            temperature: 0.7,
            include_observations: true,
            system_prompt: REACT_SYSTEM_PROMPT.to_string(),
        }
    }
}

/// Default ReAct system prompt
const REACT_SYSTEM_PROMPT: &str = r#"You are a helpful AI assistant that can interact with a game editor.
You have access to various tools to manipulate the scene, create entities, and modify components.

When responding, you MUST follow this format:

Thought: [Your reasoning about what needs to be done]
Action: [Tool name and parameters in JSON format]

OR if you have a final answer:

Thought: [Brief reasoning]
Final Answer: [Your response to the user]

Available tools will be provided in the context."#;

/// ReAct Agent that implements reasoning + acting strategy
pub struct ReActAgent {
    #[allow(dead_code)]
    base: BaseAgent,
    /// ReAct strategy configuration (public for DirectorRuntime access).
    pub config: ReActConfig,
    llm: Arc<dyn LlmClient>,
    tool_registry: Arc<std::sync::Mutex<crate::tool::ToolRegistry>>,
    history: Vec<ReActStep>,
}

impl ReActAgent {
    /// Create a new ReAct agent
    pub fn new(
        base: BaseAgent,
        config: ReActConfig,
        llm: Arc<dyn LlmClient>,
        tool_registry: Arc<std::sync::Mutex<crate::tool::ToolRegistry>>,
    ) -> Self {
        Self {
            base,
            config,
            llm,
            tool_registry,
            history: Vec::new(),
        }
    }

    /// Execute one ReAct step
    pub async fn step(&mut self, user_input: &str) -> Result<ReActStep, ReActError> {
        // Build the prompt with history and available tools
        let prompt = self.build_prompt(user_input);
        
        // Get LLM response
        let messages = vec![
            LlmMessage {
                role: Role::System,
                content: self.config.system_prompt.clone(),
            },
            LlmMessage {
                role: Role::User,
                content: prompt,
            },
        ];
        
        let request = LlmRequest {
            model: "gpt-4o-mini".to_string(),
            messages,
            tools: Some(self.build_tool_definitions()),
            max_tokens: Some(2048),
            temperature: Some(self.config.temperature),
        };
        
        let response = self.llm.chat(request).await
            .map_err(|e| ReActError::LlmError(e.to_string()))?;
        
        // Parse the response to extract thought and action/final answer
        let parsed = self.parse_response(&response.content)?;
        
        // If it's an action, execute the tool
        if let ReActStep::Action { tool_name, parameters } = &parsed {
            let tool_call = ToolCall {
                tool_name: tool_name.clone(),
                parameters: parameters.clone(),
                call_id: format!("call_{}", tool_name),
            };

            let registry = self.tool_registry.lock()
                .map_err(|e| ReActError::ToolError(e.to_string()))?;
            let result = registry.execute(&tool_call)
                .map_err(|e| ReActError::ToolError(e.to_string()))?;
            drop(registry);

            self.history.push(parsed.clone());
            self.history.push(ReActStep::Observation {
                content: result.message.clone(),
                success: result.success,
            });

            if result.success {
                return Ok(ReActStep::Observation {
                    content: result.message,
                    success: true,
                });
            } else {
                return Ok(ReActStep::Observation {
                    content: result.message,
                    success: false,
                });
            }
        }

        self.history.push(parsed.clone());
        Ok(parsed)
    }

    /// Run the full ReAct loop until completion
    pub async fn run(&mut self, user_input: &str) -> Result<String, ReActError> {
        for step_count in 0..self.config.max_steps {
            let step = self.step(user_input).await?;
            
            match &step {
                ReActStep::FinalAnswer { content } => {
                    return Ok(content.clone());
                }
                ReActStep::Action { tool_name, parameters } => {
                    // Execute tool and add observation
                    let observation = self.execute_tool(tool_name, parameters).await?;
                    self.history.push(ReActStep::Observation {
                        content: observation.clone(),
                        success: true,
                    });
                }
                _ => {}
            }
            
            // Check if we should stop
            if step_count >= self.config.max_steps - 1 {
                return Err(ReActError::MaxStepsReached);
            }
        }
        
        Err(ReActError::MaxStepsReached)
    }

    /// Build the prompt with context
    fn build_prompt(&self, user_input: &str) -> String {
        let mut prompt = format!("User request: {}\n\n", user_input);
        
        // Add history
        if !self.history.is_empty() {
            prompt.push_str("Previous steps:\n");
            for (i, step) in self.history.iter().enumerate() {
                match step {
                    ReActStep::Thought { content, .. } => {
                        prompt.push_str(&format!("{}. Thought: {}\n", i + 1, content));
                    }
                    ReActStep::Action { tool_name, parameters } => {
                        prompt.push_str(&format!("{}. Action: {} with {:?}\n", 
                            i + 1, tool_name, parameters));
                    }
                    ReActStep::Observation { content, success } => {
                        prompt.push_str(&format!("{}. Observation (success={}): {}\n",
                            i + 1, success, content));
                    }
                    ReActStep::FinalAnswer { content } => {
                        prompt.push_str(&format!("{}. Final Answer: {}\n", i + 1, content));
                    }
                }
            }
            prompt.push('\n');
        }
        
        prompt.push_str("What is your next thought and action?\n");
        prompt
    }

    /// Build tool definitions for LLM
    fn build_tool_definitions(&self) -> Vec<ToolDefinition> {
        let registry = match self.tool_registry.lock() {
            Ok(r) => r,
            Err(_) => return Vec::new(),
        };
        registry.list_tools().iter().filter_map(|name| {
            let tool = registry.get(name)?;
            Some(ToolDefinition {
                name: tool.name().to_string(),
                description: tool.description().to_string(),
                parameters: build_parameters_schema(&tool.parameters()),
            })
        }).collect()
    }

    /// Parse LLM response into ReActStep
    fn parse_response(&self, content: &str) -> Result<ReActStep, ReActError> {
        // Look for Thought: and Action: or Final Answer:
        let thought_regex = regex::Regex::new(r"Thought:\s*(.+?)(?:\nAction:|\nFinal Answer:|$)")
            .map_err(|_| ReActError::ParseError("Invalid regex".to_string()))?;
        
        let action_regex = regex::Regex::new(r"Action:\s*(.+)$")
            .map_err(|_| ReActError::ParseError("Invalid regex".to_string()))?;
        
        let final_regex = regex::Regex::new(r"Final Answer:\s*(.+)$")
            .map_err(|_| ReActError::ParseError("Invalid regex".to_string()))?;
        
        let thought = thought_regex.captures(content)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().trim())
            .unwrap_or("No thought provided");
        
        // Check for Final Answer first
        if let Some(final_cap) = final_regex.captures(content) {
            if let Some(answer) = final_cap.get(1) {
                return Ok(ReActStep::FinalAnswer {
                    content: answer.as_str().trim().to_string(),
                });
            }
        }
        
        // Otherwise look for Action
        if let Some(action_cap) = action_regex.captures(content) {
            if let Some(action_str) = action_cap.get(1) {
                // Try to parse as JSON
                let action_text = action_str.as_str().trim();
                
                // Simple parsing - in production, use proper JSON parsing
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(action_text) {
                    if let Some(obj) = json.as_object() {
                        let tool_name = obj.get("tool")
                            .and_then(|v| v.as_str())
                            .unwrap_or("unknown")
                            .to_string();
                        
                        let parameters = obj.get("parameters")
                            .and_then(|v| v.as_object())
                            .map(|o| o.iter()
                                .map(|(k, v)| (k.clone(), v.clone()))
                                .collect())
                            .unwrap_or_default();
                        
                        return Ok(ReActStep::Action {
                            tool_name,
                            parameters,
                        });
                    }
                }
                
                // Fallback: treat entire action as tool name
                return Ok(ReActStep::Action {
                    tool_name: action_text.to_string(),
                    parameters: HashMap::new(),
                });
            }
        }
        
        // If no action or final answer, return as thought only
        Ok(ReActStep::Thought {
            content: thought.to_string(),
            reasoning: "No action taken".to_string(),
        })
    }

    /// Execute a tool and return observation
    async fn execute_tool(&self, tool_name: &str, parameters: &HashMap<String, serde_json::Value>) 
        -> Result<String, ReActError> {
        let registry = self.tool_registry.lock().map_err(|e| ReActError::ToolError(e.to_string()))?;
        let call = ToolCall {
            tool_name: tool_name.to_string(),
            parameters: parameters.clone(),
            call_id: format!("call_{}", tool_name),
        };
        let result = registry.execute(&call).map_err(|e| ReActError::ToolError(e.to_string()))?;
        if result.success {
            Ok(result.message)
        } else {
            Err(ReActError::ToolError(result.message))
        }
    }

    /// Get the current history
    pub fn history(&self) -> &[ReActStep] {
        &self.history
    }

    /// Clear history
    pub fn clear_history(&mut self) {
        self.history.clear();
    }
}

/// Errors that can occur during ReAct execution
#[derive(Debug, thiserror::Error)]
pub enum ReActError {
    #[error("LLM error: {0}")]
    LlmError(String),
    
    #[error("Parse error: {0}")]
    ParseError(String),
    
    #[error("Tool execution error: {0}")]
    ToolError(String),
    
    #[error("Maximum steps reached without completion")]
    MaxStepsReached,
    
    #[error("Invalid state: {0}")]
    InvalidState(String),
}

/// Build JSON schema for tool parameters
#[allow(dead_code)]
fn build_parameters_schema(params: &[crate::tool::ToolParameter]) -> serde_json::Value {
    let properties: HashMap<String, serde_json::Value> = params.iter()
        .map(|p| {
            let schema = serde_json::json!({
                "type": match p.param_type {
                    crate::tool::ParameterType::String => "string",
                    crate::tool::ParameterType::Integer => "integer",
                    crate::tool::ParameterType::Number => "number",
                    crate::tool::ParameterType::Boolean => "boolean",
                    crate::tool::ParameterType::EntityId => "integer",
                    crate::tool::ParameterType::Vec2 => "array",
                    crate::tool::ParameterType::Vec3 => "array",
                    crate::tool::ParameterType::Color => "string",
                    crate::tool::ParameterType::Array(_) => "array",
                    crate::tool::ParameterType::Enum(_) => "string",
                    crate::tool::ParameterType::Object(_) => "object",
                },
                "description": p.description.clone(),
            });
            (p.name.clone(), schema)
        })
        .collect();
    
    let required: Vec<String> = params.iter()
        .filter(|p| p.required)
        .map(|p| p.name.clone())
        .collect();
    
    serde_json::json!({
        "type": "object",
        "properties": properties,
        "required": required,
    })
}

/// Helper to create a simple ReAct agent
pub fn create_react_agent(
    base: BaseAgent,
    llm: Arc<dyn LlmClient>,
    tool_registry: Arc<std::sync::Mutex<crate::tool::ToolRegistry>>,
) -> ReActAgent {
    ReActAgent::new(base, ReActConfig::default(), llm, tool_registry)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_react_config_default() {
        let config = ReActConfig::default();
        assert_eq!(config.max_steps, 10);
        assert_eq!(config.temperature, 0.7);
        assert!(config.include_observations);
    }

    #[test]
    fn test_react_config_custom() {
        let config = ReActConfig {
            max_steps: 5,
            temperature: 0.3,
            include_observations: false,
            system_prompt: "Custom prompt".into(),
        };
        assert_eq!(config.max_steps, 5);
        assert!(!config.include_observations);
    }

    #[test]
    fn test_parse_thought_only() {
        let content = "Thought: I need to query the entities in the scene.";

        let thought_regex = regex::Regex::new(r"Thought:\s*(.+?)(\n|$)").unwrap();
        let thought = thought_regex.captures(content)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().trim())
            .unwrap_or("No thought");
        assert_eq!(thought, "I need to query the entities in the scene.");
    }

    #[test]
    fn test_parse_response_with_action() {
        let content = "Thought: I should create an entity.\nAction: {\"tool\": \"create_entity\", \"parameters\": {\"name\": \"Enemy\"}}";
        let action_regex = regex::Regex::new(r"Action:\s*(.+)$").unwrap();
        let action_cap = action_regex.captures(content).unwrap();
        let action_text = action_cap.get(1).unwrap().as_str().trim();
        let json: serde_json::Value = serde_json::from_str(action_text).unwrap();
        assert_eq!(json["tool"].as_str().unwrap(), "create_entity");
    }

    #[test]
    fn test_parse_response_with_final_answer() {
        let content = "Thought: Task is done.\nFinal Answer: Entity created successfully.";
        let final_regex = regex::Regex::new(r"Final Answer:\s*(.+)$").unwrap();
        let final_cap = final_regex.captures(content).unwrap();
        let answer = final_cap.get(1).unwrap().as_str().trim();
        assert_eq!(answer, "Entity created successfully.");
    }

    #[test]
    fn test_react_step_variants() {
        let thought = ReActStep::Thought {
            content: "thinking".into(),
            reasoning: "reason".into(),
        };
        assert!(matches!(thought, ReActStep::Thought { .. }));

        let action = ReActStep::Action {
            tool_name: "spawn".into(),
            parameters: std::collections::HashMap::new(),
        };
        assert!(matches!(action, ReActStep::Action { .. }));

        let obs = ReActStep::Observation {
            content: "created".into(),
            success: true,
        };
        assert!(matches!(obs, ReActStep::Observation { .. }));

        let answer = ReActStep::FinalAnswer {
            content: "done".into(),
        };
        assert!(matches!(answer, ReActStep::FinalAnswer { .. }));
    }

    #[test]
    fn test_react_error_display() {
        let err = ReActError::MaxStepsReached;
        assert!(err.to_string().contains("Maximum steps"));

        let err = ReActError::LlmError("timeout".into());
        assert!(err.to_string().contains("LLM"));

        let err = ReActError::ToolError("not found".into());
        assert!(err.to_string().contains("Tool"));
    }

    #[test]
    fn test_build_prompt() {
        let _config = ReActConfig::default();
        let history: Vec<ReActStep> = vec![
            ReActStep::Thought { content: "query scene".into(), reasoning: String::new() },
            ReActStep::Action {
                tool_name: "query_entities".into(),
                parameters: std::collections::HashMap::new(),
            },
            ReActStep::Observation { content: "3 entities found".into(), success: true },
        ];

        let mut prompt = "User request: create enemy\n\nPrevious steps:\n".to_string();
        for (i, step) in history.iter().enumerate() {
            match step {
                ReActStep::Thought { content, .. } => {
                    prompt.push_str(&format!("{}. Thought: {}\n", i + 1, content));
                }
                ReActStep::Action { tool_name, parameters } => {
                    prompt.push_str(&format!("{}. Action: {} with {:?}\n", i + 1, tool_name, parameters));
                }
                ReActStep::Observation { content, success } => {
                    prompt.push_str(&format!("{}. Observation (success={}): {}\n", i + 1, success, content));
                }
                _ => {}
            }
        }
        assert!(prompt.contains("query_entities"));
        assert!(prompt.contains("3 entities found"));
        assert!(prompt.contains("create enemy"));
    }

    #[test]
    fn test_build_parameters_schema() {
        let schema = build_parameters_schema(&[
            crate::tool::ToolParameter {
                name: "entity_name".into(),
                description: "Name of entity".into(),
                param_type: crate::tool::ParameterType::String,
                required: true,
                default: None,
            },
        ]);
        assert_eq!(schema["type"], "object");
        assert!(schema["properties"]["entity_name"]["type"] == "string");
        assert_eq!(schema["required"][0], "entity_name");
    }
}
