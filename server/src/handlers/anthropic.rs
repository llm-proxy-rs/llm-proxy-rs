use anthropic_request::{V1MessagesCountTokensRequest, V1MessagesRequest};
use anthropic_response::V1MessagesCountTokensResponse;
use anyhow::anyhow;
use axum::{
    Json,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, sse::Sse},
};
use chat::provider::{BedrockV1MessagesProvider, V1MessagesProvider};
use std::sync::Arc;
use tracing::{error, info};

use crate::{AppState, error::AppError, utils::usage_callback};

pub async fn v1_messages(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<V1MessagesRequest>,
) -> Result<impl IntoResponse, AppError> {
    info!(
        "Received Anthropic v1/messages request for model: {}",
        payload.model
    );

    if payload.stream == Some(false) {
        error!("Stream is set to false");
        return Err(anyhow!("Stream is set to false").into());
    }

    let stream = BedrockV1MessagesProvider::new(state.bedrockruntime_client.clone())
        .v1_messages_stream(payload, None, usage_callback)
        .await?;

    Ok((StatusCode::OK, Sse::new(stream)))
}

pub async fn v1_messages_count_tokens(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<V1MessagesCountTokensRequest>,
) -> Result<impl IntoResponse, AppError> {
    info!(
        "Received Anthropic v1/messages/count_tokens request for model: {}",
        payload.model
    );

    let provider = BedrockV1MessagesProvider::new(state.bedrockruntime_client.clone());
    let count = provider
        .v1_messages_count_tokens(&payload, &state.inference_profile_prefixes)
        .await?;

    Ok((
        StatusCode::OK,
        Json(V1MessagesCountTokensResponse {
            input_tokens: count,
        }),
    ))
}
