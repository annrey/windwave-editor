use super::types::{
    LlmClient, LlmError, LlmMessage, LlmRequest, LlmResponse, Role, StreamCallback, ToolDefinition,
};

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

// =================================================================
// Convenience functions for streaming
// =================================================================

/// Send a streaming chat completion request using the given client.
/// Calls `on_chunk` for each streaming chunk, then returns the aggregated response.
pub async fn stream_chat_completion(
    client: &dyn LlmClient,
    request: LlmRequest,
    on_chunk: StreamCallback,
) -> Result<LlmResponse, LlmError> {
    client.chat_stream(request, on_chunk).await
}

/// Build a streaming chat request from a simple system prompt and user message.
pub fn build_stream_request(model: &str, system_prompt: &str, user_message: &str) -> LlmRequest {
    LlmRequest {
        model: model.to_string(),
        messages: vec![
            LlmMessage {
                role: Role::System,
                content: system_prompt.to_string(),
            },
            LlmMessage {
                role: Role::User,
                content: user_message.to_string(),
            },
        ],
        temperature: Some(0.7),
        max_tokens: Some(4096),
        tools: None,
    }
}
