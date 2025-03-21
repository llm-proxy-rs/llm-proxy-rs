use crate::{DONE_MESSAGE, create_sse_event, providers::ChatCompletionsProvider};
use async_stream::stream;
use async_trait::async_trait;
use axum::response::sse::Event;
use futures::StreamExt;
use futures::stream::BoxStream;
use request::ChatCompletionsRequest;
use reqwest;
use reqwest_streams::JsonStreamResponse as _;
use response::{ChatCompletionsResponse, Usage};
use tracing::{debug, error, info};

pub const OPENAI_API_CHAT_COMPLETIONS_URL: &str = "https://api.openai.com/v1/chat/completions";

pub struct OpenAIChatCompletionsProvider {
    openai_api_key: String,
}

impl OpenAIChatCompletionsProvider {
    pub fn new(openai_api_key: &str) -> Self {
        Self {
            openai_api_key: openai_api_key.to_string(),
        }
    }
}

#[async_trait]
impl ChatCompletionsProvider for OpenAIChatCompletionsProvider {
    async fn chat_completions_stream<F>(
        self,
        request: ChatCompletionsRequest,
        usage_callback: F,
    ) -> anyhow::Result<BoxStream<'async_trait, anyhow::Result<Event>>>
    where
        F: Fn(&Usage) + Send + Sync + 'static,
    {
        debug!(
            "Starting OpenAI chat completion request with model: {}",
            request.model
        );

        let client = reqwest::Client::new();
        let response = client
            .post(OPENAI_API_CHAT_COMPLETIONS_URL)
            .header("Authorization", format!("Bearer {}", self.openai_api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await?;

        let status = response.status();
        debug!("OpenAI API response status: {}", status);

        if !status.is_success() {
            let error_text = response.text().await?;
            error!("OpenAI API error: {} - {}", status, error_text);
            anyhow::bail!("OpenAI API error: {} - {}", status, error_text);
        }

        info!("Successfully connected to OpenAI API, starting stream processing");

        let stream = stream! {
            let mut stream = response
                .json_array_stream::<ChatCompletionsResponse>(1024 * 1024);

            while let Some(item) = stream.next().await {
                match item {
                    Ok(response) => {
                        if let Some(usage) = &response.usage {
                            debug!("Received usage data: prompt_tokens={}, completion_tokens={}, total_tokens={}",
                                  usage.prompt_tokens, usage.completion_tokens, usage.total_tokens);
                            usage_callback(usage);
                        }

                        match create_sse_event(&response) {
                            Ok(event) => yield Ok(event),
                            Err(e) => {
                                error!("Failed to create SSE event: {}", e);
                                yield Err(e);
                            }
                        }
                    }
                    Err(e) => {
                        error!("Failed to parse OpenAI response: {}", e);
                        let error = anyhow::anyhow!("Failed to parse response: {}", e);
                        yield Err(error);
                    }
                }
            }
            info!("OpenAI stream completed, sending DONE message");
            yield Ok(Event::default().data(DONE_MESSAGE));
        };

        Ok(stream.boxed())
    }
}
