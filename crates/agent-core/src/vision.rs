//! Vision module - Interface for vision-capable LLMs
//!
//! Provides support for sending images to vision-capable LLMs
//! like GPT-4 Vision and Claude 3 Vision.

use serde::{Deserialize, Serialize};

/// A visual observation captured from the scene
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisualObservation {
    /// Base64 encoded PNG image
    pub image_base64: String,
    /// Image dimensions (width, height)
    pub dimensions: (u32, u32),
    /// Timestamp of capture (seconds since epoch or relative time)
    pub timestamp: f64,
    /// Optional description of what the image shows
    pub description: Option<String>,
}

/// Content item for multimodal messages (text or image)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum VisionContent {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "image_url")]
    ImageUrl { image_url: ImageUrl },
}

/// Image URL structure for vision API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageUrl {
    /// URL or base64 data URL
    pub url: String,
    /// Detail level: "low", "high", or "auto"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

/// Vision-capable message for LLM
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisionMessage {
    pub role: String,
    pub content: Vec<VisionContent>,
}

/// Vision request to LLM
#[derive(Debug, Clone, Serialize)]
pub struct VisionRequest {
    pub model: String,
    pub messages: Vec<VisionMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
}

/// Vision response from LLM
#[derive(Debug, Clone, Deserialize)]
pub struct VisionResponse {
    pub content: String,
    #[serde(default)]
    pub usage: VisionUsage,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct VisionUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

/// Convert a screenshot artifact to vision content
pub fn observation_to_content(observation: &VisualObservation) -> VisionContent {
    // Create data URL for base64 image
    let data_url = format!(
        "data:image/png;base64,{}",
        observation.image_base64
    );
    
    VisionContent::ImageUrl {
        image_url: ImageUrl {
            url: data_url,
            detail: Some("high".to_string()),
        },
    }
}

/// Create a vision message with text and optional image
pub fn create_vision_message(
    role: &str,
    text: &str,
    observation: Option<&VisualObservation>,
) -> VisionMessage {
    let mut content = vec![VisionContent::Text {
        text: text.to_string(),
    }];
    
    if let Some(obs) = observation {
        content.push(observation_to_content(obs));
    }
    
    VisionMessage {
        role: role.to_string(),
        content,
    }
}

/// Trait for vision-capable LLM clients
#[async_trait::async_trait]
pub trait VisionClient: Send + Sync {
    /// Send a vision request
    async fn vision(&self, request: VisionRequest) -> Result<VisionResponse, VisionError>;
    
    /// Check if vision is supported
    fn supports_vision(&self) -> bool;
}

/// Vision errors
#[derive(Debug, thiserror::Error)]
pub enum VisionError {
    #[error("Vision not supported by this model")]
    NotSupported,
    
    #[error("Invalid image format: {0}")]
    InvalidImage(String),
    
    #[error("API error: {0}")]
    ApiError(String),
    
    #[error("HTTP error: {0}")]
    HttpError(#[from] reqwest::Error),
}

/// Vision model capabilities
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VisionModel {
    Gpt4Vision,
    Claude3Sonnet,
    Claude3Opus,
    GeminiProVision,
}

impl VisionModel {
    pub fn api_name(&self) -> &'static str {
        match self {
            VisionModel::Gpt4Vision => "gpt-4o",
            VisionModel::Claude3Sonnet => "claude-sonnet-4-20250514",
            VisionModel::Claude3Opus => "claude-opus-4-20250514",
            VisionModel::GeminiProVision => "gemini-2.0-flash",
        }
    }
    
    pub fn supports_vision(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_create_vision_message_text_only() {
        let msg = create_vision_message("user", "What's in this image?", None);
        assert_eq!(msg.role, "user");
        assert_eq!(msg.content.len(), 1);
    }
    
    #[test]
    fn test_observation_to_content() {
        let obs = VisualObservation {
            image_base64: "iVBORw0KGgo=".to_string(),
            dimensions: (100, 100),
            timestamp: 0.0,
            description: Some("test".to_string()),
        };
        
        let content = observation_to_content(&obs);
        match content {
            VisionContent::ImageUrl { image_url } => {
                assert!(image_url.url.starts_with("data:image/png;base64,"));
                assert_eq!(image_url.detail, Some("high".to_string()));
            }
            _ => unreachable!("Test expects ImageUrl content, got: {:?}", content),
        }
    }
}
