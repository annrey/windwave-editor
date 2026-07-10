#[cfg(feature = "llm-openai")]
use super::openai::OpenAiClient;
use super::types::{models, LlmClient, LlmConfig, LlmError, LlmProvider};

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
/// - `LLM_MODEL` - Model name (default: "gpt-4o-mini" or "claude-sonnet-4")
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
        LlmProvider::OpenAI => openai_key
            .map(|k| (k, LlmProvider::OpenAI))
            .or_else(|| anthropic_key.map(|k| (k, LlmProvider::Claude))),
        LlmProvider::Claude => anthropic_key
            .map(|k| (k, LlmProvider::Claude))
            .or_else(|| openai_key.map(|k| (k, LlmProvider::OpenAI))),
    }?;

    let model = std::env::var("LLM_MODEL")
        .ok()
        .unwrap_or_else(|| match provider {
            LlmProvider::OpenAI => models::OPENAI_FAST.to_string(),
            LlmProvider::Claude => models::CLAUDE_DEFAULT.to_string(),
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
        LlmProvider::OpenAI => {
            #[cfg(feature = "llm-openai")]
            {
                Ok(Box::new(OpenAiClient::new(config)?))
            }
            #[cfg(not(feature = "llm-openai"))]
            {
                Err(LlmError::ApiError(
                    "OpenAI support not enabled (enable 'llm-openai' feature)".to_string(),
                ))
            }
        }
        LlmProvider::Claude => {
            #[cfg(feature = "llm-claude")]
            {
                Ok(Box::new(super::claude::ClaudeClient::new(config)?))
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
