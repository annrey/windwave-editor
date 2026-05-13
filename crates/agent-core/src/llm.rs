//! LLM Client - Interface for OpenAI and Claude APIs
//!
//! Provides a unified async interface for LLM interactions with support for
//! structured outputs, streaming, and tool calling.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Supported LLM providers
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LlmProvider {
    OpenAI,
    Claude,
    // Future: Local, Ollama, etc.
}

/// LLM configuration
#[derive(Clone)]
pub struct LlmConfig {
    pub provider: LlmProvider,
    pub api_key: String,
    pub model: String,
    pub base_url: Option<String>,
    pub max_tokens: u32,
    pub temperature: f32,
    pub timeout_secs: u64,
}

impl std::fmt::Debug for LlmConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LlmConfig")
            .field("provider", &self.provider)
            .field("api_key", &"[REDACTED]")
            .field("model", &self.model)
            .field("base_url", &self.base_url)
            .field("max_tokens", &self.max_tokens)
            .field("temperature", &self.temperature)
            .field("timeout_secs", &self.timeout_secs)
            .finish()
    }
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            provider: LlmProvider::OpenAI,
            api_key: String::new(),
            model: "gpt-4o-mini".to_string(),
            base_url: None,
            max_tokens: 4096,
            temperature: 0.7,
            timeout_secs: 60,
        }
    }
}

/// A message in the conversation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmMessage {
    pub role: Role,
    pub content: String,
}

