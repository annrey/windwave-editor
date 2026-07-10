use crate::types::current_timestamp;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

// ============================================================================
// 第 2 层：视觉截图与 Vision LLM 分析
// ============================================================================

/// 截图 artifact
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenshotArtifact {
    pub path: PathBuf,
    pub width: u32,
    pub height: u32,
    pub timestamp: u64,
    pub format: String,              // "png", "jpg", etc.
    pub base64_data: Option<String>, // 可选：base64 编码的图像数据
}

impl ScreenshotArtifact {
    pub fn new(path: PathBuf, width: u32, height: u32) -> Self {
        Self {
            path,
            width,
            height,
            timestamp: current_timestamp(),
            format: "png".into(),
            base64_data: None,
        }
    }
}

/// 视觉观察结果 - Vision LLM 分析截图后的输出
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VisualObservation {
    /// 检测到的实体
    pub visible_entities: Vec<VisualEntity>,
    /// 检测到的异常
    pub anomalies: Vec<Anomaly>,
    /// 置信度 (0.0-1.0)
    pub confidence: f32,
    /// 原始 LLM 响应文本
    pub raw_response: Option<String>,
}

/// 视觉检测到的实体
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisualEntity {
    pub name: String,
    pub detected_type: String, // "player", "enemy", "npc", "environment"
    pub position: Option<[f32; 3]>,
    pub color: Option<[f32; 4]>,
    pub bounding_box: Option<[f32; 4]>, // [x, y, width, height]
    pub confidence: f32,
}

/// 检测到的异常
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Anomaly {
    pub description: String,
    pub severity: AnomalySeverity,
    pub location: Option<[f32; 3]>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AnomalySeverity {
    Low,
    Medium,
    High,
}

/// 视觉期望 - 用于验证操作结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VisualExpectation {
    EntityVisible(String),
    EntityColor(String, [f32; 4]),
    EntityPosition(String, [f32; 3]),
    EntityCount(usize),
    NoAnomalies,
    Custom(String),
}

/// Vision LLM 请求
#[derive(Debug, Clone)]
pub struct VisionRequest {
    pub model: String,
    pub messages: Vec<VisionMessage>,
    pub max_tokens: Option<u32>,
}

/// Vision LLM 消息
#[derive(Debug, Clone)]
pub struct VisionMessage {
    pub role: VisionRole,
    pub content: VisionContent,
}

#[derive(Debug, Clone)]
pub enum VisionRole {
    User,
    Assistant,
    System,
}

#[derive(Debug, Clone)]
pub enum VisionContent {
    Text(String),
    Image { data: Vec<u8>, format: String },
    MultiModal { text: String, image_data: Vec<u8> },
}

/// Vision LLM 响应
#[derive(Debug, Clone)]
pub struct VisionResponse {
    pub content: String,
    pub usage: VisionUsage,
}

#[derive(Debug, Clone)]
pub struct VisionUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

/// Vision LLM 客户端 trait
pub trait VisionClient: Send + Sync {
    fn vision(&self, request: VisionRequest) -> Result<VisionResponse, VisionError>;
}

/// Vision 错误
#[derive(Debug, thiserror::Error)]
pub enum VisionError {
    #[error("LLM error: {0}")]
    LlmError(String),
    #[error("Screenshot error: {0}")]
    ScreenshotError(String),
    #[error("Parse error: {0}")]
    ParseError(String),
    #[error("IO error: {0}")]
    IoError(String),
}
