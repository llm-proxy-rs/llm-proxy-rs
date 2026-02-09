use anthropic_request::{
    V1MessagesCountTokensRequest, V1MessagesRequest, tools_to_tool_configuration,
};
use anthropic_response::EventConverter;
use async_trait::async_trait;
use aws_config::BehaviorVersion;
use aws_sdk_bedrockruntime::Client;
use aws_sdk_bedrockruntime::primitives::event_stream::EventReceiver;
use aws_sdk_bedrockruntime::types::{
    ConverseStreamOutput, ConverseTokensRequest, CountTokensInput, Message, SystemContentBlock,
    TokenUsage, error::ConverseStreamOutputError,
};
use aws_smithy_types::Document;
use axum::response::sse::Event;
use futures::stream::{BoxStream, StreamExt};
use std::{sync::Arc, time::Duration};
use tokio::time::{Instant, interval_at};
use tracing::{error, info};
use uuid::Uuid;

const PING_INTERVAL: Duration = Duration::from_secs(20);

fn process_bedrock_stream(
    mut stream: EventReceiver<ConverseStreamOutput, ConverseStreamOutputError>,
    model: String,
    usage_callback: Arc<dyn Fn(&TokenUsage) + Send + Sync>,
) -> BoxStream<'static, anyhow::Result<Event>> {
    let id = format!("msg_{}", Uuid::new_v4());
    let stream = async_stream::stream! {
        let mut event_converter = EventConverter::new(id, model, usage_callback);
        let mut ping_interval = interval_at(Instant::now() + PING_INTERVAL, PING_INTERVAL);

        loop {
            tokio::select! {
                result = stream.recv() => {
                    match result {
                        Ok(Some(output)) => {
                            if let Some(events) = event_converter.convert(&output) {
                                for (event_name, event) in events {
                                    match serde_json::to_string(&event) {
                                        Ok(json) => yield Ok(Event::default().event(event_name).data(json)),
                                        Err(e) => yield Err(anyhow::anyhow!("Failed to serialize event: {}", e)),
                                    }
                                }
                            }
                        }
                        Ok(None) => break,
                        Err(e) => {
                            yield Err(anyhow::anyhow!("Stream receive error: {}", e));
                            break;
                        }
                    }
                }
                _ = ping_interval.tick() => {
                    info!("Sending ping event");
                    yield Ok(Event::default().event("ping").data(r#"{"type": "ping"}"#));
                }
            }
        }
        info!("Bedrock stream finished");
    };
    stream.boxed()
}

#[async_trait]
pub trait V1MessagesProvider {
    async fn v1_messages_stream<F>(
        self,
        request: V1MessagesRequest,
        usage_callback: F,
    ) -> anyhow::Result<BoxStream<'async_trait, anyhow::Result<Event>>>
    where
        F: Fn(&TokenUsage) + Send + Sync + 'static;

    async fn v1_messages_count_tokens(
        &self,
        request: &V1MessagesCountTokensRequest,
        inference_profile_prefixes: &[String],
    ) -> anyhow::Result<i32>;
}

pub struct BedrockV1MessagesProvider {}

impl BedrockV1MessagesProvider {
    pub async fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl V1MessagesProvider for BedrockV1MessagesProvider {
    async fn v1_messages_stream<F>(
        self,
        request: V1MessagesRequest,
        usage_callback: F,
    ) -> anyhow::Result<BoxStream<'async_trait, anyhow::Result<Event>>>
    where
        F: Fn(&TokenUsage) + Send + Sync + 'static,
    {
        let model = request.model.clone();
        let bedrock_chat_completion = crate::bedrock::BedrockChatCompletion::try_from(&request)?;
        info!(
            "Processed Anthropic request to Bedrock format with {} messages",
            bedrock_chat_completion
                .messages
                .as_ref()
                .map_or(0, |m| m.len())
        );

        let config = aws_config::load_defaults(BehaviorVersion::latest()).await;
        let client = Client::new(&config);

        info!(
            "Sending Anthropic request to Bedrock API for model: {}",
            bedrock_chat_completion.model_id
        );

        let converse_builder = client
            .converse_stream()
            .model_id(&bedrock_chat_completion.model_id)
            .set_system(bedrock_chat_completion.system_content_blocks)
            .set_messages(bedrock_chat_completion.messages)
            .set_tool_config(bedrock_chat_completion.tool_config)
            .set_inference_config(Some(bedrock_chat_completion.inference_config))
            .set_additional_model_request_fields(
                bedrock_chat_completion.additional_model_request_fields,
            );

        info!("About to send Anthropic request to Bedrock...");
        let result = converse_builder.send().await;

        match result {
            Ok(response) => {
                info!("Successfully connected to Bedrock stream for Anthropic format");
                let stream = response.stream;
                let usage_callback = Arc::new(usage_callback);

                Ok(process_bedrock_stream(stream, model, usage_callback))
            }
            Err(e) => {
                error!("Bedrock API error: {:?}", e);
                Err(anyhow::anyhow!("Bedrock API error: {}", e))
            }
        }
    }

    async fn v1_messages_count_tokens(
        &self,
        request: &V1MessagesCountTokensRequest,
        inference_profile_prefixes: &[String],
    ) -> anyhow::Result<i32> {
        let config = aws_config::load_defaults(BehaviorVersion::latest()).await;
        let client = Client::new(&config);

        let messages: Option<Vec<Message>> = Option::try_from(&request.messages)?;

        let system: Option<Vec<SystemContentBlock>> = request
            .system
            .as_ref()
            .map(Vec::<SystemContentBlock>::try_from)
            .transpose()?;

        let tool_config = request
            .tools
            .as_deref()
            .map(tools_to_tool_configuration)
            .transpose()?
            .flatten();

        let additional_model_request_fields = request.thinking.as_ref().map(Document::from);

        let converse_tokens_request = ConverseTokensRequest::builder()
            .set_additional_model_request_fields(additional_model_request_fields)
            .set_messages(messages)
            .set_system(system)
            .set_tool_config(tool_config)
            .build();

        let count_tokens_input = CountTokensInput::Converse(converse_tokens_request);

        let model_id = inference_profile_prefixes
            .iter()
            .find_map(|inference_profile_prefix| {
                request.model.strip_prefix(inference_profile_prefix)
            })
            .unwrap_or(&request.model);

        let result = client
            .count_tokens()
            .model_id(model_id)
            .input(count_tokens_input)
            .send()
            .await;

        match result {
            Ok(response) => Ok(response.input_tokens),
            Err(e) => {
                error!("Bedrock API error: {:?}", e);
                Err(anyhow::anyhow!("Bedrock API error: {}", e))
            }
        }
    }
}
