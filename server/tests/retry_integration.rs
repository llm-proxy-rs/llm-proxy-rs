use aws_sdk_bedrockruntime::{
    Client,
    config::RetryConfig,
    operation::converse::ConverseOutput as ConverseSendOutput,
    types::{
        ContentBlock, ConversationRole, ConverseOutput as ConverseOutputVariant,
        Message as BedrockMessage, StopReason, TextBlock, TokenUsage,
    },
};
use aws_smithy_mocks::{RuleMode, mock, mock_client};
use axum::body::Body;
use http_body_util::BodyExt;
use server::{AppState, get_app};
use std::sync::Arc;
use tower::ServiceExt;

fn successful_converse_output() -> ConverseSendOutput {
    ConverseSendOutput::builder()
        .output(ConverseOutputVariant::Message(
            BedrockMessage::builder()
                .role(ConversationRole::Assistant)
                .content(ContentBlock::Text(
                    TextBlock::builder()
                        .text("ok after retry")
                        .build()
                        .expect("text block"),
                ))
                .build(),
        ))
        .stop_reason(StopReason::EndTurn)
        .usage(
            TokenUsage::builder()
                .input_tokens(1)
                .output_tokens(1)
                .total_tokens(2)
                .build(),
        )
        .build()
}

fn build_app_with_client(client: Client) -> axum::Router {
    let state = Arc::new(AppState {
        bedrockruntime_client: client,
        inference_profile_prefixes: vec!["us.".to_string(), "global.".to_string()],
        anthropic_beta_whitelist: vec![],
    });
    get_app(state)
}

async fn collect_body(body: axum::body::Body) -> Vec<u8> {
    body.collect()
        .await
        .expect("failed to collect response body")
        .to_bytes()
        .to_vec()
}

/// Verifies the proxy retries transient Bedrock `converse` failures before returning
/// a successful non-streaming `/v1/messages` response.
#[tokio::test]
async fn v1_messages_non_stream_retries_transient_bedrock_errors() {
    let converse_rule = mock!(aws_sdk_bedrockruntime::Client::converse)
        .sequence()
        .http_status(429, None)
        .times(2)
        .output(|| successful_converse_output())
        .build();

    let client = mock_client!(
        aws_sdk_bedrockruntime,
        RuleMode::Sequential,
        [&converse_rule],
        |builder| builder.retry_config(RetryConfig::disabled())
    );

    let app = build_app_with_client(client);

    let body = serde_json::json!({
        "model": "global.anthropic.claude-opus-4-8",
        "max_tokens": 16,
        "messages": [{"role": "user", "content": "hi"}]
    });

    let request = axum::http::Request::builder()
        .method("POST")
        .uri("/v1/messages")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), 200, "expected success after retries");

    let body_bytes = collect_body(response.into_body()).await;
    let json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(json["content"][0]["text"], "ok after retry");
    assert_eq!(
        converse_rule.num_calls(),
        3,
        "expected two 429 retries then success"
    );
}

/// Verifies streaming connect retries transient Bedrock `converse_stream` failures.
#[tokio::test]
async fn v1_messages_stream_retries_transient_bedrock_errors_on_connect() {
    let converse_stream_rule = mock!(aws_sdk_bedrockruntime::Client::converse_stream)
        .sequence()
        .http_status(503, None)
        .times(1)
        .output(|| {
            use aws_sdk_bedrockruntime::{
                event_receiver::EventReceiver,
                operation::converse_stream::ConverseOutput as ConverseStreamSendOutput,
            };

            let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
            drop(tx);

            ConverseStreamSendOutput::builder()
                .stream(EventReceiver::new(rx))
                .build()
                .expect("stream output")
        })
        .build();

    let client = mock_client!(
        aws_sdk_bedrockruntime,
        RuleMode::Sequential,
        [&converse_stream_rule],
        |builder| builder.retry_config(RetryConfig::disabled())
    );

    let app = build_app_with_client(client);

    let body = serde_json::json!({
        "model": "global.anthropic.claude-opus-4-8",
        "max_tokens": 16,
        "stream": true,
        "messages": [{"role": "user", "content": "hi"}]
    });

    let request = axum::http::Request::builder()
        .method("POST")
        .uri("/v1/messages")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(
        response.status(),
        200,
        "expected stream connect to succeed after retry"
    );

    let body_bytes = collect_body(response.into_body()).await;
    let body_str = String::from_utf8(body_bytes).expect("utf8 body");
    assert!(
        body_str.contains("event:") || body_str.is_empty(),
        "expected SSE body, got: {body_str:?}"
    );
    assert_eq!(
        converse_stream_rule.num_calls(),
        2,
        "expected one 503 retry then success"
    );
}

/// Verifies non-retryable Bedrock validation failures are not retried.
#[tokio::test]
async fn v1_messages_non_stream_does_not_retry_validation_errors() {
    let converse_rule = mock!(aws_sdk_bedrockruntime::Client::converse)
        .sequence()
        .http_status(400, Some("invalid request"))
        .build();

    let client = mock_client!(
        aws_sdk_bedrockruntime,
        RuleMode::Sequential,
        [&converse_rule],
        |builder| builder.retry_config(RetryConfig::disabled())
    );

    let app = build_app_with_client(client);

    let body = serde_json::json!({
        "model": "global.anthropic.claude-opus-4-8",
        "max_tokens": 16,
        "messages": [{"role": "user", "content": "hi"}]
    });

    let request = axum::http::Request::builder()
        .method("POST")
        .uri("/v1/messages")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), 400);
    assert_eq!(converse_rule.num_calls(), 1);
}
