use anthropic_request::V1MessagesRequest;
use anthropic_response::{
    ContentBlockStartData, Delta, MessageDeltaData, MessageStartData, StreamEvent,
    Usage as AnthropicUsage,
};
use async_trait::async_trait;
use aws_config::BehaviorVersion;
use aws_sdk_bedrockruntime::Client;
use aws_sdk_bedrockruntime::primitives::event_stream::EventReceiver;
use aws_sdk_bedrockruntime::types::error::ConverseStreamOutputError;
use aws_sdk_bedrockruntime::types::{
    ContentBlockDelta, ContentBlockStart, ConverseStreamOutput, ReasoningContentBlockDelta,
    StopReason, TokenUsage,
};
use axum::response::sse::Event;
use futures::stream::{BoxStream, StreamExt};
use serde::Serialize;
use std::sync::Arc;
use tracing::info;
use uuid::Uuid;

use crate::bedrock::BedrockChatCompletion;

async fn process_anthropic_stream(
    mut stream: EventReceiver<ConverseStreamOutput, ConverseStreamOutputError>,
    id: String,
    model: String,
    usage_callback: Arc<dyn Fn(&TokenUsage) + Send + Sync>,
) -> BoxStream<'static, anyhow::Result<Event>> {
    let stream = async_stream::stream! {
        let mut usage_tracker = AnthropicUsage::default();
        let mut bedrock_usage: Option<TokenUsage> = None;
        let mut started_content_blocks = std::collections::HashSet::new();
        let mut thinking_content_blocks = std::collections::HashSet::new();
        let mut stop_reason_opt: Option<String> = None;

        loop {
            match stream.recv().await {
                Ok(Some(output)) => {
                    info!("Received Bedrock event for Anthropic: {:?}", std::mem::discriminant(&output));
                    // Log all event types including unknown ones
                    let event_type = match &output {
                        ConverseStreamOutput::MessageStart(_) => "MessageStart",
                        ConverseStreamOutput::ContentBlockStart(_) => "ContentBlockStart",
                        ConverseStreamOutput::ContentBlockDelta(_) => "ContentBlockDelta",
                        ConverseStreamOutput::ContentBlockStop(_) => "ContentBlockStop",
                        ConverseStreamOutput::MessageStop(_) => "MessageStop",
                        ConverseStreamOutput::Metadata(_) => "Metadata",
                        _ => "Unknown",
                    };
                    info!("Event type: {}", event_type);
                    match &output {
                        ConverseStreamOutput::MessageStart(_event) => {
                            info!("Processing MessageStart event");
                            // Usage information comes from Metadata events, not MessageStart
                            let message_start = StreamEvent::MessageStart {
                                message: MessageStartData {
                                    id: id.clone(),
                                    message_type: "message".to_string(),
                                    role: "assistant".to_string(),
                                    content: vec![],
                                    model: model.clone(),
                                    stop_reason: None,
                                    stop_sequence: None,
                                    usage: usage_tracker.clone(),
                                },
                            };

                            match create_anthropic_sse_event("message_start", &message_start) {
                                Ok(event) => yield Ok(event),
                                Err(e) => yield Err(e),
                            }
                        }

                        ConverseStreamOutput::ContentBlockStart(event) => {
                            info!("Processing ContentBlockStart event at index {}", event.content_block_index);
                            started_content_blocks.insert(event.content_block_index);
                            let content_block = match &event.start {
                                Some(ContentBlockStart::ToolUse(tool_use)) => {
                                    let tool_id = tool_use.tool_use_id().to_string();
                                    let tool_name = tool_use.name().to_string();
                                    info!("Tool use block: id='{}', name='{}'", tool_id, tool_name);
                                    ContentBlockStartData::ToolUse {
                                        id: tool_id,
                                        name: tool_name,
                                        input: serde_json::json!({}),
                                    }
                                }
                                _ => ContentBlockStartData::Text {
                                    text: String::new(),
                                },
                            };

                            let event_data = StreamEvent::ContentBlockStart {
                                index: event.content_block_index,
                                content_block: content_block.clone(),
                            };

                            info!("Serialized content_block_start: {:?}", serde_json::to_string(&event_data));

                            match create_anthropic_sse_event("content_block_start", &event_data) {
                                Ok(event) => yield Ok(event),
                                Err(e) => yield Err(e),
                            }
                        }

                        ConverseStreamOutput::ContentBlockDelta(event) => {
                            info!("Processing ContentBlockDelta event at index {}", event.content_block_index);
                            info!("Delta content: {:?}", event.delta);

                            // Bedrock may not send ContentBlockStart for text and reasoning blocks
                            // Synthesize one if we haven't seen it yet
                            if !started_content_blocks.contains(&event.content_block_index) {
                                info!("Synthesizing missing ContentBlockStart for index {}", event.content_block_index);
                                started_content_blocks.insert(event.content_block_index);

                                // Determine the block type from the delta
                                let content_block = match &event.delta {
                                    Some(ContentBlockDelta::ReasoningContent(_)) => {
                                        info!("⚠️ THINKING BLOCK DETECTED - Synthesizing thinking content block start");
                                        thinking_content_blocks.insert(event.content_block_index);
                                        ContentBlockStartData::Thinking {
                                            thinking: String::new(),
                                        }
                                    }
                                    _ => {
                                        ContentBlockStartData::Text {
                                            text: String::new(),
                                        }
                                    }
                                };

                                let event_data = StreamEvent::ContentBlockStart {
                                    index: event.content_block_index,
                                    content_block,
                                };

                                info!("⚠️ About to send ContentBlockStart SSE event");
                                match create_anthropic_sse_event("content_block_start", &event_data) {
                                    Ok(event) => {
                                        info!("⚠️ Successfully created and yielding ContentBlockStart SSE event");
                                        yield Ok(event)
                                    },
                                    Err(e) => {
                                        info!("⚠️ ERROR creating ContentBlockStart SSE event: {}", e);
                                        yield Err(e)
                                    },
                                }
                            }

                            let delta = match &event.delta {
                                Some(ContentBlockDelta::Text(text)) => {
                                    info!("Text delta content: '{}'", text);
                                    Some(Delta::TextDelta {
                                        text: text.clone(),
                                    })
                                },
                                Some(ContentBlockDelta::ToolUse(tool_use)) => {
                                    Some(Delta::InputJsonDelta {
                                        partial_json: tool_use.input.clone(),
                                    })
                                }
                                Some(ContentBlockDelta::ReasoningContent(
                                    ReasoningContentBlockDelta::Text(text),
                                )) => {
                                    info!("⚠️ THINKING DELTA RECEIVED - content: '{}'", text);
                                    Some(Delta::ThinkingDelta {
                                        thinking: text.clone(),
                                    })
                                },
                                _ => None,
                            };

                            if let Some(delta) = delta {
                                let is_thinking = matches!(delta, Delta::ThinkingDelta { .. });
                                if is_thinking {
                                    info!("⚠️ Creating ContentBlockDelta SSE event for THINKING");
                                }

                                let event_data = StreamEvent::ContentBlockDelta {
                                    index: event.content_block_index,
                                    delta,
                                };

                                match create_anthropic_sse_event("content_block_delta", &event_data) {
                                    Ok(event) => {
                                        if is_thinking {
                                            info!("⚠️ Successfully yielding THINKING content_block_delta SSE event");
                                        } else {
                                            info!("Yielding content_block_delta SSE event");
                                        }
                                        yield Ok(event)
                                    },
                                    Err(e) => {
                                        if is_thinking {
                                            info!("⚠️ ERROR yielding THINKING content_block_delta: {}", e);
                                        }
                                        yield Err(e)
                                    },
                                }
                            }
                        }

                        ConverseStreamOutput::ContentBlockStop(event) => {
                            // For thinking blocks, emit signature_delta before content_block_stop
                            if thinking_content_blocks.contains(&event.content_block_index) {
                                info!("Emitting signature_delta for thinking block at index {}", event.content_block_index);

                                // Generate a signature for the thinking block
                                // Since Bedrock doesn't provide signatures like Anthropic, we generate a placeholder
                                let signature = format!("bedrock_proxy_sig_{}", Uuid::new_v4().simple());

                                let signature_event = StreamEvent::ContentBlockDelta {
                                    index: event.content_block_index,
                                    delta: Delta::SignatureDelta { signature },
                                };

                                match create_anthropic_sse_event("content_block_delta", &signature_event) {
                                    Ok(event) => yield Ok(event),
                                    Err(e) => yield Err(e),
                                }

                                // Clean up the tracking set
                                thinking_content_blocks.remove(&event.content_block_index);
                            }

                            let event_data = StreamEvent::ContentBlockStop {
                                index: event.content_block_index,
                            };

                            match create_anthropic_sse_event("content_block_stop", &event_data) {
                                Ok(event) => yield Ok(event),
                                Err(e) => yield Err(e),
                            }
                        }

                        ConverseStreamOutput::MessageStop(event) => {
                            let stop_reason = match event.stop_reason {
                                StopReason::EndTurn => "end_turn",
                                StopReason::ToolUse => "tool_use",
                                StopReason::MaxTokens => "max_tokens",
                                StopReason::StopSequence => "stop_sequence",
                                _ => "unknown",
                            };

                            info!("MessageStop with stop_reason: {} (raw: {:?})", stop_reason, event.stop_reason);

                            // Store stop_reason but DON'T send message_delta yet
                            // We need to wait for Metadata to get usage info
                            stop_reason_opt = Some(stop_reason.to_string());
                        }

                        ConverseStreamOutput::Metadata(event) => {
                            info!("Processing Metadata event");
                            if let Some(usage) = &event.usage {
                                usage_tracker.input_tokens = usage.input_tokens;
                                usage_tracker.output_tokens = usage.output_tokens;
                                bedrock_usage = Some(usage.clone());
                                info!("Updated usage: input_tokens={}, output_tokens={}",
                                    usage.input_tokens, usage.output_tokens);
                            }

                            // If we received MessageStop earlier, now send message_delta and message_stop
                            // with the correct usage information
                            if let Some(stop_reason) = stop_reason_opt.take() {
                                let message_delta = StreamEvent::MessageDelta {
                                    delta: MessageDeltaData {
                                        stop_reason: Some(stop_reason),
                                        stop_sequence: None,
                                    },
                                    usage: usage_tracker.clone(),
                                };

                                info!("Serialized message_delta: {:?}", serde_json::to_string(&message_delta));

                                match create_anthropic_sse_event("message_delta", &message_delta) {
                                    Ok(event) => yield Ok(event),
                                    Err(e) => yield Err(e),
                                }

                                if let Some(ref usage) = bedrock_usage {
                                    usage_callback(usage);
                                }

                                let message_stop = StreamEvent::MessageStop;
                                match create_anthropic_sse_event("message_stop", &message_stop) {
                                    Ok(event) => yield Ok(event),
                                    Err(e) => yield Err(e),
                                }

                                break;
                            }
                        }

                        _ => {}
                    }
                }
                Ok(None) => {
                    info!("Anthropic stream finished naturally");
                    break;
                }
                Err(e) => {
                    yield Err(anyhow::anyhow!("Stream receive error: {}", e));
                    break;
                }
            }
        }
    };

    stream.boxed()
}

