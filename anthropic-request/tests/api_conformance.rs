use anthropic_request::*;

#[test]
fn request_with_tool_choice_auto() {
    let json = serde_json::json!({
        "model": "claude-sonnet-4-20250514",
        "max_tokens": 1024,
        "messages": [{"role": "user", "content": "Hi"}],
        "tool_choice": {"type": "auto"}
    });
    let req: V1MessagesRequest = serde_json::from_value(json).unwrap();
    assert_eq!(req.model, "claude-sonnet-4-20250514");
}

#[test]
fn request_with_tool_choice_any() {
    let json = serde_json::json!({
        "model": "claude-sonnet-4-20250514",
        "max_tokens": 1024,
        "messages": [{"role": "user", "content": "Hi"}],
        "tool_choice": {"type": "any"}
    });
    let _req: V1MessagesRequest = serde_json::from_value(json).unwrap();
}

#[test]
fn request_with_tool_choice_tool() {
    let json = serde_json::json!({
        "model": "claude-sonnet-4-20250514",
        "max_tokens": 1024,
        "messages": [{"role": "user", "content": "Hi"}],
        "tool_choice": {"type": "tool", "name": "get_weather"}
    });
    let _req: V1MessagesRequest = serde_json::from_value(json).unwrap();
}

#[test]
fn request_with_tool_choice_none() {
    let json = serde_json::json!({
        "model": "claude-sonnet-4-20250514",
        "max_tokens": 1024,
        "messages": [{"role": "user", "content": "Hi"}],
        "tool_choice": {"type": "none"}
    });
    let _req: V1MessagesRequest = serde_json::from_value(json).unwrap();
}

#[test]
fn request_with_stop_sequences() {
    let json = serde_json::json!({
        "model": "claude-sonnet-4-20250514",
        "max_tokens": 1024,
        "messages": [{"role": "user", "content": "Hi"}],
        "stop_sequences": ["STOP", "END"]
    });
    let _req: V1MessagesRequest = serde_json::from_value(json).unwrap();
}

#[test]
fn request_with_top_k() {
    let json = serde_json::json!({
        "model": "claude-sonnet-4-20250514",
        "max_tokens": 1024,
        "messages": [{"role": "user", "content": "Hi"}],
        "top_k": 40
    });
    let _req: V1MessagesRequest = serde_json::from_value(json).unwrap();
}

#[test]
fn request_with_top_p() {
    let json = serde_json::json!({
        "model": "claude-sonnet-4-20250514",
        "max_tokens": 1024,
        "messages": [{"role": "user", "content": "Hi"}],
        "top_p": 0.9
    });
    let _req: V1MessagesRequest = serde_json::from_value(json).unwrap();
}

#[test]
fn request_with_metadata() {
    let json = serde_json::json!({
        "model": "claude-sonnet-4-20250514",
        "max_tokens": 1024,
        "messages": [{"role": "user", "content": "Hi"}],
        "metadata": {"user_id": "user-123"}
    });
    let _req: V1MessagesRequest = serde_json::from_value(json).unwrap();
}

#[test]
fn request_with_all_optional_fields() {
    let json = serde_json::json!({
        "model": "claude-sonnet-4-20250514",
        "max_tokens": 1024,
        "messages": [{"role": "user", "content": "Hi"}],
        "stop_sequences": ["STOP"],
        "top_k": 40,
        "top_p": 0.9,
        "temperature": 0.7,
        "metadata": {"user_id": "user-123"},
        "tool_choice": {"type": "auto"}
    });
    let _req: V1MessagesRequest = serde_json::from_value(json).unwrap();
}

#[test]
fn request_round_trip_preserves_stop_sequences() {
    let json = serde_json::json!({
        "model": "claude-sonnet-4-20250514",
        "max_tokens": 1024,
        "messages": [{"role": "user", "content": "Hi"}],
        "stop_sequences": ["STOP", "END"]
    });
    let req: V1MessagesRequest = serde_json::from_value(json).unwrap();
    let serialized = serde_json::to_value(&req).unwrap();
    let stop_seqs = serialized["stop_sequences"].as_array().unwrap();
    assert_eq!(stop_seqs.len(), 2);
    assert_eq!(stop_seqs[0], "STOP");
    assert_eq!(stop_seqs[1], "END");
}

