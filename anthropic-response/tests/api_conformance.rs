use anthropic_response::*;

#[test]
fn usage_with_cache_tokens() {
    let json = serde_json::json!({
        "input_tokens": 100,
        "output_tokens": 50,
        "cache_creation_input_tokens": 200,
        "cache_read_input_tokens": 300
    });
    let usage: Usage = serde_json::from_value(json).unwrap();
    assert_eq!(usage.input_tokens, 100);
    assert_eq!(usage.output_tokens, 50);
}

#[test]
fn usage_round_trip_preserves_cache_tokens() {
    let json = serde_json::json!({
        "input_tokens": 100,
        "output_tokens": 50,
        "cache_creation_input_tokens": 200,
        "cache_read_input_tokens": 300
    });
    let usage: Usage = serde_json::from_value(json).unwrap();
    let serialized = serde_json::to_value(&usage).unwrap();
    assert_eq!(serialized["cache_creation_input_tokens"], 200);
    assert_eq!(serialized["cache_read_input_tokens"], 300);
}

#[test]
fn usage_without_cache_tokens_still_works() {
    let json = serde_json::json!({
        "input_tokens": 100,
        "output_tokens": 50
    });
    let usage: Usage = serde_json::from_value(json).unwrap();
    assert_eq!(usage.input_tokens, 100);
    assert_eq!(usage.output_tokens, 50);
}

#[test]
fn content_block_redacted_thinking() {
    let json = serde_json::json!({
        "type": "redacted_thinking",
        "data": "base64encodeddata"
    });
    let _block: ContentBlock = serde_json::from_value(json).unwrap();
}

#[test]
fn content_block_start_with_redacted_thinking() {
    let json = serde_json::json!({
        "type": "content_block_start",
        "index": 1,
        "content_block": {
            "type": "redacted_thinking",
            "data": "base64data"
        }
    });
    let _event: Event = serde_json::from_value(json).unwrap();
}

#[test]
fn content_block_server_tool_use() {
    let json = serde_json::json!({
        "type": "server_tool_use",
        "id": "srvtoolu_01",
        "name": "web_search",
        "input": {"query": "test"}
    });
    let _block: ContentBlock = serde_json::from_value(json).unwrap();
}

#[test]
fn message_response_with_cache_usage() {
    let json = serde_json::json!({
        "id": "msg_01",
        "type": "message",
        "role": "assistant",
        "content": [{"type": "text", "text": "Hello"}],
        "model": "claude-sonnet-4-20250514",
        "stop_reason": "end_turn",
        "stop_sequence": null,
        "usage": {
            "input_tokens": 100,
            "output_tokens": 50,
            "cache_creation_input_tokens": 200,
            "cache_read_input_tokens": 300
        }
    });
    let msg: Message = serde_json::from_value(json).unwrap();
    assert_eq!(msg.usage.input_tokens, 100);
    assert_eq!(msg.usage.output_tokens, 50);
    let re_serialized = serde_json::to_value(&msg).unwrap();
    assert_eq!(re_serialized["usage"]["cache_creation_input_tokens"], 200);
    assert_eq!(re_serialized["usage"]["cache_read_input_tokens"], 300);
}

#[test]
fn usage_delta_with_cache_tokens() {
    let json = serde_json::json!({
        "input_tokens": 0,
        "output_tokens": 25,
        "cache_creation_input_tokens": 100,
        "cache_read_input_tokens": 50
    });
    let delta: UsageDelta = serde_json::from_value(json).unwrap();
    assert_eq!(delta.cache_creation_input_tokens, Some(100));
    assert_eq!(delta.cache_read_input_tokens, Some(50));
}

#[test]
fn message_start_event_with_cache_usage() {
    let json = serde_json::json!({
        "type": "message_start",
        "message": {
            "id": "msg_01",
            "type": "message",
            "role": "assistant",
            "content": [],
            "model": "claude-sonnet-4-20250514",
            "stop_reason": null,
            "stop_sequence": null,
            "usage": {
                "input_tokens": 100,
                "output_tokens": 0,
                "cache_creation_input_tokens": 50,
                "cache_read_input_tokens": 25
            }
        }
    });
    let _event: Event = serde_json::from_value(json).unwrap();
}

#[test]
fn message_delta_event_with_all_fields() {
    let json = serde_json::json!({
        "type": "message_delta",
        "delta": {
            "stop_reason": "end_turn",
            "stop_sequence": null
        },
        "usage": {
            "input_tokens": 0,
            "output_tokens": 100,
            "cache_creation_input_tokens": 50,
            "cache_read_input_tokens": 25
        }
    });
    let event: Event = serde_json::from_value(json).unwrap();
    match event {
        Event::MessageDelta { usage, .. } => {
            assert_eq!(usage.output_tokens, 100);
            assert_eq!(usage.cache_creation_input_tokens, Some(50));
        }
        _ => panic!("expected MessageDelta"),
    }
}

#[test]
fn content_block_delta_text() {
    let json = serde_json::json!({
        "type": "content_block_delta",
        "index": 0,
        "delta": {"type": "text_delta", "text": "Hello"}
    });
    let _event: Event = serde_json::from_value(json).unwrap();
}

#[test]
fn content_block_delta_thinking() {
    let json = serde_json::json!({
        "type": "content_block_delta",
        "index": 0,
        "delta": {"type": "thinking_delta", "thinking": "I need to think..."}
    });
    let _event: Event = serde_json::from_value(json).unwrap();
}

#[test]
fn content_block_delta_signature() {
    let json = serde_json::json!({
        "type": "content_block_delta",
        "index": 0,
        "delta": {"type": "signature_delta", "signature": "sig123"}
    });
    let _event: Event = serde_json::from_value(json).unwrap();
}

#[test]
fn content_block_delta_input_json() {
    let json = serde_json::json!({
        "type": "content_block_delta",
        "index": 0,
        "delta": {"type": "input_json_delta", "partial_json": "{\"key\":"}
    });
    let _event: Event = serde_json::from_value(json).unwrap();
}
