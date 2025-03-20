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
        request: ChatCompletionsRequest,
        usage_callback: F,
    ) -> anyhow::Result<BoxStream<'async_trait, anyhow::Result<Event>>>
    where
        F: Fn(&Usage) + Send + Sync + 'static,
    {
        let client = reqwest::Client::new();
        let response = client
            .post(OPENAI_API_CHAT_COMPLETIONS_URL)
            .header("Authorization", format!("Bearer {}", self.openai_api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await?;
            return Err(anyhow::anyhow!(
                "OpenAI API error: {} - {}",
                status,
                error_text
            ));
        }

        let stream = stream! {
            let mut stream = response
                .json_array_stream::<ChatCompletionsResponse>(1024 * 1024);

            while let Some(item) = stream.next().await {
                match item {
                    Ok(response) => {
                        // Call usage callback if usage data is available
                        if let Some(usage) = &response.usage {
                            usage_callback(usage);
                        }

                        // Create SSE event from response
                        match create_sse_event(&response) {
                            Ok(event) => yield Ok(event),
                            Err(e) => yield Err(e),
                        }
                    }
                    Err(e) => {
                        let error = anyhow::anyhow!("Failed to parse response: {}", e);
                        yield Err(error);
                    }
                }
            }
            yield Ok(Event::default().data(DONE_MESSAGE));
        };

        Ok(Box::pin(stream))
    }
}
