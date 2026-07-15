use anyhow::anyhow;
use axum::{
    body::{Body, Bytes},
    extract::State,
    http::{StatusCode, header::CONTENT_TYPE},
    response::Response,
};
use chat::provider::{MantleV1ResponsesProvider, V1ResponsesProvider};
use std::sync::Arc;
use tracing::{error, info};

use crate::{AppState, error::AppError};

/// Transparent passthrough to Bedrock Mantle's OpenAI Responses API.
pub async fn handle_v1_responses(
    State(state): State<Arc<AppState>>,
    body: Bytes,
) -> Result<Response, AppError> {
    let model = serde_json::from_slice::<serde_json::Value>(&body)
        .ok()
        .and_then(|value| {
            value
                .get("model")
                .and_then(|m| m.as_str())
                .map(str::to_owned)
        });
    info!(
        "Received OpenAI Responses API request for model: {}",
        model.as_deref().unwrap_or("unknown")
    );

    let provider = MantleV1ResponsesProvider::new(
        state.http_client.clone(),
        state.aws_region.clone(),
        state.credentials_provider.clone(),
    );
    let upstream = provider.v1_responses_stream(body.to_vec()).await?;

    let status =
        StatusCode::from_u16(upstream.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
    let content_type = upstream
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("application/json")
        .to_string();

    if !status.is_success() {
        error!("Bedrock Mantle Responses request returned {}", status);
    }

    Response::builder()
        .status(status)
        .header(CONTENT_TYPE, content_type)
        .body(Body::from_stream(upstream.bytes_stream()))
        .map_err(|e| anyhow!("Failed to build Responses proxy response: {}", e).into())
}
