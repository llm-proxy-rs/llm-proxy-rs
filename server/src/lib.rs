use aws_sdk_bedrockruntime::Client;
use axum::{Router, routing::post};
use std::sync::Arc;

pub mod error;
pub mod handlers;
pub mod utils;

use handlers::anthropic::{handle_v1_messages, handle_v1_messages_count_tokens};
use handlers::openai::handle_chat_completions;

pub struct AppState {
    pub bedrockruntime_client: Client,
    pub inference_profile_prefixes: Vec<String>,
    pub anthropic_beta_whitelist: Vec<String>,
}

pub fn get_app(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/chat/completions", post(handle_chat_completions))
        .route("/v1/messages", post(handle_v1_messages))
        .route(
            "/v1/messages/count_tokens",
            post(handle_v1_messages_count_tokens),
        )
        .with_state(state)
}
