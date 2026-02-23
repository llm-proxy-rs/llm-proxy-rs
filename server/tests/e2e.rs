use aws_config::BehaviorVersion;
use aws_sdk_bedrockruntime::Client;
use axum::body::Body;
use http_body_util::BodyExt;
use server::{AppState, get_app};
use std::sync::Arc;
use tower::ServiceExt;

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
        "model": "global.anthropic.claude-opus-4-6-v1",
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
        "model": "global.anthropic.claude-opus-4-6-v1",
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
async fn v1_messages_count_tokens_returns_token_count() {
    let app = build_app().await;

    let body = serde_json::json!({
        "model": "global.anthropic.claude-opus-4-6-v1",
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
        "model": "global.anthropic.claude-opus-4-6-v1",
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
async fn v1_messages_tool_result_with_image_and_cache_control() {
    let app = build_app().await;

    let tiny_png = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==";

    let body = serde_json::json!({
        "model": "global.anthropic.claude-opus-4-6-v1",
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