#[test]
fn request_round_trip_preserves_tool_choice() {
    let json = serde_json::json!({
        "model": "claude-sonnet-4-20250514",
        "max_tokens": 1024,
        "messages": [{"role": "user", "content": "Hi"}],
        "tool_choice": {"type": "tool", "name": "get_weather"}
    });
    let req: V1MessagesRequest = serde_json::from_value(json).unwrap();
    let serialized = serde_json::to_value(&req).unwrap();
    assert_eq!(serialized["tool_choice"]["type"], "tool");
    assert_eq!(serialized["tool_choice"]["name"], "get_weather");
}

#[test]
fn request_round_trip_preserves_metadata() {
    let json = serde_json::json!({
        "model": "claude-sonnet-4-20250514",
        "max_tokens": 1024,
        "messages": [{"role": "user", "content": "Hi"}],
        "metadata": {"user_id": "user-123"}
    });
    let req: V1MessagesRequest = serde_json::from_value(json).unwrap();
    let serialized = serde_json::to_value(&req).unwrap();
    assert_eq!(serialized["metadata"]["user_id"], "user-123");
}

#[test]
fn count_tokens_with_stop_sequences() {
    let json = serde_json::json!({
        "model": "claude-sonnet-4-20250514",
        "messages": [{"role": "user", "content": "Hi"}],
        "stop_sequences": ["STOP"]
    });
    let _req: V1MessagesCountTokensRequest = serde_json::from_value(json).unwrap();
}

#[test]
fn count_tokens_with_top_k_and_top_p() {
    let json = serde_json::json!({
        "model": "claude-sonnet-4-20250514",
        "messages": [{"role": "user", "content": "Hi"}],
        "top_k": 40,
        "top_p": 0.9
    });
    let _req: V1MessagesCountTokensRequest = serde_json::from_value(json).unwrap();
}

#[test]
fn cache_control_with_ttl() {
    let json = serde_json::json!({"type": "ephemeral", "ttl": "5m"});
    let cc: CacheControl = serde_json::from_value(json).unwrap();
    assert_eq!(cc.cache_control_type, "ephemeral");
}

#[test]
fn cache_control_round_trip_preserves_ttl() {
    let json = serde_json::json!({"type": "ephemeral", "ttl": "1h"});
    let cc: CacheControl = serde_json::from_value(json).unwrap();
    let serialized = serde_json::to_value(&cc).unwrap();
    assert_eq!(serialized["ttl"], "1h");
}

#[test]
fn tool_without_description() {
    let json = serde_json::json!({
        "name": "get_weather",
        "input_schema": {"type": "object", "properties": {}}
    });
    let _tool: Tool = serde_json::from_value(json).unwrap();
}

#[test]
fn tool_with_optional_extra_fields() {
    let json = serde_json::json!({
        "name": "get_weather",
        "description": "Gets the weather",
        "input_schema": {"type": "object"},
        "type": "custom"
    });
    let _tool: Tool = serde_json::from_value(json).unwrap();
}

#[test]
fn image_source_url_variant() {
    let json = serde_json::json!({
        "type": "url",
        "url": "https://example.com/image.png"
    });
    let _source: ImageSource = serde_json::from_value(json).unwrap();
}

#[test]
fn document_source_url_variant() {
    let json = serde_json::json!({
        "type": "url",
        "url": "https://example.com/doc.pdf"
    });
    let _source: DocumentSource = serde_json::from_value(json).unwrap();
}

#[test]
fn document_source_plain_text_variant() {
    let json = serde_json::json!({
        "type": "text",
        "media_type": "text/plain",
        "data": "This is some plain text content."
    });
    let _source: DocumentSource = serde_json::from_value(json).unwrap();
}

#[test]
fn assistant_content_redacted_thinking() {
    let json = serde_json::json!({
        "type": "redacted_thinking",
        "data": "base64encodeddata"
    });
    let _content: AssistantContent = serde_json::from_value(json).unwrap();
}