fn create_anthropic_sse_event(event_name: &str, data: &impl Serialize) -> anyhow::Result<Event> {
    let json = serde_json::to_string(data)?;
    info!("Creating SSE event '{}' with data: {}", event_name, json);
    Ok(Event::default().event(event_name).data(json))
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
        let bedrock_chat_completion = BedrockChatCompletion::try_from(&request)?;
        info!(
            "Processed Anthropic request to Bedrock format with {} messages",
            bedrock_chat_completion.messages.len()
        );

        let config = aws_config::load_defaults(BehaviorVersion::latest()).await;
        let client = Client::new(&config);

        info!(
            "Sending Anthropic request to Bedrock API for model: {}",
            bedrock_chat_completion.model_id
        );
        info!(
            "System blocks count: {}",
            bedrock_chat_completion.system_content_blocks.len()
        );
        info!("Messages count: {}", bedrock_chat_completion.messages.len());
        info!(
            "Inference config max_tokens: {:?}",
            bedrock_chat_completion.inference_config.max_tokens()
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

        info!("About to send request to Bedrock...");
        let result = converse_builder.send().await;

        match result {
            Ok(response) => {
                info!("Successfully connected to Bedrock stream for Anthropic format");
                let stream = response.stream;

                let id = format!("msg_{}", Uuid::new_v4());

                let usage_callback = Arc::new(usage_callback);

                Ok(process_anthropic_stream(stream, id, model, usage_callback).await)
            }
            Err(e) => {
                tracing::error!("Bedrock API error: {:?}", e);
                Err(anyhow::anyhow!("Bedrock API error: {}", e))
            }
        }
    }
}
