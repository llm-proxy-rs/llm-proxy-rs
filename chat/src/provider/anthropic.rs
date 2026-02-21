use anthropic_request::{
    AssistantContent, AssistantContents, Message, Messages, UserContent, UserContents,
    V1MessagesCountTokensRequest, V1MessagesRequest, get_additional_model_request_fields,
    tools_to_tool_configuration,
};
use anthropic_response::EventConverter;
use anyhow::{anyhow, bail};
use async_trait::async_trait;
use aws_sdk_bedrockruntime::{
    Client,
    primitives::event_stream::EventReceiver,
    types::{
        ContentBlock, ConverseStreamOutput, ConverseTokensRequest, CountTokensInput,
        Message as BedrockMessage, SystemContentBlock, TokenUsage,
        error::ConverseStreamOutputError,
    },
};
use aws_smithy_types::Document;
use axum::response::sse::Event;
use futures::stream::{BoxStream, StreamExt};
use std::{sync::Arc, time::Duration};
use tokio::{
    sync::mpsc,
    time::{Instant, interval_at, timeout},
};
use tokio_stream::wrappers::ReceiverStream;
use tracing::{error, info};
use uuid::Uuid;

const PING_INTERVAL: Duration = Duration::from_secs(20);
const EVENT_TX_SEND_TIMEOUT: Duration = Duration::from_secs(30);

