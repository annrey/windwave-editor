use super::types::{
    LlmClient, LlmConfig, LlmError, LlmProvider, LlmRequest, LlmResponse, StreamCallback,
    StreamChunk, TokenUsage, ToolCall,
};
use serde::Deserialize;

/// OpenAI API Client
pub struct OpenAiClient {
    pub(super) config: LlmConfig,
    pub(super) client: reqwest::Client,
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
            self.config
                .base_url
                .as_deref()
                .unwrap_or("https://api.openai.com")
        );

        let response = self
            .client
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

        let choice = raw_response
            .choices
            .into_iter()
            .next()
            .ok_or_else(|| LlmError::InvalidResponse("No choices in response".to_string()))?;

        let tool_calls = choice
            .message
            .tool_calls
            .into_iter()
            .map(|tc| {
                let func_name = tc.function.name.clone();
                let args_str = tc.function.arguments.clone();
                ToolCall {
                    name: tc.function.name,
                    arguments: serde_json::from_str(&args_str)
                        .map_err(|e| {
                            log::warn!(
                                "Failed to parse tool call arguments: {} (function: {}, input: {})",
                                e,
                                func_name,
                                args_str
                            );
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

    async fn chat_stream(
        &self,
        request: LlmRequest,
        mut on_chunk: StreamCallback,
    ) -> Result<LlmResponse, LlmError> {
        let url = format!(
            "{}/v1/chat/completions",
            self.config
                .base_url
                .as_deref()
                .unwrap_or("https://api.openai.com")
        );

        // Clone the request and set stream=true
        let mut stream_request =
            serde_json::to_value(&request).map_err(|e| LlmError::InvalidResponse(e.to_string()))?;
        stream_request["stream"] = serde_json::Value::Bool(true);

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .json(&stream_request)
            .send()
            .await
            .map_err(LlmError::HttpError)?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(LlmError::ApiError(error_text));
        }

        let body = response.text().await.map_err(LlmError::HttpError)?;
        let mut accumulated_content = String::new();
        let mut final_usage: Option<TokenUsage> = None;

        for line in body.lines() {
            let line = line.trim();

            if line.is_empty() || line.starts_with(':') {
                continue;
            }

            if line == "data: [DONE]" {
                break;
            }

            if let Some(json_str) = line.strip_prefix("data: ") {
                match serde_json::from_str::<OpenAiStreamChunk>(json_str) {
                    Ok(chunk) => {
                        if let Some(usage) = chunk.usage {
                            final_usage = Some(usage);
                        }

                        for choice in &chunk.choices {
                            if let Some(content) = &choice.delta.content {
                                accumulated_content.push_str(content);

                                let stream_chunk = StreamChunk {
                                    content: Some(content.clone()),
                                    done: choice.finish_reason.as_deref() == Some("stop"),
                                    accumulated: Some(accumulated_content.clone()),
                                };

                                if !on_chunk(&stream_chunk) {
                                    return Ok(LlmResponse {
                                        content: accumulated_content,
                                        tool_calls: Vec::new(),
                                        usage: final_usage.unwrap_or_default(),
                                    });
                                }
                            }
                        }
                    }
                    Err(e) => {
                        log::debug!(
                            "Failed to parse SSE chunk: {} (input: {:.100})",
                            e,
                            json_str
                        );
                    }
                }
            }
        }

        // Send final done chunk
        let done_chunk = StreamChunk {
            content: None,
            done: true,
            accumulated: Some(accumulated_content.clone()),
        };
        on_chunk(&done_chunk);

        Ok(LlmResponse {
            content: accumulated_content,
            tool_calls: Vec::new(),
            usage: final_usage.unwrap_or_default(),
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

// ── Streaming helpers for OpenAI ──

/// Raw SSE chunk from OpenAI streaming response
#[derive(Debug, Deserialize)]
struct OpenAiStreamChunk {
    choices: Vec<OpenAiStreamChoice>,
    usage: Option<TokenUsage>,
}

#[derive(Debug, Deserialize)]
struct OpenAiStreamChoice {
    delta: OpenAiStreamDelta,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAiStreamDelta {
    content: Option<String>,
    #[allow(dead_code)]
    tool_calls: Option<Vec<OpenAiStreamToolCall>>,
}

#[derive(Debug, Deserialize)]
struct OpenAiStreamToolCall {
    #[allow(dead_code)]
    index: Option<u32>,
    #[allow(dead_code)]
    function: Option<OpenAiStreamFunction>,
}

#[derive(Debug, Deserialize)]
struct OpenAiStreamFunction {
    #[allow(dead_code)]
    name: Option<String>,
    #[allow(dead_code)]
    arguments: Option<String>,
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
