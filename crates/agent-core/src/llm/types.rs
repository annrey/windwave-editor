//! Core types for the LLM interface

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Supported LLM providers
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum LlmProvider {
    OpenAI,
    Claude,
    // Future: Local, Ollama, etc.
}

/// Centralized model name constants (single source of truth).
///
/// Update these when new model versions are released.
pub mod models {
    /// Default OpenAI model (current as of 2026-05)
    pub const OPENAI_DEFAULT: &str = "gpt-4o";
    /// Lightweight OpenAI model for fast/cheap operations
    pub const OPENAI_FAST: &str = "gpt-4o-mini";
    /// Default Anthropic/Claude model
    pub const CLAUDE_DEFAULT: &str = "claude-sonnet-4-20250514";
    /// Legacy Claude model alias (for backward compat)
    pub const CLAUDE_LEGACY: &str = "claude-3-5-sonnet-20241022";

    /// Fallback chain when primary is unavailable
    pub const FALLBACK_MODELS: &[&str] = &[
        OPENAI_DEFAULT,
        "gpt-4.1",
        OPENAI_FAST,
        CLAUDE_DEFAULT,
        CLAUDE_LEGACY,
    ];
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

impl serde::Serialize for LlmConfig {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("LlmConfig", 7)?;
        state.serialize_field("provider", &self.provider)?;
        state.serialize_field("api_key", &"[REDACTED]")?;
        state.serialize_field("model", &self.model)?;
        state.serialize_field("base_url", &self.base_url)?;
        state.serialize_field("max_tokens", &self.max_tokens)?;
        state.serialize_field("temperature", &self.temperature)?;
        state.serialize_field("timeout_secs", &self.timeout_secs)?;
        state.end()
    }
}

impl<'de> serde::Deserialize<'de> for LlmConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct LlmConfigHelper {
            provider: LlmProvider,
            api_key: String,
            model: String,
            base_url: Option<String>,
            max_tokens: u32,
            temperature: f32,
            timeout_secs: u64,
        }

        let helper = LlmConfigHelper::deserialize(deserializer)?;
        Ok(LlmConfig {
            provider: helper.provider,
            api_key: helper.api_key,
            model: helper.model,
            base_url: helper.base_url,
            max_tokens: helper.max_tokens,
            temperature: helper.temperature,
            timeout_secs: helper.timeout_secs,
        })
    }
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            provider: LlmProvider::OpenAI,
            api_key: String::new(),
            model: models::OPENAI_FAST.to_string(),
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

/// Streaming chunk from an LLM response.
/// Represents a partial response delivered via SSE.
#[derive(Debug, Clone)]
pub struct StreamChunk {
    /// Delta text content (None for tool calls)
    pub content: Option<String>,
    /// If this is the final chunk in the stream
    pub done: bool,
    /// Accumulated content so far (useful for progressive rendering)
    pub accumulated: Option<String>,
}

/// Callback type for streaming events.
/// Returns true to continue streaming, false to abort early.
pub type StreamCallback = Box<dyn FnMut(&StreamChunk) -> bool + Send>;

/// LLM Client trait
#[async_trait::async_trait]
pub trait LlmClient: Send + Sync {
    /// Send a chat completion request
    async fn chat(&self, request: LlmRequest) -> Result<LlmResponse, LlmError>;

    /// Send a chat completion request with streaming.
    /// Each chunk is passed to the callback. Returns the aggregated full response.
    async fn chat_stream(
        &self,
        request: LlmRequest,
        on_chunk: StreamCallback,
    ) -> Result<LlmResponse, LlmError>;

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

    #[cfg(feature = "reqwest")]
    #[error("HTTP error: {0}")]
    HttpError(#[from] reqwest::Error),

    #[cfg(not(feature = "reqwest"))]
    #[error("HTTP error: {0}")]
    HttpError(String),

    #[error("API error: {0}")]
    ApiError(String),

    #[error("Invalid response: {0}")]
    InvalidResponse(String),

    #[error("Timeout")]
    Timeout,
}