fn process_bedrock_stream(
    mut stream: EventReceiver<ConverseStreamOutput, ConverseStreamOutputError>,
    model: String,
    usage_callback: Arc<dyn Fn(&TokenUsage) + Send + Sync>,
) -> BoxStream<'static, anyhow::Result<Event>> {
    let id = format!("msg_{}", Uuid::new_v4());
    let (event_tx, event_rx) = mpsc::channel::<anyhow::Result<Event>>(1);

    tokio::spawn(async move {
        let mut event_converter = EventConverter::new(id, model, usage_callback);
        let mut ping_interval = interval_at(Instant::now() + PING_INTERVAL, PING_INTERVAL);
        loop {
            tokio::select! {
                biased;
                result = stream.recv() => {
                    match result {
                        Ok(Some(output)) => {
                            if let Some(events) = event_converter.convert(&output) {
                                for (event_name, event) in events {
                                    let sse_event = match serde_json::to_string(&event) {
                                        Ok(json) => Ok(Event::default().event(event_name).data(json)),
                                        Err(e) => Err(anyhow!("Failed to serialize event: {}", e)),
                                    };
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
                        }
                        Ok(None) => break,
                        Err(e) => {
                            let _ = timeout(EVENT_TX_SEND_TIMEOUT, event_tx
                                .send(Err(anyhow!("Stream receive error: {}", e))))
                                .await;
                            break;
                        }
                    }
                }
                _ = ping_interval.tick() => {
                    info!("Sending ping event");
                    let ping_event = Ok(Event::default().event("ping").data(r#"{"type": "ping"}"#));
                    match timeout(EVENT_TX_SEND_TIMEOUT, event_tx.send(ping_event)).await {
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
        }
        info!("Bedrock stream finished");
    });

    ReceiverStream::new(event_rx).boxed()
}

#[async_trait]
pub trait V1MessagesProvider {
    async fn v1_messages_stream<F>(
        self,
        request: V1MessagesRequest,
        response_model_id: Option<String>,
        anthropic_beta: Option<Vec<String>>,
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

fn log_v1_messages_request(request: &V1MessagesRequest) {
    match &request.messages {
        Messages::String(s) => {
            info!(
                "V1 Messages Request: single string message, len={}",
                s.len()
            );
        }
        Messages::Array(messages) => {
            for (i, message) in messages.iter().enumerate() {
                match message {
                    Message::User { content } => {
                        let user_content_types = match content {
                            UserContents::String(s) => format!("String(len={})", s.len()),
                            UserContents::Array(arr) => arr
                                .iter()
                                .map(|c| match c {
                                    UserContent::Document { .. } => "Document",
                                    UserContent::Image { .. } => "Image",
                                    UserContent::Text { .. } => "Text",
                                    UserContent::ToolResult { .. } => "ToolResult",
                                })
                                .collect::<Vec<_>>()
                                .join(", "),
                        };
                        info!(
                            "V1 Messages Request Message {}: role=user, content=[{}]",
                            i, user_content_types
                        );
                    }
                    Message::Assistant { content } => {
                        let assistant_content_types = match content {
                            AssistantContents::String(s) => format!("String(len={})", s.len()),
                            AssistantContents::Array(arr) => arr
                                .iter()
                                .map(|c| match c {
                                    AssistantContent::Text { .. } => "Text",
                                    AssistantContent::Thinking { .. } => "Thinking",
                                    AssistantContent::ToolUse { .. } => "ToolUse",
                                })
                                .collect::<Vec<_>>()
                                .join(", "),
                        };
                        info!(
                            "V1 Messages Request Message {}: role=assistant, content=[{}]",
                            i, assistant_content_types
                        );
                    }
                }
            }
        }
    }
}

fn log_bedrock_messages(messages: &[BedrockMessage]) {
    for (i, message) in messages.iter().enumerate() {
        let content_block_types = message
            .content()
            .iter()
            .map(|content_block| match content_block {
                ContentBlock::Document(_) => "Document",
                ContentBlock::GuardContent(_) => "GuardContent",
                ContentBlock::Image(_) => "Image",
                ContentBlock::ReasoningContent(_) => "ReasoningContent",
                ContentBlock::Text(_) => "Text",
                ContentBlock::ToolResult(_) => "ToolResult",
                ContentBlock::ToolUse(_) => "ToolUse",
                _ => "Unknown",
            })
            .collect::<Vec<_>>()
            .join(", ");
        info!(
            "Bedrock Message {}: role={:?}, content=[{}]",
            i,
            message.role(),
            content_block_types
        );
    }
}

pub struct BedrockV1MessagesProvider {
    bedrockruntime_client: Client,
}

impl BedrockV1MessagesProvider {
    pub fn new(bedrockruntime_client: Client) -> Self {
        Self {
            bedrockruntime_client,
        }
    }
}

#[async_trait]
impl V1MessagesProvider for BedrockV1MessagesProvider {
    async fn v1_messages_stream<F>(
        self,
        request: V1MessagesRequest,
        response_model_id: Option<String>,
        anthropic_beta: Option<Vec<String>>,
        usage_callback: F,
    ) -> anyhow::Result<BoxStream<'async_trait, anyhow::Result<Event>>>
    where
        F: Fn(&TokenUsage) + Send + Sync + 'static,
    {
        let model = response_model_id.unwrap_or(request.model.clone());
        log_v1_messages_request(&request);
        let bedrock_chat_completion = crate::bedrock::BedrockChatCompletion::try_from(&request)?;
        let additional_model_request_fields = get_additional_model_request_fields(
            request.thinking.as_ref(),
            request.output_config.as_ref(),
            anthropic_beta.as_deref(),
        );
        if let Some(messages) = &bedrock_chat_completion.messages {
            log_bedrock_messages(messages);
        }
        info!(
            "Processed Anthropic request to Bedrock format with {} messages",
            bedrock_chat_completion
                .messages
                .as_ref()
                .map_or(0, |m| m.len())
        );

        info!(
            "Sending Anthropic request to Bedrock API for model: {}",
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
            .set_additional_model_request_fields(additional_model_request_fields)
            .set_output_config(bedrock_chat_completion.output_config);

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
                bail!("Bedrock API error: {}", e)
            }
        }
    }

    async fn v1_messages_count_tokens(
        &self,
        request: &V1MessagesCountTokensRequest,
        inference_profile_prefixes: &[String],
    ) -> anyhow::Result<i32> {
        let messages: Option<Vec<BedrockMessage>> = Option::try_from(&request.messages)?;

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

        let result = self
            .bedrockruntime_client
            .count_tokens()
            .model_id(model_id)
            .input(count_tokens_input)
            .send()
            .await;

        match result {
            Ok(response) => Ok(response.input_tokens),
            Err(e) => {
                error!("Bedrock API error: {:?}", e);
                bail!("Bedrock API error: {}", e)
            }
        }
    }
}
