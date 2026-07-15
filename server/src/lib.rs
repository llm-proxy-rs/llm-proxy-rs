use aws_credential_types::provider::SharedCredentialsProvider;
use aws_sdk_bedrockruntime::Client;
use axum::{Router, routing::post};
use reqwest::Client as HttpClient;
use std::sync::Arc;

pub mod error;
pub mod handlers;
pub mod utils;

use handlers::v1_messages::{handle_v1_messages, handle_v1_messages_count_tokens};
use handlers::v1_responses::handle_v1_responses;

pub struct AppState {
    pub bedrockruntime_client: Client,
    pub inference_profile_prefixes: Vec<String>,
    pub anthropic_beta_whitelist: Vec<String>,
    pub aws_region: String,
    pub credentials_provider: SharedCredentialsProvider,
    pub http_client: HttpClient,
}

pub fn get_app(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/v1/responses", post(handle_v1_responses))
        .route("/v1/messages", post(handle_v1_messages))
        .route(
            "/v1/messages/count_tokens",
            post(handle_v1_messages_count_tokens),
        )
        .with_state(state)
}
