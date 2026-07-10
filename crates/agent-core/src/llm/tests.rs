use super::*;

#[test]
fn test_llm_config_default() {
    let config = LlmConfig::default();
    assert_eq!(config.provider, LlmProvider::OpenAI);
    assert_eq!(config.model, models::OPENAI_FAST);
    assert!(config.api_key.is_empty());
}

#[test]
fn test_build_chat_request() {
    let messages = vec![LlmMessage {
        role: Role::User,
        content: "Hello".to_string(),
    }];
    let request = build_chat_request("gpt-4", messages);
    assert_eq!(request.model, "gpt-4");
    assert_eq!(request.messages.len(), 1);
    assert!(request.tools.is_none());
}
