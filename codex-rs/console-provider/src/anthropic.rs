//! Anthropic Messages API translation layer.
//!
//! This module contains data types and pure functions for converting between
//! codex-rs internal formats and the Anthropic Messages API wire format
//! (<https://docs.anthropic.com/en/api/messages>).
//!
//! **No HTTP, no async, no IO** — only types and translation helpers.

use serde::{Deserialize, Serialize};

use crate::error::ProviderError;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn default_true() -> bool {
    true
}

// ---------------------------------------------------------------------------
// Request types
// ---------------------------------------------------------------------------

/// Top-level request body for the Anthropic Messages API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnthropicRequest {
    /// Model identifier, e.g. `"claude-sonnet-4-20250514"`.
    pub model: String,

    /// Conversation turns.
    pub messages: Vec<AnthropicMessage>,

    /// Optional system prompt (sent outside the messages array).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<String>,

    /// Maximum number of tokens to generate.
    pub max_tokens: u32,

    /// Tool definitions available to the model.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<AnthropicTool>>,

    /// Whether to stream the response. Defaults to `true`.
    #[serde(default = "default_true")]
    pub stream: bool,

    /// Sampling temperature.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,

    /// How the model should choose tools.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<AnthropicToolChoice>,
}

/// A single message in the conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnthropicMessage {
    /// `"user"` or `"assistant"`.
    pub role: String,

    /// Content blocks that make up the message.
    pub content: Vec<AnthropicContentBlock>,
}

/// A content block inside a message. Anthropic uses a `type` tag to
/// discriminate between text, tool-use, and tool-result blocks.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum AnthropicContentBlock {
    /// Plain text content.
    #[serde(rename = "text")]
    Text { text: String },

    /// The model is invoking a tool.
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },

    /// The result of a prior tool invocation, sent back by the caller.
    #[serde(rename = "tool_result")]
    ToolResult {
        tool_use_id: String,
        content: String,
    },
}

/// Definition of a tool the model may call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnthropicTool {
    /// The tool name (must match `[a-zA-Z0-9_-]+`).
    pub name: String,

    /// Human-readable description shown to the model.
    pub description: String,

    /// JSON Schema describing the tool's input parameters.
    pub input_schema: serde_json::Value,
}

/// Controls how the model selects tools.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum AnthropicToolChoice {
    /// Let the model decide whether to call a tool.
    #[serde(rename = "auto")]
    Auto,

    /// Force the model to call *some* tool (any).
    #[serde(rename = "any")]
    Any,

    /// Force the model to call a specific tool.
    #[serde(rename = "tool")]
    Tool { name: String },
}

// ---------------------------------------------------------------------------
// Streaming event types
// ---------------------------------------------------------------------------

/// Anthropic SSE event types for streaming responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum AnthropicStreamEvent {
    /// First event — contains the message skeleton and initial usage.
    #[serde(rename = "message_start")]
    MessageStart { message: AnthropicStreamMessage },

    /// A new content block is starting at `index`.
    #[serde(rename = "content_block_start")]
    ContentBlockStart {
        index: usize,
        content_block: AnthropicContentBlock,
    },

    /// Incremental update to the content block at `index`.
    #[serde(rename = "content_block_delta")]
    ContentBlockDelta {
        index: usize,
        delta: AnthropicDelta,
    },

    /// The content block at `index` is complete.
    #[serde(rename = "content_block_stop")]
    ContentBlockStop { index: usize },

    /// Final update — carries the stop reason and output-token usage.
    #[serde(rename = "message_delta")]
    MessageDelta {
        delta: AnthropicMessageDelta,
        usage: Option<AnthropicDeltaUsage>,
    },

    /// The message is fully complete.
    #[serde(rename = "message_stop")]
    MessageStop,

    /// Keep-alive ping.
    #[serde(rename = "ping")]
    Ping,

    /// An error occurred during streaming.
    #[serde(rename = "error")]
    Error { error: AnthropicApiError },
}

/// The message object delivered inside [`AnthropicStreamEvent::MessageStart`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnthropicStreamMessage {
    pub id: String,
    pub model: String,
    pub role: String,
    pub usage: AnthropicUsage,
}

