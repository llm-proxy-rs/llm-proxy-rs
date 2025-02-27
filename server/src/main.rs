use axum::{
    Json, Router,
    response::sse::{Event, Sse},
    routing::post,
};
use chat::{
    error::StreamError,
    providers::{BedrockChatCompletionsProvider, ChatCompletionsProvider},
};
use config::{Config, File};
use futures::stream::Stream;
use request::ChatCompletionsRequest;
use tracing::{debug, error, info};

mod error;

use crate::error::AppError;

async fn chat_completions(
    Json(payload): Json<ChatCompletionsRequest>,
) -> Result<Sse<impl Stream<Item = Result<Event, StreamError>>>, AppError> {
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

    let provider = BedrockChatCompletionsProvider::new().await;
    info!(
        "Processing chat completions request with {} messages",
        payload.messages.len()
    );
    Ok(provider.chat_completions_stream(payload).await?)
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

    let app = Router::new()
        .route("/chat/completions", post(chat_completions))
        .fallback(|| async { "LLM Proxy API - Route not found" });

    info!("Routes configured, binding to {}:{}", host, port);
    let listener = tokio::net::TcpListener::bind(format!("{}:{}", host, port)).await?;
    info!("Server started successfully, listening for requests");

    axum::serve(listener, app).await?;

    Ok(())
}
