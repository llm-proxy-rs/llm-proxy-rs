use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, sse::Sse},
    routing::post,
};
use chat::openai::OpenAICompletionsProvider;
use chat::providers::{BedrockChatCompletionsProvider, ChatCompletionsProvider};
use config::{Config, File};
use request::ChatCompletionsRequest;
use std::env;
use tracing::{debug, error, info};

mod error;

use crate::error::AppError;

// Define the AppState struct to hold configuration and API keys
#[derive(Clone)]
struct AppState {
    openai_api_key: String,
}

async fn chat_completions(
    State(state): State<AppState>,
    Json(payload): Json<ChatCompletionsRequest>,
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

    // Choose provider based on model name
    let model_name = payload.model.to_lowercase();

    let stream = if model_name.starts_with("gpt-") {
        info!("Using OpenAI provider for model: {}", payload.model);

        // Ensure stream_options with include_usage is set for OpenAI requests
        let mut openai_payload = payload;
        openai_payload.stream_options = Some(request::StreamOptions {
            include_usage: true,
        });

        let provider = OpenAICompletionsProvider::new(state.openai_api_key.clone());
        provider
            .chat_completions_stream(openai_payload, |usage| {
                info!(
                    "Usage: prompt_tokens: {}, completion_tokens: {}, total_tokens: {}",
                    usage.prompt_tokens, usage.completion_tokens, usage.total_tokens
                );
            })
            .await?
    } else {
        info!("Using Bedrock provider for model: {}", payload.model);
        let provider = BedrockChatCompletionsProvider::new().await;
        provider
            .chat_completions_stream(payload, |usage| {
                info!(
                    "Usage: prompt_tokens: {}, completion_tokens: {}, total_tokens: {}",
                    usage.prompt_tokens, usage.completion_tokens, usage.total_tokens
                );
            })
            .await?
    };

    Ok((StatusCode::OK, Sse::new(stream)))
}

async fn load_config() -> anyhow::Result<(String, u16, String)> {
    let settings = Config::builder()
        .add_source(File::with_name("config"))
        .build()?;

    let host: String = settings
        .get("host")
        .unwrap_or_else(|_| "127.0.0.1".to_string());
    let port: u16 = settings.get("port").unwrap_or(3000);

    // Load OpenAI API key from environment variable
    let openai_api_key = env::var("OPENAI_API_KEY")
        .map_err(|_| anyhow::anyhow!("OPENAI_API_KEY environment variable not set"))?;

    Ok((host, port, openai_api_key))
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    info!("Initializing LLM proxy server");

    let (host, port, openai_api_key) = load_config().await?;
    info!("Starting server on {}:{}", host, port);

    // Create AppState with the OpenAI API key
    let state = AppState { openai_api_key };

    let app = Router::new()
        .route("/chat/completions", post(chat_completions))
        .with_state(state);

    info!("Routes configured, binding to {}:{}", host, port);
    let listener = tokio::net::TcpListener::bind(format!("{}:{}", host, port)).await?;
    info!("Server started successfully, listening for requests");

    axum::serve(listener, app).await?;

    Ok(())
}