/// Token usage counters (including prompt-caching fields).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnthropicUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,

    /// Tokens written to the prompt cache on this request.
    #[serde(default)]
    pub cache_creation_input_tokens: u64,

    /// Tokens read from the prompt cache on this request.
    #[serde(default)]
    pub cache_read_input_tokens: u64,
}

/// Incremental delta inside a content block.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum AnthropicDelta {
    /// Incremental text chunk.
    #[serde(rename = "text_delta")]
    TextDelta { text: String },

    /// Incremental JSON fragment for a tool-use input.
    #[serde(rename = "input_json_delta")]
    InputJsonDelta { partial_json: String },
}

/// Delta payload inside [`AnthropicStreamEvent::MessageDelta`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnthropicMessageDelta {
    /// Why the model stopped, e.g. `"end_turn"`, `"tool_use"`, `"max_tokens"`.
    pub stop_reason: Option<String>,
}

/// Output-token count delivered alongside the message delta.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnthropicDeltaUsage {
    pub output_tokens: u64,
}

/// Error body returned by the Anthropic API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnthropicApiError {
    /// Machine-readable error type, e.g. `"rate_limit_error"`.
    #[serde(rename = "type")]
    pub error_type: String,

    /// Human-readable description.
    pub message: String,
}

// ---------------------------------------------------------------------------
// Error classification
// ---------------------------------------------------------------------------

