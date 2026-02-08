//! OpenAI Chat Completions API translation types.
//!
//! This is a **data-only** translation layer: no HTTP calls, no async, no IO.
//! It defines the request/response wire types for the Chat Completions API and
//! provides pure helper functions for error classification.
//!
//! Consumers include OpenRouter, Azure OpenAI, and any other provider that
//! speaks the Chat Completions protocol.

use serde::Deserialize;
use serde::Serialize;

use crate::error::ProviderError;

// ---------------------------------------------------------------------------
// Request types
// ---------------------------------------------------------------------------

/// A Chat Completions request body.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<ChatTool>>,
    #[serde(default = "default_true")]
    pub stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream_options: Option<StreamOptions>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<String>,
}

/// A single message within a chat conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    /// One of `"system"`, `"user"`, `"assistant"`, or `"tool"`.
    pub role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ChatToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

/// A tool call emitted by the assistant.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub call_type: String,
    pub function: ChatFunction,
}

/// The function name and JSON-encoded arguments inside a tool call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatFunction {
    pub name: String,
    pub arguments: String,
}

/// A tool definition supplied in the request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatTool {
    #[serde(rename = "type")]
    pub tool_type: String,
    pub function: ChatToolFunction,
}

/// The function schema within a tool definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatToolFunction {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

/// Options that control server-sent-event streaming behaviour.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamOptions {
    pub include_usage: bool,
}

fn default_true() -> bool {
    true
}

// ---------------------------------------------------------------------------
// Streaming response types
// ---------------------------------------------------------------------------

/// A single streaming chunk from the Chat Completions API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatStreamChunk {
    pub id: String,
    /// Always `"chat.completion.chunk"`.
    pub object: String,
    pub model: String,
    pub choices: Vec<ChatStreamChoice>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<ChatUsage>,
}

/// One choice inside a streaming chunk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatStreamChoice {
    pub index: u32,
    pub delta: ChatDelta,
    /// `None` while streaming; one of `"stop"`, `"tool_calls"`, or `"length"`
    /// on the final chunk for this choice.
    pub finish_reason: Option<String>,
}

/// The incremental delta payload inside a streaming choice.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatDelta {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ChatDeltaToolCall>>,
}

/// A partial tool call delivered across one or more streaming deltas.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatDeltaToolCall {
    pub index: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub function: Option<ChatDeltaFunction>,
}

/// Partial function data within a streaming tool-call delta.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatDeltaFunction {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arguments: Option<String>,
}

/// Token usage statistics returned (optionally) on the final streaming chunk
/// or in a non-streaming response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatUsage {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
}

// ---------------------------------------------------------------------------
// Error classification
// ---------------------------------------------------------------------------

