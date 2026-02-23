use crate::{DONE_MESSAGE, create_sse_event};
use anthropic_request::{get_additional_model_request_fields, output_config::OutputConfig};
use anyhow::anyhow;
use async_trait::async_trait;
use aws_sdk_bedrockruntime::Client;
use aws_sdk_bedrockruntime::primitives::event_stream::EventReceiver;
use aws_sdk_bedrockruntime::types::{TokenUsage, error::ConverseStreamOutputError};
use axum::response::sse::Event;
use chrono::offset::Utc;
use futures::stream::{BoxStream, StreamExt};
use request::ChatCompletionsRequest;
use response::converse_stream_output_to_chat_completions_response_builder;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::timeout;
use tokio_stream::wrappers::ReceiverStream;
use tracing::{error, info};
use uuid::Uuid;

use crate::bedrock::openai::process_chat_completions_request_to_bedrock_chat_completion;

const EVENT_TX_SEND_TIMEOUT: Duration = Duration::from_secs(30);

fn process_bedrock_stream(
    mut stream: EventReceiver<
        aws_sdk_bedrockruntime::types::ConverseStreamOutput,
        ConverseStreamOutputError,
    >,
    id: String,
    created: i64,
    usage_callback: Arc<dyn Fn(&TokenUsage) + Send + Sync>,
) -> BoxStream<'static, anyhow::Result<Event>> {
    let (event_tx, event_rx) = mpsc::channel::<anyhow::Result<Event>>(1);

    tokio::spawn(async move {
        loop {
            match stream.recv().await {
                Ok(Some(output)) => {
                    let usage_callback = usage_callback.clone();
                    if let Some(builder) =
                        converse_stream_output_to_chat_completions_response_builder(
                            &output,
                            usage_callback,
                        )
                    {
                        let response = builder.id(Some(id.clone())).created(Some(created)).build();

                        let sse_event = create_sse_event(&response);
                        match timeout(EVENT_TX_SEND_TIMEOUT, event_tx.send(sse_event)).await {
                            Ok(Ok(())) => {}
                            Ok(Err(_)) => {
                                info!("SSE client disconnected, stopping Bedrock stream");
                                return;
                            }
                            Err(_) => {
                                error!("Channel send timed out, consumer likely stuck");
                                return;
                            }
                        }
                    }
                }
                Ok(None) => break,
                Err(e) => {
                    let _ = timeout(
                        EVENT_TX_SEND_TIMEOUT,
                        event_tx.send(Err(anyhow!("Stream receive error: {}", e))),
                    )
                    .await;
                    break;
                }
            }
        }

        info!("Stream finished, sending DONE message");
        let _ = timeout(
            EVENT_TX_SEND_TIMEOUT,
            event_tx.send(Ok(Event::default().data(DONE_MESSAGE))),
        )
        .await;
    });

    ReceiverStream::new(event_rx).boxed()
}

#[async_trait]
pub trait ChatCompletionsProvider {
    async fn chat_completions_stream<F>(
        self,
        request: ChatCompletionsRequest,
        usage_callback: F,
    ) -> anyhow::Result<BoxStream<'async_trait, anyhow::Result<Event>>>
    where
        F: Fn(&TokenUsage) + Send + Sync + 'static;
}

pub struct BedrockChatCompletionsProvider {
    bedrockruntime_client: Client,
}

impl BedrockChatCompletionsProvider {
    pub fn new(bedrockruntime_client: Client) -> Self {
        Self {
            bedrockruntime_client,
        }
    }
}

#[async_trait]
impl ChatCompletionsProvider for BedrockChatCompletionsProvider {
    async fn chat_completions_stream<F>(
        self,
        request: ChatCompletionsRequest,
        usage_callback: F,
    ) -> anyhow::Result<BoxStream<'async_trait, anyhow::Result<Event>>>
    where
        F: Fn(&TokenUsage) + Send + Sync + 'static,
    {
        let bedrock_chat_completion =
            process_chat_completions_request_to_bedrock_chat_completion(&request)?;
        let output_config =
            request
                .reasoning_effort
                .as_ref()
                .map(|reasoning_effort| OutputConfig::Effort {
                    effort: reasoning_effort.clone(),
                });
        let anthropic_beta = request
            .reasoning_effort
            .as_ref()
            .map(|_| vec!["effort-2025-11-24".to_string()]);
        let additional_model_request_fields = get_additional_model_request_fields(
            None,
            output_config.as_ref(),
            anthropic_beta.as_deref(),
        );
        info!(
            "Processed OpenAI request to Bedrock format with {} messages",
            bedrock_chat_completion
                .messages
                .as_ref()
                .map_or(0, |m| m.len())
        );

        info!(
            "Sending OpenAI request to Bedrock API for model: {}",
            bedrock_chat_completion.model_id
        );

        let converse_builder = self
            .bedrockruntime_client
            .converse_stream()
            .model_id(&bedrock_chat_completion.model_id)
            .set_system(bedrock_chat_completion.system_content_blocks)
            .set_messages(bedrock_chat_completion.messages)
            .set_tool_config(bedrock_chat_completion.tool_config)
            .set_inference_config(Some(bedrock_chat_completion.inference_config))
            .set_additional_model_request_fields(additional_model_request_fields);

        info!("About to send OpenAI request to Bedrock...");
        let result = converse_builder.send().await;

        let stream = match result {
            Ok(response) => {
                info!("Successfully connected to Bedrock stream");
                response.stream
            }
            Err(e) => {
                tracing::error!("Bedrock API error: {:?}", e);
                return Err(e.into());
            }
        };

        let id = Uuid::new_v4().to_string();
        let created = Utc::now().timestamp();

        let usage_callback = Arc::new(usage_callback);

        Ok(process_bedrock_stream(stream, id, created, usage_callback))
    }
}
