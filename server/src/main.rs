use axum::{
    Json, Router,
    http::StatusCode,
    response::{IntoResponse, sse::Sse},
    routing::post,
};
use chat::providers::{BedrockChatCompletionsProvider, ChatCompletionsProvider};
use config::{Config, File};
use request::{ChatCompletionsRequest, StreamOptions};
use response::Usage;
use tracing::{debug, error, info};

mod error;

use crate::error::AppError;

async fn chat_completions(
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
        .chat_completions_stream(payload, usage_callback)
        .await?;

    Ok((StatusCode::OK, Sse::new(stream)))
}

async fn load_config() -> anyhow::Result<(String, u16)> {
    let settings = Config::builder()
        .add_source(File::with_name("config"))
        .build()?;

    let host: String = settings
        .get("host")
        .unwrap_or_else(|_| "127.0.0.1".to_string());
    let port: u16 = settings.get("port").unwrap_or(3000);

    Ok((host, port))
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    info!("Initializing LLM proxy server");

    let (host, port) = load_config().await?;
    info!("Starting server on {}:{}", host, port);

    let app = Router::new().route("/chat/completions", post(chat_completions));

    info!("Routes configured, binding to {}:{}", host, port);
    let listener = tokio::net::TcpListener::bind(format!("{host}:{port}")).await?;
    info!("Server started successfully, listening for requests");

    axum::serve(listener, app).await?;

    Ok(())
}