/// Multimodal content block for vision requests
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ContentBlock {
    Text {
        #[serde(rename = "type")]
        content_type: String,
        text: String,
    },
    ImageUrl {
        #[serde(rename = "type")]
        content_type: String,
        image_url: ImageUrlBlock,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageUrlBlock {
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl ContentBlock {
    pub fn text(content: impl Into<String>) -> Self {
        ContentBlock::Text {
            content_type: "text".into(),
            text: content.into(),
        }
    }

    pub fn image_url(url: impl Into<String>) -> Self {
        ContentBlock::ImageUrl {
            content_type: "image_url".into(),
            image_url: ImageUrlBlock {
                url: url.into(),
                detail: Some("auto".into()),
            },
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
}

/// Request to the LLM
#[derive(Debug, Clone, Serialize)]
pub struct LlmRequest {
    pub model: String,
    pub messages: Vec<LlmMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<ToolDefinition>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
}

/// Tool definition for function calling
#[derive(Debug, Clone, Serialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

/// LLM response
#[derive(Debug, Clone, Deserialize)]
pub struct LlmResponse {
    pub content: String,
    pub tool_calls: Vec<ToolCall>,
    pub usage: TokenUsage,
}

/// Tool call from the LLM
#[derive(Debug, Clone, Deserialize)]
pub struct ToolCall {
    pub name: String,
    pub arguments: HashMap<String, serde_json::Value>,
}

/// Token usage information
#[derive(Debug, Clone, Default, Deserialize)]
pub struct TokenUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

/// LLM Client trait
#[async_trait::async_trait]
pub trait LlmClient: Send + Sync {
    /// Send a chat completion request
    async fn chat(&self, request: LlmRequest) -> Result<LlmResponse, LlmError>;

    /// Check if the client is configured properly
    fn is_ready(&self) -> bool;

    /// Get the provider name
    fn provider(&self) -> LlmProvider;

    /// Get a reference to the vision client, if supported.
    /// Returns None if this provider does not support vision.
    fn as_vision_client(&self) -> Option<&dyn crate::vision::VisionClient> {
        None
    }
}

/// LLM Errors
#[derive(Debug, thiserror::Error)]
pub enum LlmError {
    #[error("API key not configured")]
    MissingApiKey,
    
    #[error("HTTP error: {0}")]
    HttpError(#[from] reqwest::Error),
    
    #[error("API error: {0}")]
    ApiError(String),
    
    #[error("Invalid response: {0}")]
    InvalidResponse(String),
    
    #[error("Timeout")]
    Timeout,
}

/// OpenAI API Client
pub struct OpenAiClient {
    config: LlmConfig,
    client: reqwest::Client,
}

impl OpenAiClient {
    pub fn new(config: LlmConfig) -> Result<Self, LlmError> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(config.timeout_secs))
            .build()
            .map_err(LlmError::HttpError)?;
        
        if config.api_key.is_empty() {
            return Err(LlmError::MissingApiKey);
        }
        
        Ok(Self { config, client })
    }
}

#[async_trait::async_trait]
impl LlmClient for OpenAiClient {
    async fn chat(&self, request: LlmRequest) -> Result<LlmResponse, LlmError> {
        let url = format!(
            "{}/v1/chat/completions",
            self.config.base_url.as_deref().unwrap_or("https://api.openai.com")
        );
        
        let response = self.client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .json(&request)
            .send()
            .await
            .map_err(LlmError::HttpError)?;
        
        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(LlmError::ApiError(error_text));
        }
        
        let raw_response: OpenAiRawResponse = response
            .json()
            .await
            .map_err(|e| LlmError::InvalidResponse(e.to_string()))?;
        
        let choice = raw_response.choices.into_iter()
            .next()
            .ok_or_else(|| LlmError::InvalidResponse("No choices in response".to_string()))?;
        
        let tool_calls = choice.message.tool_calls.into_iter()
            .map(|tc| {
                let func_name = tc.function.name.clone();
                let args_str = tc.function.arguments.clone();
                ToolCall {
                    name: tc.function.name,
                    arguments: serde_json::from_str(&args_str)
                        .map_err(|e| {
                            log::warn!("Failed to parse tool call arguments: {} (function: {}, input: {})",
                                e, func_name, args_str);
                            e
                        })
                        .unwrap_or_default(),
                }
            })
            .collect();
        
        Ok(LlmResponse {
            content: choice.message.content.unwrap_or_default(),
            tool_calls,
            usage: raw_response.usage.unwrap_or_default(),
        })
    }
    
    fn is_ready(&self) -> bool {
        !self.config.api_key.is_empty()
    }
    
    fn provider(&self) -> LlmProvider {
        LlmProvider::OpenAI
    }

    fn as_vision_client(&self) -> Option<&dyn crate::vision::VisionClient> {
        Some(self)
    }
}

/// OpenAI API response structure
#[derive(Debug, Deserialize)]
struct OpenAiRawResponse {
    choices: Vec<OpenAiChoice>,
    usage: Option<TokenUsage>,
}

#[derive(Debug, Deserialize)]
struct OpenAiChoice {
    message: OpenAiMessage,
}

#[derive(Debug, Deserialize)]
struct OpenAiMessage {
    content: Option<String>,
    tool_calls: Vec<OpenAiToolCall>,
}

#[derive(Debug, Deserialize)]
struct OpenAiToolCall {
    function: OpenAiFunction,
}

#[derive(Debug, Deserialize)]
struct OpenAiFunction {
    name: String,
    arguments: String,
}

#[cfg(feature = "llm-claude")]
pub struct ClaudeClient {
    config: LlmConfig,
    client: reqwest::Client,
}

#[cfg(feature = "llm-claude")]
impl ClaudeClient {
    pub fn new(config: LlmConfig) -> Result<Self, LlmError> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(config.timeout_secs))
            .build()
            .map_err(LlmError::HttpError)?;

        if config.api_key.is_empty() {
            return Err(LlmError::MissingApiKey);
        }

        Ok(Self { config, client })
    }
}

#[cfg(feature = "llm-claude")]
#[async_trait::async_trait]
impl LlmClient for ClaudeClient {
    async fn chat(&self, request: LlmRequest) -> Result<LlmResponse, LlmError> {
        let url = format!(
            "{}/v1/messages",
            self.config.base_url.as_deref().unwrap_or("https://api.anthropic.com")
        );

        let mut system_prompt = String::new();
        let mut messages = Vec::new();
        for msg in request.messages {
            match msg.role {
                Role::System => {
                    if !system_prompt.is_empty() {
                        system_prompt.push('\n');
                    }
                    system_prompt.push_str(&msg.content);
                }
                Role::User | Role::Assistant => {
                    messages.push(ClaudeMessage {
                        role: match msg.role {
                            Role::User => "user".to_string(),
                            Role::Assistant => "assistant".to_string(),
                            _ => unreachable!(),
                        },
                        content: msg.content,
                    });
                }
            }
        }

        let claude_tools = request.tools.map(|tools| {
            tools
                .into_iter()
                .map(|t| ClaudeToolDef {
                    name: t.name,
                    description: t.description,
                    input_schema: t.parameters,
                })
                .collect()
        });

        let claude_request = ClaudeRequest {
            model: request.model,
            max_tokens: request.max_tokens.unwrap_or(self.config.max_tokens),
            system: if system_prompt.is_empty() {
                None
            } else {
                Some(system_prompt)
            },
            messages,
            tools: claude_tools,
            temperature: request.temperature,
        };

        let response = self
            .client
            .post(&url)
            .header("x-api-key", &self.config.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&claude_request)
            .send()
            .await
            .map_err(LlmError::HttpError)?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(LlmError::ApiError(error_text));
        }

        let raw_response: ClaudeRawResponse = response
            .json()
            .await
            .map_err(|e| LlmError::InvalidResponse(e.to_string()))?;

        let mut content = String::new();
        let mut tool_calls = Vec::new();

        for block in raw_response.content {
            match block.block_type.as_str() {
                "text" => {
                    if let Some(text) = block.text {
                        content.push_str(&text);
                    }
                }
                "tool_use" => {
                    if let (Some(name), Some(input)) = (block.name, block.input) {
                        tool_calls.push(ToolCall {
                            name,
                            arguments: input
                                .as_object()
                                .map(|obj| {
                                    obj.iter()
                                        .map(|(k, v)| (k.clone(), v.clone()))
                                        .collect()
                                })
                                .unwrap_or_default(),
                        });
                    }
                }
                _ => {}
            }
        }

        Ok(LlmResponse {
            content,
            tool_calls,
            usage: TokenUsage {
                prompt_tokens: raw_response.usage.input_tokens,
                completion_tokens: raw_response.usage.output_tokens,
                total_tokens: raw_response.usage.input_tokens
                    + raw_response.usage.output_tokens,
            },
        })
    }

    fn is_ready(&self) -> bool {
        !self.config.api_key.is_empty()
    }

    fn provider(&self) -> LlmProvider {
        LlmProvider::Claude
    }

    fn as_vision_client(&self) -> Option<&dyn crate::vision::VisionClient> {
        Some(self)
    }
}