/// Classify an HTTP error from a Chat Completions-compatible endpoint into the
/// appropriate [`ProviderError`] variant.
pub fn classify_chat_error(status: u16, body: &str) -> ProviderError {
    match status {
        401 => ProviderError::InvalidConfig(format!("authentication failed: {body}")),
        429 => ProviderError::ApiError(format!("rate limited: {body}")),
        404 => ProviderError::UnsupportedProvider(format!("model not found: {body}")),
        500..=599 => ProviderError::ApiError(format!("server error ({status}): {body}")),
        _ => ProviderError::Other(format!("HTTP {status}: {body}")),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- Request serialization round-trips ----------------------------------

    #[test]
    fn chat_request_roundtrip() {
        let req = ChatRequest {
            model: "gpt-4o".into(),
            messages: vec![
                ChatMessage {
                    role: "system".into(),
                    content: Some("You are a helpful assistant.".into()),
                    tool_calls: None,
                    tool_call_id: None,
                },
                ChatMessage {
                    role: "user".into(),
                    content: Some("Hello!".into()),
                    tool_calls: None,
                    tool_call_id: None,
                },
            ],
            tools: None,
            stream: true,
            stream_options: Some(StreamOptions {
                include_usage: true,
            }),
            temperature: Some(0.7),
            max_tokens: Some(1024),
            tool_choice: Some("auto".into()),
        };

        let json = serde_json::to_string(&req).unwrap();
        let deser: ChatRequest = serde_json::from_str(&json).unwrap();

        assert_eq!(deser.model, "gpt-4o");
        assert_eq!(deser.messages.len(), 2);
        assert!(deser.stream);
        assert_eq!(deser.temperature, Some(0.7));
        assert_eq!(deser.max_tokens, Some(1024));
        assert_eq!(deser.tool_choice.as_deref(), Some("auto"));
        assert!(deser.stream_options.as_ref().unwrap().include_usage);
    }

    #[test]
    fn chat_request_omits_none_fields() {
        let req = ChatRequest {
            model: "gpt-4o-mini".into(),
            messages: vec![],
            tools: None,
            stream: true,
            stream_options: None,
            temperature: None,
            max_tokens: None,
            tool_choice: None,
        };

        let json = serde_json::to_string(&req).unwrap();
        assert!(!json.contains("tools"));
        assert!(!json.contains("temperature"));
        assert!(!json.contains("max_tokens"));
        assert!(!json.contains("tool_choice"));
        assert!(!json.contains("stream_options"));
    }

    #[test]
    fn stream_defaults_to_true() {
        let json = r#"{"model":"gpt-4o","messages":[]}"#;
        let req: ChatRequest = serde_json::from_str(json).unwrap();
        assert!(req.stream);
    }

    // -- Message with tool calls --------------------------------------------

    #[test]
    fn chat_message_with_tool_calls_roundtrip() {
        let msg = ChatMessage {
            role: "assistant".into(),
            content: None,
            tool_calls: Some(vec![ChatToolCall {
                id: "call_abc123".into(),
                call_type: "function".into(),
                function: ChatFunction {
                    name: "get_weather".into(),
                    arguments: r#"{"location":"London"}"#.into(),
                },
            }]),
            tool_call_id: None,
        };

        let json = serde_json::to_string(&msg).unwrap();

        // Verify `type` is serialized (not `call_type`)
        assert!(json.contains(r#""type":"function""#));
        assert!(!json.contains("call_type"));

        let deser: ChatMessage = serde_json::from_str(&json).unwrap();
        let tc = &deser.tool_calls.unwrap()[0];
        assert_eq!(tc.id, "call_abc123");
        assert_eq!(tc.call_type, "function");
        assert_eq!(tc.function.name, "get_weather");
        assert_eq!(tc.function.arguments, r#"{"location":"London"}"#);
    }

    #[test]
    fn tool_result_message_roundtrip() {
        let msg = ChatMessage {
            role: "tool".into(),
            content: Some(r#"{"temp_c":12}"#.into()),
            tool_calls: None,
            tool_call_id: Some("call_abc123".into()),
        };

        let json = serde_json::to_string(&msg).unwrap();
        let deser: ChatMessage = serde_json::from_str(&json).unwrap();

        assert_eq!(deser.role, "tool");
        assert_eq!(deser.tool_call_id.as_deref(), Some("call_abc123"));
        assert_eq!(deser.content.as_deref(), Some(r#"{"temp_c":12}"#));
    }

    // -- Tool definition serialization --------------------------------------

    #[test]
    fn chat_tool_serialization() {
        let tool = ChatTool {
            tool_type: "function".into(),
            function: ChatToolFunction {
                name: "search".into(),
                description: "Search the web".into(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "query": {
                            "type": "string",
                            "description": "The search query"
                        }
                    },
                    "required": ["query"]
                }),
            },
        };

        let json = serde_json::to_string_pretty(&tool).unwrap();
        assert!(json.contains(r#""type": "function""#));
        assert!(json.contains(r#""name": "search""#));
        assert!(json.contains(r#""description": "Search the web""#));

        let deser: ChatTool = serde_json::from_str(&json).unwrap();
        assert_eq!(deser.tool_type, "function");
        assert_eq!(deser.function.name, "search");
        let props = deser.function.parameters["properties"]["query"]["type"]
            .as_str()
            .unwrap();
        assert_eq!(props, "string");
    }

    // -- Streaming chunk deserialization ------------------------------------

    #[test]
    fn chat_stream_chunk_content_delta() {
        let json = r#"{
            "id": "chatcmpl-abc",
            "object": "chat.completion.chunk",
            "model": "gpt-4o",
            "choices": [{
                "index": 0,
                "delta": {
                    "content": "Hello"
                },
                "finish_reason": null
            }],
            "usage": null
        }"#;

        let chunk: ChatStreamChunk = serde_json::from_str(json).unwrap();
        assert_eq!(chunk.id, "chatcmpl-abc");
        assert_eq!(chunk.object, "chat.completion.chunk");
        assert_eq!(chunk.model, "gpt-4o");
        assert_eq!(chunk.choices.len(), 1);

        let choice = &chunk.choices[0];
        assert_eq!(choice.index, 0);
        assert_eq!(choice.delta.content.as_deref(), Some("Hello"));
        assert!(choice.finish_reason.is_none());
        assert!(chunk.usage.is_none());
    }

    #[test]
    fn chat_stream_chunk_role_delta() {
        let json = r#"{
            "id": "chatcmpl-xyz",
            "object": "chat.completion.chunk",
            "model": "gpt-4o",
            "choices": [{
                "index": 0,
                "delta": {
                    "role": "assistant"
                },
                "finish_reason": null
            }]
        }"#;

        let chunk: ChatStreamChunk = serde_json::from_str(json).unwrap();
        assert_eq!(chunk.choices[0].delta.role.as_deref(), Some("assistant"));
        assert!(chunk.choices[0].delta.content.is_none());
    }

    #[test]
    fn chat_stream_chunk_finish_stop() {
        let json = r#"{
            "id": "chatcmpl-done",
            "object": "chat.completion.chunk",
            "model": "gpt-4o",
            "choices": [{
                "index": 0,
                "delta": {},
                "finish_reason": "stop"
            }]
        }"#;

        let chunk: ChatStreamChunk = serde_json::from_str(json).unwrap();
        assert_eq!(chunk.choices[0].finish_reason.as_deref(), Some("stop"));
    }

    #[test]
    fn chat_stream_tool_call_deltas() {
        // First chunk: introduces the tool call with id and function name.
        let chunk1_json = r#"{
            "id": "chatcmpl-tc",
            "object": "chat.completion.chunk",
            "model": "gpt-4o",
            "choices": [{
                "index": 0,
                "delta": {
                    "role": "assistant",
                    "tool_calls": [{
                        "index": 0,
                        "id": "call_xyz",
                        "function": {
                            "name": "get_weather",
                            "arguments": ""
                        }
                    }]
                },
                "finish_reason": null
            }]
        }"#;

        // Subsequent chunks: stream argument fragments.
        let chunk2_json = r#"{
            "id": "chatcmpl-tc",
            "object": "chat.completion.chunk",
            "model": "gpt-4o",
            "choices": [{
                "index": 0,
                "delta": {
                    "tool_calls": [{
                        "index": 0,
                        "function": {
                            "arguments": "{\"loc"
                        }
                    }]
                },
                "finish_reason": null
            }]
        }"#;

        let chunk3_json = r#"{
            "id": "chatcmpl-tc",
            "object": "chat.completion.chunk",
            "model": "gpt-4o",
            "choices": [{
                "index": 0,
                "delta": {
                    "tool_calls": [{
                        "index": 0,
                        "function": {
                            "arguments": "ation\":\"NYC\"}"
                        }
                    }]
                },
                "finish_reason": null
            }]
        }"#;

        // Final chunk: finish_reason = "tool_calls"
        let chunk4_json = r#"{
            "id": "chatcmpl-tc",
            "object": "chat.completion.chunk",
            "model": "gpt-4o",
            "choices": [{
                "index": 0,
                "delta": {},
                "finish_reason": "tool_calls"
            }]
        }"#;

        let c1: ChatStreamChunk = serde_json::from_str(chunk1_json).unwrap();
        let c2: ChatStreamChunk = serde_json::from_str(chunk2_json).unwrap();
        let c3: ChatStreamChunk = serde_json::from_str(chunk3_json).unwrap();
        let c4: ChatStreamChunk = serde_json::from_str(chunk4_json).unwrap();

        // Chunk 1: id and name present
        let tc1 = &c1.choices[0].delta.tool_calls.as_ref().unwrap()[0];
        assert_eq!(tc1.id.as_deref(), Some("call_xyz"));
        assert_eq!(
            tc1.function.as_ref().unwrap().name.as_deref(),
            Some("get_weather")
        );

        // Chunk 2 & 3: argument fragments, no id
        let tc2 = &c2.choices[0].delta.tool_calls.as_ref().unwrap()[0];
        assert!(tc2.id.is_none());
        assert_eq!(
            tc2.function.as_ref().unwrap().arguments.as_deref(),
            Some("{\"loc")
        );

        let tc3 = &c3.choices[0].delta.tool_calls.as_ref().unwrap()[0];
        assert_eq!(
            tc3.function.as_ref().unwrap().arguments.as_deref(),
            Some("ation\":\"NYC\"}")
        );

        // Reassemble the full arguments string
        let mut args = String::new();
        for chunk in [&c1, &c2, &c3] {
            if let Some(tcs) = &chunk.choices[0].delta.tool_calls {
                if let Some(f) = &tcs[0].function {
                    if let Some(a) = &f.arguments {
                        args.push_str(a);
                    }
                }
            }
        }
        assert_eq!(args, r#"{"location":"NYC"}"#);

        // Chunk 4: done
        assert_eq!(c4.choices[0].finish_reason.as_deref(), Some("tool_calls"));
    }

    // -- Usage deserialization -----------------------------------------------

    #[test]
    fn chat_usage_deserialization() {
        let json = r#"{"prompt_tokens":150,"completion_tokens":42,"total_tokens":192}"#;
        let usage: ChatUsage = serde_json::from_str(json).unwrap();
        assert_eq!(usage.prompt_tokens, 150);
        assert_eq!(usage.completion_tokens, 42);
        assert_eq!(usage.total_tokens, 192);
    }

    #[test]
    fn chat_stream_chunk_with_usage() {
        let json = r#"{
            "id": "chatcmpl-final",
            "object": "chat.completion.chunk",
            "model": "gpt-4o",
            "choices": [],
            "usage": {
                "prompt_tokens": 50,
                "completion_tokens": 100,
                "total_tokens": 150
            }
        }"#;

        let chunk: ChatStreamChunk = serde_json::from_str(json).unwrap();
        let usage = chunk.usage.unwrap();
        assert_eq!(usage.prompt_tokens, 50);
        assert_eq!(usage.completion_tokens, 100);
        assert_eq!(usage.total_tokens, 150);
    }

    // -- Error classification ------------------------------------------------

    #[test]
    fn classify_401_as_invalid_config() {
        let err = classify_chat_error(401, "Incorrect API key");
        let msg = err.to_string();
        assert!(msg.contains("invalid config"));
        assert!(msg.contains("authentication failed"));
        assert!(msg.contains("Incorrect API key"));
    }

    #[test]
    fn classify_429_as_rate_limit() {
        let err = classify_chat_error(429, "Rate limit exceeded");
        let msg = err.to_string();
        assert!(msg.contains("api error"));
        assert!(msg.contains("rate limited"));
    }

    #[test]
    fn classify_404_as_unsupported_provider() {
        let err = classify_chat_error(404, "model not found");
        let msg = err.to_string();
        assert!(msg.contains("unsupported provider"));
        assert!(msg.contains("model not found"));
    }

    #[test]
    fn classify_500_as_server_error() {
        let err = classify_chat_error(500, "internal");
        let msg = err.to_string();
        assert!(msg.contains("api error"));
        assert!(msg.contains("server error (500)"));
    }

    #[test]
    fn classify_502_as_server_error() {
        let err = classify_chat_error(502, "bad gateway");
        let msg = err.to_string();
        assert!(msg.contains("server error (502)"));
    }

    #[test]
    fn classify_unknown_status() {
        let err = classify_chat_error(418, "I'm a teapot");
        let msg = err.to_string();
        assert!(msg.contains("HTTP 418"));
        assert!(msg.contains("I'm a teapot"));
    }
}
