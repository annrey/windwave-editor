//! LLM Client - Interface for OpenAI and Claude APIs
//!
//! Provides a unified async interface for LLM interactions with support for
//! structured outputs, streaming, and tool calling.

#[cfg(feature = "llm-claude")]
mod claude;
mod config;
mod helpers;
#[cfg(feature = "llm-openai")]
mod openai;
mod types;
mod vision;

#[cfg(test)]
mod tests;

// Re-export all public items from sub-modules
#[cfg(feature = "llm-claude")]
pub use claude::ClaudeClient;
pub use config::*;
pub use helpers::*;
#[cfg(feature = "llm-openai")]
pub use openai::OpenAiClient;
pub use types::*;