#[cfg(feature = "llm-claude")]
#[derive(Debug, Serialize)]
struct ClaudeRequest {
    model: String,
    max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
    messages: Vec<ClaudeMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<ClaudeToolDef>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
}

#[cfg(feature = "llm-claude")]
#[derive(Debug, Serialize)]
struct ClaudeMessage {
    role: String,
    content: String,
}

#[cfg(feature = "llm-claude")]
#[derive(Debug, Serialize)]
struct ClaudeToolDef {
    name: String,
    description: String,
    input_schema: serde_json::Value,
}

#[cfg(feature = "llm-claude")]
#[derive(Debug, Deserialize)]
struct ClaudeRawResponse {
    content: Vec<ClaudeContentBlock>,
    usage: ClaudeUsage,
}

#[cfg(feature = "llm-claude")]
#[derive(Debug, Deserialize)]
struct ClaudeContentBlock {
    #[serde(rename = "type")]
    block_type: String,
    text: Option<String>,
    name: Option<String>,
    input: Option<serde_json::Value>,
}

#[cfg(feature = "llm-claude")]
#[derive(Debug, Deserialize)]
struct ClaudeUsage {
    input_tokens: u32,
    output_tokens: u32,
}

/// Configuration source for LLM
#[derive(Debug, Clone)]
pub enum LlmConfigSource {
    /// Use environment variables
    Env,
    /// Use explicit config
    Explicit(LlmConfig),
}

