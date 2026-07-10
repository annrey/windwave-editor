use super::types::{
    LlmClient, LlmConfig, LlmError, LlmProvider, LlmRequest, LlmResponse, Role, StreamCallback,
    StreamChunk, TokenUsage, ToolCall,
};
use serde::{Deserialize, Serialize};

#[cfg(feature = "llm-claude")]
pub struct ClaudeClient {
    pub(super) config: LlmConfig,
    pub(super) client: reqwest::Client,
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
            self.config
                .base_url
                .as_deref()
                .unwrap_or("https://api.anthropic.com")
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
                                    obj.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
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
                total_tokens: raw_response.usage.input_tokens + raw_response.usage.output_tokens,
            },
        })
    }

    async fn chat_stream(
        &self,
        request: LlmRequest,
        mut on_chunk: StreamCallback,
    ) -> Result<LlmResponse, LlmError> {
        // Claude streaming: fall back to chat() and deliver full response as single chunk
        let response = self.chat(request).await?;
        let chunk = StreamChunk {
            content: Some(response.content.clone()),
            done: true,
            accumulated: Some(response.content.clone()),
        };
        on_chunk(&chunk);
        Ok(response)
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