#[test]
fn assistant_content_server_tool_use() {
    let json = serde_json::json!({
        "type": "server_tool_use",
        "id": "srvtoolu_01",
        "name": "web_search",
        "input": {"query": "test"}
    });
    let _content: AssistantContent = serde_json::from_value(json).unwrap();
}

#[test]
fn assistant_contents_with_mixed_thinking_and_redacted() {
    let json = serde_json::json!([
        {"type": "thinking", "thinking": "let me think", "signature": "sig123"},
        {"type": "redacted_thinking", "data": "base64data"},
        {"type": "text", "text": "hello"}
    ]);
    let _contents: AssistantContents = serde_json::from_value(json).unwrap();
}

#[test]
fn user_content_thinking_block() {
    let json = serde_json::json!({
        "type": "thinking",
        "thinking": "let me think about this",
        "signature": "sig123"
    });
    let _content: UserContent = serde_json::from_value(json).unwrap();
}

#[test]
fn user_content_redacted_thinking_block() {
    let json = serde_json::json!({
        "type": "redacted_thinking",
        "data": "base64encodeddata"
    });
    let _content: UserContent = serde_json::from_value(json).unwrap();
}

#[test]
fn user_content_server_tool_result() {
    let json = serde_json::json!({
        "type": "server_tool_result",
        "tool_use_id": "srvtoolu_01",
        "content": [
            {
                "type": "web_search_result",
                "url": "https://example.com",
                "title": "Example",
                "encrypted_content": "abc123"
            }
        ]
    });
    let _content: UserContent = serde_json::from_value(json).unwrap();
}

#[test]
fn thinking_disabled() {
    let json = serde_json::json!({"type": "disabled"});
    let _thinking: Thinking = serde_json::from_value(json).unwrap();
}

#[test]
fn thinking_disabled_round_trip() {
    let json = serde_json::json!({"type": "disabled"});
    let thinking: Thinking = serde_json::from_value(json).unwrap();
    let serialized = serde_json::to_value(&thinking).unwrap();
    assert_eq!(serialized["type"], "disabled");
}

#[test]
fn thinking_enabled_still_works() {
    let json = serde_json::json!({"type": "enabled", "budget_tokens": 5000});
    let thinking: Thinking = serde_json::from_value(json).unwrap();
    match thinking {
        Thinking::Enabled { budget_tokens } => assert_eq!(budget_tokens, 5000),
        _ => panic!("expected Enabled variant"),
    }
}

#[test]
fn thinking_adaptive() {
    let json = serde_json::json!({"type": "adaptive"});
    let thinking: Thinking = serde_json::from_value(json).unwrap();
    assert!(matches!(thinking, Thinking::Adaptive));
}

#[test]
fn thinking_adaptive_round_trip() {
    let json = serde_json::json!({"type": "adaptive"});
    let thinking: Thinking = serde_json::from_value(json).unwrap();
    let serialized = serde_json::to_value(&thinking).unwrap();
    assert_eq!(serialized["type"], "adaptive");
}

#[test]
fn system_text_with_citations() {
    let json = serde_json::json!([{
        "type": "text",
        "text": "You are a helpful assistant.",
        "citations": [
            {
                "type": "char_location",
                "cited_text": "some text",
                "document_index": 0,
                "start": 0,
                "end": 10
            }
        ]
    }]);
    let _systems: Systems = serde_json::from_value(json).unwrap();
}

#[test]
fn full_request_with_extended_thinking_disabled() {
    let json = serde_json::json!({
        "model": "claude-sonnet-4-20250514",
        "max_tokens": 1024,
        "messages": [{"role": "user", "content": "Hi"}],
        "thinking": {"type": "disabled"},
        "stop_sequences": ["STOP"],
        "top_k": 40,
        "top_p": 0.9,
        "metadata": {"user_id": "user-123"}
    });
    let _req: V1MessagesRequest = serde_json::from_value(json).unwrap();
}

