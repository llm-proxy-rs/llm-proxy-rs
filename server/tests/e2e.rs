use aws_config::BehaviorVersion;
use aws_sdk_bedrockruntime::Client;
use axum::body::Body;
use http_body_util::BodyExt;
use server::{AppState, get_app};
use std::sync::Arc;
use tower::ServiceExt;

const MODEL: &str = "global.anthropic.claude-opus-4-6-v1";

async fn build_app() -> axum::Router {
    let config = aws_config::load_defaults(BehaviorVersion::latest()).await;
    let client = Client::new(&config);

    let state = Arc::new(AppState {
        bedrockruntime_client: client,
        inference_profile_prefixes: vec!["us.".to_string(), "global.".to_string()],
        anthropic_beta_whitelist: vec!["context-1m-2025-08-07".to_string()],
    });

    get_app(state)
}

fn parse_sse_events(body: &str) -> Vec<(String, String)> {
    let mut events = Vec::new();
    let mut current_event = String::new();
    let mut current_data = String::new();

    for line in body.lines() {
        if line.starts_with("event:") {
            current_event = line["event:".len()..].trim().to_string();
        } else if line.starts_with("data:") {
            current_data = line["data:".len()..].trim().to_string();
        } else if line.is_empty() && (!current_event.is_empty() || !current_data.is_empty()) {
            events.push((current_event.clone(), current_data.clone()));
            current_event.clear();
            current_data.clear();
        }
    }

    if !current_event.is_empty() || !current_data.is_empty() {
        events.push((current_event, current_data));
    }

    events
}

#[tokio::test]
#[ignore]
async fn v1_messages_returns_complete_sse_stream() {
    let app = build_app().await;

    let body = serde_json::json!({
        "model": MODEL,
        "max_tokens": 64,
        "stream": true,
        "messages": [
            {"role": "user", "content": "Say hi in exactly one word."}
        ]
    });

    let request = axum::http::Request::builder()
        .method("POST")
        .uri("/v1/messages")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), 200);

    let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body_str = String::from_utf8(body_bytes.to_vec()).unwrap();

    let events = parse_sse_events(&body_str);
    let event_types: Vec<&str> = events.iter().map(|(e, _)| e.as_str()).collect();

    assert!(
        event_types.contains(&"message_start"),
        "missing message_start, got: {event_types:?}"
    );
    assert!(
        event_types.contains(&"content_block_start"),
        "missing content_block_start, got: {event_types:?}"
    );
    assert!(
        event_types.contains(&"content_block_delta"),
        "missing content_block_delta, got: {event_types:?}"
    );
    assert!(
        event_types.contains(&"content_block_stop"),
        "missing content_block_stop, got: {event_types:?}"
    );
    assert!(
        event_types.contains(&"message_delta"),
        "missing message_delta, got: {event_types:?}"
    );
    assert!(
        event_types.contains(&"message_stop"),
        "missing message_stop, got: {event_types:?}"
    );

    // Verify ordering: message_start is first, message_stop is last
    assert_eq!(event_types.first(), Some(&"message_start"));
    assert_eq!(event_types.last(), Some(&"message_stop"));
}

#[tokio::test]
#[ignore]
async fn chat_completions_returns_complete_sse_stream() {
    let app = build_app().await;

    let body = serde_json::json!({
        "model": MODEL,
        "max_tokens": 64,
        "stream": true,
        "messages": [
            {"role": "user", "content": "Say hi in exactly one word."}
        ]
    });

    let request = axum::http::Request::builder()
        .method("POST")
        .uri("/chat/completions")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), 200);

    let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body_str = String::from_utf8(body_bytes.to_vec()).unwrap();

    let events = parse_sse_events(&body_str);
    let data_values: Vec<&str> = events.iter().map(|(_, d)| d.as_str()).collect();

    // Chat completions should end with [DONE]
    assert!(
        data_values.contains(&"[DONE]"),
        "missing [DONE] sentinel, got: {data_values:?}"
    );

    // Should have at least one chunk before [DONE]
    assert!(
        data_values.len() >= 2,
        "expected at least 2 events (chunk + DONE), got: {}",
        data_values.len()
    );

    // All data entries except [DONE] should be valid JSON
    for data in &data_values {
        if *data != "[DONE]" {
            let parsed: serde_json::Value = serde_json::from_str(data)
                .unwrap_or_else(|e| panic!("invalid JSON in SSE data: {e}\ndata: {data}"));
            assert!(
                parsed.get("id").is_some(),
                "chunk missing 'id' field: {parsed}"
            );
        }
    }
}

