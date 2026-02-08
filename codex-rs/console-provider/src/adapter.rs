use serde::Deserialize;
use serde::Serialize;
use std::collections::HashMap;

/// Identifies which wire protocol a provider uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WireProtocol {
    /// OpenAI Responses API (native codex-rs format).
    OpenAiResponses,
    /// Anthropic Messages API (needs translation).
    AnthropicMessages,
    /// OpenAI Chat Completions (OpenRouter, Azure, etc.).
    OpenAiChat,
}

/// Configuration for a Console v2 provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsoleProviderConfig {
    /// Provider display name.
    pub name: String,
    /// Which wire protocol this provider speaks.
    pub wire_protocol: WireProtocol,
    /// Base URL for API requests.
    pub base_url: String,
    /// Environment variable holding the API key, if any.
    pub env_key: Option<String>,
    /// Default model to use with this provider.
    pub default_model: Option<String>,
    /// Extra HTTP headers to include in requests.
    pub extra_headers: HashMap<String, String>,
}

/// Returns built-in provider configurations for Console v2.
pub fn built_in_providers() -> Vec<ConsoleProviderConfig> {
    vec![
        ConsoleProviderConfig {
            name: "Anthropic".into(),
            wire_protocol: WireProtocol::AnthropicMessages,
            base_url: "https://api.anthropic.com".into(),
            env_key: Some("ANTHROPIC_API_KEY".into()),
            default_model: Some("claude-sonnet-4-5-20250929".into()),
            extra_headers: HashMap::new(),
        },
        ConsoleProviderConfig {
            name: "OpenAI".into(),
            wire_protocol: WireProtocol::OpenAiResponses,
            base_url: "https://api.openai.com/v1".into(),
            env_key: Some("OPENAI_API_KEY".into()),
            default_model: Some("gpt-4o".into()),
            extra_headers: HashMap::new(),
        },
        ConsoleProviderConfig {
            name: "OpenRouter".into(),
            wire_protocol: WireProtocol::OpenAiChat,
            base_url: "https://openrouter.ai/api/v1".into(),
            env_key: Some("OPENROUTER_API_KEY".into()),
            default_model: None,
            extra_headers: HashMap::new(),
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn built_in_providers_returns_three_entries() {
        let providers = built_in_providers();
        assert_eq!(providers.len(), 3);

        let names: Vec<&str> = providers.iter().map(|p| p.name.as_str()).collect();
        assert!(names.contains(&"Anthropic"));
        assert!(names.contains(&"OpenAI"));
        assert!(names.contains(&"OpenRouter"));
    }

    #[test]
    fn wire_protocol_serialization_roundtrip() {
        let protocols = vec![
            WireProtocol::OpenAiResponses,
            WireProtocol::AnthropicMessages,
            WireProtocol::OpenAiChat,
        ];

        for proto in &protocols {
            let json = serde_json::to_string(proto).unwrap();
            let deserialized: WireProtocol = serde_json::from_str(&json).unwrap();
            assert_eq!(*proto, deserialized);
        }
    }

    #[test]
    fn wire_protocol_serializes_to_snake_case() {
        assert_eq!(
            serde_json::to_string(&WireProtocol::OpenAiResponses).unwrap(),
            "\"open_ai_responses\""
        );
        assert_eq!(
            serde_json::to_string(&WireProtocol::AnthropicMessages).unwrap(),
            "\"anthropic_messages\""
        );
        assert_eq!(
            serde_json::to_string(&WireProtocol::OpenAiChat).unwrap(),
            "\"open_ai_chat\""
        );
    }

    #[test]
    fn provider_config_serialization_roundtrip() {
        let config = ConsoleProviderConfig {
            name: "Test".into(),
            wire_protocol: WireProtocol::OpenAiChat,
            base_url: "https://example.com".into(),
            env_key: Some("TEST_KEY".into()),
            default_model: Some("test-model".into()),
            extra_headers: HashMap::from([("X-Custom".into(), "value".into())]),
        };

        let json = serde_json::to_string(&config).unwrap();
        let deserialized: ConsoleProviderConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.name, "Test");
        assert_eq!(deserialized.wire_protocol, WireProtocol::OpenAiChat);
        assert_eq!(deserialized.base_url, "https://example.com");
    }
}
