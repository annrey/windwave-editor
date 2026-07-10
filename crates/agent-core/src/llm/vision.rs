#[cfg(feature = "llm-openai")]
use super::openai::OpenAiClient;

// ---------------------------------------------------------------------------
// Vision client implementation for OpenAI
// ---------------------------------------------------------------------------

#[cfg(feature = "llm-openai")]
#[async_trait::async_trait]
impl crate::vision::VisionClient for OpenAiClient {
    async fn vision(
        &self,
        request: crate::vision::VisionRequest,
    ) -> Result<crate::vision::VisionResponse, crate::vision::VisionError> {
        use crate::vision::{VisionError, VisionResponse};

        let url = format!(
            "{}/v1/chat/completions",
            self.config
                .base_url
                .as_deref()
                .unwrap_or("https://api.openai.com")
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
impl crate::vision::VisionClient for super::claude::ClaudeClient {
    async fn vision(
        &self,
        request: crate::vision::VisionRequest,
    ) -> Result<crate::vision::VisionResponse, crate::vision::VisionError> {
        use crate::vision::{VisionError, VisionResponse};

        let url = format!(
            "{}/v1/messages",
            self.config
                .base_url
                .as_deref()
                .unwrap_or("https://api.anthropic.com")
        );

        // Convert VisionMessage -> Claude's content block format
        let messages: Vec<serde_json::Value> = request
            .messages
            .iter()
            .map(|msg| {
                let content_blocks: Vec<serde_json::Value> = msg
                    .content
                    .iter()
                    .map(|c| match c {
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
                    })
                    .collect();

                serde_json::json!({
                    "role": msg.role,
                    "content": content_blocks
                })
            })
            .collect();

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
            return Err(VisionError::ApiError(format!(
                "HTTP {}: {}",
                status, error_text
            )));
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
