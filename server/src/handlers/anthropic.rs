use anthropic_request::{OutputConfig, V1MessagesCountTokensRequest, V1MessagesRequest};
use anthropic_response::V1MessagesCountTokensResponse;
use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, sse::Sse},
};
use chat::provider::{BedrockV1MessagesProvider, V1MessagesProvider};
use common::filter_anthropic_beta;
use std::sync::Arc;
use tracing::info;

use crate::{AppState, error::AppError, utils::log_token_usage};

pub async fn handle_v1_messages(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(payload): Json<V1MessagesRequest>,
) -> Result<impl IntoResponse, AppError> {
    info!(
        "Received Anthropic v1/messages request for model: {}",
        payload.model
    );

    if let Some(ref output_config) = payload.output_config {
        match output_config {
            OutputConfig::Format { .. } => {
                info!("Request includes output_config with JSON schema format");
            }
            OutputConfig::Effort { effort } => {
                info!("Request includes output_config with effort: {}", effort);
            }
            OutputConfig::Other(value) => {
                info!(
                    "Request includes unknown output_config (ignored): {:?}",
                    value
                );
            }
        }
    }

    let anthropic_beta = filter_anthropic_beta(&headers, &state.anthropic_beta_whitelist);
    info!("anthropic_beta: {:?}", anthropic_beta);

    let provider = BedrockV1MessagesProvider::new(state.bedrockruntime_client.clone());

    if payload.stream == Some(true) {
        let stream = provider
            .v1_messages_stream(payload, None, anthropic_beta, log_token_usage)
            .await?;
        return Ok((StatusCode::OK, Sse::new(stream)).into_response());
    }

    let message = provider
        .v1_messages(payload, None, anthropic_beta, log_token_usage)
        .await?;
    Ok((StatusCode::OK, Json(message)).into_response())
}

pub async fn handle_v1_messages_count_tokens(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<V1MessagesCountTokensRequest>,
) -> Result<impl IntoResponse, AppError> {
    info!(
        "Received Anthropic v1/messages/count_tokens request for model: {}",
        payload.model
    );

    let v1_messages_provider = BedrockV1MessagesProvider::new(state.bedrockruntime_client.clone());
    let input_token_count = v1_messages_provider
        .v1_messages_count_tokens(&payload, &state.inference_profile_prefixes)
        .await?;

    Ok((
        StatusCode::OK,
        Json(V1MessagesCountTokensResponse {
            input_tokens: input_token_count,
        }),
    ))
}
