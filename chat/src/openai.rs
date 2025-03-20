use crate::{DONE_MESSAGE, create_sse_event, providers::ChatCompletionsProvider};
use anyhow::Result;
use async_stream::stream;
use async_trait::async_trait;
use axum::response::sse::Event;
use futures::StreamExt;
use futures::stream::BoxStream;
use request::ChatCompletionsRequest;
use reqwest;
use reqwest_streams::JsonStreamResponse as _;
use response::{ChatCompletionsResponse, Usage};
use std::sync::Arc;
use tracing::{debug, info};

pub const OPENAI_API_CHAT_COMPLETIONS_URL: &str = "https://api.openai.com/v1/chat/completions";

pub struct OpenAICompletionsProvider {
    openai_api_key: String,
}

impl OpenAICompletionsProvider {
    pub fn new(openai_api_key: String) -> Self {
        Self { openai_api_key }
    }
}

#[async_trait]
impl ChatCompletionsProvider for OpenAICompletionsProvider {
    async fn chat_completions_stream<F>(
        self,
        mut request: ChatCompletionsRequest,
        usage_callback: F,
    ) -> Result<BoxStream<'async_trait, anyhow::Result<Event>>>
    where
        F: Fn(&Usage) + Send + Sync + 'static,
    {
        debug!("Creating OpenAI chat completions request");
        let client = reqwest::Client::new();

        request.stream = Some(true);

        let usage_callback = Arc::new(usage_callback);

        info!("Sending request to OpenAI API");
        let response = client
            .post(OPENAI_API_CHAT_COMPLETIONS_URL)
            .header("Authorization", format!("Bearer {}", self.openai_api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await?;

        if response.status().is_success() {
            debug!("Successfully connected to OpenAI stream");

            let stream = stream! {
                let mut json_stream = response.json_array_stream::<ChatCompletionsResponse>(1024 * 1024);

                while let Some(item) = json_stream.next().await {
                    match item {
                        Ok(chunk) => {
                            debug!("Received chunk from OpenAI stream");

                            if let Some(ref usage) = chunk.usage {
                                info!("Received usage information from OpenAI");
                                usage_callback(usage);
                            }

                            match create_sse_event(&chunk) {
                                Ok(event) => {
                                    debug!("Created SSE event");
                                    yield Ok(event);
                                },
                                Err(e) => {
                                    info!("Error creating SSE event: {}", e);
                                    yield Err(anyhow::anyhow!("Error creating SSE event: {}", e));
                                }
                            }
                        }
                        Err(e) => {
                            info!("Error parsing stream chunk: {}", e);
                            yield Err(anyhow::anyhow!("Error parsing stream: {}", e));
                        }
                    }
                }

                info!("Stream finished, sending DONE message");
                yield Ok(Event::default().data(DONE_MESSAGE));
            };

            Ok(stream.boxed())
        } else {
            Err(anyhow::anyhow!(
                "Failed to get chat completions: {}",
                response.status()
            ))
        }
    }
}
