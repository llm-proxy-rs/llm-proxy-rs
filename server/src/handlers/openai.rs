use axum::{
    Json,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, sse::Sse},
};
use chat::providers::{BedrockChatCompletionsProvider, ChatCompletionsProvider};
use request::{ChatCompletionsRequest, StreamOptions};
use std::sync::Arc;
use tracing::{error, info};

use crate::{AppState, error::AppError, utils::usage_callback};

pub async fn chat_completions(
    State(state): State<Arc<AppState>>,
    Json(mut payload): Json<ChatCompletionsRequest>,
) -> Result<impl IntoResponse, AppError> {
    info!(
        "Received OpenAI chat completions request for model: {}",
        payload.model
    );

    if payload.stream == Some(false) {
        error!("Stream is set to false");
        return Err(anyhow::anyhow!("Stream is set to false").into());
    }

    payload.stream_options = Some(StreamOptions {
        include_usage: true,
    });

    info!("Using Bedrock provider for model: {}", payload.model);

    let stream = BedrockChatCompletionsProvider::new()
        .await
        .chat_completions_stream(
            payload,
            state.reasoning_effort_to_thinking_budget_tokens,
            usage_callback,
        )
        .await?;

    Ok((StatusCode::OK, Sse::new(stream)))
}