#[tokio::test]
#[ignore]
async fn v1_messages_with_tools_missing_referenced_tool() {
    let app = build_app().await;

    // tools defines "search" but messages reference "get_weather" which is not in tools
    let body = serde_json::json!({
        "model": MODEL,
        "max_tokens": 64,
        "stream": true,
        "tools": [
            {
                "name": "search",
                "description": "Search the web.",
                "input_schema": {
                    "type": "object",
                    "properties": {
                        "query": {"type": "string"}
                    },
                    "required": ["query"]
                }
            }
        ],
        "messages": [
            {
                "role": "user",
                "content": "What's the weather?"
            },
            {
                "role": "assistant",
                "content": [
                    {
                        "type": "tool_use",
                        "id": "tooluse_missing1",
                        "name": "get_weather",
                        "input": {"city": "NYC"}
                    }
                ]
            },
            {
                "role": "user",
                "content": [
                    {
                        "type": "tool_result",
                        "tool_use_id": "tooluse_missing1",
                        "content": "Sunny, 72°F"
                    }
                ]
            }
        ]
    });

    let request = axum::http::Request::builder()
        .method("POST")
        .uri("/v1/messages")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    let status = response.status();
    let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body_str = String::from_utf8(body_bytes.to_vec()).unwrap();

    println!("status: {status}, body: {body_str}");
}

#[tokio::test]
#[ignore]
async fn v1_messages_count_tokens_returns_token_count() {
    let app = build_app().await;

    let body = serde_json::json!({
        "model": MODEL,
        "messages": [
            {"role": "user", "content": "Hello, world!"}
        ]
    });

    let request = axum::http::Request::builder()
        .method("POST")
        .uri("/v1/messages/count_tokens")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), 200);

    let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

    let input_tokens = json["input_tokens"].as_i64().unwrap();
    assert!(
        input_tokens > 0,
        "expected input_tokens > 0, got: {input_tokens}"
    );
}

#[tokio::test]
#[ignore]
async fn v1_messages_with_context_1m_beta() {
    let app = build_app().await;

    let body = serde_json::json!({
        "model": MODEL,
        "max_tokens": 64,
        "stream": true,
        "messages": [
            {"role": "user", "content": "Say hi in exactly one word."}
        ]
    });

    let request = axum::http::Request::builder()
        .method("POST")
        .uri("/v1/messages")
        .header("content-type", "application/json")
        .header("anthropic-beta", "context-1m-2025-08-07")
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), 200);

    let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body_str = String::from_utf8(body_bytes.to_vec()).unwrap();

    let events = parse_sse_events(&body_str);
    let event_types: Vec<&str> = events.iter().map(|(e, _)| e.as_str()).collect();

    assert!(
        event_types.contains(&"message_start"),
        "missing message_start, got: {event_types:?}"
    );
    assert!(
        event_types.contains(&"message_stop"),
        "missing message_stop, got: {event_types:?}"
    );
    assert_eq!(event_types.first(), Some(&"message_start"));
    assert_eq!(event_types.last(), Some(&"message_stop"));
}

