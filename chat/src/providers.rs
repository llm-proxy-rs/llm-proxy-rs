use async_trait::async_trait;
use aws_config::BehaviorVersion;
use aws_sdk_bedrockruntime::Client;
use axum::response::sse::{Event, Sse};
use chrono::offset::Utc;
use futures::stream::Stream;
use request::ChatCompletionsRequest;
use response::{
    ChatCompletionsResponse, converse_stream_output_to_chat_completions_response_builder,
};
use tracing::{debug, error, info, trace};
use uuid::Uuid;

use crate::ProcessChatCompletionsRequest;
use crate::bedrock::{BedrockChatCompletion, process_request_to_bedrock_completion};
use crate::error::StreamError;

const DONE_MESSAGE: &str = "[DONE]";

#[async_trait]
pub trait ChatCompletionsProvider {
    async fn chat_completions_stream(
        self,
        request: ChatCompletionsRequest,
    ) -> anyhow::Result<Sse<impl Stream<Item = Result<Event, StreamError>>>>;
}

pub struct BedrockChatCompletionsProvider {}

impl BedrockChatCompletionsProvider {
    pub async fn new() -> Self {
        Self {}
    }
}

impl ProcessChatCompletionsRequest<BedrockChatCompletion> for BedrockChatCompletionsProvider {
    fn process_chat_completions_request(
        &self,
        request: &ChatCompletionsRequest,
    ) -> BedrockChatCompletion {
        process_request_to_bedrock_completion(request)
    }
}

fn create_sse_event(response: &ChatCompletionsResponse) -> Result<Event, StreamError> {
    match serde_json::to_string(response) {
        Ok(data) => Ok(Event::default().data(data)),
        Err(e) => Err(StreamError(format!("Failed to serialize response: {}", e))),
    }
}

#[async_trait]
impl ChatCompletionsProvider for BedrockChatCompletionsProvider {
    async fn chat_completions_stream(
        self,
        request: ChatCompletionsRequest,
    ) -> anyhow::Result<Sse<impl Stream<Item = Result<Event, StreamError>>>> {
        debug!(
            "Processing chat completions request for model: {}",
            request.model
        );
        let bedrock_chat_completion = self.process_chat_completions_request(&request);
        info!(
            "Processed request to Bedrock format with {} messages",
            bedrock_chat_completion.messages.len()
        );

        debug!("Loading AWS config");
        let config = aws_config::load_defaults(BehaviorVersion::latest()).await;
        let client = Client::new(&config);

        info!(
            "Sending request to Bedrock API for model: {}",
            bedrock_chat_completion.model_id
        );
        let mut stream = client
            .converse_stream()
            .model_id(&bedrock_chat_completion.model_id)
            .set_system(Some(bedrock_chat_completion.system_content_blocks))
            .set_messages(Some(bedrock_chat_completion.messages))
            .send()
            .await?
            .stream;
        info!("Successfully connected to Bedrock stream");

        let id = Uuid::new_v4().to_string();
        let created = Utc::now().timestamp();
        debug!("Created response with id: {}", id);

        let stream = async_stream::stream! {
            trace!("Starting to process stream");
            loop {
                match stream.recv().await {
                    Ok(Some(output)) => {
                        trace!("Received output from Bedrock stream");
                        let builder = converse_stream_output_to_chat_completions_response_builder(&output);
                        let response = builder
                            .id(Some(id.clone()))
                            .created(Some(created))
                            .build();

                        match create_sse_event(&response) {
                            Ok(event) => {
                                trace!("Created SSE event");
                                yield Ok(event);
                            },
                            Err(e) => {
                                error!("Failed to create SSE event: {}", e);
                                yield Err(e);
                            }
                        }
                    }
                    Ok(None) => {
                        debug!("Stream completed");
                        break;
                    }
                    Err(e) => {
                        error!("Error receiving from stream: {}", e);
                        yield Err(StreamError(format!("Stream receive error: {}", e)));
                    }
                }
            }

            info!("Stream finished, sending DONE message");
            yield Ok(Event::default().data(DONE_MESSAGE));
        };

        Ok(Sse::new(stream))
    }
}
