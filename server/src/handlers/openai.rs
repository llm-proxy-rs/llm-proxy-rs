use axum::{
    Json,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, sse::Sse},
};
use chat::providers::openai::{BedrockChatCompletionsProvider, ChatCompletionsProvider};
use request::ChatCompletionsRequest;
use std::sync::Arc;
use tracing::info;

use crate::AppState;
use crate::error::AppError;
use crate::utils::usage_callback;

pub async fn chat_completions(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ChatCompletionsRequest>,
) -> Result<impl IntoResponse, AppError> {
    info!(
        "Received OpenAI chat completions request for model: {}",
        payload.model
    );

    if payload.stream == Some(false) {
        return Err(anyhow::anyhow!("stream cannot be false").into());
    }

    let stream = BedrockChatCompletionsProvider::new()
        .await
        .chat_completions_stream(
            payload,
            state.reasoning_effort_to_thinking_budget_tokens.clone(),
            usage_callback,
        )
        .await?;

    Ok((StatusCode::OK, Sse::new(stream)))
}
