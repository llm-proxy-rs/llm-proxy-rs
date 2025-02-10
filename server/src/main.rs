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

mod error;

use crate::error::AppError;

async fn chat_completions(
    Json(payload): Json<ChatCompletionsRequest>,
) -> Result<Sse<impl Stream<Item = Result<Event, StreamError>>>, AppError> {
    if payload.stream == Some(false) {
        return Err(AppError::from(anyhow::anyhow!("Streaming is disabled")));
    }

    let provider = BedrockChatCompletionsProvider::new().await;
    Ok(provider.chat_completions_stream(payload).await?)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let settings = Config::builder()
        .add_source(File::with_name("config"))
        .build()?;

    let host: String = settings.get("host")?;
    let port: u16 = settings.get("port")?;

    let app = Router::new().route("/chat/completions", post(chat_completions));

    let listener = tokio::net::TcpListener::bind(format!("{}:{}", host, port)).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