#[tokio::test]
#[ignore]
async fn v1_messages_with_tool_reference_content() {
    let app = build_app().await;

    let body = serde_json::json!({
        "model": MODEL,
        "max_tokens": 64,
        "stream": true,
        "tools": [
            {
                "name": "do_something",
                "description": "Does something and returns nothing.",
                "input_schema": {
                    "type": "object",
                    "properties": {},
                    "required": []
                }
            }
        ],
        "messages": [
            {
                "role": "user",
                "content": "Call do_something."
            },
            {
                "role": "assistant",
                "content": [
                    {
                        "type": "tool_use",
                        "id": "tooluse_ref123",
                        "name": "do_something",
                        "input": {}
                    }
                ]
            },
            {
                "role": "user",
                "content": [
                    {
                        "type": "tool_result",
                        "tool_use_id": "tooluse_ref123",
                        "cache_control": {"type": "ephemeral"},
                        "content": [
                            {
                                "type": "tool_reference",
                                "tool_name": "do_something"
                            }
                        ]
                    }
                ]
            }
        ]
    });

    let request = axum::http::Request::builder()
        .method("POST")
        .uri("/v1/messages")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), 200);

    let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body_str = String::from_utf8(body_bytes.to_vec()).unwrap();

    let events = parse_sse_events(&body_str);
    let event_types: Vec<&str> = events.iter().map(|(e, _)| e.as_str()).collect();

    assert!(
        event_types.contains(&"message_start"),
        "missing message_start, got: {event_types:?}"
    );
    assert!(
        event_types.contains(&"message_stop"),
        "missing message_stop, got: {event_types:?}"
    );
    assert_eq!(event_types.first(), Some(&"message_start"));
    assert_eq!(event_types.last(), Some(&"message_stop"));
}

#[tokio::test]
#[ignore]
async fn v1_messages_with_thinking_disabled() {
    let app = build_app().await;

    let body = serde_json::json!({
        "model": MODEL,
        "max_tokens": 64,
        "stream": true,
        "thinking": {"type": "disabled"},
        "messages": [
            {"role": "user", "content": "Say hi in exactly one word."}
        ]
    });

    let request = axum::http::Request::builder()
        .method("POST")
        .uri("/v1/messages")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), 200);

    let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body_str = String::from_utf8(body_bytes.to_vec()).unwrap();

    let events = parse_sse_events(&body_str);
    let event_types: Vec<&str> = events.iter().map(|(e, _)| e.as_str()).collect();

    assert!(
        event_types.contains(&"message_start"),
        "missing message_start, got: {event_types:?}"
    );
    assert!(
        event_types.contains(&"message_stop"),
        "missing message_stop, got: {event_types:?}"
    );
    assert_eq!(event_types.first(), Some(&"message_start"));
    assert_eq!(event_types.last(), Some(&"message_stop"));

    let has_thinking_block = events.iter().any(|(_, data)| {
        serde_json::from_str::<serde_json::Value>(data)
            .ok()
            .and_then(|v| v.get("content_block").cloned())
            .and_then(|b| b.get("type").cloned())
            .and_then(|t| t.as_str().map(|s| s == "thinking"))
            .unwrap_or(false)
    });
    assert!(
        !has_thinking_block,
        "thinking disabled but got a thinking content block"
    );
}