/// Create LLM configuration from environment variables
///
/// Environment variables:
/// - `OPENAI_API_KEY` - OpenAI API key
/// - `ANTHROPIC_API_KEY` - Anthropic API key
/// - `LLM_PROVIDER` - Provider: "openai" or "claude" (default: "openai")
/// - `LLM_MODEL` - Model name (default: "gpt-4o-mini" or "claude-3-sonnet")
/// - `LLM_BASE_URL` - Optional custom base URL
pub fn config_from_env() -> Option<LlmConfig> {
    let openai_key = std::env::var("OPENAI_API_KEY").ok();
    let anthropic_key = std::env::var("ANTHROPIC_API_KEY").ok();
    
    let provider = std::env::var("LLM_PROVIDER")
        .ok()
        .and_then(|p| match p.to_lowercase().as_str() {
            "openai" => Some(LlmProvider::OpenAI),
            "claude" | "anthropic" => Some(LlmProvider::Claude),
            _ => None,
        })
        .unwrap_or(LlmProvider::OpenAI);
    
    let (api_key, provider) = match provider {
        LlmProvider::OpenAI => {
            openai_key.map(|k| (k, LlmProvider::OpenAI))
                .or_else(|| anthropic_key.map(|k| (k, LlmProvider::Claude)))
        }
        LlmProvider::Claude => {
            anthropic_key.map(|k| (k, LlmProvider::Claude))
                .or_else(|| openai_key.map(|k| (k, LlmProvider::OpenAI)))
        }
    }?;
    
    let model = std::env::var("LLM_MODEL")
        .ok()
        .unwrap_or_else(|| match provider {
            LlmProvider::OpenAI => "gpt-4o-mini".to_string(),
            LlmProvider::Claude => "claude-3-sonnet-20240229".to_string(),
        });
    
    let base_url = std::env::var("LLM_BASE_URL").ok();
    let max_tokens = std::env::var("LLM_MAX_TOKENS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(4096);
    let temperature = std::env::var("LLM_TEMPERATURE")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0.7);
    let timeout_secs = std::env::var("LLM_TIMEOUT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(60);
    
    Some(LlmConfig {
        provider,
        api_key,
        model,
        base_url,
        max_tokens,
        temperature,
        timeout_secs,
    })
}

/// Check which API keys are available
pub fn check_api_keys() -> Vec<(LlmProvider, bool, String)> {
    vec![
        (
            LlmProvider::OpenAI,
            std::env::var("OPENAI_API_KEY").is_ok(),
            "OPENAI_API_KEY".to_string(),
        ),
        (
            LlmProvider::Claude,
            std::env::var("ANTHROPIC_API_KEY").is_ok(),
            "ANTHROPIC_API_KEY".to_string(),
        ),
    ]
}

/// Factory to create LLM clients
pub fn create_llm_client(config: LlmConfig) -> Result<Box<dyn LlmClient>, LlmError> {
    match config.provider {
        LlmProvider::OpenAI => Ok(Box::new(OpenAiClient::new(config)?)),
        LlmProvider::Claude => {
            #[cfg(feature = "llm-claude")]
            {
                Ok(Box::new(ClaudeClient::new(config)?))
            }
            #[cfg(not(feature = "llm-claude"))]
            {
                Err(LlmError::ApiError(
                    "Claude support not enabled (enable 'llm-claude' feature)".to_string(),
                ))
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Vision client implementation for OpenAI
// ---------------------------------------------------------------------------

#[async_trait::async_trait]
impl crate::vision::VisionClient for OpenAiClient {
    async fn vision(
        &self,
        request: crate::vision::VisionRequest,
    ) -> Result<crate::vision::VisionResponse, crate::vision::VisionError> {
        use crate::vision::{VisionError, VisionResponse};

        let url = format!(
            "{}/v1/chat/completions",
            self.config.base_url.as_deref().unwrap_or("https://api.openai.com")
        );

        let body = serde_json::json!({
            "model": request.model,
            "messages": request.messages,
            "max_tokens": request.max_tokens.unwrap_or(1024),
        });

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .json(&body)
            .send()
            .await
            .map_err(VisionError::HttpError)?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "unknown error".to_string());
            return Err(VisionError::ApiError(format!(
                "HTTP {}: {}",
                status, error_text
            )));
        }

        let json: serde_json::Value = response
            .json()
            .await
            .map_err(|e| VisionError::ApiError(format!("Failed to parse response: {}", e)))?;

        let content = json["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or("[no content]")
            .to_string();

        let usage = match json.get("usage") {
            Some(u) => crate::vision::VisionUsage {
                prompt_tokens: u["prompt_tokens"].as_u64().unwrap_or(0) as u32,
                completion_tokens: u["completion_tokens"].as_u64().unwrap_or(0) as u32,
                total_tokens: u["total_tokens"].as_u64().unwrap_or(0) as u32,
            },
            None => crate::vision::VisionUsage::default(),
        };

        Ok(VisionResponse { content, usage })
    }

    fn supports_vision(&self) -> bool {
        true
    }
}

#[cfg(feature = "llm-claude")]
#[async_trait::async_trait]
impl crate::vision::VisionClient for ClaudeClient {
    async fn vision(
        &self,
        request: crate::vision::VisionRequest,
    ) -> Result<crate::vision::VisionResponse, crate::vision::VisionError> {
        use crate::vision::{VisionError, VisionResponse};

        let url = format!(
            "{}/v1/messages",
            self.config.base_url.as_deref().unwrap_or("https://api.anthropic.com")
        );

        // Convert VisionMessage -> Claude's content block format
        let messages: Vec<serde_json::Value> = request.messages.iter().map(|msg| {
            let content_blocks: Vec<serde_json::Value> = msg.content.iter().map(|c| match c {
                crate::vision::VisionContent::Text { text } => serde_json::json!({
                    "type": "text", "text": text
                }),
                crate::vision::VisionContent::ImageUrl { image_url } => serde_json::json!({
                    "type": "image",
                    "source": {
                        "type": "base64",
                        "media_type": "image/png",
                        "data": image_url.url.replace("data:image/png;base64,", "")
                    }
                }),
            }).collect();

            serde_json::json!({
                "role": msg.role,
                "content": content_blocks
            })
        }).collect();

        let body = serde_json::json!({
            "model": request.model,
            "max_tokens": request.max_tokens.unwrap_or(1024),
            "messages": messages,
        });

        let response = self
            .client
            .post(&url)
            .header("x-api-key", &self.config.api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&body)
            .send()
            .await
            .map_err(VisionError::HttpError)?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "unknown error".to_string());
            return Err(VisionError::ApiError(format!("HTTP {}: {}", status, error_text)));
        }

        let json: serde_json::Value = response
            .json()
            .await
            .map_err(|e| VisionError::ApiError(format!("Failed to parse response: {}", e)))?;

        let content = json["content"][0]["text"]
            .as_str()
            .unwrap_or("[no content]")
            .to_string();

        let usage = match json.get("usage") {
            Some(u) => crate::vision::VisionUsage {
                prompt_tokens: u["input_tokens"].as_u64().unwrap_or(0) as u32,
                completion_tokens: u["output_tokens"].as_u64().unwrap_or(0) as u32,
                total_tokens: u["input_tokens"].as_u64().unwrap_or(0) as u32
                    + u["output_tokens"].as_u64().unwrap_or(0) as u32,
            },
            None => crate::vision::VisionUsage::default(),
        };

        Ok(VisionResponse { content, usage })
    }

    fn supports_vision(&self) -> bool {
        true
    }
}

/// Helper to build a simple chat request
pub fn build_chat_request(model: impl Into<String>, messages: Vec<LlmMessage>) -> LlmRequest {
    LlmRequest {
        model: model.into(),
        messages,
        tools: None,
        max_tokens: None,
        temperature: None,
    }
}

/// Helper to add tools to a request
pub fn with_tools(mut request: LlmRequest, tools: Vec<ToolDefinition>) -> LlmRequest {
    request.tools = Some(tools);
    request
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_llm_config_default() {
        let config = LlmConfig::default();
        assert_eq!(config.provider, LlmProvider::OpenAI);
        assert_eq!(config.model, "gpt-4o-mini");
        assert!(config.api_key.is_empty());
    }

    #[test]
    fn test_build_chat_request() {
        let messages = vec![
            LlmMessage {
                role: Role::User,
                content: "Hello".to_string(),
            },
        ];
        let request = build_chat_request("gpt-4", messages);
        assert_eq!(request.model, "gpt-4");
        assert_eq!(request.messages.len(), 1);
        assert!(request.tools.is_none());
    }
}