/// Classify an [`AnthropicApiError`] into the crate-level [`ProviderError`].
pub fn classify_anthropic_error(error: &AnthropicApiError) -> ProviderError {
    match error.error_type.as_str() {
        "overloaded_error" => {
            ProviderError::ApiError(format!("overloaded: {}", error.message))
        }
        "rate_limit_error" => {
            ProviderError::ApiError(format!("rate limited: {}", error.message))
        }
        "invalid_request_error" => ProviderError::InvalidConfig(error.message.clone()),
        "authentication_error" => {
            ProviderError::InvalidConfig(format!("auth: {}", error.message))
        }
        "not_found_error" => ProviderError::UnsupportedProvider(error.message.clone()),
        other => ProviderError::Other(format!("{}: {}", other, error.message)),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // -- AnthropicRequest roundtrip ----------------------------------------

    #[test]
    fn request_roundtrip_minimal() {
        let req = AnthropicRequest {
            model: "claude-sonnet-4-20250514".into(),
            messages: vec![AnthropicMessage {
                role: "user".into(),
                content: vec![AnthropicContentBlock::Text {
                    text: "Hello".into(),
                }],
            }],
            system: None,
            max_tokens: 1024,
            tools: None,
            stream: true,
            temperature: None,
            tool_choice: None,
        };

        let json_str = serde_json::to_string(&req).unwrap();
        let roundtripped: AnthropicRequest = serde_json::from_str(&json_str).unwrap();

        assert_eq!(roundtripped.model, "claude-sonnet-4-20250514");
        assert_eq!(roundtripped.max_tokens, 1024);
        assert!(roundtripped.stream);
        assert!(roundtripped.system.is_none());
        assert!(roundtripped.tools.is_none());
        assert!(roundtripped.temperature.is_none());
        assert!(roundtripped.tool_choice.is_none());
        assert_eq!(roundtripped.messages.len(), 1);
        assert_eq!(roundtripped.messages[0].role, "user");
    }

    #[test]
    fn request_roundtrip_full() {
        let req = AnthropicRequest {
            model: "claude-sonnet-4-20250514".into(),
            messages: vec![
                AnthropicMessage {
                    role: "user".into(),
                    content: vec![AnthropicContentBlock::Text {
                        text: "What files are here?".into(),
                    }],
                },
                AnthropicMessage {
                    role: "assistant".into(),
                    content: vec![AnthropicContentBlock::ToolUse {
                        id: "tu_01".into(),
                        name: "list_files".into(),
                        input: json!({"path": "."}),
                    }],
                },
                AnthropicMessage {
                    role: "user".into(),
                    content: vec![AnthropicContentBlock::ToolResult {
                        tool_use_id: "tu_01".into(),
                        content: "main.rs\nlib.rs".into(),
                    }],
                },
            ],
            system: Some("You are a coding assistant.".into()),
            max_tokens: 4096,
            tools: Some(vec![AnthropicTool {
                name: "list_files".into(),
                description: "List files in a directory".into(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "path": { "type": "string" }
                    },
                    "required": ["path"]
                }),
            }]),
            stream: false,
            temperature: Some(0.7),
            tool_choice: Some(AnthropicToolChoice::Auto),
        };

        let json_str = serde_json::to_string_pretty(&req).unwrap();
        let roundtripped: AnthropicRequest = serde_json::from_str(&json_str).unwrap();

        assert_eq!(roundtripped.model, "claude-sonnet-4-20250514");
        assert_eq!(roundtripped.max_tokens, 4096);
        assert!(!roundtripped.stream);
        assert_eq!(
            roundtripped.system.as_deref(),
            Some("You are a coding assistant.")
        );
        assert_eq!(roundtripped.messages.len(), 3);
        assert_eq!(roundtripped.temperature, Some(0.7));
        assert!(roundtripped.tools.is_some());
        assert_eq!(roundtripped.tools.as_ref().unwrap().len(), 1);
    }

    #[test]
    fn request_omits_none_fields() {
        let req = AnthropicRequest {
            model: "claude-sonnet-4-20250514".into(),
            messages: vec![],
            system: None,
            max_tokens: 100,
            tools: None,
            stream: true,
            temperature: None,
            tool_choice: None,
        };

        let val: serde_json::Value = serde_json::to_value(&req).unwrap();
        let obj = val.as_object().unwrap();

        // These fields should be absent when None.
        assert!(!obj.contains_key("system"));
        assert!(!obj.contains_key("tools"));
        assert!(!obj.contains_key("temperature"));
        assert!(!obj.contains_key("tool_choice"));

        // These should always be present.
        assert!(obj.contains_key("model"));
        assert!(obj.contains_key("messages"));
        assert!(obj.contains_key("max_tokens"));
        assert!(obj.contains_key("stream"));
    }

    // -- ContentBlock serialization ----------------------------------------

    #[test]
    fn content_block_text_serialization() {
        let block = AnthropicContentBlock::Text {
            text: "hello world".into(),
        };
        let val = serde_json::to_value(&block).unwrap();
        assert_eq!(val["type"], "text");
        assert_eq!(val["text"], "hello world");
    }

    #[test]
    fn content_block_tool_use_serialization() {
        let block = AnthropicContentBlock::ToolUse {
            id: "call_123".into(),
            name: "read_file".into(),
            input: json!({"path": "/tmp/foo.txt"}),
        };
        let val = serde_json::to_value(&block).unwrap();
        assert_eq!(val["type"], "tool_use");
        assert_eq!(val["id"], "call_123");
        assert_eq!(val["name"], "read_file");
        assert_eq!(val["input"]["path"], "/tmp/foo.txt");
    }

    #[test]
    fn content_block_tool_result_serialization() {
        let block = AnthropicContentBlock::ToolResult {
            tool_use_id: "call_123".into(),
            content: "file contents here".into(),
        };
        let val = serde_json::to_value(&block).unwrap();
        assert_eq!(val["type"], "tool_result");
        assert_eq!(val["tool_use_id"], "call_123");
        assert_eq!(val["content"], "file contents here");
    }

    #[test]
    fn content_block_roundtrip_from_json() {
        let text_json = r#"{"type":"text","text":"hi"}"#;
        let block: AnthropicContentBlock = serde_json::from_str(text_json).unwrap();
        match &block {
            AnthropicContentBlock::Text { text } => assert_eq!(text, "hi"),
            _ => panic!("expected Text variant"),
        }

        let tool_use_json =
            r#"{"type":"tool_use","id":"tu_1","name":"grep","input":{"q":"foo"}}"#;
        let block: AnthropicContentBlock = serde_json::from_str(tool_use_json).unwrap();
        match &block {
            AnthropicContentBlock::ToolUse { id, name, input } => {
                assert_eq!(id, "tu_1");
                assert_eq!(name, "grep");
                assert_eq!(input["q"], "foo");
            }
            _ => panic!("expected ToolUse variant"),
        }

        let tool_result_json =
            r#"{"type":"tool_result","tool_use_id":"tu_1","content":"result"}"#;
        let block: AnthropicContentBlock = serde_json::from_str(tool_result_json).unwrap();
        match &block {
            AnthropicContentBlock::ToolResult {
                tool_use_id,
                content,
            } => {
                assert_eq!(tool_use_id, "tu_1");
                assert_eq!(content, "result");
            }
            _ => panic!("expected ToolResult variant"),
        }
    }

    // -- ToolChoice serialization ------------------------------------------

    #[test]
    fn tool_choice_auto_serialization() {
        let tc = AnthropicToolChoice::Auto;
        let val = serde_json::to_value(&tc).unwrap();
        assert_eq!(val["type"], "auto");
    }

    #[test]
    fn tool_choice_any_serialization() {
        let tc = AnthropicToolChoice::Any;
        let val = serde_json::to_value(&tc).unwrap();
        assert_eq!(val["type"], "any");
    }

    #[test]
    fn tool_choice_tool_serialization() {
        let tc = AnthropicToolChoice::Tool {
            name: "read_file".into(),
        };
        let val = serde_json::to_value(&tc).unwrap();
        assert_eq!(val["type"], "tool");
        assert_eq!(val["name"], "read_file");
    }

    #[test]
    fn tool_choice_roundtrip() {
        for json_str in &[
            r#"{"type":"auto"}"#,
            r#"{"type":"any"}"#,
            r#"{"type":"tool","name":"bash"}"#,
        ] {
            let tc: AnthropicToolChoice = serde_json::from_str(json_str).unwrap();
            let reserialized = serde_json::to_string(&tc).unwrap();
            let reparsed: serde_json::Value = serde_json::from_str(&reserialized).unwrap();
            let original: serde_json::Value = serde_json::from_str(json_str).unwrap();
            assert_eq!(reparsed, original);
        }
    }

    // -- Tool definition serialization -------------------------------------

    #[test]
    fn tool_definition_serialization() {
        let tool = AnthropicTool {
            name: "execute_command".into(),
            description: "Run a shell command".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "The command to run"
                    },
                    "timeout": {
                        "type": "integer",
                        "description": "Timeout in seconds"
                    }
                },
                "required": ["command"]
            }),
        };

        let val = serde_json::to_value(&tool).unwrap();
        assert_eq!(val["name"], "execute_command");
        assert_eq!(val["description"], "Run a shell command");
        assert_eq!(val["input_schema"]["type"], "object");
        assert_eq!(
            val["input_schema"]["properties"]["command"]["type"],
            "string"
        );
        assert_eq!(val["input_schema"]["required"][0], "command");
    }

    #[test]
    fn tool_definition_roundtrip() {
        let tool = AnthropicTool {
            name: "search".into(),
            description: "Search codebase".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string" }
                },
                "required": ["query"]
            }),
        };

        let json_str = serde_json::to_string(&tool).unwrap();
        let roundtripped: AnthropicTool = serde_json::from_str(&json_str).unwrap();
        assert_eq!(roundtripped.name, "search");
        assert_eq!(roundtripped.description, "Search codebase");
        assert_eq!(roundtripped.input_schema["type"], "object");
    }

    // -- Streaming event parsing -------------------------------------------

    #[test]
    fn parse_message_start_event() {
        let json_str = r#"{
            "type": "message_start",
            "message": {
                "id": "msg_01XFDUDYJgAACzvnptvVoYEL",
                "model": "claude-sonnet-4-20250514",
                "role": "assistant",
                "usage": {
                    "input_tokens": 25,
                    "output_tokens": 1
                }
            }
        }"#;

        let event: AnthropicStreamEvent = serde_json::from_str(json_str).unwrap();
        match event {
            AnthropicStreamEvent::MessageStart { message } => {
                assert_eq!(message.id, "msg_01XFDUDYJgAACzvnptvVoYEL");
                assert_eq!(message.model, "claude-sonnet-4-20250514");
                assert_eq!(message.role, "assistant");
                assert_eq!(message.usage.input_tokens, 25);
                assert_eq!(message.usage.output_tokens, 1);
                assert_eq!(message.usage.cache_creation_input_tokens, 0);
                assert_eq!(message.usage.cache_read_input_tokens, 0);
            }
            _ => panic!("expected MessageStart"),
        }
    }

    #[test]
    fn parse_message_start_event_with_cache_tokens() {
        let json_str = r#"{
            "type": "message_start",
            "message": {
                "id": "msg_cache",
                "model": "claude-sonnet-4-20250514",
                "role": "assistant",
                "usage": {
                    "input_tokens": 100,
                    "output_tokens": 0,
                    "cache_creation_input_tokens": 2000,
                    "cache_read_input_tokens": 500
                }
            }
        }"#;

        let event: AnthropicStreamEvent = serde_json::from_str(json_str).unwrap();
        match event {
            AnthropicStreamEvent::MessageStart { message } => {
                assert_eq!(message.usage.input_tokens, 100);
                assert_eq!(message.usage.cache_creation_input_tokens, 2000);
                assert_eq!(message.usage.cache_read_input_tokens, 500);
            }
            _ => panic!("expected MessageStart"),
        }
    }

    #[test]
    fn parse_content_block_start_text() {
        let json_str = r#"{
            "type": "content_block_start",
            "index": 0,
            "content_block": {
                "type": "text",
                "text": ""
            }
        }"#;

        let event: AnthropicStreamEvent = serde_json::from_str(json_str).unwrap();
        match event {
            AnthropicStreamEvent::ContentBlockStart {
                index,
                content_block,
            } => {
                assert_eq!(index, 0);
                match content_block {
                    AnthropicContentBlock::Text { text } => assert_eq!(text, ""),
                    _ => panic!("expected Text block"),
                }
            }
            _ => panic!("expected ContentBlockStart"),
        }
    }

    #[test]
    fn parse_content_block_start_tool_use() {
        let json_str = r#"{
            "type": "content_block_start",
            "index": 1,
            "content_block": {
                "type": "tool_use",
                "id": "toolu_01A09q90qw90lq917835lqs8",
                "name": "get_weather",
                "input": {}
            }
        }"#;

        let event: AnthropicStreamEvent = serde_json::from_str(json_str).unwrap();
        match event {
            AnthropicStreamEvent::ContentBlockStart {
                index,
                content_block,
            } => {
                assert_eq!(index, 1);
                match content_block {
                    AnthropicContentBlock::ToolUse { id, name, input } => {
                        assert_eq!(id, "toolu_01A09q90qw90lq917835lqs8");
                        assert_eq!(name, "get_weather");
                        assert!(input.is_object());
                    }
                    _ => panic!("expected ToolUse block"),
                }
            }
            _ => panic!("expected ContentBlockStart"),
        }
    }

    #[test]
    fn parse_text_delta() {
        let json_str = r#"{
            "type": "content_block_delta",
            "index": 0,
            "delta": {
                "type": "text_delta",
                "text": "Hello"
            }
        }"#;

        let event: AnthropicStreamEvent = serde_json::from_str(json_str).unwrap();
        match event {
            AnthropicStreamEvent::ContentBlockDelta { index, delta } => {
                assert_eq!(index, 0);
                match delta {
                    AnthropicDelta::TextDelta { text } => assert_eq!(text, "Hello"),
                    _ => panic!("expected TextDelta"),
                }
            }
            _ => panic!("expected ContentBlockDelta"),
        }
    }

    #[test]
    fn parse_input_json_delta() {
        let json_str = r#"{
            "type": "content_block_delta",
            "index": 1,
            "delta": {
                "type": "input_json_delta",
                "partial_json": "{\"location\": \"San"
            }
        }"#;

        let event: AnthropicStreamEvent = serde_json::from_str(json_str).unwrap();
        match event {
            AnthropicStreamEvent::ContentBlockDelta { index, delta } => {
                assert_eq!(index, 1);
                match delta {
                    AnthropicDelta::InputJsonDelta { partial_json } => {
                        assert_eq!(partial_json, r#"{"location": "San"#);
                    }
                    _ => panic!("expected InputJsonDelta"),
                }
            }
            _ => panic!("expected ContentBlockDelta"),
        }
    }

    #[test]
    fn parse_content_block_stop() {
        let json_str = r#"{"type": "content_block_stop", "index": 0}"#;
        let event: AnthropicStreamEvent = serde_json::from_str(json_str).unwrap();
        match event {
            AnthropicStreamEvent::ContentBlockStop { index } => assert_eq!(index, 0),
            _ => panic!("expected ContentBlockStop"),
        }
    }

    #[test]
    fn parse_message_delta() {
        let json_str = r#"{
            "type": "message_delta",
            "delta": {
                "stop_reason": "end_turn"
            },
            "usage": {
                "output_tokens": 15
            }
        }"#;

        let event: AnthropicStreamEvent = serde_json::from_str(json_str).unwrap();
        match event {
            AnthropicStreamEvent::MessageDelta { delta, usage } => {
                assert_eq!(delta.stop_reason.as_deref(), Some("end_turn"));
                let usage = usage.unwrap();
                assert_eq!(usage.output_tokens, 15);
            }
            _ => panic!("expected MessageDelta"),
        }
    }

    #[test]
    fn parse_message_delta_tool_use_stop() {
        let json_str = r#"{
            "type": "message_delta",
            "delta": {
                "stop_reason": "tool_use"
            },
            "usage": {
                "output_tokens": 42
            }
        }"#;

        let event: AnthropicStreamEvent = serde_json::from_str(json_str).unwrap();
        match event {
            AnthropicStreamEvent::MessageDelta { delta, usage } => {
                assert_eq!(delta.stop_reason.as_deref(), Some("tool_use"));
                assert_eq!(usage.unwrap().output_tokens, 42);
            }
            _ => panic!("expected MessageDelta"),
        }
    }

    #[test]
    fn parse_message_stop() {
        let json_str = r#"{"type": "message_stop"}"#;
        let event: AnthropicStreamEvent = serde_json::from_str(json_str).unwrap();
        assert!(matches!(event, AnthropicStreamEvent::MessageStop));
    }

    #[test]
    fn parse_ping() {
        let json_str = r#"{"type": "ping"}"#;
        let event: AnthropicStreamEvent = serde_json::from_str(json_str).unwrap();
        assert!(matches!(event, AnthropicStreamEvent::Ping));
    }

    #[test]
    fn parse_error_event() {
        let json_str = r#"{
            "type": "error",
            "error": {
                "type": "overloaded_error",
                "message": "Overloaded"
            }
        }"#;

        let event: AnthropicStreamEvent = serde_json::from_str(json_str).unwrap();
        match event {
            AnthropicStreamEvent::Error { error } => {
                assert_eq!(error.error_type, "overloaded_error");
                assert_eq!(error.message, "Overloaded");
            }
            _ => panic!("expected Error"),
        }
    }

    // -- Error classification ----------------------------------------------

    #[test]
    fn classify_overloaded_error() {
        let err = AnthropicApiError {
            error_type: "overloaded_error".into(),
            message: "API is overloaded".into(),
        };
        let classified = classify_anthropic_error(&err);
        match classified {
            ProviderError::ApiError(msg) => {
                assert!(msg.contains("overloaded"));
                assert!(msg.contains("API is overloaded"));
            }
            _ => panic!("expected ApiError"),
        }
    }

    #[test]
    fn classify_rate_limit_error() {
        let err = AnthropicApiError {
            error_type: "rate_limit_error".into(),
            message: "Too many requests".into(),
        };
        let classified = classify_anthropic_error(&err);
        match classified {
            ProviderError::ApiError(msg) => {
                assert!(msg.contains("rate limited"));
                assert!(msg.contains("Too many requests"));
            }
            _ => panic!("expected ApiError"),
        }
    }

    #[test]
    fn classify_invalid_request_error() {
        let err = AnthropicApiError {
            error_type: "invalid_request_error".into(),
            message: "max_tokens must be positive".into(),
        };
        let classified = classify_anthropic_error(&err);
        match classified {
            ProviderError::InvalidConfig(msg) => {
                assert_eq!(msg, "max_tokens must be positive");
            }
            _ => panic!("expected InvalidConfig"),
        }
    }

    #[test]
    fn classify_authentication_error() {
        let err = AnthropicApiError {
            error_type: "authentication_error".into(),
            message: "Invalid API key".into(),
        };
        let classified = classify_anthropic_error(&err);
        match classified {
            ProviderError::InvalidConfig(msg) => {
                assert!(msg.contains("auth"));
                assert!(msg.contains("Invalid API key"));
            }
            _ => panic!("expected InvalidConfig"),
        }
    }

    #[test]
    fn classify_not_found_error() {
        let err = AnthropicApiError {
            error_type: "not_found_error".into(),
            message: "Model not found".into(),
        };
        let classified = classify_anthropic_error(&err);
        match classified {
            ProviderError::UnsupportedProvider(msg) => {
                assert_eq!(msg, "Model not found");
            }
            _ => panic!("expected UnsupportedProvider"),
        }
    }

    #[test]
    fn classify_unknown_error() {
        let err = AnthropicApiError {
            error_type: "server_error".into(),
            message: "Internal failure".into(),
        };
        let classified = classify_anthropic_error(&err);
        match classified {
            ProviderError::Other(msg) => {
                assert!(msg.contains("server_error"));
                assert!(msg.contains("Internal failure"));
            }
            _ => panic!("expected Other"),
        }
    }

    // -- AnthropicStreamEvent serialization roundtrip -----------------------

    #[test]
    fn stream_event_serialization_roundtrip() {
        let events = vec![
            AnthropicStreamEvent::Ping,
            AnthropicStreamEvent::MessageStop,
            AnthropicStreamEvent::ContentBlockStop { index: 2 },
            AnthropicStreamEvent::ContentBlockDelta {
                index: 0,
                delta: AnthropicDelta::TextDelta {
                    text: "hi".into(),
                },
            },
            AnthropicStreamEvent::MessageDelta {
                delta: AnthropicMessageDelta {
                    stop_reason: Some("end_turn".into()),
                },
                usage: Some(AnthropicDeltaUsage { output_tokens: 10 }),
            },
            AnthropicStreamEvent::Error {
                error: AnthropicApiError {
                    error_type: "rate_limit_error".into(),
                    message: "slow down".into(),
                },
            },
        ];

        for event in &events {
            let json_str = serde_json::to_string(event).unwrap();
            let reparsed: AnthropicStreamEvent = serde_json::from_str(&json_str).unwrap();
            // Re-serialize to verify structural equality.
            let json_str2 = serde_json::to_string(&reparsed).unwrap();
            assert_eq!(json_str, json_str2);
        }
    }

    // -- default_true helper -----------------------------------------------

    #[test]
    fn stream_defaults_to_true_when_missing() {
        let json_str = r#"{
            "model": "claude-sonnet-4-20250514",
            "messages": [],
            "max_tokens": 100
        }"#;
        let req: AnthropicRequest = serde_json::from_str(json_str).unwrap();
        assert!(req.stream);
    }

    #[test]
    fn stream_respects_explicit_false() {
        let json_str = r#"{
            "model": "claude-sonnet-4-20250514",
            "messages": [],
            "max_tokens": 100,
            "stream": false
        }"#;
        let req: AnthropicRequest = serde_json::from_str(json_str).unwrap();
        assert!(!req.stream);
    }

    // -- Usage defaults ----------------------------------------------------

    #[test]
    fn usage_cache_fields_default_to_zero() {
        let json_str = r#"{"input_tokens": 10, "output_tokens": 5}"#;
        let usage: AnthropicUsage = serde_json::from_str(json_str).unwrap();
        assert_eq!(usage.input_tokens, 10);
        assert_eq!(usage.output_tokens, 5);
        assert_eq!(usage.cache_creation_input_tokens, 0);
        assert_eq!(usage.cache_read_input_tokens, 0);
    }
}