#[tokio::test]
#[ignore]
async fn v1_messages_with_tools_no_tool_choice_does_not_force_tool_use() {
    let app = build_app().await;

    let body = serde_json::json!({
        "model": MODEL,
        "max_tokens": 64,
        "stream": true,
        "tools": [
            {
                "name": "get_weather",
                "description": "Get the current weather for a location.",
                "input_schema": {
                    "type": "object",
                    "properties": {
                        "location": {"type": "string"}
                    },
                    "required": ["location"]
                }
            }
        ],
        "messages": [
            {"role": "user", "content": "Say hi in exactly one word."}
        ]
    });

    let request = axum::http::Request::builder()
        .method("POST")
        .uri("/v1/messages")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), 200);

    let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body_str = String::from_utf8(body_bytes.to_vec()).unwrap();

    let events = parse_sse_events(&body_str);
    let event_types: Vec<&str> = events.iter().map(|(e, _)| e.as_str()).collect();

    assert!(
        event_types.contains(&"message_start"),
        "missing message_start, got: {event_types:?}"
    );
    assert!(
        event_types.contains(&"message_stop"),
        "missing message_stop, got: {event_types:?}"
    );

    let stop_reason = events
        .iter()
        .find(|(e, _)| e == "message_delta")
        .and_then(|(_, data)| serde_json::from_str::<serde_json::Value>(data).ok())
        .and_then(|v| v.get("delta").cloned())
        .and_then(|d| d.get("stop_reason").cloned())
        .and_then(|r| r.as_str().map(|s| s.to_string()));

    assert_eq!(
        stop_reason.as_deref(),
        Some("end_turn"),
        "expected end_turn (no forced tool use), got: {stop_reason:?}"
    );
}

#[tokio::test]
#[ignore]
async fn v1_messages_with_tool_choice_any_forces_tool_use() {
    let app = build_app().await;

    let body = serde_json::json!({
        "model": MODEL,
        "max_tokens": 256,
        "stream": true,
        "tools": [
            {
                "name": "get_weather",
                "description": "Get the current weather for a location.",
                "input_schema": {
                    "type": "object",
                    "properties": {
                        "location": {"type": "string"}
                    },
                    "required": ["location"]
                }
            }
        ],
        "tool_choice": {"type": "any"},
        "messages": [
            {"role": "user", "content": "Say hi in exactly one word."}
        ]
    });

    let request = axum::http::Request::builder()
        .method("POST")
        .uri("/v1/messages")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), 200);

    let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body_str = String::from_utf8(body_bytes.to_vec()).unwrap();

    let events = parse_sse_events(&body_str);
    let event_types: Vec<&str> = events.iter().map(|(e, _)| e.as_str()).collect();

    assert!(
        event_types.contains(&"message_start"),
        "missing message_start, got: {event_types:?}"
    );
    assert!(
        event_types.contains(&"message_stop"),
        "missing message_stop, got: {event_types:?}"
    );

    let stop_reason = events
        .iter()
        .find(|(e, _)| e == "message_delta")
        .and_then(|(_, data)| serde_json::from_str::<serde_json::Value>(data).ok())
        .and_then(|v| v.get("delta").cloned())
        .and_then(|d| d.get("stop_reason").cloned())
        .and_then(|r| r.as_str().map(|s| s.to_string()));

    assert_eq!(
        stop_reason.as_deref(),
        Some("tool_use"),
        "expected tool_use with tool_choice any, got: {stop_reason:?}"
    );
}

#[tokio::test]
#[ignore]
async fn v1_messages_tool_result_with_image_and_cache_control() {
    let app = build_app().await;

    let tiny_png = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==";

    let body = serde_json::json!({
        "model": MODEL,
        "max_tokens": 64,
        "stream": true,
        "system": [
            {
                "type": "text",
                "text": "You are a helpful assistant."
            },
            {
                "type": "text",
                "cache_control": {"type": "ephemeral"},
                "text": "You have access to tools."
            }
        ],
        "tools": [
            {
                "name": "screenshot",
                "description": "Takes a screenshot and returns it as an image.",
                "input_schema": {
                    "type": "object",
                    "properties": {},
                    "required": []
                },
                "cache_control": {"type": "ephemeral"}
            }
        ],
        "messages": [
            {
                "role": "user",
                "content": "Take a screenshot."
            },
            {
                "role": "assistant",
                "content": [
                    {
                        "type": "tool_use",
                        "id": "tooluse_test123",
                        "name": "screenshot",
                        "input": {},
                        "cache_control": {"type": "ephemeral"}
                    }
                ]
            },
            {
                "role": "user",
                "content": [
                    {
                        "type": "tool_result",
                        "tool_use_id": "tooluse_test123",
                        "cache_control": {"type": "ephemeral"},
                        "content": [
                            {
                                "type": "image",
                                "source": {
                                    "type": "base64",
                                    "media_type": "image/png",
                                    "data": tiny_png
                                }
                            }
                        ]
                    }
                ]
            }
        ]
    });

    let request = axum::http::Request::builder()
        .method("POST")
        .uri("/v1/messages")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), 200);

    let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body_str = String::from_utf8(body_bytes.to_vec()).unwrap();

    let events = parse_sse_events(&body_str);
    let event_types: Vec<&str> = events.iter().map(|(e, _)| e.as_str()).collect();

    assert!(
        event_types.contains(&"message_start"),
        "missing message_start, got: {event_types:?}"
    );
    assert!(
        event_types.contains(&"message_stop"),
        "missing message_stop, got: {event_types:?}"
    );
    assert_eq!(event_types.first(), Some(&"message_start"));
    assert_eq!(event_types.last(), Some(&"message_stop"));
}

