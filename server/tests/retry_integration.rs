use aws_sdk_bedrockruntime::{
    Client,
    config::retry::RetryConfig,
    operation::converse::ConverseOutput as ConverseSendOutput,
    types::{
        ContentBlock, ConversationRole, ConverseOutput as ConverseOutputVariant,
        Message as BedrockMessage, StopReason, TokenUsage,
    },
};
use aws_smithy_mocks::{RuleMode, mock, mock_client};
use axum::body::Body;
use http_body_util::BodyExt;
use server::{AppState, get_app};
use std::sync::Arc;
use std::time::Duration;
use tower::ServiceExt;

/// Retries are owned by the SDK (as configured in `main`); these tests just
/// shrink the backoff so the retry path runs fast. `with_max_attempts(5)` is the
/// SDK's *total* attempt budget (initial + retries).
fn sdk_retry_config() -> RetryConfig {
    RetryConfig::standard()
        .with_max_attempts(5)
        .with_initial_backoff(Duration::from_millis(1))
}

fn successful_converse_output() -> ConverseSendOutput {
    ConverseSendOutput::builder()
        .output(ConverseOutputVariant::Message(
            BedrockMessage::builder()
                .role(ConversationRole::Assistant)
                .content(ContentBlock::Text("ok after retry".to_string()))
                .build()
                .expect("message"),
        ))
        .stop_reason(StopReason::EndTurn)
        .usage(
            TokenUsage::builder()
                .input_tokens(1)
                .output_tokens(1)
                .total_tokens(2)
                .build()
                .expect("usage"),
        )
        .build()
        .expect("converse output")
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

fn v1_messages_request(stream: bool) -> Body {
    let body = serde_json::json!({
        "model": "global.anthropic.claude-opus-4-8",
        "max_tokens": 16,
        "stream": stream,
        "messages": [{"role": "user", "content": "hi"}]
    });
    Body::from(serde_json::to_vec(&body).unwrap())
}

fn post_v1_messages(body: Body) -> axum::http::Request<Body> {
    axum::http::Request::builder()
        .method("POST")
        .uri("/v1/messages")
        .header("content-type", "application/json")
        .body(body)
        .unwrap()
}

/// The SDK retries transient `converse` failures (503) before the proxy returns
/// a successful non-streaming `/v1/messages` response.
#[tokio::test]
async fn v1_messages_non_stream_retries_transient_bedrock_errors() {
    let converse_rule = mock!(aws_sdk_bedrockruntime::Client::converse)
        .sequence()
        .http_status(503, None)
        .times(2)
        .output(successful_converse_output)
        .build();

    let client = mock_client!(
        aws_sdk_bedrockruntime,
        RuleMode::Sequential,
        [&converse_rule],
        |builder| builder.retry_config(sdk_retry_config())
    );

    let app = build_app_with_client(client);
    let response = app
        .oneshot(post_v1_messages(v1_messages_request(false)))
        .await
        .unwrap();
    assert_eq!(response.status(), 200, "expected success after retries");

    let body_bytes = collect_body(response.into_body()).await;
    let json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(json["content"][0]["text"], "ok after retry");
    assert_eq!(
        converse_rule.num_calls(),
        3,
        "expected two 503 retries then success"
    );
}

/// The SDK retries transient `converse_stream` connect failures (503), then
/// stops on a non-retryable validation error. (A success stream output can't be
/// constructed via public API, so we assert retry-then-stop via the call count.)
#[tokio::test]
async fn v1_messages_stream_retries_transient_bedrock_errors_on_connect() {
    let converse_stream_rule = mock!(aws_sdk_bedrockruntime::Client::converse_stream)
        .sequence()
        .http_status(503, None)
        .times(2)
        .http_status(400, Some("invalid request".to_string()))
        .build();

    let client = mock_client!(
        aws_sdk_bedrockruntime,
        RuleMode::Sequential,
        [&converse_stream_rule],
        |builder| builder.retry_config(sdk_retry_config())
    );

    let app = build_app_with_client(client);
    let response = app
        .oneshot(post_v1_messages(v1_messages_request(true)))
        .await
        .unwrap();

    // 503s were retried; the terminal 400 surfaces as a 4xx via `AppError`.
    assert_eq!(response.status(), 400);
    assert_eq!(
        converse_stream_rule.num_calls(),
        3,
        "expected two 503 retries then the validation error"
    );
}

/// Non-retryable Bedrock validation failures (400) are not retried by the SDK.
#[tokio::test]
async fn v1_messages_non_stream_does_not_retry_validation_errors() {
    let converse_rule = mock!(aws_sdk_bedrockruntime::Client::converse)
        .sequence()
        .http_status(400, Some("invalid request".to_string()))
        .build();

    let client = mock_client!(
        aws_sdk_bedrockruntime,
        RuleMode::Sequential,
        [&converse_rule],
        |builder| builder.retry_config(sdk_retry_config())
    );

    let app = build_app_with_client(client);
    let response = app
        .oneshot(post_v1_messages(v1_messages_request(false)))
        .await
        .unwrap();

    assert_eq!(response.status(), 400);
    assert_eq!(converse_rule.num_calls(), 1);
}
