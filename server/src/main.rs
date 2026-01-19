use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, sse::Sse},
    routing::post,
};
use chat::{
    bedrock::ReasoningEffortToThinkingBudgetTokens,
    providers::{BedrockChatCompletionsProvider, ChatCompletionsProvider},
};
use config::{Config, File};
use request::{ChatCompletionsRequest, StreamOptions};
use response::Usage;
use std::sync::Arc;
use tracing::{debug, error, info};

mod error;

use crate::error::AppError;

#[derive(Clone)]
struct AppState {
    reasoning_effort_to_thinking_budget_tokens: Arc<ReasoningEffortToThinkingBudgetTokens>,
}

async fn chat_completions(
    State(state): State<Arc<AppState>>,
    Json(mut payload): Json<ChatCompletionsRequest>,
) -> Result<impl IntoResponse, AppError> {
    debug!(
        "Received chat completions request for model: {}",
        payload.model
    );

    if payload.stream == Some(false) {
        error!("Streaming is required but was disabled");
        return Err(AppError::from(anyhow::anyhow!(
            "Streaming is required but was disabled"
        )));
    }

    payload.stream_options = Some(StreamOptions {
        include_usage: true,
    });

    let usage_callback = |usage: &Usage| {
        info!(
            "Usage: prompt_tokens: {}, completion_tokens: {}, total_tokens: {}",
            usage.prompt_tokens, usage.completion_tokens, usage.total_tokens
        );
    };

    info!("Using Bedrock provider for model: {}", payload.model);

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

async fn load_config() -> anyhow::Result<(String, u16, ReasoningEffortToThinkingBudgetTokens)> {
    let settings = Config::builder()
        .add_source(File::with_name("config"))
        .build()?;

    let host: String = settings
        .get("host")
        .unwrap_or_else(|_| "127.0.0.1".to_string());
    let port: u16 = settings.get("port").unwrap_or(3000);

    let reasoning_effort_to_thinking_budget_tokens: ReasoningEffortToThinkingBudgetTokens =
        settings
            .get("reasoning_effort_to_thinking_budget_tokens")
            .unwrap_or_else(|_| ReasoningEffortToThinkingBudgetTokens::default());

    info!(
        "reasoning_effort to thinking budget_tokens - low: {}, medium: {}, high: {}",
        reasoning_effort_to_thinking_budget_tokens.low,
        reasoning_effort_to_thinking_budget_tokens.medium,
        reasoning_effort_to_thinking_budget_tokens.high
    );

    Ok((host, port, reasoning_effort_to_thinking_budget_tokens))
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    info!("Initializing LLM proxy server");

    let (host, port, reasoning_effort_to_thinking_budget_tokens) = load_config().await?;
    info!("Starting server on {}:{}", host, port);

    let state = Arc::new(AppState {
        reasoning_effort_to_thinking_budget_tokens: Arc::new(
            reasoning_effort_to_thinking_budget_tokens,
        ),
    });

    let app = Router::new()
        .route("/chat/completions", post(chat_completions))
        .with_state(state);

    info!("Routes configured, binding to {}:{}", host, port);
    let listener = tokio::net::TcpListener::bind(format!("{host}:{port}")).await?;
    info!("Server started successfully, listening for requests");

    axum::serve(listener, app).await?;

    Ok(())
}