#[tokio::test]
#[ignore]
async fn v1_messages_count_tokens_with_tools() {
    let app = build_app().await;

    let body = serde_json::json!({
        "model": MODEL,
        "tools": [
            {
                "name": "get_weather",
                "description": "Get the current weather for a location.",
                "input_schema": {
                    "type": "object",
                    "properties": {
                        "location": {"type": "string"}
                    },
                    "required": ["location"]
                }
            }
        ],
        "messages": [
            {"role": "user", "content": "What is the weather in London?"}
        ]
    });

    let request = axum::http::Request::builder()
        .method("POST")
        .uri("/v1/messages/count_tokens")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), 200);

    let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

    let input_tokens = json["input_tokens"].as_i64().unwrap();
    assert!(
        input_tokens > 0,
        "expected input_tokens > 0, got: {input_tokens}"
    );
}

#[tokio::test]
#[ignore]
async fn v1_messages_count_tokens_with_tools_and_tool_choice() {
    let app = build_app().await;

    let body = serde_json::json!({
        "model": MODEL,
        "tools": [
            {
                "name": "get_weather",
                "description": "Get the current weather for a location.",
                "input_schema": {
                    "type": "object",
                    "properties": {
                        "location": {"type": "string"}
                    },
                    "required": ["location"]
                }
            }
        ],
        "tool_choice": {"type": "any"},
        "messages": [
            {"role": "user", "content": "What is the weather in London?"}
        ]
    });

    let request = axum::http::Request::builder()
        .method("POST")
        .uri("/v1/messages/count_tokens")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), 200);

    let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

    let input_tokens = json["input_tokens"].as_i64().unwrap();
    assert!(
        input_tokens > 0,
        "expected input_tokens > 0, got: {input_tokens}"
    );
}

#[tokio::test]
#[ignore]
async fn v1_messages_with_tool_result_but_no_tools_field() {
    let app = build_app().await;

    let body = serde_json::json!({
        "model": MODEL,
        "max_tokens": 64,
        "stream": true,
        "messages": [
            {
                "role": "user",
                "content": "What's the weather?"
            },
            {
                "role": "assistant",
                "content": [
                    {
                        "type": "tool_use",
                        "id": "tooluse_inject1",
                        "name": "get_weather",
                        "input": {"city": "NYC"}
                    }
                ]
            },
            {
                "role": "user",
                "content": [
                    {
                        "type": "tool_result",
                        "tool_use_id": "tooluse_inject1",
                        "content": "Sunny, 72°F"
                    }
                ]
            }
        ]
    });

    let request = axum::http::Request::builder()
        .method("POST")
        .uri("/v1/messages")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    let status = response.status();
    let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body_str = String::from_utf8(body_bytes.to_vec()).unwrap();
    assert_eq!(status, 200, "response body: {body_str}");

    let events = parse_sse_events(&body_str);
    let event_types: Vec<&str> = events.iter().map(|(e, _)| e.as_str()).collect();

    assert!(
        event_types.contains(&"message_start"),
        "missing message_start, got: {event_types:?}"
    );
    assert!(
        event_types.contains(&"message_stop"),
        "missing message_stop, got: {event_types:?}"
    );
    assert_eq!(event_types.first(), Some(&"message_start"));
    assert_eq!(event_types.last(), Some(&"message_stop"));
}

