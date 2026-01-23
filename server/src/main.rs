use axum::{Router, routing::post};
use chat::bedrock::ReasoningEffortToThinkingBudgetTokens;
use config::{Config, File};
use std::sync::Arc;
use tracing::info;

mod error;
mod handlers;
mod utils;

use handlers::anthropic::v1_messages;
use handlers::openai::chat_completions;

pub struct AppState {
    pub reasoning_effort_to_thinking_budget_tokens: ReasoningEffortToThinkingBudgetTokens,
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
        reasoning_effort_to_thinking_budget_tokens,
    });

    let app = Router::new()
        .route("/chat/completions", post(chat_completions))
        .route("/v1/messages", post(v1_messages))
        .with_state(state);

    info!("Routes configured, binding to {}:{}", host, port);
    let listener = tokio::net::TcpListener::bind(format!("{host}:{port}")).await?;
    info!("Server started successfully, listening for requests");

    axum::serve(listener, app).await?;

    Ok(())
}
