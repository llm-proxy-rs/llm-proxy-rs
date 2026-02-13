use aws_config::BehaviorVersion;
use aws_sdk_bedrockruntime::Client;
use axum::{Router, routing::post};
use chat::bedrock::ReasoningEffortToThinkingBudgetTokens;
use config::{Config, File};
use std::sync::Arc;
use tracing::info;

mod error;
mod handlers;
mod utils;

use handlers::anthropic::{v1_messages, v1_messages_count_tokens};
use handlers::openai::chat_completions;

pub struct AppState {
    pub bedrockruntime_client: Client,
    pub reasoning_effort_to_thinking_budget_tokens: ReasoningEffortToThinkingBudgetTokens,
    pub inference_profile_prefixes: Vec<String>,
}

async fn load_config() -> anyhow::Result<(
    String,
    u16,
    ReasoningEffortToThinkingBudgetTokens,
    Vec<String>,
)> {
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

    let inference_profile_prefixes: Vec<String> = settings
        .get("inference_profile_prefixes")
        .unwrap_or_else(|_| vec!["us.".to_string()]);

    info!(
        "reasoning_effort to thinking budget_tokens - low: {}, medium: {}, high: {}",
        reasoning_effort_to_thinking_budget_tokens.low,
        reasoning_effort_to_thinking_budget_tokens.medium,
        reasoning_effort_to_thinking_budget_tokens.high
    );

    info!(
        "inference_profile_prefixes: {:?}",
        inference_profile_prefixes
    );

    Ok((
        host,
        port,
        reasoning_effort_to_thinking_budget_tokens,
        inference_profile_prefixes,
    ))
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    info!("Initializing LLM proxy server");

    let (host, port, reasoning_effort_to_thinking_budget_tokens, inference_profile_prefixes) =
        load_config().await?;
    info!("Starting server on {}:{}", host, port);

    let config = aws_config::load_defaults(BehaviorVersion::latest()).await;
    let bedrockruntime_client = Client::new(&config);
    info!("AWS Bedrock client initialized");

    let state = Arc::new(AppState {
        bedrockruntime_client,
        reasoning_effort_to_thinking_budget_tokens,
        inference_profile_prefixes,
    });

    let app = Router::new()
        .route("/chat/completions", post(chat_completions))
        .route("/v1/messages", post(v1_messages))
        .route("/v1/messages/count_tokens", post(v1_messages_count_tokens))
        .with_state(state);

    info!("Routes configured, binding to {}:{}", host, port);
    let listener = tokio::net::TcpListener::bind(format!("{host}:{port}")).await?;
    info!("Server started successfully, listening for requests");

    axum::serve(listener, app).await?;

    Ok(())
}