#[tokio::test]
#[ignore]
async fn chat_completions_with_tool_messages_but_no_tools_field() {
    let app = build_app().await;

    let body = serde_json::json!({
        "model": MODEL,
        "max_tokens": 64,
        "stream": true,
        "messages": [
            {
                "role": "user",
                "content": "What's the weather?"
            },
            {
                "role": "assistant",
                "content": null,
                "tool_calls": [
                    {
                        "id": "call_inject1",
                        "type": "function",
                        "function": {
                            "name": "get_weather",
                            "arguments": "{\"city\":\"NYC\"}"
                        }
                    }
                ]
            },
            {
                "role": "tool",
                "tool_call_id": "call_inject1",
                "content": "Sunny, 72°F"
            }
        ]
    });

    let request = axum::http::Request::builder()
        .method("POST")
        .uri("/chat/completions")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    let status = response.status();
    let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body_str = String::from_utf8(body_bytes.to_vec()).unwrap();
    assert_eq!(status, 200, "response body: {body_str}");

    let events = parse_sse_events(&body_str);
    let data_values: Vec<&str> = events.iter().map(|(_, d)| d.as_str()).collect();

    assert!(
        data_values.contains(&"[DONE]"),
        "missing [DONE] sentinel, got: {data_values:?}"
    );

    assert!(
        data_values.len() >= 2,
        "expected at least 2 events (chunk + DONE), got: {}",
        data_values.len()
    );

    for data in &data_values {
        if *data != "[DONE]" {
            let parsed: serde_json::Value = serde_json::from_str(data)
                .unwrap_or_else(|e| panic!("invalid JSON in SSE data: {e}\ndata: {data}"));
            assert!(
                parsed.get("id").is_some(),
                "chunk missing 'id' field: {parsed}"
            );
        }
    }
}

#[tokio::test]
#[ignore]
async fn v1_messages_with_tools_config_missing_referenced_tool_multiple_rounds() {
    let app = build_app().await;

    // tools config defines "search" only, but messages reference "bash" which is not in tools
    let body = serde_json::json!({
        "model": MODEL,
        "max_tokens": 64,
        "stream": true,
        "tools": [
            {
                "name": "search",
                "description": "Search the web.",
                "input_schema": {
                    "type": "object",
                    "properties": {
                        "query": {"type": "string"}
                    },
                    "required": ["query"]
                }
            }
        ],
        "messages": [
            {
                "role": "user",
                "content": "List files then check disk usage."
            },
            {
                "role": "assistant",
                "content": [
                    {
                        "type": "tool_use",
                        "id": "tooluse_bash1",
                        "name": "bash",
                        "input": {"command": "ls -la"}
                    }
                ]
            },
            {
                "role": "user",
                "content": [
                    {
                        "type": "tool_result",
                        "tool_use_id": "tooluse_bash1",
                        "content": "total 42\ndrwxr-xr-x  5 user staff 160 Jan  1 00:00 .\ndrwxr-xr-x  3 user staff  96 Jan  1 00:00 ..\n-rw-r--r--  1 user staff 1024 Jan  1 00:00 file.txt"
                    }
                ]
            },
            {
                "role": "assistant",
                "content": [
                    {
                        "type": "tool_use",
                        "id": "tooluse_bash2",
                        "name": "bash",
                        "input": {"command": "df -h"}
                    }
                ]
            },
            {
                "role": "user",
                "content": [
                    {
                        "type": "tool_result",
                        "tool_use_id": "tooluse_bash2",
                        "content": "Filesystem      Size  Used Avail Use% Mounted on\n/dev/sda1       100G   50G   50G  50% /"
                    }
                ]
            }
        ]
    });

    let request = axum::http::Request::builder()
        .method("POST")
        .uri("/v1/messages")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    let status = response.status();
    let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body_str = String::from_utf8(body_bytes.to_vec()).unwrap();

    println!("status: {status}, body: {body_str}");
}

