use anthropic_request::V1MessagesRequest;
use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, sse::Sse},
};
use chat::providers::anthropic::{BedrockV1MessagesProvider, V1MessagesProvider};
use tracing::info;

use crate::error::AppError;
use crate::utils::usage_callback;

pub async fn v1_messages(
    Json(payload): Json<V1MessagesRequest>,
) -> Result<impl IntoResponse, AppError> {
    info!(
        "Received Anthropic v1 messages request for model: {}",
        payload.model
    );

    if payload.stream == Some(false) {
        return Err(anyhow::anyhow!("stream cannot be false").into());
    }

    let stream = BedrockV1MessagesProvider::new()
        .await
        .v1_messages_stream(payload, usage_callback)
        .await?;

    Ok((StatusCode::OK, Sse::new(stream)))
}