#[test]
fn multi_turn_with_thinking_pass_through() {
    let json = serde_json::json!({
        "model": "claude-sonnet-4-20250514",
        "max_tokens": 1024,
        "thinking": {"type": "enabled", "budget_tokens": 10000},
        "messages": [
            {"role": "user", "content": "What is 2+2?"},
            {
                "role": "assistant",
                "content": [
                    {"type": "thinking", "thinking": "2+2=4", "signature": "sig1"},
                    {"type": "text", "text": "4"}
                ]
            },
            {"role": "user", "content": "And 3+3?"}
        ]
    });
    let _req: V1MessagesRequest = serde_json::from_value(json).unwrap();
}

#[test]
fn multi_turn_with_redacted_thinking_pass_through() {
    let json = serde_json::json!({
        "model": "claude-sonnet-4-20250514",
        "max_tokens": 1024,
        "thinking": {"type": "enabled", "budget_tokens": 10000},
        "messages": [
            {"role": "user", "content": "What is 2+2?"},
            {
                "role": "assistant",
                "content": [
                    {"type": "thinking", "thinking": "2+2=4", "signature": "sig1"},
                    {"type": "redacted_thinking", "data": "base64data"},
                    {"type": "text", "text": "4"}
                ]
            },
            {"role": "user", "content": "And 3+3?"}
        ]
    });
    let _req: V1MessagesRequest = serde_json::from_value(json).unwrap();
}

#[test]
fn request_with_url_image() {
    let json = serde_json::json!({
        "model": "claude-sonnet-4-20250514",
        "max_tokens": 1024,
        "messages": [{
            "role": "user",
            "content": [
                {
                    "type": "image",
                    "source": {
                        "type": "url",
                        "url": "https://example.com/image.png"
                    }
                },
                {"type": "text", "text": "What is this?"}
            ]
        }]
    });
    let _req: V1MessagesRequest = serde_json::from_value(json).unwrap();
}

#[test]
fn request_with_url_document() {
    let json = serde_json::json!({
        "model": "claude-sonnet-4-20250514",
        "max_tokens": 1024,
        "messages": [{
            "role": "user",
            "content": [
                {
                    "type": "document",
                    "source": {
                        "type": "url",
                        "url": "https://example.com/doc.pdf"
                    }
                },
                {"type": "text", "text": "Summarize this."}
            ]
        }]
    });
    let _req: V1MessagesRequest = serde_json::from_value(json).unwrap();
}

#[test]
fn request_with_server_tool_use_and_result() {
    let json = serde_json::json!({
        "model": "claude-sonnet-4-20250514",
        "max_tokens": 1024,
        "tools": [
            {"type": "web_search_20250305", "name": "web_search", "max_uses": 3}
        ],
        "messages": [
            {"role": "user", "content": "Search for Rust lang"},
            {
                "role": "assistant",
                "content": [
                    {
                        "type": "server_tool_use",
                        "id": "srvtoolu_01",
                        "name": "web_search",
                        "input": {"query": "Rust programming language"}
                    }
                ]
            },
            {
                "role": "user",
                "content": [
                    {
                        "type": "server_tool_result",
                        "tool_use_id": "srvtoolu_01",
                        "content": [
                            {
                                "type": "web_search_result",
                                "url": "https://rust-lang.org",
                                "title": "Rust",
                                "encrypted_content": "abc"
                            }
                        ]
                    }
                ]
            }
        ]
    });
    let _req: V1MessagesRequest = serde_json::from_value(json).unwrap();
}

#[test]
fn tool_without_description_in_request() {
    let json = serde_json::json!({
        "model": "claude-sonnet-4-20250514",
        "max_tokens": 1024,
        "messages": [{"role": "user", "content": "Hi"}],
        "tools": [
            {
                "name": "get_weather",
                "input_schema": {"type": "object", "properties": {"city": {"type": "string"}}}
            }
        ]
    });
    let _req: V1MessagesRequest = serde_json::from_value(json).unwrap();
}

#[test]
fn server_tool_definition() {
    let json = serde_json::json!({
        "model": "claude-sonnet-4-20250514",
        "max_tokens": 1024,
        "messages": [{"role": "user", "content": "Search the web"}],
        "tools": [
            {"type": "web_search_20250305", "name": "web_search", "max_uses": 5}
        ]
    });
    let _req: V1MessagesRequest = serde_json::from_value(json).unwrap();
}
