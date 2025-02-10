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
use uuid::Uuid;

use crate::ProcessChatCompletionsRequest;
use crate::error::StreamError;

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

fn create_sse_event(response: &ChatCompletionsResponse) -> Result<Event, StreamError> {
    serde_json::to_string(response)
        .map(|data| Event::default().data(data))
        .map_err(|e| StreamError(e.to_string()))
}

#[async_trait]
impl ChatCompletionsProvider for BedrockChatCompletionsProvider {
    async fn chat_completions_stream(
        self,
        request: ChatCompletionsRequest,
    ) -> anyhow::Result<Sse<impl Stream<Item = Result<Event, StreamError>>>> {
        let bedrock_chat_completion = request.process_chat_completions_request();

        let config = aws_config::load_defaults(BehaviorVersion::latest()).await;
        let client = Client::new(&config);

        let mut stream = client
            .converse_stream()
            .model_id(&bedrock_chat_completion.model_id)
            .set_system(Some(bedrock_chat_completion.system_content_blocks))
            .set_messages(Some(bedrock_chat_completion.messages))
            .send()
            .await?
            .stream;

        let id = Uuid::new_v4().to_string();
        let created = Utc::now().timestamp();

        let stream = async_stream::stream! {
            loop {
                match stream.recv().await {
                    Ok(Some(output)) => {
                        let builder = converse_stream_output_to_chat_completions_response_builder(&output);
                        let response = builder
                            .id(Some(id.clone()))
                            .created(Some(created))
                            .build();
                        let event = create_sse_event(&response)?;
                        yield Ok(event);
                    }
                    Ok(None) => {
                        break;
                    }
                    Err(e) => {
                        yield Err(StreamError(e.to_string()));
                    }
                }
            }

            yield Ok(Event::default().data("[DONE]"));
        };

        Ok(Sse::new(stream))
    }
}
