use crate::{
    DONE_MESSAGE,
    bedrock::{
        ReasoningEffortToThinkingBudgetTokens,
        process_chat_completions_request_to_bedrock_chat_completion,
    },
    create_sse_event,
};
use async_trait::async_trait;
use aws_config::BehaviorVersion;
use aws_sdk_bedrockruntime::Client;
use aws_sdk_bedrockruntime::primitives::event_stream::EventReceiver;
use aws_sdk_bedrockruntime::types::{TokenUsage, error::ConverseStreamOutputError};
use axum::response::sse::Event;
use chrono::offset::Utc;
use futures::stream::{BoxStream, StreamExt};
use request::ChatCompletionsRequest;
use response::converse_stream_output_to_chat_completions_response_builder;
use std::sync::Arc;
use tracing::info;
use uuid::Uuid;

async fn process_bedrock_stream(
    mut stream: EventReceiver<
        aws_sdk_bedrockruntime::types::ConverseStreamOutput,
        ConverseStreamOutputError,
    >,
    id: String,
    created: i64,
    usage_callback: Arc<dyn Fn(&TokenUsage) + Send + Sync>,
) -> BoxStream<'static, anyhow::Result<Event>> {
    let stream = async_stream::stream! {
        loop {
            match stream.recv().await {
                Ok(Some(output)) => {
                    let usage_callback = usage_callback.clone();
                    if let Some(builder) = converse_stream_output_to_chat_completions_response_builder(&output, usage_callback) {
                        let response = builder
                            .id(Some(id.clone()))
                            .created(Some(created))
                            .build();

                        match create_sse_event(&response) {
                            Ok(event) => {
                                yield Ok(event);
                            },
                            Err(e) => {
                                yield Err(e);
                            }
                        }
                    }
                }
                Ok(None) => {
                    break;
                }
                Err(e) => {
                    yield Err(anyhow::anyhow!(
                        "Stream receive error: {}",
                        e
                    ));
                }
            }
        }

        info!("Stream finished, sending DONE message");
        yield Ok(Event::default().data(DONE_MESSAGE));
    };

    stream.boxed()
}

#[async_trait]
pub trait ChatCompletionsProvider {
    async fn chat_completions_stream<F>(
        self,
        request: ChatCompletionsRequest,
        reasoning_effort_to_thinking_budget_tokens: Arc<ReasoningEffortToThinkingBudgetTokens>,
        usage_callback: F,
    ) -> anyhow::Result<BoxStream<'async_trait, anyhow::Result<Event>>>
    where
        F: Fn(&TokenUsage) + Send + Sync + 'static;
}

pub struct BedrockChatCompletionsProvider {}

impl BedrockChatCompletionsProvider {
    pub async fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl ChatCompletionsProvider for BedrockChatCompletionsProvider {
    async fn chat_completions_stream<F>(
        self,
        request: ChatCompletionsRequest,
        reasoning_effort_to_thinking_budget_tokens: Arc<ReasoningEffortToThinkingBudgetTokens>,
        usage_callback: F,
    ) -> anyhow::Result<BoxStream<'async_trait, anyhow::Result<Event>>>
    where
        F: Fn(&TokenUsage) + Send + Sync + 'static,
    {
        let bedrock_chat_completion = process_chat_completions_request_to_bedrock_chat_completion(
            &request,
            &reasoning_effort_to_thinking_budget_tokens,
        )?;
        info!(
            "Processed request to Bedrock format with {} messages",
            bedrock_chat_completion.messages.len()
        );

        let config = aws_config::load_defaults(BehaviorVersion::latest()).await;
        let client = Client::new(&config);

        info!(
            "Sending request to Bedrock API for model: {}",
            bedrock_chat_completion.model_id
        );

        let converse_builder = client
            .converse_stream()
            .model_id(&bedrock_chat_completion.model_id)
            .set_system(Some(bedrock_chat_completion.system_content_blocks))
            .set_messages(Some(bedrock_chat_completion.messages))
            .set_tool_config(bedrock_chat_completion.tool_config)
            .set_inference_config(Some(bedrock_chat_completion.inference_config))
            .set_additional_model_request_fields(
                bedrock_chat_completion.additional_model_request_fields,
            );

        let stream = converse_builder.send().await?.stream;
        info!("Successfully connected to Bedrock stream");

        let id = Uuid::new_v4().to_string();
        let created = Utc::now().timestamp();

        let usage_callback = Arc::new(usage_callback);

        Ok(process_bedrock_stream(stream, id, created, usage_callback).await)
    }
}