#[tokio::test]
#[ignore]
async fn v1_messages_with_multiple_documents() {
    use base64::{Engine as _, engine::general_purpose};

    let app = build_app().await;

    // Minimal valid PDF with text "Document A"
    let pdf_a = b"%PDF-1.0
1 0 obj<</Type/Catalog/Pages 2 0 R>>endobj
2 0 obj<</Type/Pages/Kids[3 0 R]/Count 1>>endobj
3 0 obj<</Type/Page/MediaBox[0 0 612 792]/Parent 2 0 R/Contents 4 0 R/Resources<</Font<</F1 5 0 R>>>>>>endobj
4 0 obj<</Length 44>>stream
BT /F1 12 Tf 100 700 Td (Document A) Tj ET
endstream endobj
5 0 obj<</Type/Font/Subtype/Type1/BaseFont/Helvetica>>endobj
xref
0 6
0000000000 65535 f
0000000009 00000 n
0000000058 00000 n
0000000115 00000 n
0000000266 00000 n
0000000360 00000 n
trailer<</Size 6/Root 1 0 R>>
startxref
430
%%EOF";

    // Minimal valid PDF with text "Document B"
    let pdf_b = b"%PDF-1.0
1 0 obj<</Type/Catalog/Pages 2 0 R>>endobj
2 0 obj<</Type/Pages/Kids[3 0 R]/Count 1>>endobj
3 0 obj<</Type/Page/MediaBox[0 0 612 792]/Parent 2 0 R/Contents 4 0 R/Resources<</Font<</F1 5 0 R>>>>>>endobj
4 0 obj<</Length 44>>stream
BT /F1 12 Tf 100 700 Td (Document B) Tj ET
endstream endobj
5 0 obj<</Type/Font/Subtype/Type1/BaseFont/Helvetica>>endobj
xref
0 6
0000000000 65535 f
0000000009 00000 n
0000000058 00000 n
0000000115 00000 n
0000000266 00000 n
0000000360 00000 n
trailer<</Size 6/Root 1 0 R>>
startxref
430
%%EOF";

    let pdf_data_a = general_purpose::STANDARD.encode(pdf_a);
    let pdf_data_b = general_purpose::STANDARD.encode(pdf_b);

    let body = serde_json::json!({
        "model": MODEL,
        "max_tokens": 64,
        "stream": true,
        "messages": [{
            "role": "user",
            "content": [
                {
                    "type": "document",
                    "source": {
                        "type": "base64",
                        "media_type": "application/pdf",
                        "data": pdf_data_a
                    }
                },
                {
                    "type": "document",
                    "source": {
                        "type": "base64",
                        "media_type": "application/pdf",
                        "data": pdf_data_b
                    }
                },
                {"type": "text", "text": "Compare these two documents in one sentence."}
            ]
        }]
    });

    let request = axum::http::Request::builder()
        .method("POST")
        .uri("/v1/messages")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    let status = response.status();
    let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body_str = String::from_utf8(body_bytes.to_vec()).unwrap();
    assert_eq!(status, 200, "response body: {body_str}");

    let events = parse_sse_events(&body_str);
    let event_types: Vec<&str> = events.iter().map(|(e, _)| e.as_str()).collect();

    assert!(
        event_types.contains(&"message_start"),
        "missing message_start, got: {event_types:?}"
    );
    assert!(
        event_types.contains(&"message_stop"),
        "missing message_stop, got: {event_types:?}"
    );
    assert_eq!(event_types.first(), Some(&"message_start"));
    assert_eq!(event_types.last(), Some(&"message_stop"));
}
